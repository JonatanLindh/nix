//! Setup and teardown of PipeWire monitoring.
//!
//! [`Session::spawn()`] starts a PipeWire monitoring thread.

use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;

use nix::sys::eventfd::{EfdFlags, EventFd};
use std::os::fd::AsRawFd;

use pipewire::{main_loop::MainLoopRc, proxy::ProxyT, types::ObjectType};

use super::{
    EventHandler, ObjectId, StateEvent, command::Command, device, event::Event,
    event_sender::EventSender, execute, link, metadata, node, proxy_registry::ProxyRegistry,
    sync_registry::SyncRegistry,
};

/// Handle for a PipeWire monitoring thread.
///
/// Dropping it tells the thread's main loop to quit and joins the thread. PipeWire itself stays
/// initialised: `pw_deinit` is for the end of the process (and `pipewire::init` won't run
/// `pw_init` a second time), so a later session, or another PipeWire client in the process,
/// still works.
pub struct Session {
    fd: Arc<EventFd>,
    handle: Option<thread::JoinHandle<()>>,
    /// Channel for sending [`Command`]s to be executed
    tx: pipewire::channel::Sender<Command>,
}

impl Session {
    /// Spawns a thread to monitor PipeWire, sending its [`Event`]s to `handler`.
    ///
    /// The thread ends when the session is dropped, when `handler` returns `false`, or when the
    /// connection is lost or can't be made; the last two are reported as an [`Event::Error`]
    /// first.
    pub fn spawn<F: EventHandler>(handler: F) -> Result<Self> {
        let shutdown_fd = Arc::new(EventFd::from_value_and_flags(0, EfdFlags::EFD_NONBLOCK)?);

        let (tx, rx) = pipewire::channel::channel::<Command>();

        let handle = thread::Builder::new().name("wirehose".into()).spawn({
            let shutdown_fd = Arc::clone(&shutdown_fd);
            move || run(rx, handler, shutdown_fd)
        })?;

        Ok(Self {
            fd: shutdown_fd,
            handle: Some(handle),
            tx,
        })
    }

    /// Mute a node.
    pub fn node_mute(&self, object_id: ObjectId, mute: bool) {
        let _ = self.tx.send(Command::NodeMute(object_id, mute));
    }

    /// Set the volumes on a node's channels.
    pub fn node_volumes(&self, object_id: ObjectId, volumes: Vec<f32>) {
        let _ = self.tx.send(Command::NodeVolumes(object_id, volumes));
    }

    /// Mute a device's route.
    pub fn device_mute(
        &self,
        object_id: ObjectId,
        route_index: i32,
        route_device: i32,
        mute: bool,
    ) {
        let _ = self.tx.send(Command::DeviceMute(
            object_id,
            route_index,
            route_device,
            mute,
        ));
    }

    /// Change the volumes of a device route's channels.
    pub fn device_volumes(
        &self,
        object_id: ObjectId,
        route_index: i32,
        route_device: i32,
        volumes: Vec<f32>,
    ) {
        let _ = self.tx.send(Command::DeviceVolumes(
            object_id,
            route_index,
            route_device,
            volumes,
        ));
    }

    /// Set a metadata property. Set `type_` to None to clear all metadata for
    /// the subject. Set `value` to None to clear the metadata for the key.
    pub fn metadata_set_property(
        &self,
        object_id: ObjectId,
        subject: u32,
        key: String,
        type_: Option<String>,
        value: Option<String>,
    ) {
        let _ = self.tx.send(Command::MetadataSetProperty(
            object_id, subject, key, type_, value,
        ));
    }
}

