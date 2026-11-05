use std::rc::Rc;

use pipewire::{
    device::{Device, DeviceChangeMask, DeviceInfoRef},
    proxy::Listener,
    registry::{GlobalObject, Registry},
};

use libspa::{
    param::ParamType,
    pod::{Object, Value, ValueArray},
    utils::dict::DictRef,
};

use super::event_sender::EventSender;
use super::{ObjectId, PropertyStore, StateEvent, deserialize::deserialize};

/// Watches a sound card: its properties, and its active routes (where a hardware node's volume
/// and mute live).
pub fn monitor_device(
    registry: &Registry,
    object: &GlobalObject<&DictRef>,
    sender: &Rc<EventSender>,
) -> Option<(Rc<Device>, Box<dyn Listener>)> {
    let object_id = ObjectId::from(object);

    let props = object.props?;
    if props.get("media.class")? != "Audio/Device" {
        return None;
    }

    let device: Device = registry.bind(object).ok()?;
    let device = Rc::new(device);

    let listener = device
        .add_listener_local()
        .param({
            let sender_weak = Rc::downgrade(sender);
            move |_seq, id, _index, _next, param| {
                let Some(sender) = sender_weak.upgrade() else {
                    return;
                };
                if id == ParamType::Route
                    && let Some(event) = deserialize(param).and_then(|p| device_route(object_id, p))
                {
                    sender.send(event);
                }
            }
        })
        .info({
            let sender_weak = Rc::downgrade(sender);
            let device_weak = Rc::downgrade(&device);
            move |info| {
                let Some(sender) = sender_weak.upgrade() else {
                    return;
                };
                for change in info.change_mask().iter() {
                    if change == DeviceChangeMask::PROPS {
                        device_info_props(&sender, object_id, info);
                    }
                }

                let Some(device) = device_weak.upgrade() else {
                    return;
                };
                device.enum_params(0, Some(ParamType::Route), 0, u32::MAX);
            }
        })
        .register();

    device.subscribe_params(&[ParamType::Route]);

    Some((device, Box::new(listener)))
}

fn device_route(object_id: ObjectId, param: Object) -> Option<StateEvent> {
    let mut index = None;
    let mut device = None;
    let mut channel_volumes = None;
    let mut mute = None;

    for prop in param.properties {
        match prop.key {
            libspa_sys::SPA_PARAM_ROUTE_index => {
                if let Value::Int(value) = prop.value {
                    index = Some(value);
                }
            }
            libspa_sys::SPA_PARAM_ROUTE_device => {
                if let Value::Int(value) = prop.value {
                    device = Some(value);
                }
            }
            libspa_sys::SPA_PARAM_ROUTE_props => {
                if let Value::Object(value) = prop.value {
                    for prop in value.properties {
                        match prop.key {
                            libspa_sys::SPA_PROP_channelVolumes => {
                                if let Value::ValueArray(ValueArray::Float(value)) = prop.value {
                                    channel_volumes = Some(value);
                                }
                            }
                            libspa_sys::SPA_PROP_mute => {
                                if let Value::Bool(value) = prop.value {
                                    mute = Some(value);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Some(StateEvent::DeviceRoute {
        object_id,
        index: index?,
        device: device?,
        channel_volumes: channel_volumes?,
        mute: mute?,
    })
}

fn device_info_props(sender: &EventSender, object_id: ObjectId, device_info: &DeviceInfoRef) {
    let Some(props) = device_info.props() else {
        return;
    };

    let props = PropertyStore::from(props);
    sender.send(StateEvent::DeviceProperties { object_id, props });
}
