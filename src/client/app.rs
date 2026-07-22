use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use std::collections::BTreeMap;
use std::path::Path;

use crate::assets;
use crate::assets::resource_pack::ResourcePack;
use crate::audio;
use crate::audio::AudioBackend;
use crate::client;
use crate::client::gamepad::GamepadInput;
use crate::client::interaction::DigController;
use crate::client::inventory::Inventory;
use crate::client::keybind::Action;
use crate::client::keybind::{ControlDevice, InputState, KeyBindings};
use crate::client::state::GameState;
use crate::client::tick::TickTimer;
use crate::net;
use crate::render;
use crate::ui;
use crate::world;

mod block_interaction;
mod book;
mod frame;
mod input;
mod inventory_interaction;
mod menu_actions;
mod script_bridge;
mod tasks;

pub(super) const WINDOW_TITLE: &str = concat!("RustCraft v", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Copy, Default)]
struct AppFrameProfile {
    total_us: u64,
    tasks_us: u64,
    network_us: u64,
    world_us: u64,
    tick_us: u64,
    sync_us: u64,
    render_us: u64,
    other_us: u64,
    script_us: u64,
    script_callbacks: u32,
    script_slow_callbacks: u32,
    network_debug: crate::client::network::NetworkDebugProfile,
}

struct PlayerSkinProfileCache {
    identity: String,
    skin_property: Option<String>,
}

impl PlayerSkinProfileCache {
    fn new(
        identity: String,
        listed_property: Option<Option<&str>>,
        entity_property: Option<&str>,
    ) -> Self {
        Self {
            identity,
            skin_property: listed_property
                .flatten()
                .or(entity_property)
                .map(str::to_owned),
        }
    }

    fn update(
        &mut self,
        identity: String,
        listed_property: Option<Option<&str>>,
        entity_property: Option<&str>,
    ) {
        if self.identity != identity {
            self.identity = identity;
            self.skin_property = listed_property
                .flatten()
                .or(entity_property)
                .map(str::to_owned);
        } else if let Some(property) = listed_property {
            self.skin_property = property.or(entity_property).map(str::to_owned);
        } else if self.skin_property.is_none() {
            self.skin_property = entity_property.map(str::to_owned);
        }
    }
}

#[cfg(test)]
mod player_skin_profile_tests {
    use super::PlayerSkinProfileCache;

    #[test]
    fn tab_removal_keeps_profile_until_entity_identity_changes() {
        let mut profile =
            PlayerSkinProfileCache::new("uuid:one".to_string(), Some(Some("texture")), None);

        profile.update("uuid:one".to_string(), None, None);
        assert_eq!(profile.skin_property.as_deref(), Some("texture"));

        profile.update("uuid:one".to_string(), Some(None), None);
        assert_eq!(profile.skin_property, None);

        profile.skin_property = Some("old-texture".to_string());
        profile.update("uuid:two".to_string(), None, None);
        assert_eq!(profile.skin_property, None);
    }

    #[test]
    fn entity_profile_survives_tab_removal() {
        let mut profile =
            PlayerSkinProfileCache::new("uuid:npc".to_string(), None, Some("npc-texture"));

        profile.update("uuid:npc".to_string(), None, Some("npc-texture"));

        assert_eq!(profile.skin_property.as_deref(), Some("npc-texture"));
    }
}

/// Aggregates input-related state: key bindings, input state, gamepad,
/// mouse position/delta, and capture state. Extracted from App to establish
/// a clear subsystem boundary for the input layer.
pub(crate) struct InputController {
    pub keybinds: KeyBindings,
    pub input: InputState,
    pub gamepad: GamepadInput,
    pub control_device: ControlDevice,
    pub rebinding_action: Option<Action>,
    pub mouse_dx: f64,
    pub mouse_dy: f64,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub mouse_captured: bool,
}

