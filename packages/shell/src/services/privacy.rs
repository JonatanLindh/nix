//! Who is using the microphone, the camera or the screen (Privacy.qml), read from the PipeWire
//! graph.
//!
//! Its own PipeWire client: the sound service's `wirehose` only binds audio nodes and doesn't
//! report link states, and this needs the camera and screencast nodes and which links are
//! actually running. It binds every node and link and reads their info, like Quickshell's
//! `PwObjectTracker` does.
//!
//! Only what goes through PipeWire is seen: apps that open /dev/video* directly (most browsers
//! and video call apps today, unless their PipeWire camera option is on) use the camera without
//! anything showing up here.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use pipewire::context::ContextRc;
use pipewire::link::{Link as PwLink, LinkListener, LinkState};
use pipewire::main_loop::MainLoopRc;
use pipewire::node::{Node as PwNode, NodeListener};
use pipewire::types::ObjectType;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub mic: bool,
    pub camera: bool,
    pub screen: bool,
    /// Who's recording, by app name, for each.
    pub mic_apps: Vec<String>,
    pub camera_apps: Vec<String>,
    /// The apps receiving the screencast, not the portal producing it.
    pub screen_apps: Vec<String>,
}

impl State {
    pub fn any(&self) -> bool {
        self.mic || self.camera || self.screen
    }
}

/// A node's properties (`media.class`, `node.name`, `application.name`, ...).
type Props = HashMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Link {
    output: u32,
    input: u32,
    active: bool,
}

/// The graph as far as it matters here: nodes by id and links by id.
#[derive(Debug, Default)]
struct Graph {
    nodes: HashMap<u32, Props>,
    links: HashMap<u32, Link>,
}

fn prop<'a>(node: &'a Props, key: &str) -> Option<&'a str> {
    node.get(key).map(String::as_str)
}

fn class(node: &Props) -> &str {
    prop(node, "media.class").unwrap_or("")
}

fn app_name(node: &Props) -> &str {
    [
        "application.name",
        "application.process.binary",
        "node.name",
    ]
    .into_iter()
    .filter_map(|k| prop(node, k))
    .find(|s| !s.is_empty())
    .unwrap_or("")
}

/// Streams that record without anyone being recorded: EasyEffects' own meters and spectrum,
/// level meters (pavucontrol, wiremix, Quickshell's peak detector) and corked PulseAudio streams.
fn ignored(node: &Props) -> bool {
    let name = prop(node, "node.name").unwrap_or("");
    name.starts_with("ee_")
        || name.starts_with("easyeffects")
        || prop(node, "stream.monitor") == Some("true")
        || prop(node, "application.name") == Some("Quickshell Peak Detect")
        || prop(node, "pulse.corked") == Some("true")
}

/// Hardware cameras have a device; screencasts (xdg-desktop-portal-hyprland's
/// "xdph-streaming-N") are Video/Source nodes without one.
fn is_screencast(node: &Props) -> bool {
    class(node) == "Video/Source" && !node.contains_key("device.id")
}

impl Graph {
    /// Both ends of each link whose nodes are known: (link, source, target).
    fn ends(&self) -> impl Iterator<Item = (&Link, &Props, u32, &Props)> {
        self.links.values().filter_map(|l| {
            Some((
                l,
                self.nodes.get(&l.output)?,
                l.input,
                self.nodes.get(&l.input)?,
            ))
        })
    }

    /// Stream nodes of `stream_class` fed by a running link from a node `from` accepts. Sink
    /// monitors belong to the Audio/Sink node, so recording what's playing doesn't count.
    fn recorders(&self, stream_class: &str, from: impl Fn(&Props) -> bool) -> Vec<u32> {
        let mut found = Vec::new();
        for (link, source, id, target) in self.ends() {
            if link.active
                && class(target) == stream_class
                && !ignored(target)
                && from(source)
                && !found.contains(&id)
            {
                found.push(id);
            }
        }
        found
    }

    fn names(&self, ids: impl IntoIterator<Item = u32>) -> Vec<String> {
        let mut names: Vec<String> = ids
            .into_iter()
            .filter_map(|id| self.nodes.get(&id))
            .map(app_name)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect();
        names.sort();
        names.dedup();
        names
    }

