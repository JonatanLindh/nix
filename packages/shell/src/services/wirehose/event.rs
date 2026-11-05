use pipewire::link::LinkInfoRef;

use super::{ObjectId, PropertyStore};

/// Events emitted by the PipeWire monitoring thread.
#[derive(Debug)]
pub enum Event {
    /// The PipeWire state has changed
    State(StateEvent),
    /// PipeWire reported an error. `object_id` is the object it's about: the core
    /// ([`ObjectId::CORE`]) when the connection itself is lost, which ends the session.
    /// `None` when the session couldn't be set up at all, which ends it too.
    Error {
        object_id: Option<ObjectId>,
        message: String,
    },
    /// The [StateEvent]s representing the PipeWire state at the time of
    /// connection have been sent. wirehose is listening for changes now.
    Ready,
}

#[derive(Debug)]
/// PipeWire state change events.
pub enum StateEvent {
    DeviceProperties {
        object_id: ObjectId,
        props: PropertyStore,
    },
    DeviceRoute {
        object_id: ObjectId,
        index: i32,
        device: i32,
        channel_volumes: Vec<f32>,
        mute: bool,
    },

    MetadataMetadataName {
        object_id: ObjectId,
        metadata_name: String,
    },
    MetadataProperty {
        object_id: ObjectId,
        subject: u32,
        key: Option<String>,
        value: Option<String>,
    },

    NodeProperties {
        object_id: ObjectId,
        props: PropertyStore,
    },
    NodeVolumes {
        object_id: ObjectId,
        volumes: Vec<f32>,
    },
    NodeMute {
        object_id: ObjectId,
        mute: bool,
    },

    Link {
        object_id: ObjectId,
        output_id: ObjectId,
        input_id: ObjectId,
    },

    Removed {
        object_id: ObjectId,
    },
}

impl From<&LinkInfoRef> for StateEvent {
    fn from(link_info: &LinkInfoRef) -> Self {
        StateEvent::Link {
            object_id: ObjectId::from_raw_id(link_info.id()),
            output_id: ObjectId::from_raw_id(link_info.output_node_id()),
            input_id: ObjectId::from_raw_id(link_info.input_node_id()),
        }
    }
}
