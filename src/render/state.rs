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

pub struct GameRenderState {
    // Settings
    pub gui_scale: u32,
    pub render_distance: u8,
    pub smooth_lighting: bool,
    pub camera_mode: u8,
    pub particles_label: String,
    pub particles_enabled: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub blocks_volume: f32,
    pub hostile_volume: f32,
    pub friendly_volume: f32,
    pub players_volume: f32,
    pub ambient_volume: f32,
    pub weather_volume: f32,
    pub ui_volume: f32,
    pub audio_device: String,
    pub fov: f32,
    pub max_framerate: u32,
    pub clouds: bool,
    pub weather_effects: bool,
    pub entity_shadows: bool,
    pub view_bobbing: bool,
    pub advanced_tooltips: bool,
    pub better_grass: bool,
    pub connected_textures: bool,
    pub difficulty: u8,
    pub skin_parts: u8,
    pub language_code: String,
    pub language_name: String,
    pub ui_text: UiText,

    // Account / login
    pub server_address: String,
    pub username: String,
    pub account_name: String,
    pub account_status: String,
    pub account_list: Vec<(String, String, bool)>,
    pub account_faces: HashMap<String, [[u8; 4]; 64]>,
    pub selected_account: usize,
    pub entering_offline_name: bool,
    pub offline_username_input: String,
    pub connection_status: String,
    pub server_refreshing: bool,

    // Server list
    pub server_list: Vec<ServerListRow>,
    pub selected_server: usize,
    pub server_list_scroll: usize,
    pub controls_list_scroll: usize,
    pub available_resource_pack_scroll: usize,
    pub selected_resource_pack_scroll: usize,
    pub shader_pack_scroll: usize,
    pub server_editor_name: String,
    pub server_editor_address: String,
    pub server_editor_address_focused: bool,
    pub modding_rows: Vec<ModManagerRow>,
    pub modding_scroll: usize,
    pub modding_status: String,
    pub modding_connection_active: bool,
    pub modding_selected: usize,

    // Mod config screen
    pub mod_config_title: Option<String>,
    pub mod_config_rows: Vec<ModConfigRow>,
    pub mod_config_selected: usize,
    pub mod_config_scroll: usize,
    pub mod_config_status: String,
    pub mod_config_locked: bool,

    // HUD visibility overrides
    pub hud_visible: bool,
    pub crosshair_visible: bool,

    // World info
    pub max_players: u8,
    pub world_time: i64,
    pub day_time: i64,
    pub dimension: i8,
    pub raining: bool,
    pub rain_level: f32,
    pub thunder_level: f32,
    pub gamemode: u8,
    pub level_type: String,
    pub spawn_position: Option<[i32; 3]>,

    // Player status
    pub health: f32,
    pub prev_health: f32,
    pub health_timer: u32,
    pub armor_points: u8,
    pub absorption: f32,
    pub food: i32,
    pub prev_food: i32,
    pub food_timer: u32,
    pub saturation: f32,
    pub experience_bar: f32,
    pub experience_level: i32,
    pub experience_total: i32,

    // Chat
    pub chat_lines: Vec<String>,
    pub chat_open: bool,
    pub chat_input: String,
    pub chat_visible_time: f64,
    pub chat_alpha: f32,
    pub chat_scroll: usize,
    pub chat_last_message_time: f64,
    pub chat_width: f32,
    pub chat_height: u8,
    pub chat_background: bool,
    pub chat_overlay: bool,
    pub chat_player_avatars: bool,
    pub chat_faces: Vec<Option<[[u8; 4]; 64]>>,

    // Sign editor
    pub sign_editor_open: bool,
    pub sign_editor_lines: [String; 4],
    pub sign_editor_active_line: usize,

    // Writable book editor
    pub book_editor_open: bool,
    pub book_pages: Vec<String>,
    pub book_page: usize,
    pub book_signing: bool,
    pub book_title: String,

    // Player list / tab / titles
    pub player_list_open: bool,
    pub player_list: Vec<(String, i32, i32)>,
    pub player_list_faces: Vec<[[u8; 4]; 64]>,
    pub tab_player_avatars: bool,
    pub tab_header: Option<String>,
    pub tab_footer: Option<String>,
    pub title_text: Option<String>,
    pub subtitle_text: Option<String>,
    pub title_alpha: f32,
    pub action_bar: Option<String>,

    // Scoreboard
    pub sidebar_title: Option<String>,
    pub sidebar_lines: Vec<SidebarLine>,

    // World border
    pub world_border_center: [f64; 2],
    pub world_border_diameter: f64,
    pub world_border_warning_blocks: i32,

