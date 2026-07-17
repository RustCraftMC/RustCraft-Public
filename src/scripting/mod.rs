//! Sandboxed, client-only Lua mod runtime.

pub mod api;
pub mod callback;
pub mod config;
pub mod errors;
pub mod event_bus;
pub mod loader;
pub mod manager;
pub mod manifest;
pub mod permissions;
pub mod profiler;
pub mod protocol;
pub mod runtime;
pub mod scheduler;

pub use api::context::{
    CameraMode, ClientCommand, ClientSettingsSnapshot, ClientSnapshot, ConnectionSnapshot,
    PlayerActionSnapshot, PlayerCapabilitiesSnapshot, PlayerExperienceSnapshot,
    PlayerMovementSnapshot, PlayerRotationSnapshot, PlayerSnapshot, PlayerVitalsSnapshot,
    QueuedClientCommand, Vec3Snapshot, WindowSnapshot,
};
pub use api::input::{InputConsumeRequest, InputEdge, InputSnapshot};
pub use api::resources::{ResourceRegistration, ResourceRegistrationKind};
pub use api::ui::{UiCommand, UiSnapshot};
pub use api::world::{BlockSnapshot, EntitySnapshot, WeatherSnapshot, WorldSnapshot};
pub use config::{ConfigChoice, ConfigEntryKind, ConfigEntrySnapshot, ConfigValue};
pub use event_bus::{EventOutcome, ScriptEvent};
pub use manager::{LoadReport, LoadedModInfo, QueuedUiCommand, ScriptManager};
pub use manifest::{ModId, ModManifest};
pub use permissions::{Permission, PermissionPolicy, PermissionSet};

pub const API_VERSION: u32 = 1;
