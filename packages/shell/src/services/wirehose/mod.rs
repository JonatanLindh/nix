//! An event-based wrapper around pipewire-rs: one thread per [`Session`] keeps a connection,
//! reports the graph (nodes, devices and their routes, links, metadata) as [`StateEvent`]s that
//! [`state::State`] folds into a picture of it, and runs the few commands the sound service needs.
//!
//! Ported from wirehose, the PipeWire layer of wiremix by Thomas Sowell
//! (<https://github.com/tsowell/wiremix>, version 0.11.0), under the MIT license (see
//! `LICENSE-MIT` here; it's also available under Apache-2.0). Changed from the original:
//!
//! - PipeWire is initialised once and never deinitialised, so a session can end (and another
//!   start) without breaking every other PipeWire client in the process.
//! - Errors carry the object they're about, and only one about the core (the connection lost)
//!   ends the session; one node failing used to look the same as PipeWire going away.
//! - A session that can't be set up says so with an error, rather than its thread just ending.
//! - Peak capture, client tracking, route and profile switching, and the `CommandSender` trait
//!   are gone: the shell doesn't use them.

mod command;
mod deserialize;
mod device;
mod event;
mod event_sender;
mod execute;
mod link;
mod metadata;
mod node;
mod object_id;
mod property_store;
mod proxy_registry;
mod session;
pub mod state;
mod sync_registry;

pub use event::{Event, StateEvent};
pub use event_sender::EventHandler;
pub use object_id::ObjectId;
pub use property_store::PropertyStore;
pub use session::Session;