/// Aggregates network/connection state: the active connection, network
/// protocol state, and background connection/refresh tasks. Extracted from
/// App to establish a clear subsystem boundary for networking.
pub(crate) struct NetworkController {
    pub connection: Option<net::connection::Connection>,
    pub network_state: client::network::ClientNetworkState,
    pub connect_task: Option<tasks::ConnectTask>,
    pub server_refresh_task: Option<tasks::ServerRefreshTask>,
}

pub struct App {
    renderer: Option<render::Renderer>,
    window: Option<Window>,
    player: client::player::Player,
    input_ctrl: InputController,
    inventory: Inventory,
    inventory_open: bool,
    player_list_open: bool,
    chat_open: bool,
    chat_input: String,
    chat_history: Vec<String>,
    chat_history_index: Option<usize>,
    chat_draft: Option<String>,
    book_editor: Option<client::book::BookEditor>,
    ctrl_held: bool,
    attack_held: bool,
    use_held: bool,
    /// Right-click press events accumulated since the previous fixed tick.
    /// Vanilla queues KeyBinding presses and consumes them inside runTick.
    use_presses_pending: u32,
    /// Attack click events accumulated since the previous fixed tick.
    /// Vanilla clickMouse is only called inside runTick (20 Hz), so rapid
    /// mouse clicks are coalesced to at most one entity attack per tick.
    pending_attacks: u8,
    /// Container click-window packets queued from the render-frame callback.
    /// Vanilla GuiContainer.mouseClicked sends C0E inside runTick (20 Hz),
    /// so rapid inventory clicks must be deferred to the tick boundary.
    pending_click_windows: Vec<Vec<u8>>,
    /// Vanilla isSwingInProgress — guards C0A animation packets so they are
    /// not sent more than once per 3 ticks (getArmSwingAnimationEnd / 2 = 3).
    is_swing_in_progress: bool,
    /// Vanilla swingProgressInt — countdown from -1 through getArmSwingAnimationEnd (6).
    swing_progress_int: i32,
    /// Prevent a held controller attack trigger from turning into block mining
    /// after it has already attacked an entity.
    gamepad_attack_consumed: bool,
    prev_attack_held: bool,
    /// Timer for continuous item use (eating, drinking, bow drawing) in seconds.
    item_use_timer: f32,
    prev_item_use_timer: f32,
    fov_target: f32,
    /// Cooldown ticks after eating completes before next auto-start (vanilla: ~4 ticks gap).
    food_cooldown: u8,
    /// Whether an item use action (eating/bow) is currently in progress.
    item_use_active: bool,
    /// A C07 release was sent since the previous game tick. Interaction input
    /// in this interval must not append C02/C08 to the release packet.
    use_release_pending: bool,
    /// MCP: rightClickDelayTimer — 4 tick cooldown after every right-click.
    item_place_delay: u8,
    /// MCP PlayerControllerMP.blockHitDelay — five ticks after a block finish.
    block_hit_delay: u8,
    /// Digging cancellation requested by render/input callbacks. It is flushed
    /// during the next fixed tick before the movement packet.
    pending_dig_cancel: Option<(
        crate::client::physics::BlockPos,
        crate::client::physics::BlockFace,
    )>,
    /// Tick cooldown for local ladder/vine climbing sounds.
    ladder_sound_cooldown: u8,
    dig: DigController,
    inventory_action_number: i16,
    net_ctrl: NetworkController,
    audio: audio::AudioBackendImpl,
    particles: client::particles::ParticleSystem,
    local_player_model: client::player_model::PlayerModel,
    local_skin: assets::skin::PlayerSkin,
    local_skin_dirty: bool,
    local_cape_pixels: Option<Vec<u8>>,
    local_cape_hash: u64,
    skin_cache: client::skin_cache::SkinCache,
    servers: client::server_list::ServerList,
    auth_task: Option<tasks::AuthTask>,
    account: Option<crate::auth::models::Account>,
    accounts: Vec<crate::auth::models::Account>,
    selected_account: usize,
    auth_status: String,
    offline_username_input: String,
    entering_offline_name: bool,
    session: client::session::SessionState,
    entities: crate::entity::EntityManager,
    scripts: crate::scripting::ScriptManager,
    config: client::config::ClientConfig,
    selected_server: usize,
    server_address: String,
    server_editor_name: String,
    server_editor_address: String,
    server_editor_address_focused: bool,
    last_server_click: Option<(usize, std::time::Instant)>,
    username: String,
    tick_timer: TickTimer,
    fps_timer: std::time::Instant,
    fps_frames: u32,
    frame_count: u64,
    last_redraw_request: std::time::Instant,
    last_script_frame: std::time::Instant,
    last_script_hud_frame: std::time::Instant,
    last_script_api_frame: std::time::Instant,
    /// Player block and nearby chunk generations backing the cached Lua block volume.
    last_script_world_block_key: Option<((i32, i32, i32), world::SnapshotRegionRevision)>,
    /// Vanilla `GameSettings.hideGUI`, toggled by F1 while playing.
    hud_hidden: bool,
    script_visual_overrides: BTreeMap<String, script_bridge::ScriptVisualOverrides>,
    script_base_camera_mode: Option<u8>,
    world: world::World,
    /// Latest completed mesh per chunk, drained to the renderer under a frame
    /// upload budget so dense server chunk bursts do not stall the event loop.
    pending_chunk_uploads: fnv::FnvHashMap<(i32, i32), world::mesh::ChunkMesh>,
    state: GameState,
    ui: ui::UiState,
    /// The option slider currently held by the left mouse button.
    active_slider: Option<u32>,
    /// Whether the vanilla creative-inventory scrollbar is being dragged.
    creative_scroll_dragging: bool,
    /// Controller quick-craft candidate: button and protocol slots visited while held.
    gamepad_inventory_drag: Option<(winit::event::MouseButton, Vec<i16>)>,
    resource_pack_list: Vec<ResourcePackInfo>,
    last_sent_abilities: Option<(u8, f32, f32)>,
    last_entity_hash: u64,
    last_static_render_tick: u64,
    last_ui_text_hash: u64,
    last_sign_hash: u64,
    last_skull_hash: u64,
    last_player_list_generation: u64,
    last_scoreboard_generation: u64,
    last_player_skin_roster_hash: u64,
    last_player_skin_generation: u64,
    last_player_skin_skull_hash: u64,
    last_skin_cache_generation: u64,
    player_skin_slims: fnv::FnvHashMap<i32, bool>,
    player_cape_ready: fnv::FnvHashSet<i32>,
    player_skin_profiles: fnv::FnvHashMap<i32, PlayerSkinProfileCache>,
    modding_status: String,
    modding_selected: usize,
    mod_config_selected: usize,
    mod_config_status: String,
    last_frame_profile: AppFrameProfile,
    last_profile_frame_start: Option<std::time::Instant>,
    next_predicted_entity_id: i32,
}

