use super::command::Command;
use super::proxy_registry::ProxyRegistry;

use pipewire::{device::Device, node::Node};

use libspa::param::ParamType;
use libspa::pod::{
    Object, Pod, Property, PropertyFlags, Value, ValueArray, serialize::PodSerializer,
};

pub fn execute_command(proxies: &ProxyRegistry, command: Command) {
    match command {
        Command::NodeMute(obj_id, mute) => {
            if let Some(node) = proxies.nodes.get(&obj_id) {
                node_set_mute(node, mute);
            }
        }
        Command::DeviceMute(obj_id, route_index, route_device, mute) => {
            if let Some(device) = proxies.devices.get(&obj_id) {
                device_set_mute(device, route_index, route_device, mute);
            }
        }
        Command::NodeVolumes(obj_id, volumes) => {
            if let Some(node) = proxies.nodes.get(&obj_id) {
                node_set_volumes(node, volumes);
            }
        }
        Command::DeviceVolumes(obj_id, route_index, route_device, volumes) => {
            if let Some(device) = proxies.devices.get(&obj_id) {
                device_set_volumes(device, route_index, route_device, volumes);
            }
        }
        Command::MetadataSetProperty(obj_id, subject, key, type_, value) => {
            if let Some(metadata) = proxies.metadatas.get(&obj_id) {
                metadata.set_property(subject, &key, type_.as_deref(), value.as_deref());
            }
        }
    }
}

fn node_set_mute(node: &Node, mute: bool) {
    node_set_properties(
        node,
        vec![
            Property {
                key: libspa_sys::SPA_PROP_mute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
            Property {
                key: libspa_sys::SPA_PROP_mute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
        ],
    );
}

fn node_set_volumes(node: &Node, volumes: Vec<f32>) {
    node_set_properties(
        node,
        vec![Property {
            key: libspa_sys::SPA_PROP_channelVolumes,
            flags: PropertyFlags::empty(),
            value: Value::ValueArray(ValueArray::Float(volumes.clone())),
        }],
    );
}

fn node_set_properties(node: &Node, properties: Vec<Property>) {
    let values = PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(Object {
            type_: libspa_sys::SPA_TYPE_OBJECT_Props,
            id: libspa_sys::SPA_PARAM_Props,
            properties,
        }),
    );

    if let Ok((values, _)) = values
        && let Some(pod) = Pod::from_bytes(&values.into_inner())
    {
        node.set_param(ParamType::Props, 0, pod);
    }
}

fn device_set_mute(device: &Device, route_index: i32, route_device: i32, mute: bool) {
    device_set_route_properties(
        device,
        route_index,
        route_device,
        vec![
            Property {
                key: libspa_sys::SPA_PROP_mute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
            Property {
                key: libspa_sys::SPA_PROP_mute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
        ],
    );
}

fn device_set_volumes(device: &Device, route_index: i32, route_device: i32, volumes: Vec<f32>) {
    device_set_route_properties(
        device,
        route_index,
        route_device,
        vec![Property {
            key: libspa_sys::SPA_PROP_channelVolumes,
            flags: PropertyFlags::empty(),
            value: Value::ValueArray(ValueArray::Float(volumes.clone())),
        }],
    );
}

fn device_set_route_properties(
    device: &Device,
    route_index: i32,
    route_device: i32,
    properties: Vec<Property>,
) {
    let mut route_properties = Vec::new();
    route_properties.push(Property {
        key: libspa_sys::SPA_PARAM_ROUTE_index,
        flags: PropertyFlags::empty(),
        value: Value::Int(route_index),
    });
    route_properties.push(Property {
        key: libspa_sys::SPA_PARAM_ROUTE_device,
        flags: PropertyFlags::empty(),
        value: Value::Int(route_device),
    });
    if !properties.is_empty() {
        route_properties.push(Property {
            key: libspa_sys::SPA_PARAM_ROUTE_props,
            flags: PropertyFlags::empty(),
            value: Value::Object(Object {
                type_: libspa_sys::SPA_TYPE_OBJECT_Props,
                id: libspa_sys::SPA_PARAM_Route,
                properties,
            }),
        });
    }
    route_properties.push(Property {
        key: libspa_sys::SPA_PARAM_ROUTE_save,
        flags: PropertyFlags::empty(),
        value: Value::Bool(true),
    });
    let route_properties = route_properties;

    let values = PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(Object {
            type_: libspa_sys::SPA_TYPE_OBJECT_ParamRoute,
            id: libspa_sys::SPA_PARAM_Route,
            properties: route_properties,
        }),
    );

    if let Ok((values, _)) = values
        && let Some(pod) = Pod::from_bytes(&values.into_inner())
    {
        device.set_param(ParamType::Route, 0, pod);
    }
}