impl Drop for Session {
    /// Shut down the PipeWire monitoring thread.
    fn drop(&mut self) {
        let _ = self.fd.arm();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The thread: a main loop, then monitoring until it's told to stop or the connection goes.
fn run<F: EventHandler>(
    rx: pipewire::channel::Receiver<Command>,
    mut handler: F,
    shutdown_fd: Arc<EventFd>,
) {
    pipewire::init();

    let main_loop = match MainLoopRc::new(None) {
        Ok(main_loop) => main_loop,
        Err(e) => {
            handler.handle_event(Event::Error {
                object_id: None,
                message: e.to_string(),
            });
            return;
        }
    };
    let sender = Rc::new(EventSender::new(handler, main_loop.downgrade()));

    if let Err(e) = monitor_pipewire(main_loop, Rc::clone(&sender), rx, shutdown_fd) {
        sender.send_error(None, e.to_string());
    }
}

/// Monitors PipeWire.
///
/// Sets up core listeners and runs the PipeWire main loop.
fn monitor_pipewire(
    main_loop: MainLoopRc,
    sender: Rc<EventSender>,
    rx: pipewire::channel::Receiver<Command>,
    shutdown_fd: Arc<EventFd>,
) -> Result<()> {
    let context = pipewire::context::ContextRc::new(&main_loop, None)?;
    let core = context.connect_rc(None)?;

    let fd = shutdown_fd.as_raw_fd();
    let _shutdown_watch = main_loop
        .loop_()
        .add_io(fd, libspa::support::system::IoFlags::IN, {
            let main_loop_weak = main_loop.downgrade();
            move |_status| {
                if let Some(main_loop) = main_loop_weak.upgrade() {
                    main_loop.quit();
                }
            }
        });

    let syncs = Rc::new(RefCell::new(SyncRegistry::default()));

    let _core_listener = core
        .add_listener_local()
        .done({
            let sender_weak = Rc::downgrade(&sender);
            let syncs_weak = Rc::downgrade(&syncs);
            move |_id, seq| {
                let Some(sender) = sender_weak.upgrade() else {
                    return;
                };
                let Some(syncs) = syncs_weak.upgrade() else {
                    return;
                };
                if syncs.borrow_mut().done(seq) {
                    sender.send_ready();
                }
            }
        })
        .error({
            // Errors arrive here for any object, a node failing among them. Only one about the
            // core means the connection is gone (PipeWire restarting, say): that ends the loop.
            let sender_weak = Rc::downgrade(&sender);
            let main_loop_weak = main_loop.downgrade();
            move |id, _seq, _res, message| {
                let object_id = ObjectId::from_raw_id(id);
                if let Some(sender) = sender_weak.upgrade() {
                    sender.send_error(Some(object_id), message.to_string());
                };
                if object_id == ObjectId::CORE
                    && let Some(main_loop) = main_loop_weak.upgrade()
                {
                    main_loop.quit();
                }
            }
        })
        .register();

    let registry = core.get_registry_rc()?;
    let registry_weak = registry.downgrade();

    // Proxies and their listeners need to stay alive so store them here
    let proxies = Rc::new(RefCell::new(ProxyRegistry::try_new()?));
    // It's not safe to delete proxies and listeners during PipeWire callbacks,
    // so registries defer cleanup and use an EventFd to signal that objects
    // are pending deletion.
    let _proxy_gc_watch = main_loop.loop_().add_io(
        proxies.borrow().gc_fd.as_raw_fd(),
        libspa::support::system::IoFlags::IN,
        {
            let proxies = Rc::clone(&proxies);
            move |_status| {
                proxies.borrow_mut().collect_garbage();
            }
        },
    );

    let _registry_listener = registry
        .add_listener_local()
        .global({
            let core_weak = core.downgrade();
            let proxies = Rc::clone(&proxies);
            let sender_weak = Rc::downgrade(&sender);
            let syncs_weak = Rc::downgrade(&syncs);
            move |object| {
                let object_id = ObjectId::from(object);
                let Some(registry) = registry_weak.upgrade() else {
                    return;
                };

                let Some(sender) = sender_weak.upgrade() else {
                    return;
                };

                let Some(core) = core_weak.upgrade() else {
                    return;
                };

                let Some(syncs) = syncs_weak.upgrade() else {
                    return;
                };

                let proxy_spe = match object.type_ {
                    ObjectType::Node => {
                        let result = node::monitor_node(&registry, object, &sender);
                        if let Some((node, listener)) = result {
                            proxies
                                .borrow_mut()
                                .add_node(object_id, Rc::clone(&node), listener);
                            Some(node as Rc<dyn ProxyT>)
                        } else {
                            None
                        }
                    }
                    ObjectType::Device => {
                        let result = device::monitor_device(&registry, object, &sender);
                        match result {
                            Some((device, listener)) => {
                                proxies.borrow_mut().add_device(
                                    object_id,
                                    Rc::clone(&device),
                                    listener,
                                );
                                Some(device as Rc<dyn ProxyT>)
                            }
                            None => None,
                        }
                    }
                    ObjectType::Link => {
                        let result = link::monitor_link(&registry, object, &sender);
                        match result {
                            Some((link, listener)) => {
                                proxies.borrow_mut().add_link(
                                    object_id,
                                    Rc::clone(&link),
                                    listener,
                                );
                                Some(link as Rc<dyn ProxyT>)
                            }
                            None => None,
                        }
                    }
                    ObjectType::Metadata => {
                        let result = metadata::monitor_metadata(&registry, object, &sender);
                        match result {
                            Some((metadata, listener)) => {
                                proxies.borrow_mut().add_metadata(
                                    object_id,
                                    Rc::clone(&metadata),
                                    listener,
                                );
                                Some(metadata as Rc<dyn ProxyT>)
                            }
                            None => None,
                        }
                    }
                    _ => None,
                };
                let Some(proxy_spe) = proxy_spe else {
                    return;
                };

                let proxy = proxy_spe.upcast_ref();

                // Use a weak ref to prevent references cycle between Proxy and proxies:
                // - ref on proxies in the closure, bound to the Proxy lifetime
                // - proxies owning a ref on Proxy as well
                let proxies_weak = Rc::downgrade(&proxies);
                let sender_weak = Rc::downgrade(&sender);
                let listener = proxy
                    .add_listener_local()
                    .removed(move || {
                        if let Some(sender) = sender_weak.upgrade() {
                            sender.send(StateEvent::Removed { object_id });
                        };
                        if let Some(proxies) = proxies_weak.upgrade() {
                            proxies.borrow_mut().remove(object_id);
                        };
                    })
                    .register();

                proxies.borrow_mut().add_proxy_listener(object_id, listener);

                syncs.borrow_mut().global(&core);
            }
        })
        .register();

    let _receiver = rx.attach(main_loop.loop_(), {
        let proxies = Rc::clone(&proxies);
        move |command| execute::execute_command(&proxies.borrow(), command)
    });

    main_loop.run();

    Ok(())
}