impl App {
    pub fn new() -> Self {
        log::debug!("initialising client application state");
        let mut config = client::config::ClientConfig::load_default();
        log::info!(
            "client settings: language={}, gui_scale={}, render_distance={}, smooth_lighting={}, particles={:?}, fov={:.1}, max_framerate={}, resource_packs={}, audio_device={}",
            config.language,
            config.gui_scale,
            config.render_distance,
            config.smooth_lighting,
            config.particles,
            config.fov,
            config.max_framerate,
            config.enabled_resource_packs.len(),
            config.audio_device
        );
        let account = crate::auth::cache::load_account().ok().flatten();
        let accounts = crate::auth::cache::load_accounts().unwrap_or_default();
        log::info!(
            "account cache: saved_accounts={}, active_account={}",
            accounts.len(),
            if account.is_some() { "present" } else { "none" }
        );
        let selected_account = account
            .as_ref()
            .and_then(|selected| accounts.iter().position(|item| item.uuid == selected.uuid))
            .unwrap_or(0);
        let authenticated_username = account
            .as_ref()
            .and_then(|account| account.username.clone());
        let mut world = world::World::new();
        world.set_smooth_lighting(config.smooth_lighting);
        let spawn_y = 64.0;
        let mut local_player_model = client::player_model::PlayerModel::steve();
        let local_skin = load_local_skin(account.as_ref(), &mut local_player_model);
        let local_cape_raw = account.as_ref().and_then(load_local_cape_pixels);
        let local_cape_hash = local_cape_raw.as_ref().map_or(0, |raw| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash_slice(raw, &mut h);
            std::hash::Hasher::finish(&h)
        });
        let skin_cache = client::skin_cache::SkinCache::new(&local_skin);
        // Audio uses the same enabled resource-pack stack as textures.
        let mut audio_resolver = assets::resolver::AssetResolver::with_resource_packs(
            "assets/minecraft",
            "resourcepacks",
            &config.enabled_resource_packs,
        );
        let mut audio =
            match ResourcePack::load_with_resource_packs("assets", "1.8", &mut audio_resolver) {
                Ok(pack) => {
                    log::info!(
                        "audio resource pack ready: assets={}, sound_events={}",
                        pack.index.len(),
                        pack.sounds.len()
                    );
                    audio::AudioBackendImpl::new(pack.index, pack.sounds)
                }
                Err(e) => {
                    log::error!("failed to load audio resource pack: {e}; using null audio");
                    audio::AudioBackendImpl::new_null()
                }
            };
        apply_audio_config(&mut audio, &config);
        let mut scripts = crate::scripting::ScriptManager::new("mods");
        let script_report = scripts.load_all();
        // Restore persisted enabled/disabled state, then prune missing mods.
        {
            let saved = config.mod_enabled.clone();
            for info in scripts.loaded_mods() {
                if let Some(&enabled) = saved.get(&info.id) {
                    if enabled != info.enabled {
                        let _ = if enabled {
                            scripts.enable(&info.id)
                        } else {
                            scripts.disable(&info.id)
                        };
                    }
                }
            }
            let known_ids: std::collections::HashSet<_> =
                scripts.loaded_mods().into_iter().map(|m| m.id).collect();
            config
                .mod_enabled
                .retain(|id, _| known_ids.contains(id.as_str()));
            config.save_default();
        }
        for id in &script_report.loaded {
            log::info!(target: "rustcraft::lua", "loaded mod '{id}'");
        }
        for (id, permission) in &script_report.denied_permissions {
            log::warn!(
                target: "rustcraft::lua",
                "mod '{id}' denied sensitive permission '{permission}'"
            );
        }
        for error in &script_report.errors {
            log::error!(target: "rustcraft::lua", "mod load failed: {error}");
        }
        log::info!(
            target: "rustcraft::lua",
            "Lua startup complete: loaded={}, denied_permissions={}, errors={}",
            script_report.loaded.len(),
            script_report.denied_permissions.len(),
            script_report.errors.len()
        );
        let modding_status = format!("{} mod(s) loaded", scripts.mod_count());

