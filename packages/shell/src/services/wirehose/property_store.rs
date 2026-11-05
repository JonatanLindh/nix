//! A PipeWire object's "info.props": every property as its string, with accessors for the few
//! the sound service reads. (wiremix generated typed accessors for every standard key; only these
//! are used here.)

use std::collections::HashMap;
use std::str::FromStr;

use libspa::utils::dict::DictRef;

use super::ObjectId;

#[derive(Default, Debug, Clone)]
pub struct PropertyStore {
    properties: HashMap<String, String>,
    /// "device.id", parsed.
    device_id: Option<ObjectId>,
    /// "card.profile.device" (set by WirePlumber), parsed.
    card_profile_device: Option<i32>,
}

impl From<&DictRef> for PropertyStore {
    fn from(dict: &DictRef) -> Self {
        let properties: HashMap<String, String> = dict
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        fn parsed<T: FromStr>(properties: &HashMap<String, String>, key: &str) -> Option<T> {
            properties.get(key)?.parse().ok()
        }
        PropertyStore {
            device_id: parsed(&properties, "device.id"),
            card_profile_device: parsed(&properties, "card.profile.device"),
            properties,
        }
    }
}

impl PropertyStore {
    /// Any property, as its string.
    pub fn raw(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(String::as_str)
    }

    pub fn node_name(&self) -> Option<&String> {
        self.properties.get("node.name")
    }

    pub fn media_class(&self) -> Option<&String> {
        self.properties.get("media.class")
    }

    /// The device a node belongs to.
    pub fn device_id(&self) -> Option<&ObjectId> {
        self.device_id.as_ref()
    }

    /// Which of its card's devices a node is: the route with this device is the node's.
    pub fn card_profile_device(&self) -> Option<&i32> {
        self.card_profile_device.as_ref()
    }
}

impl FromStr for ObjectId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u32::from_str(s).map(ObjectId::from_raw_id)
    }
}
