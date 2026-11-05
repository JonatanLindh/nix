use libspa::pod::{Object, Pod, Value, deserialize::PodDeserializer};

pub fn deserialize(param: Option<&Pod>) -> Option<Object> {
    param
        .and_then(|pod| PodDeserializer::deserialize_any_from(pod.as_bytes()).ok())
        .and_then(|(_, value)| match value {
            Value::Object(obj) => Some(obj),
            _ => None,
        })
}