    // Server info
    pub server_brand: Option<String>,
    pub resource_pack_status: Option<String>,
    pub available_resource_packs: Vec<ResourcePackInfo>,
    pub selected_resource_packs: Vec<ResourcePackInfo>,
    pub shader_packs: Vec<ShaderPackInfo>,
    pub selected_shader_pack: Option<String>,
    pub shader_pack_status: String,
    pub ray_tracing_available: bool,
    pub fsr3_available: bool,

    // Stats / debug
    pub debug_overlay: bool,
    pub completed_frame_profile: FrameDebugProfile,
    pub particle_count: usize,
    pub entity_count: usize,
    pub entity_state_hash: u64,
    pub entity_billboard_generation: u64,
    pub sound_event_count: u64,
    pub recent_sounds: Vec<String>,

    // Entities
    pub entity_billboards: Vec<EntityBillboard>,
    pub skull_entries: Vec<SkullRenderEntry>,
    pub chest_entries: Vec<ChestRenderEntry>,
    /// Shared player skin data, sorted by key for deterministic atlas slots.
    pub pending_player_skins: Vec<super::PendingPlayerSkin>,
    pub player_skin_content_hash: u64,
    pub player_skin_layout_hash: u64,

    // Creative inventory
    pub creative_tab: usize,
    pub creative_scroll: f32,
    pub creative_search: String,

    // Controls
    pub control_bindings: Vec<ControlBindingRow>,
    pub rebinding_action: Option<Action>,
    pub controls_gamepad: bool,
    pub mouse_sensitivity: f32,
    pub invert_mouse: bool,
    pub gamepad_look_sensitivity: f32,
    pub gamepad_cursor_speed: f32,

    // Tooltip (accessed from client/app/frame.rs)
    pub hovered_tooltip: Vec<TooltipLine>,

    // Local skin / model parts
    pub local_model_parts: usize,
    pub local_skin_size: [u32; 2],
    pub local_skin_slim: bool,
    pub local_skin_face: [[u8; 4]; 64],
    pub local_skin_preview: SkinPreviewPixels,
    pub local_skin: PlayerSkin,

    // First-person hand / arm
    pub hand_swing_progress: f32,
    pub hand_item_id: u16,
    pub hand_item_damage: u16,
    pub hand_item_nbt: Option<Vec<u8>>,
    pub hand_swing_timer: f32,
    /// 0=idle, 1=block, 2=eat, 3=drink, 4=bow draw.
    pub hand_use_kind: u8,
    /// Seconds spent using the active item.
    pub hand_use_progress: f32,
    pub first_person_arm_yaw: f32,
    pub first_person_arm_pitch: f32,
    pub first_person_prev_arm_yaw: f32,
    pub first_person_prev_arm_pitch: f32,
    pub first_person_arm_transform: nalgebra::Matrix4<f32>,
    pub first_person_item_transform: nalgebra::Matrix4<f32>,
    pub script_hud_before_commands: Vec<crate::render::hooks::ScriptDrawCommand>,
    pub script_hud_commands: Vec<crate::render::hooks::ScriptDrawCommand>,
    pub fp_vanilla_flags: crate::render::first_person::VanillaTransformFlags,

    // Active potion effects for HUD / inventory display
    pub active_potion_effects: Vec<crate::entity::EntityEffectState>,

    /// Wall-clock seconds for animation (glint, etc.)
    pub time: f32,