        App {
            renderer: None,
            window: None,
            player: client::player::Player::new(
                nalgebra::Point3::new(8.0, spawn_y, 25.0),
                1280.0 / 720.0,
            ),
            input_ctrl: InputController {
                keybinds: config.keybinds.clone(),
                input: InputState::new(),
                gamepad: GamepadInput::new(),
                control_device: ControlDevice::KeyboardMouse,
                rebinding_action: None,
                mouse_dx: 0.0,
                mouse_dy: 0.0,
                mouse_x: 0.0,
                mouse_y: 0.0,
                mouse_captured: false,
            },
            inventory: Inventory::creative(),
            inventory_open: false,
            player_list_open: false,
            chat_open: false,
            chat_input: String::new(),
            chat_history: Vec::new(),
            chat_history_index: None,
            chat_draft: None,
            book_editor: None,
            ctrl_held: false,
            attack_held: false,
            use_held: false,
            use_presses_pending: 0,
            pending_attacks: 0,
            pending_click_windows: Vec::new(),
            is_swing_in_progress: false,
            swing_progress_int: 0,
            gamepad_attack_consumed: false,
            prev_attack_held: false,
            item_use_timer: 0.0,
            prev_item_use_timer: 0.0,
            fov_target: 1.0,
            food_cooldown: 0,
            item_use_active: false,
            use_release_pending: false,
            item_place_delay: 0,
            block_hit_delay: 0,
            pending_dig_cancel: None,
            ladder_sound_cooldown: 0,
            dig: DigController::new(),
            inventory_action_number: 1,
            net_ctrl: NetworkController {
                connection: None,
                network_state: client::network::ClientNetworkState::new(),
                connect_task: None,
                server_refresh_task: None,
            },
            audio,
            particles: client::particles::ParticleSystem::new(4096),
            local_player_model,
            local_skin,
            local_skin_dirty: true,
            local_cape_pixels: local_cape_raw,
            local_cape_hash,
            skin_cache,
            servers: client::server_list::ServerList::load_default(),
            auth_task: None,
            account,
            accounts,
            selected_account,
            auth_status: if authenticated_username.is_some() {
                "Microsoft account loaded from cache".to_string()
            } else {
                "No Microsoft account selected".to_string()
            },
            offline_username_input: String::new(),
            entering_offline_name: false,
            session: client::session::SessionState::new(),
            entities: {
                let mut em = crate::entity::EntityManager::new();
                em.spawn_local_player_marker();
                em
            },
            scripts,
            selected_server: 0,
            // A direct-connect address is transient UI state, not a game setting.
            // Keep the environment override useful for development without writing it
            // into options.json.
            server_address: std::env::var("RUSTCRAFT_SERVER")
                .unwrap_or_else(|_| "127.0.0.1:25565".to_string()),
            server_editor_name: String::new(),
            server_editor_address: String::new(),
            server_editor_address_focused: false,
            last_server_click: None,
            username: authenticated_username.unwrap_or_else(|| config.username.clone()),
            tick_timer: TickTimer::new(),
            fps_timer: std::time::Instant::now(),
            fps_frames: 0,
            frame_count: 0,
            last_redraw_request: std::time::Instant::now(),
            last_script_frame: std::time::Instant::now(),
            last_script_hud_frame: std::time::Instant::now(),
            last_script_api_frame: std::time::Instant::now(),
            last_script_world_block_key: None,
            hud_hidden: false,
            script_visual_overrides: BTreeMap::new(),
            script_base_camera_mode: None,
            world,
            pending_chunk_uploads: fnv::FnvHashMap::default(),
            state: GameState::MainMenu,
            ui: ui::UiState::new(&config.language),
            active_slider: None,
            creative_scroll_dragging: false,
            gamepad_inventory_drag: None,
            resource_pack_list: scan_resource_packs(&config.enabled_resource_packs),
            last_sent_abilities: None,
            last_entity_hash: 0,
            last_static_render_tick: u64::MAX,
            last_ui_text_hash: 0,
            last_sign_hash: 0,
            last_skull_hash: 0,
            last_player_list_generation: u64::MAX,
            last_scoreboard_generation: u64::MAX,
            last_player_skin_roster_hash: u64::MAX,
            last_player_skin_generation: u64::MAX,
            last_player_skin_skull_hash: u64::MAX,
            last_skin_cache_generation: u64::MAX,
            player_skin_slims: fnv::FnvHashMap::default(),
            player_cape_ready: fnv::FnvHashSet::default(),
            player_skin_profiles: fnv::FnvHashMap::default(),
            modding_status,
            modding_selected: 0,
            mod_config_selected: 0,
            mod_config_status: String::new(),
            last_frame_profile: AppFrameProfile::default(),
            last_profile_frame_start: None,
            next_predicted_entity_id: -1,
            config,
        }
    }
}

