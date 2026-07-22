use std::collections::{BTreeMap, HashMap, VecDeque};

use super::{
    ChestRenderEntry, ControlBindingRow, EntityBillboard, ModConfigRow, ModManagerRow,
    SelectionBox, ServerListRow, SidebarLine, SkullRenderEntry,
};
use crate::assets::skin::{PlayerSkin, SkinPreviewPixels};
use crate::client::app::ResourcePackInfo;
use crate::client::inventory::ItemStackView;
use crate::client::keybind::Action;
use crate::render::hud::entities::SignEntry;
use crate::render::hud::tooltip::TooltipLine;
use crate::render::shader_pack::ShaderPackInfo;
use crate::ui::text::UiText;

#[derive(Clone, Copy, Default)]
pub struct FrameSpikeBreakdown {
    pub interval_us: u64,
    pub total_us: u64,
    pub outside_us: u64,
    pub tasks_us: u64,
    pub network_us: u64,
    pub world_us: u64,
    pub tick_us: u64,
    pub sync_us: u64,
    pub render_us: u64,
    pub other_us: u64,
    pub script_us: u64,
    pub fence_us: u64,
    pub cpu_us: u64,
    pub acquire_us: u64,
    pub command_us: u64,
    pub submit_us: u64,
    pub present_us: u64,
    pub mesh_us: u64,
    pub entity_us: u64,
    pub entity_loop_us: u64,
    pub entity_hash_us: u64,
    pub entity_lookup_us: u64,
    pub entity_generate_us: u64,
    pub entity_upload_us: u64,
    pub entity_skin_sync_us: u64,
    pub entity_prune_us: u64,
    pub entity_extras_us: u64,
    pub entity_cache_hits: u32,
    pub entity_cache_misses: u32,
    pub entity_visible_count: u32,
    pub entity_culled_count: u32,
    pub particle_us: u64,
    pub nametag_us: u64,
    pub local_us: u64,
    pub gui_us: u64,
    pub chunk_upload_us: u64,
    pub network_packet_kind: &'static str,
    pub network_packet_us: u64,
    pub network_packet_units: u32,
    pub network_hook_us: u64,
    pub network_session_us: u64,
    pub network_inventory_us: u64,
    pub network_entity_us: u64,
    pub network_world_us: u64,
    pub network_scheduler_us: u64,
    pub network_scanned_packets: u32,
    pub network_handled_packets: u32,
    pub network_deferred_packets: u32,
    sequence: u64,
    end_us: u64,
}

#[derive(Clone, Copy, Default)]
pub struct FrameDebugProfile {
    pub max_framerate: u32,
    pub particle_count: usize,
    pub entity_count: usize,
    pub chunk_count_loaded: usize,
    pub total_us: u64,
    pub interval_us: u64,
    pub one_percent_low_fps: f32,
    pub zero_point_one_percent_low_fps: f32,
    pub p99_interval_us: u64,
    pub p99_9_interval_us: u64,
    pub max_interval_us: u64,
    pub interval_sample_count: u32,
    pub worst: FrameSpikeBreakdown,
    pub worst_age_us: u64,
    pub outside_us: u64,
    pub tasks_us: u64,
    pub network_us: u64,
    pub world_us: u64,
    pub tick_us: u64,
    pub sync_us: u64,
    pub render_us: u64,
    pub other_us: u64,
    pub script_us: u64,
    pub script_callbacks: u32,
    pub script_slow_callbacks: u32,
    pub fence_us: u64,
    pub cpu_us: u64,
    pub acquire_us: u64,
    pub command_us: u64,
    pub submit_us: u64,
    pub present_us: u64,
    pub mesh_us: u64,
    pub particle_us: u64,
    pub nametag_us: u64,
    pub entity_us: u64,
    pub entity_loop_us: u64,
    pub entity_visible_count: u32,
    pub entity_culled_count: u32,
    pub entity_batch_reused: bool,
    pub entity_cache_hits: u32,
    pub entity_cache_misses: u32,
    pub entity_hash_us: u64,
    pub entity_lookup_us: u64,
    pub entity_upload_us: u64,
    pub entity_skin_sync_us: u64,
    pub entity_generate_us: u64,
    pub entity_prune_us: u64,
    pub entity_extras_us: u64,
    pub local_us: u64,
    pub gui_us: u64,
    pub chunk_upload_us: u64,
    pub chunk_upload_bytes: u64,
    pub chunk_upload_count: u32,
    pub particle_batch_reused: bool,
    pub local_batch_reused: bool,
}

/// Generates a copy getter, a mutable reference getter, and a simple setter
/// for a `Copy` field stored under `<field>_`.
macro_rules! copy_field {
    ($field:ident, $getter:ident, $getter_mut:ident, $setter:ident, $type:ty) => {
        pub fn $getter(&self) -> $type { self.$field }
        pub fn $getter_mut(&mut self) -> &mut $type { &mut self.$field }
        pub fn $setter(&mut self, v: $type) { self.$field = v; }
    };
}

/// Generates a reference getter, a mutable reference getter, and a simple
/// setter for a non-`Copy` field stored under `<field>_`.
macro_rules! ref_field {
    ($field:ident, $getter:ident, $getter_mut:ident, $setter:ident, $type:ty) => {
        pub fn $getter(&self) -> &$type { &self.$field }
        pub fn $getter_mut(&mut self) -> &mut $type { &mut self.$field }
        pub fn $setter(&mut self, v: $type) { self.$field = v; }
    };
}

// =========================================================================
// RenderSettings — rendering, audio, controls and gameplay presentation
// =========================================================================
pub struct RenderSettings {
    gui_scale_: u32,
    render_distance_: u8,
    smooth_lighting_: bool,
    camera_mode_: u8,
    particles_label_: String,
    particles_enabled_: bool,
    master_volume_: f32,
    music_volume_: f32,
    blocks_volume_: f32,
    hostile_volume_: f32,
    friendly_volume_: f32,
    players_volume_: f32,
    ambient_volume_: f32,
    weather_volume_: f32,
    ui_volume_: f32,
    audio_device_: String,
    fov_: f32,
    max_framerate_: u32,
    clouds_: bool,
    weather_effects_: bool,
    entity_shadows_: bool,
    view_bobbing_: bool,
    advanced_tooltips_: bool,
    better_grass_: bool,
    connected_textures_: bool,
    difficulty_: u8,
    skin_parts_: u8,
    language_code_: String,
    language_name_: String,
    ui_text_: UiText,
    mouse_sensitivity_: f32,
    invert_mouse_: bool,
    gamepad_look_sensitivity_: f32,
    gamepad_cursor_speed_: f32,
    controls_gamepad_: bool,
    control_bindings_: Vec<ControlBindingRow>,
    rebinding_action_: Option<Action>,
    debug_overlay_: bool,
    hud_visible_: bool,
    crosshair_visible_: bool,
    local_model_parts_: usize,
    local_skin_size_: [u32; 2],
    local_skin_slim_: bool,
    local_skin_face_: [[u8; 4]; 64],
    local_skin_preview_: SkinPreviewPixels,
    local_skin_: PlayerSkin,
    underwater_: bool,
    underwater_yaw_: f32,
    underwater_pitch_: f32,
    controls_list_scroll_: usize,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            gui_scale_: 0,
            render_distance_: 0,
            smooth_lighting_: false,
            camera_mode_: 0,
            particles_label_: String::new(),
            particles_enabled_: false,
            master_volume_: 0.0,
            music_volume_: 0.0,
            blocks_volume_: 0.0,
            hostile_volume_: 0.0,
            friendly_volume_: 0.0,
            players_volume_: 0.0,
            ambient_volume_: 0.0,
            weather_volume_: 0.0,
            ui_volume_: 0.0,
            audio_device_: String::new(),
            fov_: 0.0,
            max_framerate_: 0,
            clouds_: false,
            weather_effects_: true,
            entity_shadows_: false,
            view_bobbing_: false,
            advanced_tooltips_: false,
            better_grass_: false,
            connected_textures_: true,
            difficulty_: 0,
            skin_parts_: 0,
            language_code_: String::new(),
            language_name_: String::new(),
            ui_text_: UiText::default(),
            mouse_sensitivity_: 0.5,
            invert_mouse_: false,
            gamepad_look_sensitivity_: 0.5,
            gamepad_cursor_speed_: 0.25,
            controls_gamepad_: false,
            control_bindings_: Vec::new(),
            rebinding_action_: None,
            debug_overlay_: false,
            hud_visible_: true,
            crosshair_visible_: true,
            local_model_parts_: 0,
            local_skin_size_: [0; 2],
            local_skin_slim_: false,
            local_skin_face_: [[0; 4]; 64],
            local_skin_preview_: SkinPreviewPixels::default(),
            local_skin_: PlayerSkin::default(),
            underwater_: false,
            underwater_yaw_: 0.0,
            underwater_pitch_: 0.0,
            controls_list_scroll_: 0,
        }
    }
}

impl RenderSettings {
    copy_field!(gui_scale_, gui_scale, gui_scale_mut, set_gui_scale, u32);
    copy_field!(smooth_lighting_, smooth_lighting, smooth_lighting_mut, set_smooth_lighting, bool);
    copy_field!(camera_mode_, camera_mode, camera_mode_mut, set_camera_mode, u8);
    copy_field!(particles_enabled_, particles_enabled, particles_enabled_mut, set_particles_enabled, bool);
    copy_field!(clouds_, clouds, clouds_mut, set_clouds, bool);
    copy_field!(weather_effects_, weather_effects, weather_effects_mut, set_weather_effects, bool);
    copy_field!(entity_shadows_, entity_shadows, entity_shadows_mut, set_entity_shadows, bool);
    copy_field!(view_bobbing_, view_bobbing, view_bobbing_mut, set_view_bobbing, bool);
    copy_field!(advanced_tooltips_, advanced_tooltips, advanced_tooltips_mut, set_advanced_tooltips, bool);
    copy_field!(better_grass_, better_grass, better_grass_mut, set_better_grass, bool);
    copy_field!(connected_textures_, connected_textures, connected_textures_mut, set_connected_textures, bool);
    copy_field!(skin_parts_, skin_parts, skin_parts_mut, set_skin_parts, u8);
    copy_field!(invert_mouse_, invert_mouse, invert_mouse_mut, set_invert_mouse, bool);
    copy_field!(controls_gamepad_, controls_gamepad, controls_gamepad_mut, set_controls_gamepad, bool);
    copy_field!(debug_overlay_, debug_overlay, debug_overlay_mut, set_debug_overlay, bool);
    copy_field!(hud_visible_, hud_visible, hud_visible_mut, set_hud_visible, bool);
    copy_field!(crosshair_visible_, crosshair_visible, crosshair_visible_mut, set_crosshair_visible, bool);
    copy_field!(local_model_parts_, local_model_parts, local_model_parts_mut, set_local_model_parts, usize);
    copy_field!(local_skin_size_, local_skin_size, local_skin_size_mut, set_local_skin_size, [u32; 2]);
    copy_field!(local_skin_slim_, local_skin_slim, local_skin_slim_mut, set_local_skin_slim, bool);
    copy_field!(local_skin_face_, local_skin_face, local_skin_face_mut, set_local_skin_face, [[u8; 4]; 64]);
    copy_field!(underwater_, underwater, underwater_mut, set_underwater, bool);
    copy_field!(underwater_yaw_, underwater_yaw, underwater_yaw_mut, set_underwater_yaw, f32);
    copy_field!(underwater_pitch_, underwater_pitch, underwater_pitch_mut, set_underwater_pitch, f32);
    copy_field!(controls_list_scroll_, controls_list_scroll, controls_list_scroll_mut, set_controls_list_scroll, usize);
    copy_field!(rebinding_action_, rebinding_action, rebinding_action_mut, set_rebinding_action, Option<Action>);