    // Environment
    pub sky_brightness_cached: f32,
    pub chunk_count_loaded: usize,
    pub frame_total_us: u64,
    pub frame_interval_us: u64,
    pub frame_outside_us: u64,
    pub frame_tasks_us: u64,
    pub frame_network_us: u64,
    pub frame_world_us: u64,
    pub frame_tick_us: u64,
    pub frame_render_us: u64,
    pub frame_other_us: u64,
    pub frame_script_us: u64,
    pub frame_script_callbacks: u32,
    pub frame_script_slow_callbacks: u32,
    pub frame_gpu_us: u64,
    pub frame_cpu_us: u64,
    pub frame_acquire_us: u64,
    pub frame_command_us: u64,
    pub frame_submit_us: u64,
    pub frame_present_us: u64,
    pub frame_sync_us: u64,
    pub frame_mesh_us: u64,
    pub frame_particle_us: u64,
    pub frame_nametag_us: u64,
    pub frame_entity_us: u64,
    pub entity_loop_us: u64,
    pub entity_visible_count: u32,
    pub entity_culled_count: u32,
    pub entity_batch_reused: bool,
    pub entity_cache_hits: u32,
    pub entity_cache_misses: u32,
    pub entity_hash_us: u64,
    pub entity_lookup_us: u64,
    pub entity_append_us: u64,
    pub entity_upload_us: u64,
    pub entity_skin_sync_us: u64,
    pub entity_generate_us: u64,
    pub entity_prune_us: u64,
    pub entity_extras_us: u64,
    pub frame_local_us: u64,
    pub frame_gui_us: u64,
    pub frame_chunk_upload_us: u64,
    pub frame_chunk_upload_bytes: u64,
    pub frame_chunk_upload_count: u32,
    pub frame_network_debug: crate::client::network::NetworkDebugProfile,
    frame_interval_history: VecDeque<FrameSpikeBreakdown>,
    frame_worst_candidates: VecDeque<FrameSpikeBreakdown>,
    frame_interval_counts: BTreeMap<u64, usize>,
    frame_interval_history_span_us: u64,
    frame_interval_clock_us: u64,
    frame_interval_sequence: u64,
    frame_interval_collecting: bool,
    frame_interval_warmup_remaining_us: u64,
    frame_interval_stats_elapsed_us: u64,
    /// Whether the current frame was captured during active gameplay (Playing
    /// state + window focused).  Only these frames feed the 1% low / p99
    /// statistics so menu transitions, loading screens, and alt-tab spikes
    /// don't pollute the gameplay performance window.
    pub frame_interval_in_gameplay: bool,
    frame_one_percent_low_fps: f32,
    frame_zero_point_one_percent_low_fps: f32,
    frame_p99_interval_us: u64,
    frame_p99_9_interval_us: u64,
    frame_max_interval_us: u64,
    frame_worst_breakdown: FrameSpikeBreakdown,
    frame_worst_age_us: u64,
    pub particle_batch_reused: bool,
    pub local_batch_reused: bool,
    pub underwater: bool,
    pub underwater_yaw: f32,
    pub underwater_pitch: f32,

    // Hotbar
    /// Hotbar slots as (item_id, count, damage).
    pub hotbar_slots: [(u16, u8, u16); 9],
    pub hotbar_selected: usize,

    // Inventory
    pub inventory_open: bool,
    pub inventory_window_id: u8,
    pub inventory_window_type: String,
    pub inventory_window_title: String,
    pub inventory_window_slot_count: usize,
    pub inventory_window_slots: Vec<ItemStackView>,
    pub inventory_window_properties: Vec<(i16, i16)>,
    pub inventory_slots: [ItemStackView; 36],
    pub inventory_armor_slots: [ItemStackView; 4],
    pub inventory_crafting_slots: [ItemStackView; 5],
    pub inventory_cursor_slot: ItemStackView,

    // Block selection / mining
    pub block_selection_boxes: Vec<SelectionBox>,
    pub dig_progress: f32,
    pub dig_position: Option<[i32; 3]>,

    // FPS
    pub fps_count: u32,

    // Signs in world
    pub sign_entries: Vec<SignEntry>,

    // Local player billboard (3rd person)
    pub local_player_billboard: Option<EntityBillboard>,
}

