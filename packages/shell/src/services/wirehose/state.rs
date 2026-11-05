//! Representation of PipeWire state.

use std::collections::HashMap;

use super::{ObjectId, PropertyStore, StateEvent};

#[derive(Debug)]
pub struct Route {
    pub index: i32,
    pub device: i32,
    pub volumes: Vec<f32>,
    pub mute: bool,
}

#[derive(Default, Debug)]
pub struct Device {
    pub props: PropertyStore,
    /// The active routes, by their device.
    pub routes: HashMap<i32, Route>,
}

#[derive(Default, Debug)]
pub struct Node {
    pub props: PropertyStore,
    pub volumes: Option<Vec<f32>>,
    pub mute: Option<bool>,
}

#[derive(Debug)]
pub struct Link {
    pub output_id: ObjectId,
    pub input_id: ObjectId,
}

#[derive(Default, Debug)]
pub struct Metadata {
    pub object_id: ObjectId,
    pub metadata_name: Option<String>,
    /// Properties for each subject
    pub properties: HashMap<u32, HashMap<String, String>>,
}

#[derive(Default)]
/// PipeWire state, maintained from [`StateEvent`]s from the
/// [`wirehose`](`super`) module.
///
/// This is primarily for maintaining a representation of the PipeWire state.
pub struct State {
    pub nodes: HashMap<ObjectId, Node>,
    pub devices: HashMap<ObjectId, Device>,
    pub links: HashMap<ObjectId, Link>,
    pub metadatas: HashMap<ObjectId, Metadata>,
    pub metadatas_by_name: HashMap<String, ObjectId>,
}

impl State {
    /// Update the state based on the supplied event.
    pub fn update(&mut self, event: StateEvent) {
        match event {
            StateEvent::DeviceProperties { object_id, props } => {
                self.device_entry(object_id).props = props;
            }
            StateEvent::DeviceRoute {
                object_id: id,
                index,
                device,
                channel_volumes,
                mute,
            } => {
                self.device_entry(id).routes.insert(
                    device,
                    Route {
                        index,
                        device,
                        volumes: channel_volumes,
                        mute,
                    },
                );
            }
            StateEvent::NodeProperties { object_id, props } => {
                self.node_entry(object_id).props = props;
            }
            StateEvent::NodeMute { object_id, mute } => {
                self.node_entry(object_id).mute = Some(mute);
            }
            StateEvent::NodeVolumes { object_id, volumes } => {
                self.node_entry(object_id).volumes = Some(volumes);
            }
            StateEvent::Link {
                object_id,
                output_id,
                input_id,
            } => {
                self.links.insert(
                    object_id,
                    Link {
                        output_id,
                        input_id,
                    },
                );
            }
            StateEvent::MetadataMetadataName {
                object_id,
                metadata_name,
            } => {
                self.metadata_entry(object_id).metadata_name = Some(metadata_name.clone());
                self.metadatas_by_name.insert(metadata_name, object_id);
            }
            StateEvent::MetadataProperty {
                object_id,
                subject,
                key,
                value,
            } => {
                let properties = self
                    .metadata_entry(object_id)
                    .properties
                    .entry(subject)
                    .or_default();
                match key {
                    Some(key) => {
                        match value {
                            Some(value) => properties.insert(key, value.clone()),
                            None => properties.remove(&key),
                        };
                    }
                    None => properties.clear(),
                };
            }
            StateEvent::Removed { object_id } => {
                self.links.remove(&object_id);
                self.devices.remove(&object_id);
                self.nodes.remove(&object_id);

                if let Some(metadata) = self.metadatas.remove(&object_id)
                    && let Some(metadata_name) = metadata.metadata_name
                {
                    self.metadatas_by_name.remove(&metadata_name);
                }
            }
        }
    }

    pub fn get_metadata_by_name(&self, metadata_name: &str) -> Option<&Metadata> {
        self.metadatas
            .get(self.metadatas_by_name.get(metadata_name)?)
    }

    fn node_entry(&mut self, object_id: ObjectId) -> &mut Node {
        self.nodes.entry(object_id).or_default()
    }

    fn device_entry(&mut self, object_id: ObjectId) -> &mut Device {
        self.devices.entry(object_id).or_default()
    }

    fn metadata_entry(&mut self, object_id: ObjectId) -> &mut Metadata {
        self.metadatas.entry(object_id).or_insert_with(|| Metadata {
            object_id,
            ..Default::default()
        })
    }