fn apply_audio_config(audio: &mut audio::AudioBackendImpl, config: &client::config::ClientConfig) {
    audio.set_volume(audio::SoundCategory::Master, config.master_volume);
    audio.set_volume(audio::SoundCategory::Music, config.music_volume);
    audio.set_volume(audio::SoundCategory::Blocks, config.blocks_volume);
    audio.set_volume(audio::SoundCategory::Hostile, config.hostile_volume);
    audio.set_volume(audio::SoundCategory::Friendly, config.friendly_volume);
    audio.set_volume(audio::SoundCategory::Players, config.players_volume);
    audio.set_volume(audio::SoundCategory::Ambient, config.ambient_volume);
    audio.set_volume(audio::SoundCategory::Weather, config.weather_volume);
    audio.set_volume(audio::SoundCategory::Ui, config.ui_volume);
}

fn load_local_skin(
    account: Option<&crate::auth::models::Account>,
    model: &mut client::player_model::PlayerModel,
) -> assets::skin::PlayerSkin {
    let mut skin = if let Some(acct) = account {
        let uuid_key = acct.uuid.as_deref().unwrap_or("default").replace('-', "");
        let skin_info = acct.skins.as_ref().and_then(|s| s.first());
        model.slim_arms = skin_info.map_or(false, |s| s.variant == "SLIM");
        let texture_key = skin_info.and_then(|s| s.url.rsplit('/').next());
        let skin_url = skin_info.map(|s| s.url.as_str());
        // Try local cached file first (new UUID-directory format)
        Some(if let Some(tk) = texture_key {
            let path = format!("assets/skins/{}/{}.png", uuid_key, tk);
            if let Ok(skin) = assets::skin::PlayerSkin::load(&path) {
                skin
            } else {
                // Fallback: old flat format
                let old_path = format!("assets/skins/{}.png", tk);
                if let Ok(skin) = assets::skin::PlayerSkin::load(&old_path) {
                    skin
                } else {
                    download_and_save_skin(skin_url, texture_key, &uuid_key)
                        .unwrap_or_else(|| fallback_skin())
                }
            }
        } else {
            download_and_save_skin(skin_url, texture_key, &uuid_key)
                .unwrap_or_else(|| fallback_skin())
        })
    } else {
        None
    }
    .unwrap_or_else(fallback_skin);

    skin
}

