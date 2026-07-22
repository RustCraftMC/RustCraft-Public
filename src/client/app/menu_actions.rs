use super::App;
use crate::audio::AudioBackend;
use crate::client::state::GameState;
use crate::client::tick::TickTimer;
use crate::ui::button_ids as btn;
use winit::event_loop::ActiveEventLoop;

impl App {
    pub(super) fn connect_to_configured_server(&mut self) {
        if self.net_ctrl.connection.is_some() || self.net_ctrl.connect_task.is_some() {
            return;
        }
        let addr = if matches!(self.state, GameState::Multiplayer) {
            self.servers
                .selected_address(self.selected_server)
                .unwrap_or(&self.server_address)
                .trim()
                .to_string()
        } else {
            self.server_address.trim().to_string()
        };
        if addr.is_empty() {
            return;
        }
        self.server_address = addr.clone();
        self.config.username = self.username.clone();
        self.config.save_default();
        self.session
            .push_system_line(self.ui.i18n.tf("rustcraft.connection.connecting", &[&addr]));
        self.net_ctrl.connect_task = Some(super::tasks::ConnectTask::spawn(
            addr,
            self.username.clone(),
            self.account.clone(),
        ));
    }

    pub(super) fn handle_button_click(&mut self, button_id: u32, event_loop: &ActiveEventLoop) {
        // MC 1.8: UI button click sound
        self.audio.play(crate::audio::SoundEvent {
            name: "random.click".to_string(),
            category: crate::audio::SoundCategory::Ui,
            volume: 1.0,
            pitch: 1.0,
            position: None,
        });
        if self.handle_slider_click(button_id) {
            return;
        }
        match button_id {
            btn::SINGLEPLAYER => {}
            btn::OPTIONS => self.open_options(),
            btn::QUIT => {
                // Drop renderer while window is still alive so Vulkan surface
                // cleanup doesn't access a destroyed window handle.
                self.renderer = None;
                self.net_ctrl.connection = None;
                event_loop.exit();
            }
            btn::MULTIPLAYER => {
                self.state = GameState::Multiplayer;
                self.start_server_refresh();
            }
            btn::ALT_MANAGER => self.state = GameState::AltManager,
            btn::MODDING => self.open_modding(),
            btn::MODDING_BACK => self.return_to_previous_screen(),
            btn::MODDING_CONFIGURE => self.open_selected_mod_config(),
            btn::MOD_CONFIG_BACK => self.return_to_previous_screen(),
            btn::MOD_CONFIG_RESET_ALL => self.reset_all_mod_config(),
            btn::MODDING_RELOAD_ALL => {
                let report = self.scripts.reload_all();
                let loaded = report.loaded.len();
                let errors = report.errors.len();
                // Prune stale entries for mods that disappeared from disk.
                {
                    let known_ids: std::collections::HashSet<_> = self
                        .scripts
                        .loaded_mods()
                        .into_iter()
                        .map(|m| m.id)
                        .collect();
                    self.config
                        .mod_enabled
                        .retain(|id, _| known_ids.contains(id.as_str()));
                    self.config.save_default();
                }
                self.modding_status = if errors == 0 {
                    format!("Reloaded {loaded} mod(s) successfully")
                } else {
                    format!(
                        "Reloaded {loaded} mod(s); {errors} error(s): {}",
                        report.errors.join(" | ")
                    )
                };
            }
            btn::MODDING_TOGGLE => {
                let selected = self
                    .scripts
                    .loaded_mods()
                    .into_iter()
                    .nth(self.modding_selected);
                if let Some(mod_info) = selected {
                    let (verb, result) = if mod_info.enabled {
                        ("Disabled", self.scripts.disable(&mod_info.id))
                    } else {
                        ("Enabled", self.scripts.enable(&mod_info.id))
                    };
                    self.modding_status = match result {
                        Ok(()) => {
                            self.config
                                .mod_enabled
                                .insert(mod_info.id.clone(), !mod_info.enabled);
                            self.config.save_default();
                            format!("{verb} {}", mod_info.name)
                        }
                        Err(error) => format!("Could not change {}: {error}", mod_info.name),
                    };
                } else {
                    self.modding_status = "Select a mod first".to_string();
                }
            }
            btn::MODDING_RELOAD => {
                let selected = self
                    .scripts
                    .loaded_mods()
                    .into_iter()
                    .nth(self.modding_selected);
                if let Some(mod_info) = selected {
                    self.modding_status = match self.scripts.reload(&mod_info.id) {
                        Ok(()) => format!("Reloaded {}", mod_info.name),
                        Err(error) => format!("Could not reload {}: {error}", mod_info.name),
                    };
                } else {
                    self.modding_status = "Select a mod first".to_string();
                }
            }
            id if id >= btn::MODDING_ROW_BASE
                && id < btn::MODDING_ROW_BASE + btn::MODDING_ROW_MAX as u32 =>
            {
                let index = (id - btn::MODDING_ROW_BASE) as usize;
                if index < self.scripts.mod_count() {
                    self.modding_selected = index;
                }
            }
            id if id >= btn::MOD_CONFIG_PREVIOUS_BASE
                && id < btn::MOD_CONFIG_PREVIOUS_BASE + btn::MOD_CONFIG_ENTRY_MAX as u32 =>
            {
                let index = (id - btn::MOD_CONFIG_PREVIOUS_BASE) as usize;
                self.adjust_mod_config(index, -1);
            }
            id if id >= btn::MOD_CONFIG_NEXT_BASE
                && id < btn::MOD_CONFIG_NEXT_BASE + btn::MOD_CONFIG_ENTRY_MAX as u32 =>
            {
                let index = (id - btn::MOD_CONFIG_NEXT_BASE) as usize;
                self.adjust_mod_config(index, 1);
            }
            id if id >= btn::MOD_CONFIG_RESET_BASE
                && id < btn::MOD_CONFIG_RESET_BASE + btn::MOD_CONFIG_ENTRY_MAX as u32 =>
            {
                let index = (id - btn::MOD_CONFIG_RESET_BASE) as usize;
                self.reset_mod_config(index);
            }
            id if id >= btn::MOD_CONFIG_ROW_BASE
                && id < btn::MOD_CONFIG_ROW_BASE + btn::MOD_CONFIG_ENTRY_MAX as u32 =>
            {
                let index = (id - btn::MOD_CONFIG_ROW_BASE) as usize;
                if let Some(mod_id) = self.active_mod_config_id() {
                    if index
                        < self
                            .scripts
                            .config_entries(&mod_id)
                            .map_or(0, |rows| rows.len())
                    {
                        self.mod_config_selected = index;
                    }
                }
            }
            btn::ALT_LOGIN => {
                if self.auth_task.is_none() {
                    self.auth_status = "Opening Microsoft sign-in in your browser...".to_string();
                    self.auth_task = Some(super::tasks::AuthTask::login());
                }
            }
            btn::ALT_OFFLINE => {
                self.offline_username_input.clear();
                self.entering_offline_name = true;
            }
            btn::ALT_LOGOUT => {
                if let Some(uuid) = self
                    .accounts
                    .get(self.selected_account)
                    .and_then(|account| account.uuid.as_deref())
                {
                    let _ = crate::auth::cache::remove_account(uuid);
                }
                self.accounts = crate::auth::cache::load_accounts().unwrap_or_default();
                self.selected_account = self
                    .selected_account
                    .min(self.accounts.len().saturating_sub(1));
                self.account = crate::auth::cache::load_account().ok().flatten();
                self.auth_status = "Account removed".to_string();
            }
            btn::ALT_USE => {
                if let Some(selected) = self.accounts.get(self.selected_account).cloned() {
                    if let Some(uuid) = selected.uuid.as_deref() {
                        let _ = crate::auth::cache::select_account(uuid);
                    }
                    self.username = selected
                        .username
                        .clone()
                        .unwrap_or_else(|| self.username.clone());
                    self.account = Some(selected);
                    self.update_local_skin();
                    self.config.username = self.username.clone();
                    self.config.save_default();
                    self.auth_status = "Selected account is now active".to_string();
                }
            }
            btn::ALT_BACK => self.state = GameState::MainMenu,
            btn::BACK_TO_GAME => self.capture_back_to_game(),
            btn::PAUSE_OPTIONS => self.open_options(),
            btn::DISCONNECT => self.disconnect_to_main_menu(),
            btn::RESPAWN => {
                crate::client::network::send_respawn_request(&self.net_ctrl.connection);
            }
            btn::DEATH_TITLE_SCREEN => self.disconnect_to_main_menu(),
            btn::GUI_SCALE_DOWN => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.settings.set_gui_scale(renderer.state.settings.gui_scale().saturating_sub(1).max(1));
                    self.config.gui_scale = renderer.state.settings.gui_scale();
                    self.config.save_default();
                }
            }
            btn::GUI_SCALE_UP => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.settings.set_gui_scale((renderer.state.settings.gui_scale() + 1).min(4));
                    self.config.gui_scale = renderer.state.settings.gui_scale();
                    self.config.save_default();
                }
            }
            btn::RENDER_DISTANCE_DOWN => {
                self.config.render_distance = self.config.render_distance.saturating_sub(2).max(2);
                self.save_and_sync_client_settings();
            }
            btn::RENDER_DISTANCE_UP => {
                self.config.render_distance = (self.config.render_distance + 2).min(16);
                self.save_and_sync_client_settings();
            }
            btn::SMOOTH_LIGHTING_TOGGLE => {
                self.config.smooth_lighting = !self.config.smooth_lighting;
                self.world.set_smooth_lighting(self.config.smooth_lighting);
                self.world.mesh_options.better_grass = self.config.better_grass;
                self.world.mesh_options.connected_textures = self.config.connected_textures;
                let meshes = self.world.build_all_meshes();
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_world(&meshes);
                }
                self.config.save_default();
            }
            btn::PARTICLES_TOGGLE => {
                self.config.particles = self.config.particles.next();
                self.config.save_default();
            }
            btn::MASTER_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Master, -0.1);
            }
            btn::MASTER_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Master, 0.1);
            }
            btn::MUSIC_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Music, -0.1);
            }
            btn::MUSIC_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Music, 0.1);
            }
            btn::BLOCKS_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Blocks, -0.1);
            }
            btn::BLOCKS_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Blocks, 0.1);
            }
            btn::HOSTILE_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Hostile, -0.1);
            }
            btn::HOSTILE_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Hostile, 0.1);
            }
            btn::FRIENDLY_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Friendly, -0.1);
            }
            btn::FRIENDLY_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Friendly, 0.1);
            }
            btn::WEATHER_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Weather, -0.1);
            }
            btn::WEATHER_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Weather, 0.1);
            }
            btn::AMBIENT_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Ambient, -0.1);
            }
            btn::AMBIENT_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Ambient, 0.1);
            }
            btn::PLAYERS_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Players, -0.1);
            }
            btn::PLAYERS_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Players, 0.1);
            }
            btn::UI_VOLUME_DOWN => {
                self.adjust_volume(crate::audio::SoundCategory::Ui, -0.1);
            }
            btn::UI_VOLUME_UP => {
                self.adjust_volume(crate::audio::SoundCategory::Ui, 0.1);
            }
            btn::AUDIO_DEVICE_CYCLE => {
                let devices = crate::audio::list_audio_devices();
                let current = &self.config.audio_device;
                let next_idx = devices
                    .iter()
                    .position(|d| d == current)
                    .map(|i| (i + 1) % devices.len())
                    .unwrap_or(0);
                self.config.audio_device = devices[next_idx].clone();
                self.config.save_default();
                self.audio.reinit(&self.config.audio_device);
            }
            btn::FOV_DOWN => {
                self.config.fov = (self.config.fov - 5.0).max(30.0);
                self.config.save_default();
            }
            btn::FOV_UP => {
                self.config.fov = (self.config.fov + 5.0).min(110.0);
                self.config.save_default();
            }
            btn::FRAMERATE_DOWN => {
                self.config.max_framerate = previous_framerate(self.config.max_framerate);
                self.config.save_default();
            }
            btn::FRAMERATE_UP => {
                self.config.max_framerate = next_framerate(self.config.max_framerate);
                self.config.save_default();
            }
            btn::CLOUDS_TOGGLE => {
                self.config.clouds = !self.config.clouds;
                self.config.save_default();
            }
            btn::WEATHER_EFFECTS_TOGGLE => {
                self.config.weather_effects = !self.config.weather_effects;
                self.config.save_default();
            }
            btn::ENTITY_SHADOWS_TOGGLE => {
                self.config.entity_shadows = !self.config.entity_shadows;
                self.config.save_default();
            }
            btn::VIEW_BOBBING_TOGGLE => {
                self.config.view_bobbing = !self.config.view_bobbing;
                self.config.save_default();
            }
            btn::ADVANCED_TOOLTIPS_TOGGLE => {
                self.config.advanced_tooltips = !self.config.advanced_tooltips;
                self.config.save_default();
            }
            btn::BETTER_GRASS_TOGGLE => {
                self.config.better_grass = !self.config.better_grass;
                self.world.mesh_options.better_grass = self.config.better_grass;
                let meshes = self.world.build_all_meshes();
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_world(&meshes);
                }
                self.config.save_default();
            }
            btn::CONNECTED_TEXTURES_TOGGLE => {
                self.config.connected_textures = !self.config.connected_textures;
                self.world.mesh_options.connected_textures = self.config.connected_textures;
                let meshes = self.world.build_all_meshes();
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_world(&meshes);
                }
                self.config.save_default();
            }
            btn::LANGUAGE_TOGGLE => {
                self.config.language = if self.config.language == "zh_CN" {
                    "en_US".to_string()
                } else {
                    "zh_CN".to_string()
                };
                self.ui = crate::ui::UiState::new(&self.config.language);
                self.save_and_sync_client_settings();
            }
            btn::RESET_CONTROLS => {
                for action in crate::client::keybind::Action::all_bindable() {
                    match self.input_ctrl.control_device {
                        crate::client::keybind::ControlDevice::KeyboardMouse => {
                            self.input_ctrl.keybinds.reset_action(*action)
                        }
                        crate::client::keybind::ControlDevice::Gamepad => {
                            self.input_ctrl.keybinds.reset_gamepad_action(*action)
                        }
                    }
                }
                self.config.keybinds = self.input_ctrl.keybinds.clone();
                self.config.save_default();
                self.input_ctrl.rebinding_action = None;
                self.session
                    .push_system_line(self.ui.t("controls.resetAll"));
            }
            btn::CONTROL_DEVICE_TOGGLE => {
                self.input_ctrl.rebinding_action = None;
                self.input_ctrl.control_device = match self.input_ctrl.control_device {
                    crate::client::keybind::ControlDevice::KeyboardMouse => {
                        crate::client::keybind::ControlDevice::Gamepad
                    }
                    crate::client::keybind::ControlDevice::Gamepad => {
                        crate::client::keybind::ControlDevice::KeyboardMouse
                    }
                };
            }
            btn::INVERT_MOUSE_TOGGLE => {
                self.config.invert_mouse = !self.config.invert_mouse;
                self.config.save_default();
            }
            btn::SKIN_CUSTOMIZATION => self.push_subscreen(GameState::SkinCustomization {
                previous: Box::new(GameState::MainMenu),
            }),
            btn::SKIN_CAPE_TOGGLE => self.toggle_skin_part(0x01),
            btn::SKIN_JACKET_TOGGLE => self.toggle_skin_part(0x02),
            btn::SKIN_LEFT_SLEEVE_TOGGLE => self.toggle_skin_part(0x04),
            btn::SKIN_RIGHT_SLEEVE_TOGGLE => self.toggle_skin_part(0x08),
            btn::SKIN_LEFT_PANTS_TOGGLE => self.toggle_skin_part(0x10),
            btn::SKIN_RIGHT_PANTS_TOGGLE => self.toggle_skin_part(0x20),
            btn::SKIN_HAT_TOGGLE => self.toggle_skin_part(0x40),
            btn::SKIN_ALL_TOGGLE => {
                self.config.skin_parts = if self.config.skin_parts == 0x7f {
                    0
                } else {
                    0x7f
                };
                self.save_and_sync_client_settings();
            }
            btn::RESOURCE_PACK_ACCEPT => self.accept_resource_pack(),
            btn::RESOURCE_PACK_DECLINE => self.decline_resource_pack(),
            btn::RESOURCE_PACKS => {
                self.scan_and_update_resource_packs();
                self.push_subscreen(GameState::ResourcePacks {
                    previous: Box::new(GameState::MainMenu),
                })
            }
            btn::RESOURCE_PACK_OPEN_FOLDER => {
                let path = std::path::Path::new("resourcepacks");
                let _ = std::fs::create_dir_all(path);
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer").arg(path).spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open").arg(path).spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
                }
            }
            btn::SHADER_PACKS => {
                self.scan_and_update_shader_packs();
                self.push_subscreen(GameState::ShaderPacks {
                    previous: Box::new(GameState::MainMenu),
                });
            }
            btn::SHADER_PACK_OPEN_FOLDER => {
                open_folder(std::path::Path::new(
                    crate::render::shader_pack::SHADER_PACK_DIR,
                ));
                self.scan_and_update_shader_packs();
            }
            btn::SHADER_PACK_OFF => {
                self.config.shader_pack = None;
                self.config.save_default();
                self.scan_and_update_shader_packs();
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.server_list.set_shader_pack_status("Shaders disabled; restart to apply".to_string());
                }
            }
            btn::DONE => self.return_to_previous_screen(),
            btn::VIDEO_SETTINGS => self.push_subscreen(GameState::VideoSettings {
                previous: Box::new(GameState::MainMenu),
            }),
            btn::CONTROLS => self.push_subscreen(GameState::Controls {
                previous: Box::new(GameState::MainMenu),
            }),
            btn::LANGUAGE => self.push_subscreen(GameState::Language {
                previous: Box::new(GameState::MainMenu),
            }),
            btn::AUDIO_SETTINGS => self.push_subscreen(GameState::AudioSettings {
                previous: Box::new(GameState::MainMenu),
            }),
            btn::CHAT_SETTINGS => self.push_subscreen(GameState::ChatSettings {
                previous: Box::new(GameState::MainMenu),
            }),
            btn::CHAT_WIDTH_DOWN => {
                self.config.chat_width = (self.config.chat_width - 0.05).clamp(0.1, 1.0);
                self.sync_chat_settings();
                self.config.save_default();
            }
            btn::CHAT_WIDTH_UP => {
                self.config.chat_width = (self.config.chat_width + 0.05).clamp(0.1, 1.0);
                self.sync_chat_settings();
                self.config.save_default();
            }
            btn::CHAT_HEIGHT_DOWN => {
                self.config.chat_height = self.config.chat_height.saturating_sub(1).max(1);
                self.sync_chat_settings();
                self.config.save_default();
            }
            btn::CHAT_HEIGHT_UP => {
                self.config.chat_height = self.config.chat_height.saturating_add(1).min(30);
                self.sync_chat_settings();
                self.config.save_default();
            }
            btn::CHAT_BACKGROUND_TOGGLE => {
                self.config.chat_background = !self.config.chat_background;
                self.config.save_default();
            }
            btn::CHAT_OVERLAY_TOGGLE => {
                self.config.chat_overlay = !self.config.chat_overlay;
                self.config.save_default();
            }
            btn::CHAT_AVATARS_TOGGLE => {
                self.config.chat_player_avatars = !self.config.chat_player_avatars;
                self.config.save_default();
            }
            btn::TAB_AVATARS_TOGGLE => {
                self.config.tab_player_avatars = !self.config.tab_player_avatars;
                self.config.save_default();
            }
            btn::DIRECT_CONNECT => {
                self.state = GameState::DirectConnect;
                if let Some(window) = &self.window {
                    window.set_ime_allowed(true);
                }
            }
            btn::ADD_SERVER => {
                self.server_editor_name.clear();
                self.server_editor_address.clear();
                self.server_editor_address_focused = false;
                self.state = GameState::ServerEditor { edit_index: None };
                if let Some(window) = &self.window {
                    window.set_ime_allowed(true);
                }
            }
            btn::EDIT_SERVER => {
                if let Some(server) = self.servers.servers.get(self.selected_server).cloned() {
                    self.server_editor_name = server.name;
                    self.server_editor_address = server.address;
                    self.server_editor_address_focused = false;
                    self.state = GameState::ServerEditor {
                        edit_index: Some(self.selected_server),
                    };
                    if let Some(window) = &self.window {
                        window.set_ime_allowed(true);
                    }
                }
            }
            btn::SERVER_EDITOR_SAVE => {
                self.save_server_editor();
            }
            btn::SERVER_EDITOR_CANCEL => {
                self.close_server_editor();
            }
            btn::SERVER_NAME_FIELD => {
                self.server_editor_address_focused = false;
            }
            btn::SERVER_ADDRESS_FIELD => {
                self.server_editor_address_focused = true;
            }
            btn::DIRECT_ADDRESS_FIELD => {}
            btn::CONNECT => {
                self.net_ctrl.connection = None;
                self.state = GameState::Connecting;
                self.connect_to_configured_server();
                self.input_ctrl.mouse_captured = false;
                self.set_cursor_captured(false);
            }
            btn::REFRESH_SERVER_LIST => {
                self.start_server_refresh();
            }
            btn::DELETE_SERVER => {
                if self.servers.remove(self.selected_server) {
                    self.selected_server = self
                        .selected_server
                        .min(self.servers.servers.len().saturating_sub(1));
                }
            }
            btn::SAVE_DIRECT_SERVER => {
                let addr = self.server_address.trim().to_string();
                if !addr.is_empty() {
                    self.selected_server = self.servers.upsert(addr.clone(), addr);
                }
            }
            btn::MULTIPLAYER_BACK => {
                self.state = GameState::Multiplayer;
                if let Some(window) = &self.window {
                    window.set_ime_allowed(false);
                }
            }
            btn::MULTIPLAYER_CANCEL => {
                self.state = GameState::MainMenu;
                if let Some(window) = &self.window {
                    window.set_ime_allowed(false);
                }
            }
            id if id >= btn::SERVER_ROW_BASE
                && id < btn::SERVER_ROW_BASE + btn::SERVER_ROW_MAX as u32 =>
            {
                let idx = (id - btn::SERVER_ROW_BASE) as usize;
                if let Some(server) = self.servers.servers.get(idx).cloned() {
                    self.selected_server = idx;
                    self.server_address = server.address;
                    let now = std::time::Instant::now();
                    let double_clicked =
                        self.last_server_click
                            .is_some_and(|(last_idx, last_click)| {
                                last_idx == idx
                                    && now.duration_since(last_click)
                                        <= std::time::Duration::from_millis(250)
                            });
                    self.last_server_click = Some((idx, now));
                    if double_clicked {
                        self.net_ctrl.connection = None;
                        self.state = GameState::Connecting;
                        self.connect_to_configured_server();
                        self.input_ctrl.mouse_captured = false;
                        self.set_cursor_captured(false);
                    }
                }
            }
            id if id >= btn::ALT_ACCOUNT_ROW_BASE
                && id < btn::ALT_ACCOUNT_ROW_BASE + btn::ALT_ACCOUNT_ROW_MAX as u32 =>
            {
                let index = (id - btn::ALT_ACCOUNT_ROW_BASE) as usize;
                if index < self.accounts.len() {
                    self.selected_account = index;
                }
            }
            id if id >= btn::RESOURCE_PACK_BASE
                && id < btn::RESOURCE_PACK_BASE + btn::RESOURCE_PACK_MAX as u32 =>
            {
                let available_idx = (id - btn::RESOURCE_PACK_BASE) as usize;
                let name = self
                    .resource_pack_list
                    .iter()
                    .filter(|p| !p.enabled)
                    .nth(available_idx)
                    .map(|p| p.name.clone());
                if let Some(ref name) = name {
                    if !self.config.enabled_resource_packs.contains(name) {
                        self.config.enabled_resource_packs.insert(0, name.clone());
                    }
                }
                self.config.save_default();
                self.scan_and_update_resource_packs();
            }
            id if id >= btn::RESOURCE_PACK_SELECTED_BASE
                && id
                    < btn::RESOURCE_PACK_SELECTED_BASE + btn::RESOURCE_PACK_SELECTED_MAX as u32 =>
            {
                let selected_idx = (id - btn::RESOURCE_PACK_SELECTED_BASE) as usize;
                let name = self
                    .resource_pack_list
                    .iter()
                    .filter(|p| p.enabled)
                    .nth(selected_idx)
                    .map(|p| p.name.clone());
                if let Some(ref name) = name {
                    self.config
                        .enabled_resource_packs
                        .retain(|enabled| enabled != name);
                }
                self.config.save_default();
                self.scan_and_update_resource_packs();
            }
            id if id >= btn::RESOURCE_PACK_SELECTED_UP_BASE
                && id
                    < btn::RESOURCE_PACK_SELECTED_UP_BASE
                        + btn::RESOURCE_PACK_SELECTED_MAX as u32 =>
            {
                let index = (id - btn::RESOURCE_PACK_SELECTED_UP_BASE) as usize;
                if index > 0 && index < self.config.enabled_resource_packs.len() {
                    self.config.enabled_resource_packs.swap(index, index - 1);
                    self.config.save_default();
                    self.scan_and_update_resource_packs();
                }
            }
            id if id >= btn::RESOURCE_PACK_SELECTED_DOWN_BASE
                && id
                    < btn::RESOURCE_PACK_SELECTED_DOWN_BASE
                        + btn::RESOURCE_PACK_SELECTED_MAX as u32 =>
            {
                let index = (id - btn::RESOURCE_PACK_SELECTED_DOWN_BASE) as usize;
                if index + 1 < self.config.enabled_resource_packs.len() {
                    self.config.enabled_resource_packs.swap(index, index + 1);
                    self.config.save_default();
                    self.scan_and_update_resource_packs();
                }
            }
            id if id >= btn::SHADER_PACK_BASE
                && id < btn::SHADER_PACK_BASE + btn::SHADER_PACK_MAX as u32 =>
            {
                let index = (id - btn::SHADER_PACK_BASE) as usize;
                let selected = self.renderer.as_ref().and_then(|renderer| {
                    renderer.state.server_list.shader_packs().get(index)
                        .filter(|pack| pack.compatible)
                        .map(|pack| pack.source_name.clone())
                });
                if let Some(selected) = selected {
                    self.config.shader_pack = Some(selected.clone());
                    self.config.save_default();
                    self.scan_and_update_shader_packs();
                    if let Some(renderer) = &mut self.renderer {
                        renderer.state.server_list.set_shader_pack_status(format!("Selected {selected}; restart to apply"));
                    }
                }
            }
            id if id >= btn::CONTROL_BIND_BASE
                && id < btn::CONTROL_BIND_BASE + btn::CONTROL_BIND_MAX as u32 =>
            {
                let idx = (id - btn::CONTROL_BIND_BASE) as usize;
                self.input_ctrl.rebinding_action = crate::client::keybind::Action::from_bindable_index(idx);
            }
            id if id >= btn::CONTROL_RESET_BASE
                && id < btn::CONTROL_RESET_BASE + btn::CONTROL_BIND_MAX as u32 =>
            {
                let idx = (id - btn::CONTROL_RESET_BASE) as usize;
                if let Some(action) = crate::client::keybind::Action::from_bindable_index(idx) {
                    match self.input_ctrl.control_device {
                        crate::client::keybind::ControlDevice::KeyboardMouse => {
                            self.input_ctrl.keybinds.reset_action(action)
                        }
                        crate::client::keybind::ControlDevice::Gamepad => {
                            self.input_ctrl.keybinds.reset_gamepad_action(action)
                        }
                    }
                    self.config.keybinds = self.input_ctrl.keybinds.clone();
                    self.config.save_default();
                    if self.input_ctrl.rebinding_action == Some(action) {
                        self.input_ctrl.rebinding_action = None;
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn save_server_editor(&mut self) -> bool {
        let name = self.server_editor_name.trim().to_string();
        let address = self.server_editor_address.trim().to_string();
        if name.is_empty() || address.is_empty() {
            return false;
        }

        let edit_index = match &self.state {
            GameState::ServerEditor { edit_index } => *edit_index,
            _ => return false,
        };
        self.selected_server = if let Some(index) = edit_index {
            if !self.servers.update(index, name, address.clone()) {
                return false;
            }
            index
        } else {
            self.servers.add(name, address.clone())
        };
        self.server_address = address;
        self.close_server_editor();
        true
    }

    pub(super) fn close_server_editor(&mut self) {
        self.state = GameState::Multiplayer;
        self.last_server_click = None;
        if let Some(window) = &self.window {
            window.set_ime_allowed(false);
        }
    }

    fn handle_slider_click(&mut self, button_id: u32) -> bool {
        let hit = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gui_hit_rect(button_id))
            .copied();
        let Some(hit) = hit else {
            return false;
        };
        let value = slider_value_from_mouse(self.input_ctrl.mouse_x as f32, hit.x, hit.w, hit.h);

        if self.set_slider_value(button_id, value) {
            // Vanilla GuiOptionSlider starts dragging immediately on mouse press.
            self.active_slider = Some(button_id);
            true
        } else {
            false
        }
    }

    pub(super) fn update_active_slider(&mut self) {
        let Some(button_id) = self.active_slider else {
            return;
        };
        let hit = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gui_hit_rect(button_id))
            .copied();
        let Some(hit) = hit else {
            self.active_slider = None;
            return;
        };
        let value = slider_value_from_mouse(self.input_ctrl.mouse_x as f32, hit.x, hit.w, hit.h);
        self.set_slider_value(button_id, value);
    }

    fn set_slider_value(&mut self, button_id: u32, value: f32) -> bool {
        match button_id {
            btn::GUI_SCALE_DOWN => {
                self.config.gui_scale = (1 + (value * 3.0).round() as u32).clamp(1, 4);
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.settings.set_gui_scale(self.config.gui_scale);
                }
                self.config.save_default();
                true
            }
            btn::RENDER_DISTANCE_DOWN => {
                self.config.render_distance = 2 + (value * 7.0).round() as u8 * 2;
                self.save_and_sync_client_settings();
                true
            }
            btn::FOV_DOWN => {
                self.config.fov = 30.0 + value * 80.0;
                self.config.save_default();
                true
            }
            btn::FRAMERATE_DOWN => {
                self.config.max_framerate = slider_to_framerate(value);
                self.config.save_default();
                true
            }
            btn::MASTER_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Master, value);
                true
            }
            btn::MUSIC_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Music, value);
                true
            }
            btn::BLOCKS_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Blocks, value);
                true
            }
            btn::HOSTILE_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Hostile, value);
                true
            }
            btn::FRIENDLY_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Friendly, value);
                true
            }
            btn::WEATHER_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Weather, value);
                true
            }
            btn::AMBIENT_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Ambient, value);
                true
            }
            btn::PLAYERS_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Players, value);
                true
            }
            btn::UI_VOLUME_DOWN => {
                self.set_volume_absolute(crate::audio::SoundCategory::Ui, value);
                true
            }
            btn::MOUSE_SENSITIVITY => {
                self.config.mouse_sensitivity = value;
                self.config.save_default();
                true
            }
            btn::GAMEPAD_LOOK_SENSITIVITY => {
                self.config.gamepad_look_sensitivity = value;
                self.config.save_default();
                true
            }
            btn::GAMEPAD_CURSOR_SPEED => {
                self.config.gamepad_cursor_speed = value;
                self.config.save_default();
                true
            }
            btn::CHAT_WIDTH_DOWN => {
                self.config.chat_width = (0.1 + value * 0.9).clamp(0.1, 1.0);
                self.sync_chat_settings();
                self.config.save_default();
                true
            }
            btn::CHAT_HEIGHT_DOWN => {
                self.config.chat_height = ((1.0 + value * 29.0).round() as u8).clamp(1, 30);
                self.sync_chat_settings();
                self.config.save_default();
                true
            }
            _ => false,
        }
    }

    pub(super) fn start_server_refresh(&mut self) {
        if self.net_ctrl.server_refresh_task.is_some() {
            return;
        }
        let servers = crate::client::server_list::ServerList::load_default();
        self.servers = servers.clone();
        self.selected_server = self
            .selected_server
            .min(self.servers.servers.len().saturating_sub(1));
        self.net_ctrl.server_refresh_task = Some(super::tasks::ServerRefreshTask::spawn(servers));
    }

    pub(super) fn return_to_previous_screen(&mut self) {
        let was_rp = matches!(self.state, GameState::ResourcePacks { .. });
        match std::mem::replace(&mut self.state, GameState::MainMenu) {
            GameState::Options { previous }
            | GameState::VideoSettings { previous }
            | GameState::SkinCustomization { previous }
            | GameState::Language { previous }
            | GameState::AudioSettings { previous }
            | GameState::ChatSettings { previous }
            | GameState::ResourcePacks { previous }
            | GameState::ShaderPacks { previous } => {
                self.state = *previous;
                if was_rp {
                    self.scan_and_update_resource_packs();
                    self.reload_assets();
                }
            }
            GameState::Modding { previous } | GameState::ModConfig { previous, .. } => {
                self.state = *previous
            }
            GameState::Controls { previous } => {
                self.input_ctrl.rebinding_action = None;
                self.state = *previous;
            }
            _ => {}
        }
    }

    fn enter_playing(&mut self, connect: bool) {
        self.state = GameState::Playing;
        if connect {
            self.connect_to_configured_server();
        }
        self.world.set_smooth_lighting(self.config.smooth_lighting);
        let meshes = self.world.build_all_meshes();
        if let Some(renderer) = &mut self.renderer {
            renderer.upload_world(&meshes);
        }
        self.set_cursor_captured(true);
        self.input_ctrl.mouse_captured = true;
        self.tick_timer = TickTimer::new();
    }

    fn capture_back_to_game(&mut self) {
        if matches!(self.state, GameState::Disconnected { .. }) {
            self.disconnect_to_main_menu();
        } else {
            self.state = GameState::Playing;
            self.input_ctrl.mouse_captured = true;
            self.set_cursor_captured(true);
        }
    }

    pub(super) fn disconnect_world(&mut self, reason: &str) {
        self.attack_held = false;
        self.use_held = false;
        self.use_presses_pending = 0;
        self.pending_attacks = 0;
        self.dig.cancel();
        self.pending_dig_cancel = None;
        self.block_hit_delay = 0;
        self.input_ctrl.mouse_captured = false;
        self.set_cursor_captured(false);
        self.inventory_open = false;
        self.inventory_action_number = 0;
        self.unload_server_state();
        self.net_ctrl.connection = None;
        self.scripts.notify_disconnect(reason);
        self.state = GameState::Disconnected {
            reason: reason.to_string(),
        };
    }

    fn disconnect_to_main_menu(&mut self) {
        self.state = GameState::MainMenu;
        self.attack_held = false;
        self.use_held = false;
        self.use_presses_pending = 0;
        self.pending_attacks = 0;
        self.dig.cancel();
        self.pending_dig_cancel = None;
        self.block_hit_delay = 0;
        self.inventory_open = false;
        self.inventory_action_number = 0;
        self.net_ctrl.connection = None;
        self.scripts.set_connection_active(false);
        self.input_ctrl.mouse_captured = false;
        self.set_cursor_captured(false);
        self.unload_server_state();
    }

    /// Drop every piece of state owned by the current server connection.
    ///
    /// This mirrors the client-side part of vanilla `Minecraft.loadWorld(null)`: the
    /// world and its renderer, entities, player/session data, transient effects, and
    /// sounds must all stop together.  Leaving the empty chunk meshes unapplied kept
    /// the old world allocated in the Vulkan renderer even after `World` was cleared.
    fn unload_server_state(&mut self) {
        self.scripts.clear_world_snapshot();
        self.last_script_world_block_key = None;
        let removed_meshes = self.world.clear_server_world();
        if let Some(renderer) = &mut self.renderer {
            renderer.upload_world_partial(&removed_meshes);
        }

        self.entities = crate::entity::EntityManager::new();
        self.net_ctrl.network_state = crate::client::network::ClientNetworkState::new();
        self.inventory = crate::client::inventory::Inventory::new();
        self.inventory_open = false;
        self.particles = crate::client::particles::ParticleSystem::new(4096);
        self.audio.stop_all();

        // Keep only client preferences; all server-provided session state (including
        // scoreboard, tab list, titles, chat, resource-pack offer, and world border)
        // belongs to the world that was just unloaded.
        self.session = crate::client::session::SessionState::new();
        self.session.locale = self.config.language.clone();
        self.session.view_distance = self.config.render_distance;
        self.session.skin_parts = self.config.skin_parts;

        // Vanilla discards its local player when loading a null world. Preserve only
        // the viewport aspect ratio needed by the menu renderer.
        let aspect = self.player.camera.aspect;
        self.player =
            crate::client::player::Player::new(nalgebra::Point3::new(0.0, 64.0, 0.0), aspect);

        self.last_entity_hash = 0;
        self.last_ui_text_hash = 0;
        self.last_sign_hash = 0;
        self.last_skull_hash = 0;
        self.last_player_list_generation = u64::MAX;
        self.last_scoreboard_generation = u64::MAX;
        self.last_player_skin_roster_hash = u64::MAX;
        self.last_player_skin_generation = u64::MAX;
        self.last_player_skin_skull_hash = u64::MAX;
        self.last_skin_cache_generation = u64::MAX;
        self.player_skin_slims.clear();
        self.player_skin_profiles.clear();
    }

    fn open_options(&mut self) {
        let previous = std::mem::replace(&mut self.state, GameState::MainMenu);
        self.state = GameState::Options {
            previous: Box::new(previous),
        };
    }

    fn open_modding(&mut self) {
        let previous = std::mem::replace(&mut self.state, GameState::MainMenu);
        self.state = GameState::Modding {
            previous: Box::new(previous),
        };
    }

    fn open_selected_mod_config(&mut self) {
        let selected = self
            .scripts
            .loaded_mods()
            .into_iter()
            .nth(self.modding_selected);
        let Some(mod_info) = selected else {
            self.modding_status = "Select a mod first".to_string();
            return;
        };
        if mod_info.config_entries == 0 {
            self.modding_status =
                format!("{} does not expose configurable settings", mod_info.name);
            return;
        }

        let previous = std::mem::replace(&mut self.state, GameState::MainMenu);
        self.state = GameState::ModConfig {
            previous: Box::new(previous),
            mod_id: mod_info.id,
        };
        self.mod_config_selected = 0;
        self.mod_config_status.clear();
        if let Some(renderer) = &mut self.renderer {
            renderer.state.server_list.set_mod_config_scroll(0);
        }
    }

    fn active_mod_config_id(&self) -> Option<String> {
        match &self.state {
            GameState::ModConfig { mod_id, .. } => Some(mod_id.clone()),
            _ => None,
        }
    }

    fn adjust_mod_config(&mut self, index: usize, direction: i32) {
        let Some(mod_id) = self.active_mod_config_id() else {
            return;
        };
        let Ok(entries) = self.scripts.config_entries(&mod_id) else {
            self.mod_config_status = "The selected mod is no longer available".to_string();
            return;
        };
        let Some(entry) = entries.get(index) else {
            return;
        };
        self.mod_config_selected = index;

        let Some(next) = adjusted_config_value(&entry.kind, &entry.value, direction) else {
            self.mod_config_status = format!("{} has an invalid configuration value", entry.label);
            return;
        };
        let value_label = config_value_label(&entry.kind, &next);
        self.mod_config_status = match self.scripts.set_config_value(&mod_id, &entry.key, next) {
            Ok(()) => format!("Set {} to {value_label}", entry.label),
            Err(error) => format!("Could not change {}: {error}", entry.label),
        };
    }

    fn reset_mod_config(&mut self, index: usize) {
        let Some(mod_id) = self.active_mod_config_id() else {
            return;
        };
        let Ok(entries) = self.scripts.config_entries(&mod_id) else {
            self.mod_config_status = "The selected mod is no longer available".to_string();
            return;
        };
        let Some(entry) = entries.get(index) else {
            return;
        };
        self.mod_config_selected = index;
        self.mod_config_status =
            match self
                .scripts
                .set_config_value(&mod_id, &entry.key, entry.default_value.clone())
            {
                Ok(()) => format!("Reset {}", entry.label),
                Err(error) => format!("Could not reset {}: {error}", entry.label),
            };
    }

    fn reset_all_mod_config(&mut self) {
        let Some(mod_id) = self.active_mod_config_id() else {
            return;
        };
        let Ok(entries) = self.scripts.config_entries(&mod_id) else {
            self.mod_config_status = "The selected mod is no longer available".to_string();
            return;
        };
        let mut reset = 0usize;
        for entry in entries {
            if entry.value == entry.default_value {
                continue;
            }
            if let Err(error) =
                self.scripts
                    .set_config_value(&mod_id, &entry.key, entry.default_value)
            {
                self.mod_config_status = format!("Could not reset settings: {error}");
                return;
            }
            reset += 1;
        }
        self.mod_config_status = format!("Reset {reset} setting(s) to defaults");
    }

    fn push_subscreen(&mut self, placeholder: GameState) {
        let previous = std::mem::replace(&mut self.state, GameState::MainMenu);
        self.state = match placeholder {
            GameState::VideoSettings { .. } => GameState::VideoSettings {
                previous: Box::new(previous),
            },
            GameState::Controls { .. } => GameState::Controls {
                previous: Box::new(previous),
            },
            GameState::SkinCustomization { .. } => GameState::SkinCustomization {
                previous: Box::new(previous),
            },
            GameState::Language { .. } => GameState::Language {
                previous: Box::new(previous),
            },
            GameState::AudioSettings { .. } => GameState::AudioSettings {
                previous: Box::new(previous),
            },
            GameState::ResourcePacks { .. } => GameState::ResourcePacks {
                previous: Box::new(previous),
            },
            GameState::ShaderPacks { .. } => GameState::ShaderPacks {
                previous: Box::new(previous),
            },
            GameState::ChatSettings { .. } => GameState::ChatSettings {
                previous: Box::new(previous),
            },
            _ => previous,
        };
    }

    pub(super) fn set_cursor_captured(&self, captured: bool) {
        if let Some(window) = &self.window {
            let mode = if captured {
                winit::window::CursorGrabMode::Locked
            } else {
                winit::window::CursorGrabMode::None
            };
            window.set_cursor_grab(mode).ok();
            window.set_cursor_visible(!captured);
        }
    }

    fn adjust_volume(&mut self, category: crate::audio::SoundCategory, delta: f32) {
        let target = match category {
            crate::audio::SoundCategory::Master => &mut self.config.master_volume,
            crate::audio::SoundCategory::Music => &mut self.config.music_volume,
            crate::audio::SoundCategory::Blocks => &mut self.config.blocks_volume,
            crate::audio::SoundCategory::Weather => &mut self.config.weather_volume,
            crate::audio::SoundCategory::Hostile => &mut self.config.hostile_volume,
            crate::audio::SoundCategory::Friendly => &mut self.config.friendly_volume,
            crate::audio::SoundCategory::Players => &mut self.config.players_volume,
            crate::audio::SoundCategory::Ambient => &mut self.config.ambient_volume,
            crate::audio::SoundCategory::Ui => &mut self.config.ui_volume,
        };
        *target = (*target + delta).clamp(0.0, 1.0);
        self.audio.set_volume(category, *target);
        self.config.save_default();
    }

    fn set_volume_absolute(&mut self, category: crate::audio::SoundCategory, value: f32) {
        let target = match category {
            crate::audio::SoundCategory::Master => &mut self.config.master_volume,
            crate::audio::SoundCategory::Music => &mut self.config.music_volume,
            crate::audio::SoundCategory::Blocks => &mut self.config.blocks_volume,
            crate::audio::SoundCategory::Weather => &mut self.config.weather_volume,
            crate::audio::SoundCategory::Hostile => &mut self.config.hostile_volume,
            crate::audio::SoundCategory::Friendly => &mut self.config.friendly_volume,
            crate::audio::SoundCategory::Players => &mut self.config.players_volume,
            crate::audio::SoundCategory::Ambient => &mut self.config.ambient_volume,
            crate::audio::SoundCategory::Ui => &mut self.config.ui_volume,
        };
        *target = value.clamp(0.0, 1.0);
        self.audio.set_volume(category, *target);
        self.config.save_default();
    }

    fn sync_chat_settings(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.state.hud.set_chat_width(self.config.chat_width);
            renderer.state.hud.set_chat_height(self.config.chat_height);
            renderer.state.hud.set_chat_background(self.config.chat_background);
            renderer.state.hud.set_chat_overlay(self.config.chat_overlay);
            renderer.state.hud.set_chat_player_avatars(self.config.chat_player_avatars);
            renderer.state.hud.set_tab_player_avatars(self.config.tab_player_avatars);
        }
    }

    fn save_and_sync_client_settings(&mut self) {
        self.config.save_default();
        self.session.locale = self.config.language.clone();
        self.session.view_distance = self.config.render_distance;
        self.session.skin_parts = self.config.skin_parts;
        crate::client::network::send_client_settings(
            &self.net_ctrl.connection,
            &self.session.locale,
            self.session.view_distance,
            self.session.skin_parts,
        );
    }

    fn toggle_skin_part(&mut self, mask: u8) {
        self.config.skin_parts ^= mask;
        self.config.skin_parts &= 0x7f;
        self.save_and_sync_client_settings();
    }

    fn accept_resource_pack(&mut self) {
        let Some(pack) = &mut self.session.resource_pack else {
            return;
        };
        let hash = pack.hash.clone();
        crate::client::network::send_resource_pack_status(&self.net_ctrl.connection, &hash, 3);
        pack.status = "accepted".to_string();
        self.session
            .push_system_line(self.ui.t("rustcraft.resourcePack.acceptedPending"));
        crate::client::network::send_resource_pack_status(&self.net_ctrl.connection, &hash, 0);
        if let Some(pack) = &mut self.session.resource_pack {
            pack.status = "loaded".to_string();
        }
        if matches!(self.state, GameState::Playing) && !self.chat_open && !self.inventory_open {
            self.input_ctrl.mouse_captured = true;
            self.set_cursor_captured(true);
        }
    }

    fn decline_resource_pack(&mut self) {
        let Some(pack) = &mut self.session.resource_pack else {
            return;
        };
        let hash = pack.hash.clone();
        crate::client::network::send_resource_pack_status(&self.net_ctrl.connection, &hash, 1);
        pack.status = "declined".to_string();
        self.session
            .push_system_line(self.ui.t("rustcraft.resourcePack.declined"));
        if matches!(self.state, GameState::Playing) && !self.chat_open && !self.inventory_open {
            self.input_ctrl.mouse_captured = true;
            self.set_cursor_captured(true);
        }
    }

    pub(super) fn handle_dropped_file(&mut self, path: std::path::PathBuf) {
        let shader_pack = matches!(self.state, GameState::ShaderPacks { .. });
        if !shader_pack && !matches!(self.state, GameState::ResourcePacks { .. }) {
            return;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("zip") {
            return;
        }
        let rp_dir = std::path::Path::new(if shader_pack {
            crate::render::shader_pack::SHADER_PACK_DIR
        } else {
            "resourcepacks"
        });
        let _ = std::fs::create_dir_all(rp_dir);
        let dest = rp_dir.join(path.file_name().unwrap_or_default());
        if let Err(e) = std::fs::copy(&path, &dest) {
            log::error!(
                "failed to copy dropped resource pack from '{}' to '{}': {}",
                path.display(),
                dest.display(),
                e
            );
            return;
        }
        if shader_pack {
            self.scan_and_update_shader_packs();
        } else if let Some(name) = dest.file_stem().and_then(|s| s.to_str()) {
            let name = name.to_string();
            if !self.resource_pack_list.iter().any(|p| p.name == name) {
                self.resource_pack_list
                    .push(crate::client::app::ResourcePackInfo {
                        name,
                        enabled: false,
                        is_default: false,
                        description: String::new(),
                        pack_format: 0,
                        icon: None,
                    });
            }
            self.scan_and_update_resource_packs();
        }
    }

    /// Reload all game assets (textures, models, sounds, language) from current resource packs.
    fn reload_assets(&mut self) {
        // Running mesh jobs contain atlas UVs captured before this reload.
        // Invalidate them before publishing the replacement texture layout.
        self.world.invalidate_mesh_jobs_for_resource_reload();
        let enabled = self.config.enabled_resource_packs.clone();
        let mut resolver = crate::assets::resolver::AssetResolver::with_resource_packs(
            "assets/minecraft",
            "resourcepacks",
            &enabled,
        );
        match crate::assets::resource_pack::ResourcePack::load_with_resource_packs(
            "assets",
            "1.8",
            &mut resolver,
        ) {
            Ok(pack) => self.audio.reload_assets(pack.index, pack.sounds),
            Err(error) => log::error!("sound asset reload failed: {error}"),
        }
        // Rebuild block texture atlas
        let tex_atlas = crate::assets::texture::TextureAtlas::load_with_resolver(&mut resolver);
        crate::assets::texture::init_texture_map(&tex_atlas);

        // Rebuild block model cache
        let mut model_registry = crate::assets::model::ModelRegistry::new();
        model_registry.load_with_resolver(&mut resolver);
        model_registry.texture_map = tex_atlas.name_to_index.clone();
        let model_cache = crate::world::block_models::BlockModelCache::build(
            &mut model_registry,
            tex_atlas.name_to_index.clone(),
        );
        crate::world::block_models::BlockModelCache::init(model_cache);

        // Rebuild entity texture atlas
        let entity_atlas =
            crate::render::entity::atlas::EntityTextureAtlas::load_with_resolver(&mut resolver);
        let item_atlas = crate::render::item_icons::build_item_icon_atlas(&mut resolver);
        crate::render::item_icons::precompute_item_meshes_with_resolver(&mut resolver);

        // Push reload to renderer
        if let Some(renderer) = &mut self.renderer {
            renderer.schedule_resource_reload(tex_atlas, entity_atlas, item_atlas, &mut resolver);
            let meshes = self.world.build_all_meshes();
            renderer.upload_world(&meshes);

            // Reload custom sky from mcpatcher
            if let Some(mut custom_sky) =
                crate::render::custom_sky::CustomSky::load(&mut resolver, 0)
            {
                let pixels = custom_sky.layers.first().map(|l| l.pixels.clone());
                let (w, h) = custom_sky
                    .layers
                    .first()
                    .map_or((1, 1), |l| (l.width, l.height));
                if let Some(ref pixels) = pixels {
                    renderer.reload_custom_sky(pixels, w, h, custom_sky);
                }
            }
        }

        // Reload language and UI text
        self.ui = crate::ui::UiState::new(&self.config.language);

        log::info!(
            "asset reload complete: enabled_resource_packs={}",
            enabled.len()
        );
    }

    pub(super) fn scan_and_update_resource_packs(&mut self) {
        let enabled = self.config.enabled_resource_packs.clone();
        self.resource_pack_list = super::scan_resource_packs(&enabled);
        if let Some(renderer) = &mut self.renderer {
            renderer.state.server_list.set_available_resource_packs(self
                .resource_pack_list
                .iter()
                .filter(|p| !p.enabled)
                .cloned()
                .collect());
            renderer.state.server_list.set_selected_resource_packs(enabled
                .iter()
                .filter_map(|name| {
                    self.resource_pack_list
                        .iter()
                        .find(|pack| pack.name == *name)
                        .cloned()
                })
                .collect());
            renderer.state.server_list.selected_resource_packs_mut().push(crate::client::app::ResourcePackInfo {
                    name: "Default".to_string(),
                    enabled: true,
                    is_default: true,
                    description: "The default look and feel of Minecraft".to_string(),
                    pack_format: super::MC_PACK_FORMAT,
                    icon: None,
                });
        }
    }

    pub(super) fn scan_and_update_shader_packs(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let capabilities = crate::render::shader_pack::RenderCapabilities {
            ray_tracing: renderer.state.server_list.ray_tracing_available(),
            fsr3: renderer.state.server_list.fsr3_available(),
        };
        renderer.state.server_list.set_shader_packs(crate::render::shader_pack::discover_shader_packs(capabilities));
        renderer.state.server_list.set_selected_shader_pack(self.config.shader_pack.clone());
    }
}

fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn config_value_label(
    kind: &crate::scripting::ConfigEntryKind,
    value: &crate::scripting::ConfigValue,
) -> String {
    use crate::scripting::{ConfigEntryKind, ConfigValue};

    match (kind, value) {
        (ConfigEntryKind::Boolean, ConfigValue::Boolean(value)) => {
            if *value { "On" } else { "Off" }.to_string()
        }
        (ConfigEntryKind::Number { .. }, ConfigValue::Number(value)) => {
            let formatted = format!("{value:.4}");
            formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
        (ConfigEntryKind::Choice { options }, ConfigValue::Choice(value)) => options
            .iter()
            .find(|option| option.value == *value)
            .map(|option| option.label.clone())
            .unwrap_or_else(|| value.clone()),
        _ => "Invalid value".to_string(),
    }
}

fn adjusted_config_value(
    kind: &crate::scripting::ConfigEntryKind,
    value: &crate::scripting::ConfigValue,
    direction: i32,
) -> Option<crate::scripting::ConfigValue> {
    use crate::scripting::{ConfigEntryKind, ConfigValue};

    match (kind, value) {
        (ConfigEntryKind::Boolean, ConfigValue::Boolean(value)) => {
            Some(ConfigValue::Boolean(!value))
        }
        (ConfigEntryKind::Number { min, max, step }, ConfigValue::Number(value)) => {
            let raw = if direction < 0 {
                value - step
            } else {
                value + step
            }
            .clamp(*min, *max);
            let snapped = (min + ((raw - min) / step).round() * step).clamp(*min, *max);
            Some(ConfigValue::Number(snapped))
        }
        (ConfigEntryKind::Choice { options }, ConfigValue::Choice(value))
            if !options.is_empty() =>
        {
            let current = options
                .iter()
                .position(|option| option.value == *value)
                .unwrap_or(0);
            let next = if direction < 0 {
                current.checked_sub(1).unwrap_or(options.len() - 1)
            } else {
                (current + 1) % options.len()
            };
            Some(ConfigValue::Choice(options[next].value.clone()))
        }
        _ => None,
    }
}

/// Matches GuiOptionSlider's track: the 8 px thumb travels between the two
/// 4 px insets of the 20 px-tall button, scaled with the GUI.
fn slider_value_from_mouse(mouse_x: f32, x: f32, width: f32, height: f32) -> f32 {
    let inset = 4.0 * (height / 20.0);
    ((mouse_x - (x + inset)) / (width - inset * 2.0)).clamp(0.0, 1.0)
}

fn next_framerate(current: u32) -> u32 {
    match current {
        0..=29 => 30,
        30..=59 => 60,
        60..=119 => 120,
        120..=239 => 240,
        240..=499 => 500,
        500..=999 => crate::client::config::UNLIMITED_FRAMERATE,
        _ => crate::client::config::UNLIMITED_FRAMERATE,
    }
}

fn previous_framerate(current: u32) -> u32 {
    match current {
        0..=30 => 30,
        31..=60 => 30,
        61..=120 => 60,
        121..=240 => 120,
        241..=500 => 240,
        _ => 500,
    }
}

fn slider_to_framerate(value: f32) -> u32 {
    if value >= 0.995 {
        crate::client::config::UNLIMITED_FRAMERATE
    } else {
        (30.0 + value.clamp(0.0, 1.0) * (crate::client::config::UNLIMITED_FRAMERATE - 30) as f32)
            .round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::adjusted_config_value;
    use crate::scripting::{ConfigChoice, ConfigEntryKind, ConfigValue};

    #[test]
    fn mod_config_controls_toggle_step_clamp_and_cycle() {
        assert_eq!(
            adjusted_config_value(&ConfigEntryKind::Boolean, &ConfigValue::Boolean(true), 1,),
            Some(ConfigValue::Boolean(false))
        );

        let number = ConfigEntryKind::Number {
            min: 0.0,
            max: 2.0,
            step: 0.1,
        };
        assert_eq!(
            adjusted_config_value(&number, &ConfigValue::Number(1.0), 1),
            Some(ConfigValue::Number(1.1))
        );
        assert_eq!(
            adjusted_config_value(&number, &ConfigValue::Number(2.0), 1),
            Some(ConfigValue::Number(2.0))
        );

        let choice = ConfigEntryKind::Choice {
            options: vec![
                ConfigChoice {
                    value: "classic".into(),
                    label: "Classic".into(),
                },
                ConfigChoice {
                    value: "subtle".into(),
                    label: "Subtle".into(),
                },
            ],
        };
        assert_eq!(
            adjusted_config_value(&choice, &ConfigValue::Choice("classic".into()), -1),
            Some(ConfigValue::Choice("subtle".into()))
        );
        assert_eq!(
            adjusted_config_value(&choice, &ConfigValue::Choice("subtle".into()), 1),
            Some(ConfigValue::Choice("classic".into()))
        );
    }
}