    ref_field!(particles_label_, particles_label, particles_label_mut, set_particles_label, String);
    ref_field!(audio_device_, audio_device, audio_device_mut, set_audio_device, String);
    ref_field!(language_code_, language_code, language_code_mut, set_language_code, String);
    ref_field!(language_name_, language_name, language_name_mut, set_language_name, String);
    ref_field!(ui_text_, ui_text, ui_text_mut, set_ui_text, UiText);
    ref_field!(control_bindings_, control_bindings, control_bindings_mut, set_control_bindings, Vec<ControlBindingRow>);
    ref_field!(local_skin_preview_, local_skin_preview, local_skin_preview_mut, set_local_skin_preview, SkinPreviewPixels);
    ref_field!(local_skin_, local_skin, local_skin_mut, set_local_skin, PlayerSkin);

    /// Field-of-view in degrees. Must stay positive.
    pub fn fov(&self) -> f32 { self.fov_ }
    pub fn set_fov(&mut self, v: f32) {
        assert!(v > 0.0, "fov must be positive, got {}", v);
        self.fov_ = v;
    }

    /// Maximum framerate cap. Must be at least 1.
    pub fn max_framerate(&self) -> u32 { self.max_framerate_ }
    pub fn set_max_framerate(&mut self, v: u32) {
        assert!(v >= 1, "max_framerate must be >= 1, got {}", v);
        self.max_framerate_ = v;
    }

    /// Render distance in chunks. Clamped to 0..=32.
    pub fn render_distance(&self) -> u8 { self.render_distance_ }
    pub fn set_render_distance(&mut self, v: u8) {
        self.render_distance_ = v.min(32);
    }