impl Default for GameRenderState {
    fn default() -> Self {
        Self {
            gui_scale: 0,
            render_distance: 0,
            smooth_lighting: false,
            camera_mode: 0,
            particles_label: String::new(),
            particles_enabled: false,
            master_volume: 0.0,
            music_volume: 0.0,
            blocks_volume: 0.0,
            hostile_volume: 0.0,
            friendly_volume: 0.0,
            players_volume: 0.0,
            ambient_volume: 0.0,
            weather_volume: 0.0,
            ui_volume: 0.0,
            audio_device: String::new(),
            fov: 0.0,
            max_framerate: 0,
            clouds: false,
            weather_effects: true,
            entity_shadows: false,
            view_bobbing: false,
            advanced_tooltips: false,
            better_grass: false,
            connected_textures: true,
            difficulty: 0,
            skin_parts: 0,
            language_code: String::new(),
            language_name: String::new(),
            ui_text: UiText::default(),
            server_address: String::new(),
            username: String::new(),
            account_name: String::new(),
            account_status: String::new(),
            account_list: Vec::new(),
            account_faces: HashMap::new(),
            selected_account: 0,
            entering_offline_name: false,
            offline_username_input: String::new(),
            connection_status: String::new(),
            server_refreshing: false,
            server_list: Vec::new(),
            selected_server: 0,
            server_list_scroll: 0,
            controls_list_scroll: 0,
            available_resource_pack_scroll: 0,
            selected_resource_pack_scroll: 0,
            shader_pack_scroll: 0,
            server_editor_name: String::new(),
            server_editor_address: String::new(),
            server_editor_address_focused: false,
            modding_rows: Vec::new(),
            modding_scroll: 0,
            modding_status: String::new(),
            modding_connection_active: false,
            modding_selected: 0,
            mod_config_title: None,
            mod_config_rows: Vec::new(),
            mod_config_selected: 0,
            mod_config_scroll: 0,
            mod_config_status: String::new(),
            mod_config_locked: false,
            hud_visible: true,
            crosshair_visible: true,
            max_players: 0,
            world_time: 0,
            day_time: 0,
            dimension: 0,
            raining: false,
            rain_level: 0.0,
            thunder_level: 0.0,
            gamemode: 0,
            level_type: String::new(),
            spawn_position: None,
            health: 0.0,
            prev_health: 0.0,
            health_timer: 0,
            armor_points: 0,
            absorption: 0.0,
            food: 0,
            prev_food: 0,
            food_timer: 0,
            saturation: 0.0,
            experience_bar: 0.0,
            experience_level: 0,
            experience_total: 0,
            chat_lines: Vec::new(),
            chat_open: false,
            chat_input: String::new(),
            chat_visible_time: 0.0,
            chat_alpha: 0.0,
            chat_scroll: 0,
            chat_last_message_time: 0.0,
            chat_width: 0.4,
            chat_height: 12,
            chat_background: true,
            chat_overlay: true,
            chat_player_avatars: true,
            chat_faces: Vec::new(),
            sign_editor_open: false,
            sign_editor_lines: Default::default(),
            sign_editor_active_line: 0,
            book_editor_open: false,
            book_pages: Vec::new(),
            book_page: 0,
            book_signing: false,
            book_title: String::new(),
            player_list_open: false,
            player_list: Vec::new(),
            player_list_faces: Vec::new(),
            tab_player_avatars: true,
            tab_header: None,
            tab_footer: None,
            title_text: None,
            subtitle_text: None,
            title_alpha: 0.0,
            action_bar: None,
            sidebar_title: None,
            sidebar_lines: Vec::new(),
            world_border_center: [0.0; 2],
            world_border_diameter: 0.0,
            world_border_warning_blocks: 0,
            server_brand: None,
            resource_pack_status: None,
            available_resource_packs: Vec::new(),
            selected_resource_packs: Vec::new(),
            shader_packs: Vec::new(),
            selected_shader_pack: None,
            shader_pack_status: String::new(),
            ray_tracing_available: false,
            fsr3_available: false,
            debug_overlay: false,
            completed_frame_profile: FrameDebugProfile::default(),
            particle_count: 0,
            entity_count: 0,
            entity_state_hash: 0,
            entity_billboard_generation: 0,
            sound_event_count: 0,
            recent_sounds: Vec::new(),
            entity_billboards: Vec::new(),
            skull_entries: Vec::new(),
            chest_entries: Vec::new(),
            pending_player_skins: Vec::new(),
            player_skin_content_hash: 0,
            player_skin_layout_hash: 0,
            creative_tab: 0,
            creative_scroll: 0.0,
            creative_search: String::new(),
            control_bindings: Vec::new(),
            rebinding_action: None,
            controls_gamepad: false,
            mouse_sensitivity: 0.5,
            invert_mouse: false,
            gamepad_look_sensitivity: 0.5,
            gamepad_cursor_speed: 0.25,
            hovered_tooltip: Vec::new(),
            local_model_parts: 0,
            local_skin_size: [0; 2],
            local_skin_slim: false,
            local_skin_face: [[0; 4]; 64],
            local_skin_preview: SkinPreviewPixels::default(),
            local_skin: PlayerSkin::default(),
            hand_swing_progress: 0.0,
            hand_item_id: 0,
            hand_item_damage: 0,
            hand_item_nbt: None,
            hand_swing_timer: 0.0,
            hand_use_kind: 0,
            hand_use_progress: 0.0,
            first_person_arm_yaw: 0.0,
            first_person_arm_pitch: 0.0,
            first_person_prev_arm_yaw: 0.0,
            first_person_prev_arm_pitch: 0.0,
            first_person_arm_transform: nalgebra::Matrix4::identity(),
            first_person_item_transform: nalgebra::Matrix4::identity(),
            script_hud_before_commands: Vec::new(),
            script_hud_commands: Vec::new(),
            fp_vanilla_flags: crate::render::first_person::VanillaTransformFlags::default(),
            active_potion_effects: Vec::new(),
            time: 0.0,
            sky_brightness_cached: 0.0,
            chunk_count_loaded: 0,
            frame_total_us: 0,
            frame_interval_us: 0,
            frame_outside_us: 0,
            frame_tasks_us: 0,
            frame_network_us: 0,
            frame_world_us: 0,
            frame_tick_us: 0,
            frame_render_us: 0,
            frame_other_us: 0,
            frame_script_us: 0,
            frame_script_callbacks: 0,
            frame_script_slow_callbacks: 0,
            frame_gpu_us: 0,
            frame_cpu_us: 0,
            frame_acquire_us: 0,
            frame_command_us: 0,
            frame_submit_us: 0,
            frame_present_us: 0,
            frame_sync_us: 0,
            frame_mesh_us: 0,
            frame_particle_us: 0,
            frame_nametag_us: 0,
            frame_entity_us: 0,
            entity_loop_us: 0,
            entity_visible_count: 0,
            entity_culled_count: 0,
            entity_batch_reused: false,
            entity_cache_hits: 0,
            entity_cache_misses: 0,
            entity_hash_us: 0,
            entity_lookup_us: 0,
            entity_append_us: 0,
            entity_upload_us: 0,
            entity_skin_sync_us: 0,
            entity_generate_us: 0,
            entity_prune_us: 0,
            entity_extras_us: 0,
            frame_local_us: 0,
            frame_gui_us: 0,
            frame_chunk_upload_us: 0,
            frame_chunk_upload_bytes: 0,
            frame_chunk_upload_count: 0,
            frame_network_debug: crate::client::network::NetworkDebugProfile::default(),
            frame_interval_history: VecDeque::new(),
            frame_worst_candidates: VecDeque::new(),
            frame_interval_counts: BTreeMap::new(),
            frame_interval_history_span_us: 0,
            frame_interval_clock_us: 0,
            frame_interval_sequence: 0,
            frame_interval_collecting: false,
            frame_interval_warmup_remaining_us: 0,
            frame_interval_stats_elapsed_us: 0,
            frame_interval_in_gameplay: false,
            frame_one_percent_low_fps: 0.0,
            frame_zero_point_one_percent_low_fps: 0.0,
            frame_p99_interval_us: 0,
            frame_p99_9_interval_us: 0,
            frame_max_interval_us: 0,
            frame_worst_breakdown: FrameSpikeBreakdown::default(),
            frame_worst_age_us: 0,
            particle_batch_reused: false,
            local_batch_reused: false,
            underwater: false,
            underwater_yaw: 0.0,
            underwater_pitch: 0.0,
            hotbar_slots: [(0, 0, 0); 9],
            hotbar_selected: 0,
            inventory_open: false,
            inventory_window_id: 0,
            inventory_window_type: String::new(),
            inventory_window_title: String::new(),
            inventory_window_slot_count: 0,
            inventory_window_slots: Vec::new(),
            inventory_window_properties: Vec::new(),
            inventory_slots: std::array::from_fn(|_| ItemStackView::default()),
            inventory_armor_slots: std::array::from_fn(|_| ItemStackView::default()),
            inventory_crafting_slots: std::array::from_fn(|_| ItemStackView::default()),
            inventory_cursor_slot: ItemStackView::default(),
            block_selection_boxes: Vec::new(),
            dig_progress: 0.0,
            dig_position: None,
            fps_count: 0,
            sign_entries: Vec::new(),
            local_player_billboard: None,
        }
    }
}

