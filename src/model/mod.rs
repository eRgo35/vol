//! PipeWire-derived state as plain data (`docs/specs/01-architecture.md` §4–§5).
//!
//! Pure: nothing here imports the PipeWire bindings, the terminal stack or the
//! async runtime (VOL-ARCH-001, checked by `tests/layering.rs`).

pub mod device;
pub mod ids;
pub mod link;
pub mod metadata;
pub mod node;
pub mod port;
pub mod props;
pub mod store;
pub mod view;
pub mod volume;

pub use device::{ActiveRoute, Availability, Device, Profile, RouteInfo, is_audio_device};
pub use ids::{ObjectId, Serial};
pub use link::Link;
pub use metadata::{Defaults, StreamTarget, parse_default_name};
pub use node::{Channel, Node, NodeAudio, NodeKind, NodeState};
pub use port::{Port, PortDir};
pub use props::{PropMap, PropMapExt};
pub use store::{ConnState, Store};
