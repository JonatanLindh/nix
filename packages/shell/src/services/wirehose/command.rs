//! What the session can be asked to do, run on its PipeWire thread.

use super::ObjectId;

pub enum Command {
    NodeMute(ObjectId, bool),
    /// Device, route index, route device, mute.
    DeviceMute(ObjectId, i32, i32, bool),
    NodeVolumes(ObjectId, Vec<f32>),
    /// Device, route index, route device, per-channel volumes.
    DeviceVolumes(ObjectId, i32, i32, Vec<f32>),
    /// Metadata, subject, key, type, value.
    MetadataSetProperty(ObjectId, u32, String, Option<String>, Option<String>),
}