fn download_and_save_skin(
    skin_url: Option<&str>,
    texture_key: Option<&str>,
    uuid_key: &str,
) -> Option<assets::skin::PlayerSkin> {
    let url = skin_url?;
    let resp = reqwest::blocking::get(url).ok()?;
    let bytes = resp.bytes().ok()?;
    let tk = texture_key.unwrap_or("local");
    let path = format!("assets/skins/{}/{}.png", uuid_key, tk);
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &bytes);
    assets::skin::PlayerSkin::load(&path).ok()
}

fn fallback_skin() -> assets::skin::PlayerSkin {
    if let Ok(path) = std::env::var("RUSTCRAFT_SKIN") {
        if let Ok(skin) = assets::skin::PlayerSkin::load(path) {
            return skin;
        }
    }
    assets::skin::PlayerSkin::load("assets/skin.png")
        .or_else(|_| assets::skin::PlayerSkin::load("assets/minecraft/textures/entity/steve.png"))
        .unwrap_or_else(|_| assets::skin::PlayerSkin::default_steve())
}

fn load_local_cape_pixels(account: &crate::auth::models::Account) -> Option<Vec<u8>> {
    let cape = account
        .capes
        .as_ref()?
        .iter()
        .find(|c| c.state.eq_ignore_ascii_case("active"))?;
    let texture_key = cape.url.rsplit('/').next()?;
    let path = format!("assets/capes/{texture_key}.png");
    if let Ok(img) = image::open(&path) {
        return Some(crate::client::skin_cache::normalize_cape_image(img));
    }
    let resp = reqwest::blocking::get(&cape.url).ok()?;
    let bytes = resp.bytes().ok()?;
    let _ = std::fs::create_dir_all("assets/capes");
    let _ = std::fs::write(&path, &bytes);
    image::load_from_memory(&bytes)
        .ok()
        .map(crate::client::skin_cache::normalize_cape_image)
}