impl GameRenderState {
    pub fn snapshot_completed_frame_profile(&mut self) {
        self.update_frame_interval_statistics();
        self.completed_frame_profile = FrameDebugProfile {
            max_framerate: self.max_framerate,
            particle_count: self.particle_count,
            entity_count: self.entity_count,
            chunk_count_loaded: self.chunk_count_loaded,
            total_us: self.frame_total_us,
            interval_us: self.frame_interval_us,
            one_percent_low_fps: self.frame_one_percent_low_fps,
            zero_point_one_percent_low_fps: self.frame_zero_point_one_percent_low_fps,
            p99_interval_us: self.frame_p99_interval_us,
            p99_9_interval_us: self.frame_p99_9_interval_us,
            max_interval_us: self.frame_max_interval_us,
            interval_sample_count: self.frame_interval_history.len() as u32,
            worst: self.frame_worst_breakdown,
            worst_age_us: self.frame_worst_age_us,
            outside_us: self.frame_outside_us,
            tasks_us: self.frame_tasks_us,
            network_us: self.frame_network_us,
            world_us: self.frame_world_us,
            tick_us: self.frame_tick_us,
            sync_us: self.frame_sync_us,
            render_us: self.frame_render_us,
            other_us: self.frame_other_us,
            script_us: self.frame_script_us,
            script_callbacks: self.frame_script_callbacks,
            script_slow_callbacks: self.frame_script_slow_callbacks,
            fence_us: self.frame_gpu_us,
            cpu_us: self.frame_cpu_us,
            acquire_us: self.frame_acquire_us,
            command_us: self.frame_command_us,
            submit_us: self.frame_submit_us,
            present_us: self.frame_present_us,
            mesh_us: self.frame_mesh_us,
            particle_us: self.frame_particle_us,
            nametag_us: self.frame_nametag_us,
            entity_us: self.frame_entity_us,
            entity_loop_us: self.entity_loop_us,
            entity_visible_count: self.entity_visible_count,
            entity_culled_count: self.entity_culled_count,
            entity_batch_reused: self.entity_batch_reused,
            entity_cache_hits: self.entity_cache_hits,
            entity_cache_misses: self.entity_cache_misses,
            entity_hash_us: self.entity_hash_us,
            entity_lookup_us: self.entity_lookup_us,
            entity_upload_us: self.entity_upload_us,
            entity_skin_sync_us: self.entity_skin_sync_us,
            entity_generate_us: self.entity_generate_us,
            entity_prune_us: self.entity_prune_us,
            entity_extras_us: self.entity_extras_us,
            local_us: self.frame_local_us,
            gui_us: self.frame_gui_us,
            chunk_upload_us: self.frame_chunk_upload_us,
            chunk_upload_bytes: self.frame_chunk_upload_bytes,
            chunk_upload_count: self.frame_chunk_upload_count,
            particle_batch_reused: self.particle_batch_reused,
            local_batch_reused: self.local_batch_reused,
        };
    }