    /// Master volume in [0, 1].
    pub fn master_volume(&self) -> f32 { self.master_volume_ }
    pub fn set_master_volume(&mut self, v: f32) {
        self.master_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn music_volume(&self) -> f32 { self.music_volume_ }
    pub fn set_music_volume(&mut self, v: f32) {
        self.music_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn blocks_volume(&self) -> f32 { self.blocks_volume_ }
    pub fn set_blocks_volume(&mut self, v: f32) {
        self.blocks_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn hostile_volume(&self) -> f32 { self.hostile_volume_ }
    pub fn set_hostile_volume(&mut self, v: f32) {
        self.hostile_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn friendly_volume(&self) -> f32 { self.friendly_volume_ }
    pub fn set_friendly_volume(&mut self, v: f32) {
        self.friendly_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn players_volume(&self) -> f32 { self.players_volume_ }
    pub fn set_players_volume(&mut self, v: f32) {
        self.players_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn ambient_volume(&self) -> f32 { self.ambient_volume_ }
    pub fn set_ambient_volume(&mut self, v: f32) {
        self.ambient_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn weather_volume(&self) -> f32 { self.weather_volume_ }
    pub fn set_weather_volume(&mut self, v: f32) {
        self.weather_volume_ = v.clamp(0.0, 1.0);
    }

    pub fn ui_volume(&self) -> f32 { self.ui_volume_ }
    pub fn set_ui_volume(&mut self, v: f32) {
        self.ui_volume_ = v.clamp(0.0, 1.0);
    }

    /// Mouse look sensitivity in [0, 1].
    pub fn mouse_sensitivity(&self) -> f32 { self.mouse_sensitivity_ }
    pub fn set_mouse_sensitivity(&mut self, v: f32) {
        self.mouse_sensitivity_ = v.clamp(0.0, 1.0);
    }

    pub fn gamepad_look_sensitivity(&self) -> f32 { self.gamepad_look_sensitivity_ }
    pub fn set_gamepad_look_sensitivity(&mut self, v: f32) {
        self.gamepad_look_sensitivity_ = v.clamp(0.0, 1.0);
    }

    pub fn gamepad_cursor_speed(&self) -> f32 { self.gamepad_cursor_speed_ }
    pub fn set_gamepad_cursor_speed(&mut self, v: f32) {
        self.gamepad_cursor_speed_ = v.clamp(0.0, 1.0);
    }

    /// Difficulty in 0..=3 (peaceful/easy/normal/hard).
    pub fn difficulty(&self) -> u8 { self.difficulty_ }
    pub fn set_difficulty(&mut self, v: u8) {
        assert!(v <= 3, "difficulty must be 0..=3, got {}", v);
        self.difficulty_ = v;
    }
}

// =========================================================================
// AccountState — login, alt manager and connection status
// =========================================================================
pub struct AccountState {
    username_: String,
    account_name_: String,
    account_status_: String,
    account_list_: Vec<(String, String, bool)>,
    account_faces_: HashMap<String, [[u8; 4]; 64]>,
    selected_account_: usize,
    entering_offline_name_: bool,
    offline_username_input_: String,
    connection_status_: String,
    server_refreshing_: bool,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            username_: String::new(),
            account_name_: String::new(),
            account_status_: String::new(),
            account_list_: Vec::new(),
            account_faces_: HashMap::new(),
            selected_account_: 0,
            entering_offline_name_: false,
            offline_username_input_: String::new(),
            connection_status_: String::new(),
            server_refreshing_: false,
        }
    }
}

impl AccountState {
    copy_field!(selected_account_, selected_account, selected_account_mut, set_selected_account, usize);
    copy_field!(entering_offline_name_, entering_offline_name, entering_offline_name_mut, set_entering_offline_name, bool);
    copy_field!(server_refreshing_, server_refreshing, server_refreshing_mut, set_server_refreshing, bool);

    ref_field!(username_, username, username_mut, set_username, String);
    ref_field!(account_name_, account_name, account_name_mut, set_account_name, String);
    ref_field!(account_status_, account_status, account_status_mut, set_account_status, String);
    ref_field!(account_list_, account_list, account_list_mut, set_account_list, Vec<(String, String, bool)>);
    ref_field!(account_faces_, account_faces, account_faces_mut, set_account_faces, HashMap<String, [[u8; 4]; 64]>);
    ref_field!(offline_username_input_, offline_username_input, offline_username_input_mut, set_offline_username_input, String);
    ref_field!(connection_status_, connection_status, connection_status_mut, set_connection_status, String);
}

// =========================================================================
// ServerListState — server browser, resource packs, shader packs and mods
// =========================================================================
pub struct ServerListState {
    server_address_: String,
    server_list_: Vec<ServerListRow>,
    selected_server_: usize,
    server_list_scroll_: usize,
    available_resource_pack_scroll_: usize,
    selected_resource_pack_scroll_: usize,
    shader_pack_scroll_: usize,
    server_editor_name_: String,
    server_editor_address_: String,
    server_editor_address_focused_: bool,
    modding_rows_: Vec<ModManagerRow>,
    modding_scroll_: usize,
    modding_status_: String,
    modding_connection_active_: bool,
    modding_selected_: usize,
    mod_config_title_: Option<String>,
    mod_config_rows_: Vec<ModConfigRow>,
    mod_config_selected_: usize,
    mod_config_scroll_: usize,
    mod_config_status_: String,
    mod_config_locked_: bool,
    server_brand_: Option<String>,
    resource_pack_status_: Option<String>,
    available_resource_packs_: Vec<ResourcePackInfo>,
    selected_resource_packs_: Vec<ResourcePackInfo>,
    shader_packs_: Vec<ShaderPackInfo>,
    selected_shader_pack_: Option<String>,
    shader_pack_status_: String,
    ray_tracing_available_: bool,
    fsr3_available_: bool,
}

impl Default for ServerListState {
    fn default() -> Self {
        Self {
            server_address_: String::new(),
            server_list_: Vec::new(),
            selected_server_: 0,
            server_list_scroll_: 0,
            available_resource_pack_scroll_: 0,
            selected_resource_pack_scroll_: 0,
            shader_pack_scroll_: 0,
            server_editor_name_: String::new(),
            server_editor_address_: String::new(),
            server_editor_address_focused_: false,
            modding_rows_: Vec::new(),
            modding_scroll_: 0,
            modding_status_: String::new(),
            modding_connection_active_: false,
            modding_selected_: 0,
            mod_config_title_: None,
            mod_config_rows_: Vec::new(),
            mod_config_selected_: 0,
            mod_config_scroll_: 0,
            mod_config_status_: String::new(),
            mod_config_locked_: false,
            server_brand_: None,
            resource_pack_status_: None,
            available_resource_packs_: Vec::new(),
            selected_resource_packs_: Vec::new(),
            shader_packs_: Vec::new(),
            selected_shader_pack_: None,
            shader_pack_status_: String::new(),
            ray_tracing_available_: false,
            fsr3_available_: false,
        }
    }
}

impl ServerListState {
    copy_field!(selected_server_, selected_server, selected_server_mut, set_selected_server, usize);
    copy_field!(server_list_scroll_, server_list_scroll, server_list_scroll_mut, set_server_list_scroll, usize);
    copy_field!(available_resource_pack_scroll_, available_resource_pack_scroll, available_resource_pack_scroll_mut, set_available_resource_pack_scroll, usize);
    copy_field!(selected_resource_pack_scroll_, selected_resource_pack_scroll, selected_resource_pack_scroll_mut, set_selected_resource_pack_scroll, usize);
    copy_field!(shader_pack_scroll_, shader_pack_scroll, shader_pack_scroll_mut, set_shader_pack_scroll, usize);
    copy_field!(server_editor_address_focused_, server_editor_address_focused, server_editor_address_focused_mut, set_server_editor_address_focused, bool);
    copy_field!(modding_scroll_, modding_scroll, modding_scroll_mut, set_modding_scroll, usize);
    copy_field!(modding_connection_active_, modding_connection_active, modding_connection_active_mut, set_modding_connection_active, bool);
    copy_field!(modding_selected_, modding_selected, modding_selected_mut, set_modding_selected, usize);
    copy_field!(mod_config_selected_, mod_config_selected, mod_config_selected_mut, set_mod_config_selected, usize);
    copy_field!(mod_config_scroll_, mod_config_scroll, mod_config_scroll_mut, set_mod_config_scroll, usize);
    copy_field!(mod_config_locked_, mod_config_locked, mod_config_locked_mut, set_mod_config_locked, bool);
    copy_field!(ray_tracing_available_, ray_tracing_available, ray_tracing_available_mut, set_ray_tracing_available, bool);
    copy_field!(fsr3_available_, fsr3_available, fsr3_available_mut, set_fsr3_available, bool);

    ref_field!(server_address_, server_address, server_address_mut, set_server_address, String);
    ref_field!(server_list_, server_list, server_list_mut, set_server_list, Vec<ServerListRow>);
    ref_field!(server_editor_name_, server_editor_name, server_editor_name_mut, set_server_editor_name, String);
    ref_field!(server_editor_address_, server_editor_address, server_editor_address_mut, set_server_editor_address, String);
    ref_field!(modding_rows_, modding_rows, modding_rows_mut, set_modding_rows, Vec<ModManagerRow>);
    ref_field!(modding_status_, modding_status, modding_status_mut, set_modding_status, String);
    ref_field!(mod_config_title_, mod_config_title, mod_config_title_mut, set_mod_config_title, Option<String>);
    ref_field!(mod_config_rows_, mod_config_rows, mod_config_rows_mut, set_mod_config_rows, Vec<ModConfigRow>);
    ref_field!(mod_config_status_, mod_config_status, mod_config_status_mut, set_mod_config_status, String);
    ref_field!(server_brand_, server_brand, server_brand_mut, set_server_brand, Option<String>);
    ref_field!(resource_pack_status_, resource_pack_status, resource_pack_status_mut, set_resource_pack_status, Option<String>);
    ref_field!(available_resource_packs_, available_resource_packs, available_resource_packs_mut, set_available_resource_packs, Vec<ResourcePackInfo>);
    ref_field!(selected_resource_packs_, selected_resource_packs, selected_resource_packs_mut, set_selected_resource_packs, Vec<ResourcePackInfo>);
    ref_field!(shader_packs_, shader_packs, shader_packs_mut, set_shader_packs, Vec<ShaderPackInfo>);
    ref_field!(selected_shader_pack_, selected_shader_pack, selected_shader_pack_mut, set_selected_shader_pack, Option<String>);
    ref_field!(shader_pack_status_, shader_pack_status, shader_pack_status_mut, set_shader_pack_status, String);
}

// =========================================================================
// InventoryState — inventory windows, hotbar and creative inventory
// =========================================================================
pub struct InventoryState {
    inventory_open_: bool,
    inventory_window_id_: u8,
    inventory_window_type_: String,
    inventory_window_title_: String,
    inventory_window_slot_count_: usize,
    inventory_window_slots_: Vec<ItemStackView>,
    inventory_window_properties_: Vec<(i16, i16)>,
    inventory_slots_: [ItemStackView; 36],
    inventory_armor_slots_: [ItemStackView; 4],
    inventory_crafting_slots_: [ItemStackView; 5],
    inventory_cursor_slot_: ItemStackView,
    hotbar_slots_: [(u16, u8, u16); 9],
    hotbar_selected_: usize,
    creative_tab_: usize,
    creative_scroll_: f32,
    creative_search_: String,
}

impl Default for InventoryState {
    fn default() -> Self {
        Self {
            inventory_open_: false,
            inventory_window_id_: 0,
            inventory_window_type_: String::new(),
            inventory_window_title_: String::new(),
            inventory_window_slot_count_: 0,
            inventory_window_slots_: Vec::new(),
            inventory_window_properties_: Vec::new(),
            inventory_slots_: std::array::from_fn(|_| ItemStackView::default()),
            inventory_armor_slots_: std::array::from_fn(|_| ItemStackView::default()),
            inventory_crafting_slots_: std::array::from_fn(|_| ItemStackView::default()),
            inventory_cursor_slot_: ItemStackView::default(),
            hotbar_slots_: [(0, 0, 0); 9],
            hotbar_selected_: 0,
            creative_tab_: 0,
            creative_scroll_: 0.0,
            creative_search_: String::new(),
        }
    }
}

impl InventoryState {
    copy_field!(inventory_open_, inventory_open, inventory_open_mut, set_inventory_open, bool);
    copy_field!(inventory_window_id_, inventory_window_id, inventory_window_id_mut, set_inventory_window_id, u8);
    copy_field!(inventory_window_slot_count_, inventory_window_slot_count, inventory_window_slot_count_mut, set_inventory_window_slot_count, usize);
    copy_field!(hotbar_slots_, hotbar_slots, hotbar_slots_mut, set_hotbar_slots, [(u16, u8, u16); 9]);
    copy_field!(creative_tab_, creative_tab, creative_tab_mut, set_creative_tab, usize);
    copy_field!(creative_scroll_, creative_scroll, creative_scroll_mut, set_creative_scroll, f32);

    ref_field!(inventory_window_type_, inventory_window_type, inventory_window_type_mut, set_inventory_window_type, String);
    ref_field!(inventory_window_title_, inventory_window_title, inventory_window_title_mut, set_inventory_window_title, String);
    ref_field!(inventory_window_slots_, inventory_window_slots, inventory_window_slots_mut, set_inventory_window_slots, Vec<ItemStackView>);
    ref_field!(inventory_window_properties_, inventory_window_properties, inventory_window_properties_mut, set_inventory_window_properties, Vec<(i16, i16)>);
    ref_field!(inventory_slots_, inventory_slots, inventory_slots_mut, set_inventory_slots, [ItemStackView; 36]);
    ref_field!(inventory_armor_slots_, inventory_armor_slots, inventory_armor_slots_mut, set_inventory_armor_slots, [ItemStackView; 4]);
    ref_field!(inventory_crafting_slots_, inventory_crafting_slots, inventory_crafting_slots_mut, set_inventory_crafting_slots, [ItemStackView; 5]);
    ref_field!(inventory_cursor_slot_, inventory_cursor_slot, inventory_cursor_slot_mut, set_inventory_cursor_slot, ItemStackView);
    ref_field!(creative_search_, creative_search, creative_search_mut, set_creative_search, String);

    /// Selected hotbar slot index, clamped to 0..=8.
    pub fn hotbar_selected(&self) -> usize { self.hotbar_selected_ }
    pub fn set_hotbar_selected(&mut self, v: usize) {
        assert!(v < 9, "hotbar_selected must be 0..=8, got {}", v);
        self.hotbar_selected_ = v;
    }
}

// =========================================================================
// HudState — world/player/chat/signbook/scoreboard/tooltip presentation
// =========================================================================
pub struct HudState {
    max_players_: u8,
    world_time_: i64,
    day_time_: i64,
    dimension_: i8,
    raining_: bool,
    rain_level_: f32,
    thunder_level_: f32,
    gamemode_: u8,
    level_type_: String,
    spawn_position_: Option<[i32; 3]>,
    health_: f32,
    prev_health_: f32,
    health_timer_: u32,
    armor_points_: u8,
    absorption_: f32,
    food_: i32,
    prev_food_: i32,
    food_timer_: u32,
    saturation_: f32,
    experience_bar_: f32,
    experience_level_: i32,
    experience_total_: i32,
    chat_lines_: Vec<String>,
    chat_open_: bool,
    chat_input_: String,
    chat_visible_time_: f64,
    chat_alpha_: f32,
    chat_scroll_: usize,
    chat_last_message_time_: f64,
    chat_width_: f32,
    chat_height_: u8,
    chat_background_: bool,
    chat_overlay_: bool,
    chat_player_avatars_: bool,
    chat_faces_: Vec<Option<[[u8; 4]; 64]>>,
    sign_editor_open_: bool,
    sign_editor_lines_: [String; 4],
    sign_editor_active_line_: usize,
    book_editor_open_: bool,
    book_pages_: Vec<String>,
    book_page_: usize,
    book_signing_: bool,
    book_title_: String,
    player_list_open_: bool,
    player_list_: Vec<(String, i32, i32)>,
    player_list_faces_: Vec<[[u8; 4]; 64]>,
    tab_player_avatars_: bool,
    tab_header_: Option<String>,
    tab_footer_: Option<String>,
    title_text_: Option<String>,
    subtitle_text_: Option<String>,
    title_alpha_: f32,
    action_bar_: Option<String>,
    sidebar_title_: Option<String>,
    sidebar_lines_: Vec<SidebarLine>,
    world_border_center_: [f64; 2],
    world_border_diameter_: f64,
    world_border_warning_blocks_: i32,
    active_potion_effects_: Vec<crate::entity::EntityEffectState>,
    block_selection_boxes_: Vec<SelectionBox>,
    dig_progress_: f32,
    dig_position_: Option<[i32; 3]>,
    sign_entries_: Vec<SignEntry>,
    hovered_tooltip_: Vec<TooltipLine>,
    entity_billboards_: Vec<EntityBillboard>,
    skull_entries_: Vec<SkullRenderEntry>,
    chest_entries_: Vec<ChestRenderEntry>,
    pending_player_skins_: Vec<super::PendingPlayerSkin>,
    player_skin_content_hash_: u64,
    player_skin_layout_hash_: u64,
    hand_swing_progress_: f32,
    hand_item_id_: u16,
    hand_item_damage_: u16,
    hand_item_nbt_: Option<Vec<u8>>,
    hand_swing_timer_: f32,
    hand_use_kind_: u8,
    hand_use_progress_: f32,
    first_person_arm_yaw_: f32,
    first_person_arm_pitch_: f32,
    first_person_prev_arm_yaw_: f32,
    first_person_prev_arm_pitch_: f32,
    first_person_arm_transform_: nalgebra::Matrix4<f32>,
    first_person_item_transform_: nalgebra::Matrix4<f32>,
    script_hud_before_commands_: Vec<crate::render::hooks::ScriptDrawCommand>,
    script_hud_commands_: Vec<crate::render::hooks::ScriptDrawCommand>,
    fp_vanilla_flags_: crate::render::first_person::VanillaTransformFlags,
    local_player_billboard_: Option<EntityBillboard>,
    fps_count_: u32,
    recent_sounds_: Vec<String>,
    sound_event_count_: u64,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            max_players_: 0,
            world_time_: 0,
            day_time_: 0,
            dimension_: 0,
            raining_: false,
            rain_level_: 0.0,
            thunder_level_: 0.0,
            gamemode_: 0,
            level_type_: String::new(),
            spawn_position_: None,
            health_: 0.0,
            prev_health_: 0.0,
            health_timer_: 0,
            armor_points_: 0,
            absorption_: 0.0,
            food_: 0,
            prev_food_: 0,
            food_timer_: 0,
            saturation_: 0.0,
            experience_bar_: 0.0,
            experience_level_: 0,
            experience_total_: 0,
            chat_lines_: Vec::new(),
            chat_open_: false,
            chat_input_: String::new(),
            chat_visible_time_: 0.0,
            chat_alpha_: 0.0,
            chat_scroll_: 0,
            chat_last_message_time_: 0.0,
            chat_width_: 0.4,
            chat_height_: 12,
            chat_background_: true,
            chat_overlay_: true,
            chat_player_avatars_: true,
            chat_faces_: Vec::new(),
            sign_editor_open_: false,
            sign_editor_lines_: Default::default(),
            sign_editor_active_line_: 0,
            book_editor_open_: false,
            book_pages_: Vec::new(),
            book_page_: 0,
            book_signing_: false,
            book_title_: String::new(),
            player_list_open_: false,
            player_list_: Vec::new(),
            player_list_faces_: Vec::new(),
            tab_player_avatars_: true,
            tab_header_: None,
            tab_footer_: None,
            title_text_: None,
            subtitle_text_: None,
            title_alpha_: 0.0,
            action_bar_: None,
            sidebar_title_: None,
            sidebar_lines_: Vec::new(),
            world_border_center_: [0.0; 2],
            world_border_diameter_: 0.0,
            world_border_warning_blocks_: 0,
            active_potion_effects_: Vec::new(),
            block_selection_boxes_: Vec::new(),
            dig_progress_: 0.0,
            dig_position_: None,
            sign_entries_: Vec::new(),
            hovered_tooltip_: Vec::new(),
            entity_billboards_: Vec::new(),
            skull_entries_: Vec::new(),
            chest_entries_: Vec::new(),
            pending_player_skins_: Vec::new(),
            player_skin_content_hash_: 0,
            player_skin_layout_hash_: 0,
            hand_swing_progress_: 0.0,
            hand_item_id_: 0,
            hand_item_damage_: 0,
            hand_item_nbt_: None,
            hand_swing_timer_: 0.0,
            hand_use_kind_: 0,
            hand_use_progress_: 0.0,
            first_person_arm_yaw_: 0.0,
            first_person_arm_pitch_: 0.0,
            first_person_prev_arm_yaw_: 0.0,
            first_person_prev_arm_pitch_: 0.0,
            first_person_arm_transform_: nalgebra::Matrix4::identity(),
            first_person_item_transform_: nalgebra::Matrix4::identity(),
            script_hud_before_commands_: Vec::new(),
            script_hud_commands_: Vec::new(),
            fp_vanilla_flags_: crate::render::first_person::VanillaTransformFlags::default(),
            local_player_billboard_: None,
            fps_count_: 0,
            recent_sounds_: Vec::new(),
            sound_event_count_: 0,
        }
    }
}

impl HudState {
    copy_field!(max_players_, max_players, max_players_mut, set_max_players, u8);
    copy_field!(world_time_, world_time, world_time_mut, set_world_time, i64);
    copy_field!(day_time_, day_time, day_time_mut, set_day_time, i64);
    copy_field!(raining_, raining, raining_mut, set_raining, bool);
    copy_field!(rain_level_, rain_level, rain_level_mut, set_rain_level, f32);
    copy_field!(thunder_level_, thunder_level, thunder_level_mut, set_thunder_level, f32);
    copy_field!(armor_points_, armor_points, armor_points_mut, set_armor_points, u8);
    copy_field!(absorption_, absorption, absorption_mut, set_absorption, f32);
    copy_field!(food_, food, food_mut, set_food, i32);
    copy_field!(prev_food_, prev_food, prev_food_mut, set_prev_food, i32);
    copy_field!(food_timer_, food_timer, food_timer_mut, set_food_timer, u32);
    copy_field!(saturation_, saturation, saturation_mut, set_saturation, f32);
    copy_field!(experience_bar_, experience_bar, experience_bar_mut, set_experience_bar, f32);
    copy_field!(experience_level_, experience_level, experience_level_mut, set_experience_level, i32);
    copy_field!(experience_total_, experience_total, experience_total_mut, set_experience_total, i32);
    copy_field!(chat_open_, chat_open, chat_open_mut, set_chat_open, bool);
    copy_field!(chat_visible_time_, chat_visible_time, chat_visible_time_mut, set_chat_visible_time, f64);
    copy_field!(chat_alpha_, chat_alpha, chat_alpha_mut, set_chat_alpha, f32);
    copy_field!(chat_scroll_, chat_scroll, chat_scroll_mut, set_chat_scroll, usize);
    copy_field!(chat_last_message_time_, chat_last_message_time, chat_last_message_time_mut, set_chat_last_message_time, f64);
    copy_field!(chat_width_, chat_width, chat_width_mut, set_chat_width, f32);
    copy_field!(chat_height_, chat_height, chat_height_mut, set_chat_height, u8);
    copy_field!(chat_background_, chat_background, chat_background_mut, set_chat_background, bool);
    copy_field!(chat_overlay_, chat_overlay, chat_overlay_mut, set_chat_overlay, bool);
    copy_field!(chat_player_avatars_, chat_player_avatars, chat_player_avatars_mut, set_chat_player_avatars, bool);
    copy_field!(sign_editor_open_, sign_editor_open, sign_editor_open_mut, set_sign_editor_open, bool);
    copy_field!(sign_editor_active_line_, sign_editor_active_line, sign_editor_active_line_mut, set_sign_editor_active_line, usize);
    copy_field!(book_editor_open_, book_editor_open, book_editor_open_mut, set_book_editor_open, bool);
    copy_field!(book_page_, book_page, book_page_mut, set_book_page, usize);
    copy_field!(book_signing_, book_signing, book_signing_mut, set_book_signing, bool);
    copy_field!(player_list_open_, player_list_open, player_list_open_mut, set_player_list_open, bool);
    copy_field!(tab_player_avatars_, tab_player_avatars, tab_player_avatars_mut, set_tab_player_avatars, bool);
    copy_field!(title_alpha_, title_alpha, title_alpha_mut, set_title_alpha, f32);
    copy_field!(world_border_center_, world_border_center, world_border_center_mut, set_world_border_center, [f64; 2]);
    copy_field!(world_border_diameter_, world_border_diameter, world_border_diameter_mut, set_world_border_diameter, f64);
    copy_field!(world_border_warning_blocks_, world_border_warning_blocks, world_border_warning_blocks_mut, set_world_border_warning_blocks, i32);
    copy_field!(dig_progress_, dig_progress, dig_progress_mut, set_dig_progress, f32);
    copy_field!(dig_position_, dig_position, dig_position_mut, set_dig_position, Option<[i32; 3]>);
    copy_field!(player_skin_content_hash_, player_skin_content_hash, player_skin_content_hash_mut, set_player_skin_content_hash, u64);
    copy_field!(player_skin_layout_hash_, player_skin_layout_hash, player_skin_layout_hash_mut, set_player_skin_layout_hash, u64);
    copy_field!(hand_swing_progress_, hand_swing_progress, hand_swing_progress_mut, set_hand_swing_progress, f32);
    copy_field!(hand_item_id_, hand_item_id, hand_item_id_mut, set_hand_item_id, u16);
    copy_field!(hand_item_damage_, hand_item_damage, hand_item_damage_mut, set_hand_item_damage, u16);
    ref_field!(hand_item_nbt_, hand_item_nbt, hand_item_nbt_mut, set_hand_item_nbt, Option<Vec<u8>>);
    copy_field!(hand_swing_timer_, hand_swing_timer, hand_swing_timer_mut, set_hand_swing_timer, f32);
    copy_field!(hand_use_kind_, hand_use_kind, hand_use_kind_mut, set_hand_use_kind, u8);
    copy_field!(hand_use_progress_, hand_use_progress, hand_use_progress_mut, set_hand_use_progress, f32);
    copy_field!(first_person_arm_yaw_, first_person_arm_yaw, first_person_arm_yaw_mut, set_first_person_arm_yaw, f32);
    copy_field!(first_person_arm_pitch_, first_person_arm_pitch, first_person_arm_pitch_mut, set_first_person_arm_pitch, f32);
    copy_field!(first_person_prev_arm_yaw_, first_person_prev_arm_yaw, first_person_prev_arm_yaw_mut, set_first_person_prev_arm_yaw, f32);
    copy_field!(first_person_prev_arm_pitch_, first_person_prev_arm_pitch, first_person_prev_arm_pitch_mut, set_first_person_prev_arm_pitch, f32);
    copy_field!(first_person_arm_transform_, first_person_arm_transform, first_person_arm_transform_mut, set_first_person_arm_transform, nalgebra::Matrix4<f32>);
    copy_field!(first_person_item_transform_, first_person_item_transform, first_person_item_transform_mut, set_first_person_item_transform, nalgebra::Matrix4<f32>);
    copy_field!(fps_count_, fps_count, fps_count_mut, set_fps_count, u32);
    copy_field!(sound_event_count_, sound_event_count, sound_event_count_mut, set_sound_event_count, u64);
    copy_field!(health_, health, health_mut, set_health, f32);
    copy_field!(prev_health_, prev_health, prev_health_mut, set_prev_health, f32);
    copy_field!(health_timer_, health_timer, health_timer_mut, set_health_timer, u32);
    copy_field!(spawn_position_, spawn_position, spawn_position_mut, set_spawn_position, Option<[i32; 3]>);

    ref_field!(level_type_, level_type, level_type_mut, set_level_type, String);
    ref_field!(chat_lines_, chat_lines, chat_lines_mut, set_chat_lines, Vec<String>);
    ref_field!(chat_input_, chat_input, chat_input_mut, set_chat_input, String);
    ref_field!(chat_faces_, chat_faces, chat_faces_mut, set_chat_faces, Vec<Option<[[u8; 4]; 64]>>);
    ref_field!(sign_editor_lines_, sign_editor_lines, sign_editor_lines_mut, set_sign_editor_lines, [String; 4]);
    ref_field!(book_pages_, book_pages, book_pages_mut, set_book_pages, Vec<String>);
    ref_field!(book_title_, book_title, book_title_mut, set_book_title, String);
    ref_field!(player_list_, player_list, player_list_mut, set_player_list, Vec<(String, i32, i32)>);
    ref_field!(player_list_faces_, player_list_faces, player_list_faces_mut, set_player_list_faces, Vec<[[u8; 4]; 64]>);
    ref_field!(tab_header_, tab_header, tab_header_mut, set_tab_header, Option<String>);
    ref_field!(tab_footer_, tab_footer, tab_footer_mut, set_tab_footer, Option<String>);
    ref_field!(title_text_, title_text, title_text_mut, set_title_text, Option<String>);
    ref_field!(subtitle_text_, subtitle_text, subtitle_text_mut, set_subtitle_text, Option<String>);
    ref_field!(action_bar_, action_bar, action_bar_mut, set_action_bar, Option<String>);
    ref_field!(sidebar_title_, sidebar_title, sidebar_title_mut, set_sidebar_title, Option<String>);
    ref_field!(sidebar_lines_, sidebar_lines, sidebar_lines_mut, set_sidebar_lines, Vec<SidebarLine>);
    ref_field!(active_potion_effects_, active_potion_effects, active_potion_effects_mut, set_active_potion_effects, Vec<crate::entity::EntityEffectState>);
    ref_field!(block_selection_boxes_, block_selection_boxes, block_selection_boxes_mut, set_block_selection_boxes, Vec<SelectionBox>);
    ref_field!(sign_entries_, sign_entries, sign_entries_mut, set_sign_entries, Vec<SignEntry>);
    ref_field!(hovered_tooltip_, hovered_tooltip, hovered_tooltip_mut, set_hovered_tooltip, Vec<TooltipLine>);
    ref_field!(entity_billboards_, entity_billboards, entity_billboards_mut, set_entity_billboards, Vec<EntityBillboard>);
    ref_field!(skull_entries_, skull_entries, skull_entries_mut, set_skull_entries, Vec<SkullRenderEntry>);
    ref_field!(chest_entries_, chest_entries, chest_entries_mut, set_chest_entries, Vec<ChestRenderEntry>);
    ref_field!(pending_player_skins_, pending_player_skins, pending_player_skins_mut, set_pending_player_skins, Vec<super::PendingPlayerSkin>);
    ref_field!(script_hud_before_commands_, script_hud_before_commands, script_hud_before_commands_mut, set_script_hud_before_commands, Vec<crate::render::hooks::ScriptDrawCommand>);
    ref_field!(script_hud_commands_, script_hud_commands, script_hud_commands_mut, set_script_hud_commands, Vec<crate::render::hooks::ScriptDrawCommand>);
    ref_field!(fp_vanilla_flags_, fp_vanilla_flags, fp_vanilla_flags_mut, set_fp_vanilla_flags, crate::render::first_person::VanillaTransformFlags);
    ref_field!(local_player_billboard_, local_player_billboard, local_player_billboard_mut, set_local_player_billboard, Option<EntityBillboard>);
    ref_field!(recent_sounds_, recent_sounds, recent_sounds_mut, set_recent_sounds, Vec<String>);

    /// Dimension id. Vanilla values: -1 (nether), 0 (overworld), 1 (end).
    pub fn dimension(&self) -> i8 { self.dimension_ }
    pub fn set_dimension(&mut self, v: i8) {
        assert!((-1..=1).contains(&v), "dimension must be -1..=1, got {}", v);
        self.dimension_ = v;
    }

    /// Gamemode. 0=survival, 1=creative, 2=adventure, 3=spectator.
    pub fn gamemode(&self) -> u8 { self.gamemode_ }
    pub fn set_gamemode(&mut self, v: u8) {
        assert!(v <= 3, "gamemode must be 0..=3, got {}", v);
        self.gamemode_ = v;
    }
}

// =========================================================================
// FrameProfile — per-frame timing data, rolling statistics and counters
// =========================================================================
pub struct FrameProfile {
    frame_total_us_: u64,
    frame_interval_us_: u64,
    frame_outside_us_: u64,
    frame_tasks_us_: u64,
    frame_network_us_: u64,
    frame_world_us_: u64,
    frame_tick_us_: u64,
    frame_render_us_: u64,
    frame_other_us_: u64,
    frame_script_us_: u64,
    frame_script_callbacks_: u32,
    frame_script_slow_callbacks_: u32,
    frame_gpu_us_: u64,
    frame_cpu_us_: u64,
    frame_acquire_us_: u64,
    frame_command_us_: u64,
    frame_submit_us_: u64,
    frame_present_us_: u64,
    frame_sync_us_: u64,
    frame_mesh_us_: u64,
    frame_particle_us_: u64,
    frame_nametag_us_: u64,
    frame_entity_us_: u64,
    entity_loop_us_: u64,
    entity_visible_count_: u32,
    entity_culled_count_: u32,
    entity_batch_reused_: bool,
    entity_cache_hits_: u32,
    entity_cache_misses_: u32,
    entity_hash_us_: u64,
    entity_lookup_us_: u64,
    entity_append_us_: u64,
    entity_upload_us_: u64,
    entity_skin_sync_us_: u64,
    entity_generate_us_: u64,
    entity_prune_us_: u64,
    entity_extras_us_: u64,
    frame_local_us_: u64,
    frame_gui_us_: u64,
    frame_chunk_upload_us_: u64,
    frame_chunk_upload_bytes_: u64,
    frame_chunk_upload_count_: u32,
    frame_network_debug_: crate::client::network::NetworkDebugProfile,
    frame_interval_in_gameplay_: bool,
    particle_batch_reused_: bool,
    local_batch_reused_: bool,
    particle_count_: usize,
    entity_count_: usize,
    entity_state_hash_: u64,
    entity_billboard_generation_: u64,
    sky_brightness_cached_: f32,
    chunk_count_loaded_: usize,
    completed_frame_profile_: FrameDebugProfile,
    time_: f32,
    // Rolling-window statistics (private).
    frame_interval_history_: VecDeque<FrameSpikeBreakdown>,
    frame_worst_candidates_: VecDeque<FrameSpikeBreakdown>,
    frame_interval_counts_: BTreeMap<u64, usize>,
    frame_interval_history_span_us_: u64,
    frame_interval_clock_us_: u64,
    frame_interval_sequence_: u64,
    frame_interval_collecting_: bool,
    frame_interval_warmup_remaining_us_: u64,
    frame_interval_stats_elapsed_us_: u64,
    frame_one_percent_low_fps_: f32,
    frame_zero_point_one_percent_low_fps_: f32,
    frame_p99_interval_us_: u64,
    frame_p99_9_interval_us_: u64,
    frame_max_interval_us_: u64,
    frame_worst_breakdown_: FrameSpikeBreakdown,
    frame_worst_age_us_: u64,
}

impl Default for FrameProfile {
    fn default() -> Self {
        Self {
            frame_total_us_: 0,
            frame_interval_us_: 0,
            frame_outside_us_: 0,
            frame_tasks_us_: 0,
            frame_network_us_: 0,
            frame_world_us_: 0,
            frame_tick_us_: 0,
            frame_render_us_: 0,
            frame_other_us_: 0,
            frame_script_us_: 0,
            frame_script_callbacks_: 0,
            frame_script_slow_callbacks_: 0,
            frame_gpu_us_: 0,
            frame_cpu_us_: 0,
            frame_acquire_us_: 0,
            frame_command_us_: 0,
            frame_submit_us_: 0,
            frame_present_us_: 0,
            frame_sync_us_: 0,
            frame_mesh_us_: 0,
            frame_particle_us_: 0,
            frame_nametag_us_: 0,
            frame_entity_us_: 0,
            entity_loop_us_: 0,
            entity_visible_count_: 0,
            entity_culled_count_: 0,
            entity_batch_reused_: false,
            entity_cache_hits_: 0,
            entity_cache_misses_: 0,
            entity_hash_us_: 0,
            entity_lookup_us_: 0,
            entity_append_us_: 0,
            entity_upload_us_: 0,
            entity_skin_sync_us_: 0,
            entity_generate_us_: 0,
            entity_prune_us_: 0,
            entity_extras_us_: 0,
            frame_local_us_: 0,
            frame_gui_us_: 0,
            frame_chunk_upload_us_: 0,
            frame_chunk_upload_bytes_: 0,
            frame_chunk_upload_count_: 0,
            frame_network_debug_: crate::client::network::NetworkDebugProfile::default(),
            frame_interval_in_gameplay_: false,
            particle_batch_reused_: false,
            local_batch_reused_: false,
            particle_count_: 0,
            entity_count_: 0,
            entity_state_hash_: 0,
            entity_billboard_generation_: 0,
            sky_brightness_cached_: 0.0,
            chunk_count_loaded_: 0,
            completed_frame_profile_: FrameDebugProfile::default(),
            time_: 0.0,
            frame_interval_history_: VecDeque::new(),
            frame_worst_candidates_: VecDeque::new(),
            frame_interval_counts_: BTreeMap::new(),
            frame_interval_history_span_us_: 0,
            frame_interval_clock_us_: 0,
            frame_interval_sequence_: 0,
            frame_interval_collecting_: false,
            frame_interval_warmup_remaining_us_: 0,
            frame_interval_stats_elapsed_us_: 0,
            frame_one_percent_low_fps_: 0.0,
            frame_zero_point_one_percent_low_fps_: 0.0,
            frame_p99_interval_us_: 0,
            frame_p99_9_interval_us_: 0,
            frame_max_interval_us_: 0,
            frame_worst_breakdown_: FrameSpikeBreakdown::default(),
            frame_worst_age_us_: 0,
        }
    }
}

impl FrameProfile {
    copy_field!(frame_total_us_, frame_total_us, frame_total_us_mut, set_frame_total_us, u64);
    copy_field!(frame_interval_us_, frame_interval_us, frame_interval_us_mut, set_frame_interval_us, u64);
    copy_field!(frame_outside_us_, frame_outside_us, frame_outside_us_mut, set_frame_outside_us, u64);
    copy_field!(frame_tasks_us_, frame_tasks_us, frame_tasks_us_mut, set_frame_tasks_us, u64);
    copy_field!(frame_network_us_, frame_network_us, frame_network_us_mut, set_frame_network_us, u64);
    copy_field!(frame_world_us_, frame_world_us, frame_world_us_mut, set_frame_world_us, u64);
    copy_field!(frame_tick_us_, frame_tick_us, frame_tick_us_mut, set_frame_tick_us, u64);
    copy_field!(frame_render_us_, frame_render_us, frame_render_us_mut, set_frame_render_us, u64);
    copy_field!(frame_other_us_, frame_other_us, frame_other_us_mut, set_frame_other_us, u64);
    copy_field!(frame_script_us_, frame_script_us, frame_script_us_mut, set_frame_script_us, u64);
    copy_field!(frame_script_callbacks_, frame_script_callbacks, frame_script_callbacks_mut, set_frame_script_callbacks, u32);
    copy_field!(frame_script_slow_callbacks_, frame_script_slow_callbacks, frame_script_slow_callbacks_mut, set_frame_script_slow_callbacks, u32);
    copy_field!(frame_gpu_us_, frame_gpu_us, frame_gpu_us_mut, set_frame_gpu_us, u64);
    copy_field!(frame_cpu_us_, frame_cpu_us, frame_cpu_us_mut, set_frame_cpu_us, u64);
    copy_field!(frame_acquire_us_, frame_acquire_us, frame_acquire_us_mut, set_frame_acquire_us, u64);
    copy_field!(frame_command_us_, frame_command_us, frame_command_us_mut, set_frame_command_us, u64);
    copy_field!(frame_submit_us_, frame_submit_us, frame_submit_us_mut, set_frame_submit_us, u64);
    copy_field!(frame_present_us_, frame_present_us, frame_present_us_mut, set_frame_present_us, u64);
    copy_field!(frame_sync_us_, frame_sync_us, frame_sync_us_mut, set_frame_sync_us, u64);
    copy_field!(frame_mesh_us_, frame_mesh_us, frame_mesh_us_mut, set_frame_mesh_us, u64);
    copy_field!(frame_particle_us_, frame_particle_us, frame_particle_us_mut, set_frame_particle_us, u64);
    copy_field!(frame_nametag_us_, frame_nametag_us, frame_nametag_us_mut, set_frame_nametag_us, u64);
    copy_field!(frame_entity_us_, frame_entity_us, frame_entity_us_mut, set_frame_entity_us, u64);
    copy_field!(entity_loop_us_, entity_loop_us, entity_loop_us_mut, set_entity_loop_us, u64);
    copy_field!(entity_visible_count_, entity_visible_count, entity_visible_count_mut, set_entity_visible_count, u32);
    copy_field!(entity_culled_count_, entity_culled_count, entity_culled_count_mut, set_entity_culled_count, u32);
    copy_field!(entity_batch_reused_, entity_batch_reused, entity_batch_reused_mut, set_entity_batch_reused, bool);
    copy_field!(entity_cache_hits_, entity_cache_hits, entity_cache_hits_mut, set_entity_cache_hits, u32);
    copy_field!(entity_cache_misses_, entity_cache_misses, entity_cache_misses_mut, set_entity_cache_misses, u32);
    copy_field!(entity_hash_us_, entity_hash_us, entity_hash_us_mut, set_entity_hash_us, u64);
    copy_field!(entity_lookup_us_, entity_lookup_us, entity_lookup_us_mut, set_entity_lookup_us, u64);
    copy_field!(entity_append_us_, entity_append_us, entity_append_us_mut, set_entity_append_us, u64);
    copy_field!(entity_upload_us_, entity_upload_us, entity_upload_us_mut, set_entity_upload_us, u64);
    copy_field!(entity_skin_sync_us_, entity_skin_sync_us, entity_skin_sync_us_mut, set_entity_skin_sync_us, u64);
    copy_field!(entity_generate_us_, entity_generate_us, entity_generate_us_mut, set_entity_generate_us, u64);
    copy_field!(entity_prune_us_, entity_prune_us, entity_prune_us_mut, set_entity_prune_us, u64);
    copy_field!(entity_extras_us_, entity_extras_us, entity_extras_us_mut, set_entity_extras_us, u64);
    copy_field!(frame_local_us_, frame_local_us, frame_local_us_mut, set_frame_local_us, u64);
    copy_field!(frame_gui_us_, frame_gui_us, frame_gui_us_mut, set_frame_gui_us, u64);
    copy_field!(frame_chunk_upload_us_, frame_chunk_upload_us, frame_chunk_upload_us_mut, set_frame_chunk_upload_us, u64);
    copy_field!(frame_chunk_upload_bytes_, frame_chunk_upload_bytes, frame_chunk_upload_bytes_mut, set_frame_chunk_upload_bytes, u64);
    copy_field!(frame_chunk_upload_count_, frame_chunk_upload_count, frame_chunk_upload_count_mut, set_frame_chunk_upload_count, u32);
    copy_field!(frame_network_debug_, frame_network_debug, frame_network_debug_mut, set_frame_network_debug, crate::client::network::NetworkDebugProfile);
    copy_field!(frame_interval_in_gameplay_, frame_interval_in_gameplay, frame_interval_in_gameplay_mut, set_frame_interval_in_gameplay, bool);
    copy_field!(particle_batch_reused_, particle_batch_reused, particle_batch_reused_mut, set_particle_batch_reused, bool);
    copy_field!(local_batch_reused_, local_batch_reused, local_batch_reused_mut, set_local_batch_reused, bool);
    copy_field!(particle_count_, particle_count, particle_count_mut, set_particle_count, usize);
    copy_field!(entity_count_, entity_count, entity_count_mut, set_entity_count, usize);
    copy_field!(entity_state_hash_, entity_state_hash, entity_state_hash_mut, set_entity_state_hash, u64);
    copy_field!(entity_billboard_generation_, entity_billboard_generation, entity_billboard_generation_mut, set_entity_billboard_generation, u64);
    copy_field!(sky_brightness_cached_, sky_brightness_cached, sky_brightness_cached_mut, set_sky_brightness_cached, f32);
    copy_field!(chunk_count_loaded_, chunk_count_loaded, chunk_count_loaded_mut, set_chunk_count_loaded, usize);
    copy_field!(time_, time, time_mut, set_time, f32);

    /// Snapshot of the previously completed frame's debug profile (read-only).
    pub fn completed_frame_profile(&self) -> &FrameDebugProfile { &self.completed_frame_profile_ }
    pub fn completed_frame_profile_mut(&mut self) -> &mut FrameDebugProfile { &mut self.completed_frame_profile_ }

    /// Records the wall-clock time for the current frame.
    pub fn record_frame_total_us(&mut self, v: u64) { self.frame_total_us_ = v; }
    /// Records the inter-frame interval for the current frame.
    pub fn record_frame_interval_us(&mut self, v: u64) { self.frame_interval_us_ = v; }
    /// Records time spent outside renderer tasks for the current frame.
    pub fn record_frame_outside_us(&mut self, v: u64) { self.frame_outside_us_ = v; }
    pub fn record_frame_tasks_us(&mut self, v: u64) { self.frame_tasks_us_ = v; }
    pub fn record_frame_network_us(&mut self, v: u64) { self.frame_network_us_ = v; }
    pub fn record_frame_world_us(&mut self, v: u64) { self.frame_world_us_ = v; }
    pub fn record_frame_tick_us(&mut self, v: u64) { self.frame_tick_us_ = v; }
    pub fn record_frame_render_us(&mut self, v: u64) { self.frame_render_us_ = v; }
    pub fn record_frame_other_us(&mut self, v: u64) { self.frame_other_us_ = v; }
    pub fn record_frame_script_us(&mut self, v: u64) { self.frame_script_us_ = v; }
    pub fn record_frame_script_callbacks(&mut self, v: u32) { self.frame_script_callbacks_ = v; }
    pub fn record_frame_script_slow_callbacks(&mut self, v: u32) { self.frame_script_slow_callbacks_ = v; }
    pub fn record_frame_gpu_us(&mut self, v: u64) { self.frame_gpu_us_ = v; }
    pub fn record_frame_cpu_us(&mut self, v: u64) { self.frame_cpu_us_ = v; }
    pub fn record_frame_acquire_us(&mut self, v: u64) { self.frame_acquire_us_ = v; }
    pub fn record_frame_command_us(&mut self, v: u64) { self.frame_command_us_ = v; }
    pub fn record_frame_submit_us(&mut self, v: u64) { self.frame_submit_us_ = v; }
    pub fn record_frame_present_us(&mut self, v: u64) { self.frame_present_us_ = v; }
    pub fn record_frame_sync_us(&mut self, v: u64) { self.frame_sync_us_ = v; }
    pub fn record_frame_mesh_us(&mut self, v: u64) { self.frame_mesh_us_ = v; }
    pub fn record_frame_particle_us(&mut self, v: u64) { self.frame_particle_us_ = v; }
    pub fn record_frame_nametag_us(&mut self, v: u64) { self.frame_nametag_us_ = v; }
    pub fn record_frame_entity_us(&mut self, v: u64) { self.frame_entity_us_ = v; }
    pub fn record_entity_loop_us(&mut self, v: u64) { self.entity_loop_us_ = v; }
    pub fn record_entity_visible_count(&mut self, v: u32) { self.entity_visible_count_ = v; }
    pub fn record_entity_culled_count(&mut self, v: u32) { self.entity_culled_count_ = v; }
    pub fn record_entity_batch_reused(&mut self, v: bool) { self.entity_batch_reused_ = v; }
    pub fn record_entity_cache_hits(&mut self, v: u32) { self.entity_cache_hits_ = v; }
    pub fn record_entity_cache_misses(&mut self, v: u32) { self.entity_cache_misses_ = v; }
    pub fn record_entity_hash_us(&mut self, v: u64) { self.entity_hash_us_ = v; }
    pub fn record_entity_lookup_us(&mut self, v: u64) { self.entity_lookup_us_ = v; }
    pub fn record_entity_append_us(&mut self, v: u64) { self.entity_append_us_ = v; }
    pub fn record_entity_upload_us(&mut self, v: u64) { self.entity_upload_us_ = v; }
    pub fn record_entity_skin_sync_us(&mut self, v: u64) { self.entity_skin_sync_us_ = v; }
    pub fn record_entity_generate_us(&mut self, v: u64) { self.entity_generate_us_ = v; }
    pub fn record_entity_prune_us(&mut self, v: u64) { self.entity_prune_us_ = v; }
    pub fn record_entity_extras_us(&mut self, v: u64) { self.entity_extras_us_ = v; }
    pub fn record_frame_local_us(&mut self, v: u64) { self.frame_local_us_ = v; }
    pub fn record_frame_gui_us(&mut self, v: u64) { self.frame_gui_us_ = v; }
    pub fn record_frame_chunk_upload_us(&mut self, v: u64) { self.frame_chunk_upload_us_ = v; }
    pub fn record_frame_chunk_upload_bytes(&mut self, v: u64) { self.frame_chunk_upload_bytes_ = v; }
    pub fn record_frame_chunk_upload_count(&mut self, v: u32) { self.frame_chunk_upload_count_ = v; }
    pub fn record_frame_network_debug(&mut self, v: crate::client::network::NetworkDebugProfile) { self.frame_network_debug_ = v; }
    pub fn record_frame_interval_in_gameplay(&mut self, v: bool) { self.frame_interval_in_gameplay_ = v; }
    pub fn record_particle_batch_reused(&mut self, v: bool) { self.particle_batch_reused_ = v; }
    pub fn record_local_batch_reused(&mut self, v: bool) { self.local_batch_reused_ = v; }
    pub fn record_particle_count(&mut self, v: usize) { self.particle_count_ = v; }
    pub fn record_entity_count(&mut self, v: usize) { self.entity_count_ = v; }
    pub fn record_entity_state_hash(&mut self, v: u64) { self.entity_state_hash_ = v; }
    pub fn record_entity_billboard_generation(&mut self, v: u64) { self.entity_billboard_generation_ = v; }
    pub fn record_sky_brightness_cached(&mut self, v: f32) { self.sky_brightness_cached_ = v; }
    pub fn record_chunk_count_loaded(&mut self, v: usize) { self.chunk_count_loaded_ = v; }
    pub fn record_time(&mut self, v: f32) { self.time_ = v; }

    /// Increments `entity_cache_hits` by 1.
    pub fn inc_entity_cache_hits(&mut self) { self.entity_cache_hits_ = self.entity_cache_hits_.saturating_add(1); }
    /// Increments `entity_cache_misses` by 1.
    pub fn inc_entity_cache_misses(&mut self) { self.entity_cache_misses_ = self.entity_cache_misses_.saturating_add(1); }
    /// Increments `entity_visible_count` by 1.
    pub fn inc_entity_visible_count(&mut self) { self.entity_visible_count_ = self.entity_visible_count_.saturating_add(1); }
    /// Increments `entity_culled_count` by 1.
    pub fn inc_entity_culled_count(&mut self) { self.entity_culled_count_ = self.entity_culled_count_.saturating_add(1); }

    /// Resets all per-frame counters back to zero. Called once the
    /// completed-frame snapshot has been captured so the next frame starts
    /// from a clean slate.
    pub fn reset_current_frame_profile(&mut self) {
        self.frame_gpu_us_ = 0;
        self.frame_cpu_us_ = 0;
        self.frame_acquire_us_ = 0;
        self.frame_command_us_ = 0;
        self.frame_submit_us_ = 0;
        self.frame_present_us_ = 0;
        self.frame_mesh_us_ = 0;
        self.frame_particle_us_ = 0;
        self.frame_nametag_us_ = 0;
        self.frame_entity_us_ = 0;
        self.entity_loop_us_ = 0;
        self.entity_visible_count_ = 0;
        self.entity_culled_count_ = 0;
        self.entity_batch_reused_ = false;
        self.entity_cache_hits_ = 0;
        self.entity_cache_misses_ = 0;
        self.entity_hash_us_ = 0;
        self.entity_lookup_us_ = 0;
        self.entity_append_us_ = 0;
        self.entity_upload_us_ = 0;
        self.entity_skin_sync_us_ = 0;
        self.entity_generate_us_ = 0;
        self.entity_prune_us_ = 0;
        self.entity_extras_us_ = 0;
        self.frame_local_us_ = 0;
        self.frame_gui_us_ = 0;
        self.frame_chunk_upload_us_ = 0;
        self.frame_chunk_upload_bytes_ = 0;
        self.frame_chunk_upload_count_ = 0;
        self.particle_batch_reused_ = false;
        self.local_batch_reused_ = false;
    }

    /// Captures the current per-frame metrics into `completed_frame_profile_`
    /// and recomputes the rolling 1% low / p99 statistics.
    pub fn snapshot_completed_frame_profile(&mut self, debug_overlay: bool, max_framerate: u32) {
        self.update_frame_interval_statistics(debug_overlay);
        self.completed_frame_profile_ = FrameDebugProfile {
            max_framerate,
            particle_count: self.particle_count_,
            entity_count: self.entity_count_,
            chunk_count_loaded: self.chunk_count_loaded_,
            total_us: self.frame_total_us_,
            interval_us: self.frame_interval_us_,
            one_percent_low_fps: self.frame_one_percent_low_fps_,
            zero_point_one_percent_low_fps: self.frame_zero_point_one_percent_low_fps_,
            p99_interval_us: self.frame_p99_interval_us_,
            p99_9_interval_us: self.frame_p99_9_interval_us_,
            max_interval_us: self.frame_max_interval_us_,
            interval_sample_count: self.frame_interval_history_.len() as u32,
            worst: self.frame_worst_breakdown_,
            worst_age_us: self.frame_worst_age_us_,
            outside_us: self.frame_outside_us_,
            tasks_us: self.frame_tasks_us_,
            network_us: self.frame_network_us_,
            world_us: self.frame_world_us_,
            tick_us: self.frame_tick_us_,
            sync_us: self.frame_sync_us_,
            render_us: self.frame_render_us_,
            other_us: self.frame_other_us_,
            script_us: self.frame_script_us_,
            script_callbacks: self.frame_script_callbacks_,
            script_slow_callbacks: self.frame_script_slow_callbacks_,
            fence_us: self.frame_gpu_us_,
            cpu_us: self.frame_cpu_us_,
            acquire_us: self.frame_acquire_us_,
            command_us: self.frame_command_us_,
            submit_us: self.frame_submit_us_,
            present_us: self.frame_present_us_,
            mesh_us: self.frame_mesh_us_,
            particle_us: self.frame_particle_us_,
            nametag_us: self.frame_nametag_us_,
            entity_us: self.frame_entity_us_,
            entity_loop_us: self.entity_loop_us_,
            entity_visible_count: self.entity_visible_count_,
            entity_culled_count: self.entity_culled_count_,
            entity_batch_reused: self.entity_batch_reused_,
            entity_cache_hits: self.entity_cache_hits_,
            entity_cache_misses: self.entity_cache_misses_,
            entity_hash_us: self.entity_hash_us_,
            entity_lookup_us: self.entity_lookup_us_,
            entity_upload_us: self.entity_upload_us_,
            entity_skin_sync_us: self.entity_skin_sync_us_,
            entity_generate_us: self.entity_generate_us_,
            entity_prune_us: self.entity_prune_us_,
            entity_extras_us: self.entity_extras_us_,
            local_us: self.frame_local_us_,
            gui_us: self.frame_gui_us_,
            chunk_upload_us: self.frame_chunk_upload_us_,
            chunk_upload_bytes: self.frame_chunk_upload_bytes_,
            chunk_upload_count: self.frame_chunk_upload_count_,
            particle_batch_reused: self.particle_batch_reused_,
            local_batch_reused: self.local_batch_reused_,
        };
    }

    fn update_frame_interval_statistics(&mut self, debug_overlay: bool) {
        const HISTORY_WINDOW_US: u64 = 10_000_000;
        const STATS_INTERVAL_US: u64 = 1_000_000;
        const MAX_HISTORY_SAMPLES: usize = 30_000;
        const ENABLE_WARMUP_US: u64 = 250_000;

        if !debug_overlay {
            self.frame_interval_history_.clear();
            self.frame_worst_candidates_.clear();
            self.frame_interval_counts_.clear();
            self.frame_interval_history_span_us_ = 0;
            self.frame_interval_clock_us_ = 0;
            self.frame_interval_sequence_ = 0;
            self.frame_interval_collecting_ = false;
            self.frame_interval_warmup_remaining_us_ = 0;
            self.frame_interval_stats_elapsed_us_ = 0;
            self.frame_one_percent_low_fps_ = 0.0;
            self.frame_zero_point_one_percent_low_fps_ = 0.0;
            self.frame_p99_interval_us_ = 0;
            self.frame_p99_9_interval_us_ = 0;
            self.frame_max_interval_us_ = 0;
            self.frame_worst_breakdown_ = FrameSpikeBreakdown::default();
            self.frame_worst_age_us_ = 0;
            return;
        }
        if !self.frame_interval_in_gameplay_ {
            return;
        }
        if self.frame_interval_us_ == 0 {
            return;
        }
        if !self.frame_interval_collecting_ {
            self.frame_interval_collecting_ = true;
            self.frame_interval_warmup_remaining_us_ = ENABLE_WARMUP_US;
        }
        if self.frame_interval_warmup_remaining_us_ > 0 {
            self.frame_interval_warmup_remaining_us_ = self
                .frame_interval_warmup_remaining_us_
                .saturating_sub(self.frame_interval_us_);
            return;
        }

        self.frame_interval_sequence_ = self.frame_interval_sequence_.wrapping_add(1);
        self.frame_interval_clock_us_ = self
            .frame_interval_clock_us_
            .saturating_add(self.frame_interval_us_);
        let sample = FrameSpikeBreakdown {
            interval_us: self.frame_interval_us_,
            total_us: self.frame_total_us_,
            outside_us: self.frame_outside_us_,
            tasks_us: self.frame_tasks_us_,
            network_us: self.frame_network_us_,
            world_us: self.frame_world_us_,
            tick_us: self.frame_tick_us_,
            sync_us: self.frame_sync_us_,
            render_us: self.frame_render_us_,
            other_us: self.frame_other_us_,
            script_us: self.frame_script_us_,
            fence_us: self.frame_gpu_us_,
            cpu_us: self.frame_cpu_us_,
            acquire_us: self.frame_acquire_us_,
            command_us: self.frame_command_us_,
            submit_us: self.frame_submit_us_,
            present_us: self.frame_present_us_,
            mesh_us: self.frame_mesh_us_,
            entity_us: self.frame_entity_us_,
            entity_loop_us: self.entity_loop_us_,
            entity_hash_us: self.entity_hash_us_,
            entity_lookup_us: self.entity_lookup_us_,
            entity_generate_us: self.entity_generate_us_,
            entity_upload_us: self.entity_upload_us_,
            entity_skin_sync_us: self.entity_skin_sync_us_,
            entity_prune_us: self.entity_prune_us_,
            entity_extras_us: self.entity_extras_us_,
            entity_cache_hits: self.entity_cache_hits_,
            entity_cache_misses: self.entity_cache_misses_,
            entity_visible_count: self.entity_visible_count_,
            entity_culled_count: self.entity_culled_count_,
            particle_us: self.frame_particle_us_,
            nametag_us: self.frame_nametag_us_,
            local_us: self.frame_local_us_,
            gui_us: self.frame_gui_us_,
            chunk_upload_us: self.frame_chunk_upload_us_,
            network_packet_kind: self.frame_network_debug_.worst_packet_kind,
            network_packet_us: self.frame_network_debug_.worst_packet_us,
            network_packet_units: self.frame_network_debug_.worst_packet_units,
            network_hook_us: self.frame_network_debug_.worst_hook_us,
            network_session_us: self.frame_network_debug_.worst_session_us,
            network_inventory_us: self.frame_network_debug_.worst_inventory_us,
            network_entity_us: self.frame_network_debug_.worst_entity_us,
            network_world_us: self.frame_network_debug_.worst_world_us,
            network_scheduler_us: self.frame_network_debug_.scheduler_us,
            network_scanned_packets: self.frame_network_debug_.scanned_packets,
            network_handled_packets: self.frame_network_debug_.handled_packets,
            network_deferred_packets: self.frame_network_debug_.deferred_packets,
            sequence: self.frame_interval_sequence_,
            end_us: self.frame_interval_clock_us_,
        };
        self.frame_interval_history_.push_back(sample);
        while self
            .frame_worst_candidates_
            .back()
            .is_some_and(|candidate| candidate.interval_us <= sample.interval_us)
        {
            self.frame_worst_candidates_.pop_back();
        }
        self.frame_worst_candidates_.push_back(sample);
        *self
            .frame_interval_counts_
            .entry(self.frame_interval_us_)
            .or_default() += 1;
        self.frame_interval_history_span_us_ = self
            .frame_interval_history_span_us_
            .saturating_add(self.frame_interval_us_);
        self.frame_interval_stats_elapsed_us_ = self
            .frame_interval_stats_elapsed_us_
            .saturating_add(self.frame_interval_us_);
        while self.frame_interval_history_.len() > 1
            && (self.frame_interval_history_span_us_ > HISTORY_WINDOW_US
                || self.frame_interval_history_.len() > MAX_HISTORY_SAMPLES)
        {
            if let Some(removed) = self.frame_interval_history_.pop_front() {
                self.frame_interval_history_span_us_ = self
                    .frame_interval_history_span_us_
                    .saturating_sub(removed.interval_us);
                let remove_entry =
                    if let Some(count) = self.frame_interval_counts_.get_mut(&removed.interval_us) {
                        *count -= 1;
                        *count == 0
                    } else {
                        false
                    };
                if remove_entry {
                    self.frame_interval_counts_.remove(&removed.interval_us);
                }
            }
        }

        let oldest_sequence = self
            .frame_interval_history_
            .front()
            .map_or(self.frame_interval_sequence_, |sample| sample.sequence);
        while self
            .frame_worst_candidates_
            .front()
            .is_some_and(|candidate| candidate.sequence < oldest_sequence)
        {
            self.frame_worst_candidates_.pop_front();
        }
        self.frame_worst_breakdown_ = self
            .frame_worst_candidates_
            .front()
            .copied()
            .unwrap_or_default();
        self.frame_worst_age_us_ = self
            .frame_interval_clock_us_
            .saturating_sub(self.frame_worst_breakdown_.end_us);
        self.frame_max_interval_us_ = self.frame_worst_breakdown_.interval_us;

        if self.frame_interval_stats_elapsed_us_ < STATS_INTERVAL_US {
            return;
        }
        self.frame_interval_stats_elapsed_us_ %= STATS_INTERVAL_US;
        let sample_count = self.frame_interval_history_.len();
        let slow_count = sample_count.div_ceil(100).max(1);
        let mut remaining_slow = slow_count;
        let mut slow_sum_us = 0u128;
        for (&interval_us, &count) in self.frame_interval_counts_.iter().rev() {
            let take = remaining_slow.min(count);
            slow_sum_us += u128::from(interval_us) * take as u128;
            remaining_slow -= take;
            if remaining_slow == 0 {
                break;
            }
        }
        let slow_average_us = slow_sum_us / slow_count as u128;
        self.frame_one_percent_low_fps_ = if slow_average_us == 0 {
            0.0
        } else {
            1_000_000.0 / slow_average_us as f32
        };
        let p99_rank = (sample_count * 99).div_ceil(100).max(1);
        let mut cumulative = 0usize;
        self.frame_p99_interval_us_ = 0;
        for (&interval_us, &count) in &self.frame_interval_counts_ {
            cumulative += count;
            if cumulative >= p99_rank {
                self.frame_p99_interval_us_ = interval_us;
                break;
            }
        }
        let slowest_permille = sample_count.div_ceil(1000).max(1);
        let mut remaining_slowest = slowest_permille;
        let mut slowest_sum_us = 0u128;
        for (&interval_us, &count) in self.frame_interval_counts_.iter().rev() {
            let take = remaining_slowest.min(count);
            slowest_sum_us += u128::from(interval_us) * take as u128;
            remaining_slowest -= take;
            if remaining_slowest == 0 {
                break;
            }
        }
        let slowest_avg_us = slowest_sum_us / slowest_permille as u128;
        self.frame_zero_point_one_percent_low_fps_ = if slowest_avg_us == 0 {
            0.0
        } else {
            1_000_000.0 / slowest_avg_us as f32
        };
        let p99_9_rank = (sample_count * 999).div_ceil(1000).max(1);
        let mut cumulative_p999 = 0usize;
        self.frame_p99_9_interval_us_ = 0;
        for (&interval_us, &count) in &self.frame_interval_counts_ {
            cumulative_p999 += count;
            if cumulative_p999 >= p99_9_rank {
                self.frame_p99_9_interval_us_ = interval_us;
                break;
            }
        }
    }
}

// =========================================================================
// GameRenderState — top-level container for renderer-side shared state.
// Only six sub-structure fields are exposed; all leaf data is private and
// goes through the getter/setter/record API on each sub-structure.
// =========================================================================
pub struct GameRenderState {
    pub settings: RenderSettings,
    pub account: AccountState,
    pub server_list: ServerListState,
    pub inventory: InventoryState,
    pub hud: HudState,
    pub frame_profile: FrameProfile,
}

impl Default for GameRenderState {
    fn default() -> Self {
        Self {
            settings: RenderSettings::default(),
            account: AccountState::default(),
            server_list: ServerListState::default(),
            inventory: InventoryState::default(),
            hud: HudState::default(),
            frame_profile: FrameProfile::default(),
        }
    }
}

impl GameRenderState {
    /// Captures the current per-frame metrics into the completed-frame
    /// profile and recomputes the rolling 1% low / p99 statistics.
    pub fn snapshot_completed_frame_profile(&mut self) {
        let debug_overlay = self.settings.debug_overlay();
        let max_framerate = self.settings.max_framerate();
        self.frame_profile
            .snapshot_completed_frame_profile(debug_overlay, max_framerate);
    }

    /// Resets all per-frame counters back to zero. Called once the
    /// completed-frame snapshot has been captured so the next frame starts
    /// from a clean slate.
    pub fn reset_current_frame_profile(&mut self) {
        self.frame_profile.reset_current_frame_profile();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish_debug_warmup(state: &mut GameRenderState) {
        state.frame_profile.set_frame_interval_in_gameplay(true);
        for _ in 0..250 {
            state.frame_profile.set_frame_interval_us(1_000);
            state.snapshot_completed_frame_profile();
        }
    }

    #[test]
    fn completed_frame_profile_is_snapshotted_before_current_metrics_reset() {
        let mut state = GameRenderState::default();
        state.settings.set_max_framerate(120);
        state.frame_profile.set_entity_count(97);
        state.frame_profile.set_frame_total_us(6_000);
        state.frame_profile.set_frame_interval_us(11_700);
        state.frame_profile.set_frame_outside_us(5_700);
        state.frame_profile.set_frame_render_us(3_500);
        state.frame_profile.set_frame_cpu_us(3_488);
        state.frame_profile.set_frame_gpu_us(12);
        state.frame_profile.set_frame_entity_us(2_600);
        state.frame_profile.set_entity_cache_hits(47);
        state.frame_profile.set_frame_chunk_upload_count(2);

        state.snapshot_completed_frame_profile();
        state.reset_current_frame_profile();

        let completed = state.frame_profile.completed_frame_profile();
        assert_eq!(completed.max_framerate, 120);
        assert_eq!(completed.entity_count, 97);
        assert_eq!(completed.total_us, 6_000);
        assert_eq!(completed.interval_us, 11_700);
        assert_eq!(completed.outside_us, 5_700);
        assert_eq!(completed.render_us, 3_500);
        assert_eq!(completed.cpu_us, 3_488);
        assert_eq!(completed.fence_us, 12);
        assert_eq!(completed.entity_us, 2_600);
        assert_eq!(completed.entity_cache_hits, 47);
        assert_eq!(completed.chunk_upload_count, 2);

        assert_eq!(state.frame_profile.frame_cpu_us(), 0);
        assert_eq!(state.frame_profile.frame_gpu_us(), 0);
        assert_eq!(state.frame_profile.frame_entity_us(), 0);
        assert_eq!(state.frame_profile.entity_cache_hits(), 0);
        assert_eq!(state.frame_profile.frame_chunk_upload_count(), 0);
        assert_eq!(state.frame_profile.completed_frame_profile().total_us, 6_000);
    }

    #[test]
    fn debug_history_reports_slowest_one_percent_as_fps() {
        let mut state = GameRenderState::default();
        state.settings.set_debug_overlay(true);
        finish_debug_warmup(&mut state);
        for _ in 0..900 {
            state.frame_profile.set_frame_interval_us(1_000);
            state.snapshot_completed_frame_profile();
        }
        for _ in 0..10 {
            state.frame_profile.set_frame_interval_us(10_000);
            state.frame_profile.set_frame_network_us(8_000);
            state.snapshot_completed_frame_profile();
        }

        let profile = state.frame_profile.completed_frame_profile();
        assert_eq!(profile.interval_sample_count, 910);
        assert_eq!(profile.p99_interval_us, 10_000);
        assert_eq!(profile.max_interval_us, 10_000);
        assert!((profile.one_percent_low_fps - 100.0).abs() < 0.01);
        assert_eq!(profile.worst.network_us, 8_000);
    }

    #[test]
    fn rolling_worst_frame_expires_with_the_ten_second_window() {
        let mut state = GameRenderState::default();
        state.settings.set_debug_overlay(true);
        finish_debug_warmup(&mut state);
        state.frame_profile.set_frame_interval_us(20_000);
        state.frame_profile.set_frame_tick_us(15_000);
        state.snapshot_completed_frame_profile();
        assert_eq!(
            state.frame_profile.completed_frame_profile().worst.tick_us,
            15_000
        );

        state.frame_profile.set_frame_tick_us(0);
        for _ in 0..10_001 {
            state.frame_profile.set_frame_interval_us(1_000);
            state.snapshot_completed_frame_profile();
        }
        assert_eq!(
            state.frame_profile.completed_frame_profile().max_interval_us,
            1_000
        );
        assert_eq!(
            state.frame_profile.completed_frame_profile().worst.tick_us,
            0
        );
    }

    #[test]
    fn fov_setter_rejects_non_positive_values() {
        let mut s = RenderSettings::default();
        s.set_fov(70.0);
        assert_eq!(s.fov(), 70.0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.set_fov(0.0)));
        assert!(result.is_err());
    }

    #[test]
    fn volume_setters_clamp_to_unit_range() {
        let mut s = RenderSettings::default();
        s.set_master_volume(2.0);
        assert_eq!(s.master_volume(), 1.0);
        s.set_master_volume(-1.0);
        assert_eq!(s.master_volume(), 0.0);
    }

    #[test]
    fn hotbar_selected_setter_rejects_out_of_range() {
        let mut s = InventoryState::default();
        s.set_hotbar_selected(8);
        assert_eq!(s.hotbar_selected(), 8);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.set_hotbar_selected(9)));
        assert!(result.is_err());
    }
}