#[derive(Clone, Debug)]
pub struct ResourcePackInfo {
    pub name: String,
    pub enabled: bool,
    pub is_default: bool,
    pub description: String,
    pub pack_format: i32,
    /// 32×32 RGBA pack icon pixels, or None if no icon
    pub icon: Option<[u8; 4096]>,
}

/// Expected pack format for MC 1.8.9
const MC_PACK_FORMAT: i32 = 1;

fn scan_resource_packs(enabled: &[String]) -> Vec<ResourcePackInfo> {
    let rp_dir = Path::new("resourcepacks");
    if !rp_dir.is_dir() {
        return Vec::new();
    }
    let mut packs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(rp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_zip = path.extension().and_then(|s| s.to_str()) == Some("zip");
            if is_zip || (path.is_dir() && path.file_stem().map_or(false, |s| s != "resourcepacks"))
            {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    let enabled_flag = enabled.iter().any(|e| e == name);
                    let meta_path = if is_zip {
                        path.clone()
                    } else {
                        path.join("pack.mcmeta")
                    };
                    let (desc, pack_format) = read_pack_meta(&meta_path, is_zip);
                    let icon = read_pack_icon(&path, is_zip);
                    packs.push(ResourcePackInfo {
                        name: name.to_string(),
                        enabled: enabled_flag,
                        is_default: false,
                        description: desc,
                        pack_format,
                        icon,
                    });
                }
            }
        }
    }
    packs
}

