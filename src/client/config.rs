use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::Path;

use crate::client::keybind::KeyBindings;

const CONFIG_PATH: &str = "options.json";
pub const UNLIMITED_FRAMERATE: u32 = 1000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    pub username: String,
    pub language: String,
    pub gui_scale: u32,
    pub render_distance: u8,
    pub smooth_lighting: bool,
    #[serde(default, deserialize_with = "deserialize_particles")]
    pub particles: ParticleSetting,
    pub master_volume: f32,
    pub music_volume: f32,
    pub blocks_volume: f32,
    pub hostile_volume: f32,
    pub friendly_volume: f32,
    pub players_volume: f32,
    pub ambient_volume: f32,
    pub weather_volume: f32,
    pub ui_volume: f32,
    /// Vanilla `GameSettings.mouseSensitivity`, normalized to 0.0..=1.0.
    pub mouse_sensitivity: f32,
    pub invert_mouse: bool,
    /// Independent controller camera multiplier, normalized like vanilla
    /// mouse sensitivity (0.5 is 100%).
    pub gamepad_look_sensitivity: f32,
    /// Virtual cursor speed in menus and inventories, 0.0..=1.0.
    pub gamepad_cursor_speed: f32,
    pub fov: f32,
    /// Vanilla `GameSettings.gamma` / Brightness option, normalized to 0.0..=1.0.
    #[serde(default)]
    pub gamma: f32,
    pub max_framerate: u32,
    pub clouds: bool,
    #[serde(default = "default_weather_effects")]
    pub weather_effects: bool,
    pub entity_shadows: bool,
    pub view_bobbing: bool,
    pub advanced_tooltips: bool,
    /// Fraction of the window used by the chat panel (0.1..=1.0).
    pub chat_width: f32,
    /// Number of wrapped chat rows retained in the visible panel (1..=30).
    pub chat_height: u8,
    pub chat_background: bool,
    pub chat_overlay: bool,
    pub chat_player_avatars: bool,
    pub tab_player_avatars: bool,
    pub better_grass: bool,
    pub connected_textures: bool,
    pub skin_parts: u8,
    pub keybinds: KeyBindings,
    #[serde(default)]
    pub enabled_resource_packs: Vec<String>,
    #[serde(default)]
    pub shader_pack: Option<String>,
    #[serde(default)]
    pub fsr3_enabled: bool,
    #[serde(default = "default_audio_device")]
    pub audio_device: String,
    /// Map of mod-id → enabled state.  Entries for mods that no longer exist
    /// are cleaned up when the mod list is refreshed.
    #[serde(default)]
    pub mod_enabled: std::collections::HashMap<String, bool>,
}

fn default_audio_device() -> String {
    "default".to_string()
}

fn default_weather_effects() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParticleSetting {
    All,
    Decreased,
    Minimal,
}

impl ParticleSetting {
    pub fn next(self) -> Self {
        match self {
            ParticleSetting::All => ParticleSetting::Decreased,
            ParticleSetting::Decreased => ParticleSetting::Minimal,
            ParticleSetting::Minimal => ParticleSetting::All,
        }
    }

    pub fn enabled(self) -> bool {
        !matches!(self, ParticleSetting::Minimal)
    }

    pub fn label(self) -> &'static str {
        match self {
            ParticleSetting::All => "All",
            ParticleSetting::Decreased => "Decreased",
            ParticleSetting::Minimal => "Minimal",
        }
    }
}

impl Default for ParticleSetting {
    fn default() -> Self {
        Self::All
    }
}

fn deserialize_particles<'de, D>(deserializer: D) -> Result<ParticleSetting, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Bool(true) => ParticleSetting::All,
        serde_json::Value::Bool(false) => ParticleSetting::Minimal,
        serde_json::Value::String(s) if s.eq_ignore_ascii_case("decreased") => {
            ParticleSetting::Decreased
        }
        serde_json::Value::String(s) if s.eq_ignore_ascii_case("minimal") => {
            ParticleSetting::Minimal
        }
        _ => ParticleSetting::All,
    })
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            username: std::env::var("RUSTCRAFT_USERNAME")
                .unwrap_or_else(|_| "RustCraft".to_string()),
            language: "zh_CN".to_string(),
            gui_scale: 2,
            render_distance: 8,
            smooth_lighting: true,
            particles: ParticleSetting::All,
            master_volume: 1.0,
            music_volume: 1.0,
            blocks_volume: 1.0,
            hostile_volume: 1.0,
            friendly_volume: 1.0,
            players_volume: 1.0,
            ambient_volume: 1.0,
            weather_volume: 1.0,
            ui_volume: 1.0,
            mouse_sensitivity: 0.5,
            invert_mouse: false,
            gamepad_look_sensitivity: 0.5,
            // The former fixed 12 px/frame was needlessly fast.  This gives a
            // controllable default of 3 px/frame at full stick deflection.
            gamepad_cursor_speed: 0.25,
            fov: 70.0,
            gamma: 0.0,
            max_framerate: 165,
            clouds: true,
            weather_effects: true,
            entity_shadows: true,
            view_bobbing: true,
            advanced_tooltips: false,
            chat_width: 0.4,
            chat_height: 12,
            chat_background: true,
            chat_overlay: true,
            chat_player_avatars: true,
            tab_player_avatars: true,
            better_grass: false,
            connected_textures: true,
            skin_parts: 0x7f,
            keybinds: KeyBindings::defaults(),
            enabled_resource_packs: Vec::new(),
            shader_pack: None,
            fsr3_enabled: false,
            audio_device: default_audio_device(),
            mod_enabled: std::collections::HashMap::new(),
        }
    }
}

impl ClientConfig {
    pub fn load_default() -> Self {
        let path = Path::new(CONFIG_PATH);
        let mut config = match fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(config) => {
                    log::info!("loaded client configuration from {}", path.display());
                    config
                }
                Err(error) => {
                    log::warn!(
                        "failed to parse client configuration {}: {}; using defaults",
                        path.display(),
                        error
                    );
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                log::info!(
                    "client configuration {} does not exist; using defaults",
                    path.display()
                );
                Self::default()
            }
            Err(error) => {
                log::warn!(
                    "failed to read client configuration {}: {}; using defaults",
                    path.display(),
                    error
                );
                Self::default()
            }
        };
        config.keybinds = config.keybinds.merged_with_defaults();
        config
    }

    pub fn save_default(&self) {
        let path = Path::new(CONFIG_PATH);
        match serde_json::to_string_pretty(self) {
            Ok(text) => match fs::write(path, text) {
                Err(error) => log::error!(
                    "failed to write client configuration {}: {}",
                    path.display(),
                    error
                ),
                _ => {}
            },
            Err(error) => log::error!("failed to serialize client configuration: {error}"),
        }
    }
}