    fn update_frame_interval_statistics(&mut self) {
        const HISTORY_WINDOW_US: u64 = 10_000_000;
        const STATS_INTERVAL_US: u64 = 1_000_000;
        const MAX_HISTORY_SAMPLES: usize = 30_000;
        const ENABLE_WARMUP_US: u64 = 250_000;

        if !self.debug_overlay {
            self.frame_interval_history.clear();
            self.frame_worst_candidates.clear();
            self.frame_interval_counts.clear();
            self.frame_interval_history_span_us = 0;
            self.frame_interval_clock_us = 0;
            self.frame_interval_sequence = 0;
            self.frame_interval_collecting = false;
            self.frame_interval_warmup_remaining_us = 0;
            self.frame_interval_stats_elapsed_us = 0;
            self.frame_one_percent_low_fps = 0.0;
            self.frame_zero_point_one_percent_low_fps = 0.0;
            self.frame_p99_interval_us = 0;
            self.frame_p99_9_interval_us = 0;
            self.frame_max_interval_us = 0;
            self.frame_worst_breakdown = FrameSpikeBreakdown::default();
            self.frame_worst_age_us = 0;
            return;
        }
        // Only collect samples during active gameplay.  Menu transitions,
        // loading screens, and alt-tab blur spikes can produce 200+ ms frames
        // that would otherwise sit in the 10-second sliding window and depress
        // the 1% low even after gameplay resumes.
        if !self.frame_interval_in_gameplay {
            return;
        }
        if self.frame_interval_us == 0 {
            return;
        }
        if !self.frame_interval_collecting {
            self.frame_interval_collecting = true;
            self.frame_interval_warmup_remaining_us = ENABLE_WARMUP_US;
        }
        if self.frame_interval_warmup_remaining_us > 0 {
            self.frame_interval_warmup_remaining_us = self
                .frame_interval_warmup_remaining_us
                .saturating_sub(self.frame_interval_us);
            return;
        }

        self.frame_interval_sequence = self.frame_interval_sequence.wrapping_add(1);
        self.frame_interval_clock_us = self
            .frame_interval_clock_us
            .saturating_add(self.frame_interval_us);
        let sample = FrameSpikeBreakdown {
            interval_us: self.frame_interval_us,
            total_us: self.frame_total_us,
            outside_us: self.frame_outside_us,
            tasks_us: self.frame_tasks_us,
            network_us: self.frame_network_us,
            world_us: self.frame_world_us,
            tick_us: self.frame_tick_us,
            sync_us: self.frame_sync_us,
            render_us: self.frame_render_us,
            other_us: self.frame_other_us,
            script_us: self.frame_script_us,
            fence_us: self.frame_gpu_us,
            cpu_us: self.frame_cpu_us,
            acquire_us: self.frame_acquire_us,
            command_us: self.frame_command_us,
            submit_us: self.frame_submit_us,
            present_us: self.frame_present_us,
            mesh_us: self.frame_mesh_us,
            entity_us: self.frame_entity_us,
            entity_loop_us: self.entity_loop_us,
            entity_hash_us: self.entity_hash_us,
            entity_lookup_us: self.entity_lookup_us,
            entity_generate_us: self.entity_generate_us,
            entity_upload_us: self.entity_upload_us,
            entity_skin_sync_us: self.entity_skin_sync_us,
            entity_prune_us: self.entity_prune_us,
            entity_extras_us: self.entity_extras_us,
            entity_cache_hits: self.entity_cache_hits,
            entity_cache_misses: self.entity_cache_misses,
            entity_visible_count: self.entity_visible_count,
            entity_culled_count: self.entity_culled_count,
            particle_us: self.frame_particle_us,
            nametag_us: self.frame_nametag_us,
            local_us: self.frame_local_us,
            gui_us: self.frame_gui_us,
            chunk_upload_us: self.frame_chunk_upload_us,
            network_packet_kind: self.frame_network_debug.worst_packet_kind,
            network_packet_us: self.frame_network_debug.worst_packet_us,
            network_packet_units: self.frame_network_debug.worst_packet_units,
            network_hook_us: self.frame_network_debug.worst_hook_us,
            network_session_us: self.frame_network_debug.worst_session_us,
            network_inventory_us: self.frame_network_debug.worst_inventory_us,
            network_entity_us: self.frame_network_debug.worst_entity_us,
            network_world_us: self.frame_network_debug.worst_world_us,
            network_scheduler_us: self.frame_network_debug.scheduler_us,
            network_scanned_packets: self.frame_network_debug.scanned_packets,
            network_handled_packets: self.frame_network_debug.handled_packets,
            network_deferred_packets: self.frame_network_debug.deferred_packets,
            sequence: self.frame_interval_sequence,
            end_us: self.frame_interval_clock_us,
        };
        self.frame_interval_history.push_back(sample);
        while self
            .frame_worst_candidates
            .back()
            .is_some_and(|candidate| candidate.interval_us <= sample.interval_us)
        {
            self.frame_worst_candidates.pop_back();
        }
        self.frame_worst_candidates.push_back(sample);
        *self
            .frame_interval_counts
            .entry(self.frame_interval_us)
            .or_default() += 1;
        self.frame_interval_history_span_us = self
            .frame_interval_history_span_us
            .saturating_add(self.frame_interval_us);
        self.frame_interval_stats_elapsed_us = self
            .frame_interval_stats_elapsed_us
            .saturating_add(self.frame_interval_us);
        while self.frame_interval_history.len() > 1
            && (self.frame_interval_history_span_us > HISTORY_WINDOW_US
                || self.frame_interval_history.len() > MAX_HISTORY_SAMPLES)
        {
            if let Some(removed) = self.frame_interval_history.pop_front() {
                self.frame_interval_history_span_us = self
                    .frame_interval_history_span_us
                    .saturating_sub(removed.interval_us);
                let remove_entry =
                    if let Some(count) = self.frame_interval_counts.get_mut(&removed.interval_us) {
                        *count -= 1;
                        *count == 0
                    } else {
                        false
                    };
                if remove_entry {
                    self.frame_interval_counts.remove(&removed.interval_us);
                }
            }
        }

        let oldest_sequence = self
            .frame_interval_history
            .front()
            .map_or(self.frame_interval_sequence, |sample| sample.sequence);
        while self
            .frame_worst_candidates
            .front()
            .is_some_and(|candidate| candidate.sequence < oldest_sequence)
        {
            self.frame_worst_candidates.pop_front();
        }
        self.frame_worst_breakdown = self
            .frame_worst_candidates
            .front()
            .copied()
            .unwrap_or_default();
        self.frame_worst_age_us = self
            .frame_interval_clock_us
            .saturating_sub(self.frame_worst_breakdown.end_us);
        self.frame_max_interval_us = self.frame_worst_breakdown.interval_us;

        if self.frame_interval_stats_elapsed_us < STATS_INTERVAL_US {
            return;
        }
        self.frame_interval_stats_elapsed_us %= STATS_INTERVAL_US;
        let sample_count = self.frame_interval_history.len();
        let slow_count = sample_count.div_ceil(100).max(1);
        let mut remaining_slow = slow_count;
        let mut slow_sum_us = 0u128;
        for (&interval_us, &count) in self.frame_interval_counts.iter().rev() {
            let take = remaining_slow.min(count);
            slow_sum_us += u128::from(interval_us) * take as u128;
            remaining_slow -= take;
            if remaining_slow == 0 {
                break;
            }
        }
        let slow_average_us = slow_sum_us / slow_count as u128;
        self.frame_one_percent_low_fps = if slow_average_us == 0 {
            0.0
        } else {
            1_000_000.0 / slow_average_us as f32
        };
        let p99_rank = (sample_count * 99).div_ceil(100).max(1);
        let mut cumulative = 0usize;
        self.frame_p99_interval_us = 0;
        for (&interval_us, &count) in &self.frame_interval_counts {
            cumulative += count;
            if cumulative >= p99_rank {
                self.frame_p99_interval_us = interval_us;
                break;
            }
        }
        // 0.1 % low
        let slowest_permille = sample_count.div_ceil(1000).max(1);
        let mut remaining_slowest = slowest_permille;
        let mut slowest_sum_us = 0u128;
        for (&interval_us, &count) in self.frame_interval_counts.iter().rev() {
            let take = remaining_slowest.min(count);
            slowest_sum_us += u128::from(interval_us) * take as u128;
            remaining_slowest -= take;
            if remaining_slowest == 0 {
                break;
            }
        }
        let slowest_avg_us = slowest_sum_us / slowest_permille as u128;
        self.frame_zero_point_one_percent_low_fps = if slowest_avg_us == 0 {
            0.0
        } else {
            1_000_000.0 / slowest_avg_us as f32
        };
        let p99_9_rank = (sample_count * 999).div_ceil(1000).max(1);
        let mut cumulative_p999 = 0usize;
        self.frame_p99_9_interval_us = 0;
        for (&interval_us, &count) in &self.frame_interval_counts {
            cumulative_p999 += count;
            if cumulative_p999 >= p99_9_rank {
                self.frame_p99_9_interval_us = interval_us;
                break;
            }
        }
    }