    /// Returns the objects that the given object outputs to.
    pub fn outputs(&self, object_id: ObjectId) -> Vec<ObjectId> {
        self.links
            .iter()
            .filter(|(_key, l)| l.output_id == object_id)
            .map(|(_key, l)| l.input_id)
            .collect()
    }

    /// Returns the objects that input to the given object.
    pub fn inputs(&self, object_id: ObjectId) -> Vec<ObjectId> {
        self.links
            .iter()
            .filter(|(_key, l)| l.input_id == object_id)
            .map(|(_key, l)| l.output_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_metadata_insert() {
        let mut state = State::default();
        let object_id = ObjectId::from_raw_id(0);
        let metadata_name = String::from("metadata0");
        state.update(StateEvent::MetadataMetadataName {
            object_id,
            metadata_name: metadata_name.clone(),
        });

        let metadata = state.metadatas.get(&object_id).unwrap();
        assert_eq!(metadata.metadata_name, Some(metadata_name.clone()));

        let metadata = state.get_metadata_by_name(&metadata_name).unwrap();
        assert_eq!(metadata.metadata_name, Some(metadata_name));
    }

    #[test]
    fn state_metadata_remove() {
        let mut state = State::default();
        let object_id = ObjectId::from_raw_id(0);
        let metadata_name = String::from("metadata0");
        state.update(StateEvent::MetadataMetadataName {
            object_id,
            metadata_name: metadata_name.clone(),
        });

        state.update(StateEvent::Removed { object_id });

        assert!(!state.metadatas.contains_key(&object_id));
        assert!(!state.metadatas_by_name.contains_key(&metadata_name));
        assert!(state.get_metadata_by_name(&metadata_name).is_none());
    }

    fn get_metadata_properties<'a>(
        state: &'a State,
        object_id: &ObjectId,
        subject: u32,
    ) -> &'a HashMap<String, String> {
        state
            .metadatas
            .get(object_id)
            .unwrap()
            .properties
            .get(&subject)
            .unwrap()
    }

    #[test]
    fn state_metadata_clear_property() {
        let mut state = State::default();
        let object_id = ObjectId::from_raw_id(0);
        let metadata_name = String::from("metadata0");
        state.update(StateEvent::MetadataMetadataName {
            object_id,
            metadata_name: metadata_name.clone(),
        });

        let key = String::from("key");
        let value = String::from("value");

        state.update(StateEvent::MetadataProperty {
            object_id,
            subject: 0,
            key: Some(key.clone()),
            value: Some(value.clone()),
        });
        state.update(StateEvent::MetadataProperty {
            object_id,
            subject: 1,
            key: Some(key.clone()),
            value: Some(value.clone()),
        });
        assert_eq!(
            get_metadata_properties(&state, &object_id, 0).get(&key),
            Some(&value)
        );
        assert_eq!(
            get_metadata_properties(&state, &object_id, 1).get(&key),
            Some(&value)
        );

        state.update(StateEvent::MetadataProperty {
            object_id,
            subject: 0,
            key: Some(key.clone()),
            value: None,
        });
        assert_eq!(
            get_metadata_properties(&state, &object_id, 0).get(&key),
            None
        );
        assert_eq!(
            get_metadata_properties(&state, &object_id, 1).get(&key),
            Some(&value)
        );
    }

    #[test]
    fn state_metadata_clear_all_properties() {
        let mut state = State::default();
        let object_id = ObjectId::from_raw_id(0);
        let metadata_name = String::from("metadata0");
        state.update(StateEvent::MetadataMetadataName {
            object_id,
            metadata_name: metadata_name.clone(),
        });

        let key = String::from("key");
        let value = String::from("value");

        state.update(StateEvent::MetadataProperty {
            object_id,
            subject: 0,
            key: Some(key.clone()),
            value: Some(value.clone()),
        });
        state.update(StateEvent::MetadataProperty {
            object_id,
            subject: 1,
            key: Some(key.clone()),
            value: Some(value.clone()),
        });
        assert!(!get_metadata_properties(&state, &object_id, 0).is_empty());
        assert!(!get_metadata_properties(&state, &object_id, 1).is_empty());

        state.update(StateEvent::MetadataProperty {
            object_id,
            subject: 0,
            key: None,
            value: None,
        });

        assert!(get_metadata_properties(&state, &object_id, 0).is_empty());
        assert!(!get_metadata_properties(&state, &object_id, 1).is_empty());
    }
}