    fn state(&self) -> State {
        let mic = self.recorders("Stream/Input/Audio", |n| {
            class(n).starts_with("Audio/Source")
        });
        let camera = self.recorders("Stream/Input/Video", |n| {
            class(n) == "Video/Source" && !is_screencast(n)
        });
        // A screencast counts while something is linked to it, i.e. someone is watching.
        let watchers: Vec<u32> = self
            .ends()
            .filter(|(_, source, _, _)| is_screencast(source))
            .map(|(_, _, id, _)| id)
            .collect();
        State {
            mic: !mic.is_empty(),
            camera: !camera.is_empty(),
            screen: !watchers.is_empty(),
            mic_apps: self.names(mic),
            camera_apps: self.names(camera),
            screen_apps: self.names(watchers),
        }
    }
}

pub fn watch() -> UnboundedReceiver<State> {
    let (tx, rx) = unbounded();
    std::thread::Builder::new()
        .name("pipewire-privacy".into())
        .spawn(move || run(tx))
        .expect("pipewire thread");
    rx
}

/// Keeps a connection up for as long as anyone listens, reconnecting after PipeWire restarts.
fn run(tx: UnboundedSender<State>) {
    pipewire::init();
    let mut last_error = String::new();
    while !tx.is_closed() {
        if let Err(e) = connect(&tx) {
            let e = e.to_string();
            if e != last_error {
                eprintln!("privacy: {e}");
                last_error = e;
            }
        }
        // Whatever was in use isn't known any more.
        if tx.unbounded_send(State::default()).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// A bound object and its listener, kept alive while the object exists. The listener goes
/// first, so it's dropped before its proxy.
enum Bound {
    Node {
        _listener: NodeListener,
        _proxy: PwNode,
    },
    Link {
        _listener: LinkListener,
        _proxy: PwLink,
    },
}

/// What the callbacks share.
struct Shared {
    graph: RefCell<Graph>,
    last: RefCell<Option<State>>,
    tx: UnboundedSender<State>,
    main_loop: MainLoopRc,
}

impl Shared {
    /// Sends the state on if it changed; stops the loop once nobody listens.
    fn changed(&self) {
        let state = self.graph.borrow().state();
        let mut last = self.last.borrow_mut();
        if last.as_ref() == Some(&state) {
            return;
        }
        *last = Some(state.clone());
        if self.tx.unbounded_send(state).is_err() {
            self.main_loop.quit();
        }
    }
}

/// One connection to PipeWire, until it's lost or nobody listens any more.
fn connect(tx: &UnboundedSender<State>) -> anyhow::Result<()> {
    let main_loop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&main_loop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry_rc()?;

    let shared = Rc::new(Shared {
        graph: RefCell::default(),
        last: RefCell::default(),
        tx: tx.clone(),
        main_loop: main_loop.clone(),
    });
    let error = Rc::new(RefCell::new(None));

    // The connection going away (PipeWire restarting) shows up as an error on the core.
    let _core_listener = core
        .add_listener_local()
        .error({
            let (main_loop, error) = (main_loop.clone(), error.clone());
            move |id, _, _, message| {
                if id == pipewire::core::PW_ID_CORE {
                    *error.borrow_mut() = Some(message.to_string());
                    main_loop.quit();
                }
            }
        })
        .register();

    let bound: Rc<RefCell<HashMap<u32, Bound>>> = Rc::default();
    // Proxies aren't dropped inside PipeWire's callbacks: removed ones wait here until the loop
    // comes round to the channel.
    let garbage: Rc<RefCell<Vec<Bound>>> = Rc::default();
    let (collect, collector) = pipewire::channel::channel::<()>();
    let _collector = collector.attach(main_loop.loop_(), {
        let garbage = garbage.clone();
        move |()| garbage.borrow_mut().clear()
    });

    let registry_weak = registry.downgrade();
    let _registry_listener = registry
        .add_listener_local()
        .global({
            let (shared, bound) = (shared.clone(), bound.clone());
            move |global| {
                let Some(registry) = registry_weak.upgrade() else {
                    return;
                };
                let id = global.id;
                let object = match global.type_ {
                    ObjectType::Node => {
                        let Ok(node) = registry.bind::<PwNode, _>(global) else {
                            return;
                        };
                        let shared = shared.clone();
                        let listener = node
                            .add_listener_local()
                            .info(move |info| {
                                let Some(props) = info.props() else {
                                    return;
                                };
                                // Merged, not replaced: later info events can carry only some
                                // of the props, which would lose the media class.
                                shared
                                    .graph
                                    .borrow_mut()
                                    .nodes
                                    .entry(id)
                                    .or_default()
                                    .extend(props.iter().map(|(k, v)| (k.to_string(), v.to_string())));
                                shared.changed();
                            })
                            .register();
                        Bound::Node {
                            _listener: listener,
                            _proxy: node,
                        }
                    }
                    ObjectType::Link => {
                        let Ok(link) = registry.bind::<PwLink, _>(global) else {
                            return;
                        };
                        let shared = shared.clone();
                        let listener = link
                            .add_listener_local()
                            .info(move |info| {
                                let link = Link {
                                    output: info.output_node_id(),
                                    input: info.input_node_id(),
                                    active: matches!(info.state(), LinkState::Active),
                                };
                                shared.graph.borrow_mut().links.insert(id, link);
                                shared.changed();
                            })
                            .register();
                        Bound::Link {
                            _listener: listener,
                            _proxy: link,
                        }
                    }
                    _ => return,
                };
                bound.borrow_mut().insert(id, object);
            }
        })
        .global_remove({
            let shared = shared.clone();
            move |id| {
                if let Some(object) = bound.borrow_mut().remove(&id) {
                    garbage.borrow_mut().push(object);
                    let _ = collect.send(());
                }
                {
                    let mut graph = shared.graph.borrow_mut();
                    graph.nodes.remove(&id);
                    graph.links.remove(&id);
                }
                shared.changed();
            }
        })
        .register();

    main_loop.run();
    match error.borrow_mut().take() {
        Some(e) => anyhow::bail!(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(props: &[(&str, &str)]) -> Props {
        props
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn graph(nodes: Vec<(u32, Props)>, links: &[(u32, u32, bool)]) -> Graph {
        Graph {
            nodes: nodes.into_iter().collect(),
            links: links
                .iter()
                .enumerate()
                .map(|(i, &(output, input, active))| {
                    (
                        1000 + i as u32,
                        Link {
                            output,
                            input,
                            active,
                        },
                    )
                })
                .collect(),
        }
    }

    fn mic() -> Props {
        node(&[("media.class", "Audio/Source"), ("device.id", "40")])
    }

    fn recorder(app: &str) -> Props {
        node(&[
            ("media.class", "Stream/Input/Audio"),
            ("application.name", app),
            ("node.name", "rec"),
        ])
    }

    #[test]
    fn nothing_linked_is_nothing_in_use() {
        let g = graph(vec![(1, mic()), (2, recorder("Discord"))], &[]);
        assert_eq!(g.state(), State::default());
    }

    #[test]
    fn active_mic_recording() {
        let g = graph(
            vec![
                (1, mic()),
                (2, recorder("Discord")),
                (3, recorder("Discord")),
            ],
            &[(1, 2, true), (1, 2, true), (1, 3, true)],
        );
        let s = g.state();
        assert!(s.mic && !s.camera && !s.screen);
        assert_eq!(s.mic_apps, ["Discord"]);
    }

    #[test]
    fn paused_links_and_meters_dont_count() {
        let meter = node(&[
            ("media.class", "Stream/Input/Audio"),
            ("node.name", "wiremix-capture"),
            ("stream.monitor", "true"),
        ]);
        let corked = node(&[
            ("media.class", "Stream/Input/Audio"),
            ("node.name", "x"),
            ("pulse.corked", "true"),
        ]);
        let ee = node(&[
            ("media.class", "Stream/Input/Audio"),
            ("node.name", "ee_soe_spectrum"),
        ]);
        let g = graph(
            vec![
                (1, mic()),
                (2, recorder("A")),
                (3, meter),
                (4, corked),
                (5, ee),
            ],
            &[(1, 2, false), (1, 3, true), (1, 4, true), (1, 5, true)],
        );
        assert!(!g.state().mic);
    }

    #[test]
    fn recording_a_sink_monitor_isnt_the_mic() {
        let sink = node(&[("media.class", "Audio/Sink"), ("device.id", "40")]);
        let g = graph(vec![(1, sink), (2, recorder("OBS"))], &[(1, 2, true)]);
        assert!(!g.state().mic);
    }

    #[test]
    fn camera_and_screencast_told_apart() {
        let camera = node(&[("media.class", "Video/Source"), ("device.id", "60")]);
        let cast = node(&[
            ("media.class", "Video/Source"),
            ("node.name", "xdph-streaming-0"),
        ]);
        let viewer = |app| {
            node(&[
                ("media.class", "Stream/Input/Video"),
                ("application.name", app),
            ])
        };
        let g = graph(
            vec![
                (1, camera),
                (2, cast),
                (3, viewer("Firefox")),
                (4, viewer("OBS")),
            ],
            &[(1, 3, true), (2, 4, false)],
        );
        let s = g.state();
        assert!(s.camera && s.screen && !s.mic);
        assert_eq!(s.camera_apps, ["Firefox"]);
        // Linked is enough for a screencast, running or not.
        assert_eq!(s.screen_apps, ["OBS"]);
    }
}