fn read_pack_icon(path: &std::path::Path, is_zip: bool) -> Option<[u8; 4096]> {
    let data = if is_zip {
        if let Ok(file) = std::fs::File::open(path) {
            if let Ok(mut zip) = zip::ZipArchive::new(file) {
                if let Ok(mut entry) = zip.by_name("pack.png") {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut entry, &mut buf).ok();
                    buf
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        let icon_path = path.join("pack.png");
        if icon_path.exists() {
            std::fs::read(&icon_path).ok()?
        } else {
            return None;
        }
    };
    if let Ok(img) = image::load_from_memory(&data) {
        let resized = img.resize_exact(32, 32, image::imageops::FilterType::Nearest);
        let rgba = resized.to_rgba8();
        let mut pixels = [0u8; 4096];
        let src = rgba.into_raw();
        let copy_len = src.len().min(4096);
        pixels[..copy_len].copy_from_slice(&src[..copy_len]);
        Some(pixels)
    } else {
        None
    }
}

fn read_pack_meta(path: &std::path::Path, is_zip: bool) -> (String, i32) {
    let text = if is_zip {
        if let Ok(file) = std::fs::File::open(path) {
            if let Ok(mut zip) = zip::ZipArchive::new(file) {
                if let Ok(mut entry) = zip.by_name("pack.mcmeta") {
                    let mut t = String::new();
                    std::io::Read::read_to_string(&mut entry, &mut t).ok();
                    t
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } else if path.exists() {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };
    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&text) {
        let desc = meta["pack"]["description"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let fmt = meta["pack"]["pack_format"].as_i64().unwrap_or(0) as i32;
        (desc, fmt)
    } else {
        (String::new(), 0)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            log::debug!("ignoring resume event because the window already exists");
            return;
        }
        let startup_started = std::time::Instant::now();
        log::info!("creating 1280x720 application window");
        let attrs = WindowAttributes::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32));
        let window = event_loop
            .create_window(attrs)
            .expect("Failed to create window");
        // Set the window icon from assets/icon.ico
        if let Ok(img) = image::load_from_memory(include_bytes!("../../assets/icon.ico")) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                window.set_window_icon(Some(icon));
            }
        }
        let window_size = window.inner_size();
        log::info!(
            "window created: physical_size={}x{}, scale_factor={:.3}",
            window_size.width,
            window_size.height,
            window.scale_factor()
        );
        // IME is enabled/disabled per-input (chat, sign editor, text fields)
        // to avoid showing the candidate window during gameplay.
        window.set_ime_allowed(false);
        let mut sky_resolver = crate::assets::resolver::AssetResolver::with_resource_packs(
            "assets/minecraft",
            "resourcepacks",
            &self.config.enabled_resource_packs,
        );
        let mut renderer = render::Renderer::new(
            &window,
            &mut sky_resolver,
            self.config.shader_pack.as_deref(),
        );
        log::debug!("Vulkan renderer created; initialising GUI resources");
        renderer.init_gui(&mut sky_resolver);
        renderer.state.settings.set_gui_scale(self.config.gui_scale.max(1).min(4));
        renderer.state.server_list.set_selected_shader_pack(self.config.shader_pack.clone());
        renderer.load_custom_sky_from_packs(&mut sky_resolver, 0);
        self.renderer = Some(renderer);
        self.window = Some(window);
        log::info!(
            "window and renderer initialised in {:.2} ms",
            startup_started.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("window close requested; shutting down client systems");
                // Drop renderer first so Vulkan resources (surface, swapchain)
                // are destroyed while the window handle is still valid.
                self.renderer = None;
                // Also drop the network connection so the server is notified.
                self.net_ctrl.connection = None;
                self.scripts.set_connection_active(false);
                self.scripts.shutdown();
                event_loop.exit();
            }
            WindowEvent::Focused(false) => {
                // Auto-pause when window loses focus while in-game (not in a menu)
                if matches!(self.state, GameState::Playing)
                    && self.input_ctrl.mouse_captured
                    && !self.chat_open
                    && !self.inventory_open
                {
                    self.state = GameState::Paused;
                    self.input_ctrl.mouse_captured = false;
                    self.set_cursor_captured(false);
                }
            }
            WindowEvent::Resized(size) => {
                self.player.update_aspect(size.width, size.height);
                if let Some(renderer) = &mut self.renderer {
                    renderer.notify_resize(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input_ctrl.mouse_x = position.x;
                self.input_ctrl.mouse_y = position.y;
                if let Some(renderer) = &mut self.renderer {
                    renderer.set_gui_mouse_pos(position.x as f32, position.y as f32);
                }
                self.update_active_slider();
                self.update_creative_scroll_drag();
                if let Some(window) = &self.window {
                    // Slider drags must be visible immediately, independent of
                    // the frame limiter's next scheduled redraw.
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => self.handle_keyboard_input(event_loop, key_event),
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.handle_ime_commit(&text);
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_mouse_wheel(delta),
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(event_loop, button, state.is_pressed());
            }
            WindowEvent::DroppedFile(path) => {
                self.handle_dropped_file(path);
            }
            WindowEvent::RedrawRequested => self.handle_redraw(event_loop),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.input_ctrl.mouse_captured && matches!(self.state, GameState::Playing) {
            if let DeviceEvent::MouseMotion { delta } = event {
                self.input_ctrl.mouse_dx += delta.0;
                self.input_ctrl.mouse_dy += delta.1;
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.config.max_framerate >= crate::client::config::UNLIMITED_FRAMERATE {
            event_loop.set_control_flow(ControlFlow::Poll);
            self.last_redraw_request = std::time::Instant::now();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        let frame_interval =
            std::time::Duration::from_secs_f64(1.0 / self.config.max_framerate.max(30) as f64);
        let now = std::time::Instant::now();
        let next_redraw = self.last_redraw_request + frame_interval;
        if now < next_redraw {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_redraw));
            return;
        }
        event_loop.set_control_flow(ControlFlow::Poll);
        self.last_redraw_request = now;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run() -> i32 {
    log::debug!("creating winit event loop");
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();
    log::info!("entering application event loop");
    match event_loop.run_app(&mut app) {
        Ok(()) => {
            log::info!("application event loop exited normally");
            0
        }
        Err(e) => {
            log::error!("application event loop failed: {e}");
            1
        }
    }
}