    pub fn reset_current_frame_profile(&mut self) {
        self.frame_gpu_us = 0;
        self.frame_cpu_us = 0;
        self.frame_acquire_us = 0;
        self.frame_command_us = 0;
        self.frame_submit_us = 0;
        self.frame_present_us = 0;
        self.frame_mesh_us = 0;
        self.frame_particle_us = 0;
        self.frame_nametag_us = 0;
        self.frame_entity_us = 0;
        self.entity_loop_us = 0;
        self.entity_visible_count = 0;
        self.entity_culled_count = 0;
        self.entity_batch_reused = false;
        self.entity_cache_hits = 0;
        self.entity_cache_misses = 0;
        self.entity_hash_us = 0;
        self.entity_lookup_us = 0;
        self.entity_append_us = 0;
        self.entity_upload_us = 0;
        self.entity_skin_sync_us = 0;
        self.entity_generate_us = 0;
        self.entity_prune_us = 0;
        self.entity_extras_us = 0;
        self.frame_local_us = 0;
        self.frame_gui_us = 0;
        self.frame_chunk_upload_us = 0;
        self.frame_chunk_upload_bytes = 0;
        self.frame_chunk_upload_count = 0;
        self.particle_batch_reused = false;
        self.local_batch_reused = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish_debug_warmup(state: &mut GameRenderState) {
        state.frame_interval_in_gameplay = true;
        for _ in 0..250 {
            state.frame_interval_us = 1_000;
            state.snapshot_completed_frame_profile();
        }
    }

    #[test]
    fn completed_frame_profile_is_snapshotted_before_current_metrics_reset() {
        let mut state = GameRenderState::default();
        state.max_framerate = 120;
        state.entity_count = 97;
        state.frame_total_us = 6_000;
        state.frame_interval_us = 11_700;
        state.frame_outside_us = 5_700;
        state.frame_render_us = 3_500;
        state.frame_cpu_us = 3_488;
        state.frame_gpu_us = 12;
        state.frame_entity_us = 2_600;
        state.entity_cache_hits = 47;
        state.frame_chunk_upload_count = 2;

        state.snapshot_completed_frame_profile();
        state.reset_current_frame_profile();

        let completed = state.completed_frame_profile;
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

        assert_eq!(state.frame_cpu_us, 0);
        assert_eq!(state.frame_gpu_us, 0);
        assert_eq!(state.frame_entity_us, 0);
        assert_eq!(state.entity_cache_hits, 0);
        assert_eq!(state.frame_chunk_upload_count, 0);
        assert_eq!(state.completed_frame_profile.total_us, 6_000);
    }

    #[test]
    fn debug_history_reports_slowest_one_percent_as_fps() {
        let mut state = GameRenderState::default();
        state.debug_overlay = true;
        finish_debug_warmup(&mut state);
        for _ in 0..900 {
            state.frame_interval_us = 1_000;
            state.snapshot_completed_frame_profile();
        }
        for _ in 0..10 {
            state.frame_interval_us = 10_000;
            state.frame_network_us = 8_000;
            state.snapshot_completed_frame_profile();
        }

        let profile = state.completed_frame_profile;
        assert_eq!(profile.interval_sample_count, 910);
        assert_eq!(profile.p99_interval_us, 10_000);
        assert_eq!(profile.max_interval_us, 10_000);
        assert!((profile.one_percent_low_fps - 100.0).abs() < 0.01);
        assert_eq!(profile.worst.network_us, 8_000);
    }

    #[test]
    fn rolling_worst_frame_expires_with_the_ten_second_window() {
        let mut state = GameRenderState::default();
        state.debug_overlay = true;
        finish_debug_warmup(&mut state);
        state.frame_interval_us = 20_000;
        state.frame_tick_us = 15_000;
        state.snapshot_completed_frame_profile();
        assert_eq!(state.completed_frame_profile.worst.tick_us, 15_000);

        state.frame_tick_us = 0;
        for _ in 0..10_001 {
            state.frame_interval_us = 1_000;
            state.snapshot_completed_frame_profile();
        }
        assert_eq!(state.completed_frame_profile.max_interval_us, 1_000);
        assert_eq!(state.completed_frame_profile.worst.tick_us, 0);
    }
}
