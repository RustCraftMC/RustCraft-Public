//! Shared, owned data exchanged between the game client and sandboxed Lua states.
//!
//! Lua only receives copies of [`ClientSnapshot`] and can only append validated
//! [`ClientCommand`] values. It never receives a client pointer or a mutable game
//! object.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

const MAX_QUEUED_CLIENT_COMMANDS: usize = 1_024;
const MAX_QUEUED_CLIENT_COMMANDS_PER_MOD: usize = 128;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3Snapshot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<[f64; 3]> for Vec3Snapshot {
    fn from(value: [f64; 3]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowSnapshot {
    pub width: u32,
    pub height: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub scale_factor: f64,
    pub focused: bool,
    pub fullscreen: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraMode {
    #[default]
    FirstPerson,
    ThirdPersonBack,
    ThirdPersonFront,
}

impl CameraMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FirstPerson => "first_person",
            Self::ThirdPersonBack => "third_person_back",
            Self::ThirdPersonFront => "third_person_front",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "first_person" => Some(Self::FirstPerson),
            "third_person_back" => Some(Self::ThirdPersonBack),
            "third_person_front" => Some(Self::ThirdPersonFront),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientSettingsSnapshot {
    pub fov_degrees: f32,
    pub gui_scale: f32,
    pub view_bobbing: bool,
    pub hud_visible: bool,
    pub camera_mode: CameraMode,
}

impl Default for ClientSettingsSnapshot {
    fn default() -> Self {
        Self {
            fov_degrees: 70.0,
            gui_scale: 1.0,
            view_bobbing: true,
            hud_visible: true,
            camera_mode: CameraMode::FirstPerson,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionSnapshot {
    /// Stable values are `disconnected`, `connecting`, `login`, and `play`.
    pub state: String,
    pub server_address: Option<String>,
    pub protocol_version: Option<i32>,
    pub protocol_name: Option<String>,
    pub latency_ms: Option<u32>,
    /// `None` when the current connection implementation cannot report it.
    pub encrypted: Option<bool>,
    pub server_brand: Option<String>,
}

impl Default for ConnectionSnapshot {
    fn default() -> Self {
        Self {
            state: "disconnected".into(),
            server_address: None,
            protocol_version: None,
            protocol_name: None,
            latency_ms: None,
            encrypted: None,
            server_brand: None,
        }
    }
}

impl ConnectionSnapshot {
    pub fn is_connected(&self) -> bool {
        self.state == "play"
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerRotationSnapshot {
    pub yaw: f32,
    pub pitch: f32,
    pub body_yaw: f32,
    pub head_yaw: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerMovementSnapshot {
    pub on_ground: bool,
    pub collided_horizontally: bool,
    pub sneaking: bool,
    pub sprinting: bool,
    pub jumping: bool,
    pub in_water: bool,
    pub in_lava: bool,
    pub fall_distance: f32,
    pub input_strafe: f32,
    pub input_forward: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerActionSnapshot {
    pub using_item: bool,
    pub use_action: Option<String>,
    pub use_ticks: u32,
    pub blocking: bool,
    pub swinging: bool,
    pub swing_progress: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerCapabilitiesSnapshot {
    pub invulnerable: bool,
    pub creative_mode: bool,
    pub allow_flying: bool,
    pub flying: bool,
    pub walk_speed: f32,
    pub fly_speed: f32,
}

impl Default for PlayerCapabilitiesSnapshot {
    fn default() -> Self {
        Self {
            invulnerable: false,
            creative_mode: false,
            allow_flying: false,
            flying: false,
            walk_speed: 0.1,
            fly_speed: 0.05,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerVitalsSnapshot {
    pub health: f32,
    /// `None` until the client has an authoritative generic.maxHealth value.
    pub max_health: Option<f32>,
    /// `None` until the client has an authoritative absorption value.
    pub absorption: Option<f32>,
    pub food: i32,
    pub saturation: f32,
    pub oxygen: i32,
}

impl Default for PlayerVitalsSnapshot {
    fn default() -> Self {
        Self {
            health: 20.0,
            max_health: None,
            absorption: None,
            food: 20,
            saturation: 5.0,
            oxygen: 300,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerExperienceSnapshot {
    pub level: i32,
    pub progress: f32,
    pub total: i32,
}

/// A read-only snapshot of the local player.
///
/// Authentication tokens and mutable inventory/network handles are
/// intentionally absent. The client constructs this value from authoritative
/// local state once per tick or frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub entity_id: Option<i32>,
    pub name: Option<String>,
    pub gamemode: u8,
    pub dimension: i8,
    pub position: Vec3Snapshot,
    pub previous_position: Vec3Snapshot,
    pub velocity: Vec3Snapshot,
    pub rotation: PlayerRotationSnapshot,
    pub movement: PlayerMovementSnapshot,
    pub action: PlayerActionSnapshot,
    pub capabilities: PlayerCapabilitiesSnapshot,
    pub vitals: PlayerVitalsSnapshot,
    pub experience: PlayerExperienceSnapshot,
    pub selected_hotbar_slot: u8,
}

/// A stable, owned snapshot shared by every Lua state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientSnapshot {
    pub tick: u64,
    pub frame_delta_seconds: f32,
    pub fps: f32,
    pub active_screen: String,
    pub paused: bool,
    pub window: WindowSnapshot,
    pub settings: ClientSettingsSnapshot,
    pub connection: ConnectionSnapshot,
    pub player: Option<PlayerSnapshot>,
}

/// Safe client-side effects requested by Lua with `client.modify`.
///
/// There are deliberately no movement, inventory, identity, authentication,
/// or raw-network commands in this enum.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientCommand {
    /// Internal lifecycle command used to discard every override owned by a mod.
    ClearVisualOverrides,
    SetFovOverride(Option<f32>),
    SetViewBobbingOverride(Option<bool>),
    SetHudVisibilityOverride(Option<bool>),
    SetCameraModeOverride(Option<CameraMode>),
    SetFullscreen(bool),
    SetWindowTitle(Option<String>),
    SetHurtcamOverride(Option<bool>),
    SetFovchangeOverride(Option<bool>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueuedClientCommand {
    pub mod_id: String,
    pub command: ClientCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientCommandRejection {
    GlobalQueueFull,
    ModQueueFull,
}

impl fmt::Display for ClientCommandRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GlobalQueueFull => f.write_str("client command queue limit exceeded (1024)"),
            Self::ModQueueFull => f.write_str("per-mod client command queue limit exceeded (128)"),
        }
    }
}

/// Shared bridge cloned into each Lua runtime.
///
/// The snapshot can only be replaced by Rust. Lua closures clone snapshot data
/// into fresh tables and can only append typed commands to the bounded queue.
#[derive(Clone, Default)]
pub struct SharedApiContext {
    snapshot: Rc<RefCell<ClientSnapshot>>,
    commands: Rc<RefCell<VecDeque<QueuedClientCommand>>>,
}

impl SharedApiContext {
    pub fn update_snapshot(&self, snapshot: ClientSnapshot) {
        *self.snapshot.borrow_mut() = snapshot;
    }

    pub fn snapshot(&self) -> ClientSnapshot {
        self.snapshot.borrow().clone()
    }

    pub fn enqueue_client_command(
        &self,
        mod_id: &str,
        command: ClientCommand,
    ) -> Result<(), ClientCommandRejection> {
        let mut commands = self.commands.borrow_mut();
        if !matches!(&command, ClientCommand::ClearVisualOverrides)
            && commands
                .iter()
                .filter(|queued| !matches!(&queued.command, ClientCommand::ClearVisualOverrides))
                .count()
                >= MAX_QUEUED_CLIENT_COMMANDS
        {
            return Err(ClientCommandRejection::GlobalQueueFull);
        }
        if !matches!(&command, ClientCommand::ClearVisualOverrides)
            && commands
                .iter()
                .filter(|queued| {
                    queued.mod_id == mod_id
                        && !matches!(&queued.command, ClientCommand::ClearVisualOverrides)
                })
                .count()
                >= MAX_QUEUED_CLIENT_COMMANDS_PER_MOD
        {
            return Err(ClientCommandRejection::ModQueueFull);
        }
        commands.push_back(QueuedClientCommand {
            mod_id: mod_id.to_owned(),
            command,
        });
        Ok(())
    }

    pub fn drain_client_commands(&self) -> Vec<QueuedClientCommand> {
        self.commands.borrow_mut().drain(..).collect()
    }

    pub fn clear_client_commands_for(&self, mod_id: &str) {
        self.commands
            .borrow_mut()
            .retain(|queued| queued.mod_id != mod_id);
    }

    pub(crate) fn client_command_checkpoint(&self) -> Vec<QueuedClientCommand> {
        self.commands.borrow().iter().cloned().collect()
    }

    pub(crate) fn restore_client_command_checkpoint(&self, checkpoint: Vec<QueuedClientCommand>) {
        *self.commands.borrow_mut() = checkpoint.into_iter().collect();
    }

    pub(crate) fn take_client_commands_for(&self, mod_id: &str) -> Vec<QueuedClientCommand> {
        let mut commands = self.commands.borrow_mut();
        let mut retained = VecDeque::with_capacity(commands.len());
        let mut taken = Vec::new();
        while let Some(command) = commands.pop_front() {
            if command.mod_id == mod_id {
                taken.push(command);
            } else {
                retained.push_back(command);
            }
        }
        *commands = retained;
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_updates_are_live_across_clones() {
        let context = SharedApiContext::default();
        let lua_view = context.clone();
        context.update_snapshot(ClientSnapshot {
            tick: 42,
            active_screen: "playing".into(),
            ..ClientSnapshot::default()
        });
        assert_eq!(lua_view.snapshot().tick, 42);
        assert_eq!(lua_view.snapshot().active_screen, "playing");
    }

    #[test]
    fn command_queue_is_tagged_bounded_and_clearable() {
        let context = SharedApiContext::default();
        for _ in 0..MAX_QUEUED_CLIENT_COMMANDS_PER_MOD {
            context
                .enqueue_client_command("visual", ClientCommand::SetFullscreen(false))
                .unwrap();
        }
        assert_eq!(
            context.enqueue_client_command("visual", ClientCommand::SetFullscreen(true)),
            Err(ClientCommandRejection::ModQueueFull)
        );
        context
            .enqueue_client_command("other", ClientCommand::SetFullscreen(true))
            .unwrap();
        context.clear_client_commands_for("visual");
        assert_eq!(
            context.drain_client_commands(),
            vec![QueuedClientCommand {
                mod_id: "other".into(),
                command: ClientCommand::SetFullscreen(true),
            }]
        );
    }
}
