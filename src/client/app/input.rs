use super::App;
use crate::audio::AudioBackend;
use crate::client;
use crate::client::keybind::{Action, ControlDevice};
use crate::client::state::GameState;
use winit::event::{KeyEvent, MouseButton, MouseScrollDelta};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

/// Simple pseudo-random f32 for pitch variation.
static mut RAND_STATE: u32 = 54321;
fn rand_f32() -> f32 {
    unsafe {
        RAND_STATE ^= RAND_STATE << 13;
        RAND_STATE ^= RAND_STATE >> 17;
        RAND_STATE ^= RAND_STATE << 5;
        (RAND_STATE as f32) / (u32::MAX as f32)
    }
}

impl App {
    /// Merge the current cross-platform controller state into the normal action
    /// input.  Gameplay consumers therefore do not need a gamepad-specific path.
    pub(super) fn poll_gamepad(&mut self, event_loop: &ActiveEventLoop) {
        let frame = self.gamepad.poll(&self.keybinds);
        self.input.set_gamepad_held(frame.held.clone());

        if matches!(self.state, GameState::Controls { .. }) && frame.used {
            self.control_device = ControlDevice::Gamepad;
        }
        if matches!(self.state, GameState::Controls { .. })
            && matches!(self.control_device, ControlDevice::Gamepad)
            && self.rebinding_action.is_some()
        {
            if let Some(binding) = frame.binding_pressed.first().copied() {
                let action = self.rebinding_action.expect("checked above");
                self.keybinds.set_gamepad(action, binding);
                self.config.keybinds = self.keybinds.clone();
                self.config.save_default();
                self.rebinding_action = None;
            }
            // While listening, the same button must not activate a menu item.
            return;
        }

        if self.mouse_captured {
            // `process_mouse` is also the camera's sensitivity/clamping path.
            // Feed stick deflection through it so mouse and controller cameras
            // share exactly the same view behaviour.
            const STICK_LOOK_SPEED: f32 = 10.0;
            self.player.process_mouse(
                frame.look_x * STICK_LOOK_SPEED * (self.config.gamepad_look_sensitivity / 0.5),
                frame.look_y * STICK_LOOK_SPEED * (self.config.gamepad_look_sensitivity / 0.5),
                0.5,
                false,
            );
        } else if frame.look_x != 0.0 || frame.look_y != 0.0 {
            self.move_virtual_cursor(frame.look_x, frame.look_y);
        }

        if !self.mouse_captured {
            if let Some(direction) = frame.navigation {
                self.move_menu_cursor(direction);
                self.extend_gamepad_inventory_drag();
            }
        }

        for action in &frame.released {
            if self.inventory_open {
                let button = match action {
                    Action::Attack => Some(MouseButton::Left),
                    Action::Use => Some(MouseButton::Right),
                    _ => None,
                };
                if let Some(button) = button {
                    self.finish_gamepad_inventory_drag(button);
                }
            }
        }

        let gameplay_input = matches!(self.state, GameState::Playing)
            && self.mouse_captured
            && !self.inventory_open
            && !self.chat_open
            && self.session.health > 0.0
            && !self
                .session
                .resource_pack
                .as_ref()
                .is_some_and(|pack| pack.status == "available");
        if gameplay_input {
            let attack_pressed = frame.pressed.contains(&Action::Attack);
            let use_pressed = frame.pressed.contains(&Action::Use);
            let gamepad_attack_down = frame.held.contains(&Action::Attack);
            let gamepad_use_down = frame.held.contains(&Action::Use);
            let attack_down = self.input.is_held(Action::Attack);
            let use_down = self.input.is_held(Action::Use);

            if !gamepad_attack_down {
                self.gamepad_attack_consumed = false;
            }

            // Mouse input used to be the only path that maintained these
            // interaction latches.  Drive the same state from merged input so
            // controller bindings can mine, attack, place blocks and use items.
            // Entity attacks are dispatched once per game tick (vanilla
            // clickMouse runs inside runTick at 20 Hz), so we only record an
            // edge here; the actual C0A/C02 packets are sent from the tick loop.
            if attack_pressed {
                self.pending_attacks = self.pending_attacks.saturating_add(1);
                self.attack_held = true;
                self.gamepad_attack_consumed = true;
            } else if gamepad_attack_down && !self.gamepad_attack_consumed {
                self.attack_held = true;
            } else if !attack_down {
                self.attack_held = false;
            }

            if use_pressed {
                self.use_held = true;
                self.use_presses_pending = self.use_presses_pending.saturating_add(1);
            } else if !gamepad_use_down && !use_down && self.use_held {
                self.use_held = false;
            }
        }

        for action in frame.pressed {
            if !self.mouse_captured {
                self.handle_gamepad_ui_press(event_loop, action);
                continue;
            }
            if self.dispatch_script_input_edge(
                action,
                crate::scripting::api::input::InputEdge::Pressed,
                false,
            ) {
                self.input.just_pressed.remove(&action);
                continue;
            }
            self.handle_key_press(event_loop, Some(action));
        }
    }

    fn move_virtual_cursor(&mut self, x: f32, y: f32) {
        const MAX_CURSOR_SPEED: f64 = 12.0;

        let Some(window) = self.window.as_ref() else {
            return;
        };
        let size = window.inner_size();
        let max_x = size.width.saturating_sub(1) as f64;
        let max_y = size.height.saturating_sub(1) as f64;
        if self.mouse_x == 0.0 && self.mouse_y == 0.0 {
            self.mouse_x = max_x * 0.5;
            self.mouse_y = max_y * 0.5;
        }
        let speed = MAX_CURSOR_SPEED * self.config.gamepad_cursor_speed.clamp(0.0, 1.0) as f64;
        self.mouse_x = (self.mouse_x + x as f64 * speed).clamp(0.0, max_x);
        self.mouse_y = (self.mouse_y + y as f64 * speed).clamp(0.0, max_y);

        if let Some(renderer) = &mut self.renderer {
            renderer.set_gui_mouse_pos(self.mouse_x as f32, self.mouse_y as f32);
        }
        // Reuse the platform cursor as the virtual cursor. This is visible on
        // every platform supported by winit and keeps hover/tooltip rendering
        // identical to mouse operation.
        let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
            self.mouse_x,
            self.mouse_y,
        ));
    }

    fn move_menu_cursor(&mut self, direction: [f32; 2]) {
        let viewport_center = self.window.as_ref().map(|window| {
            let size = window.inner_size();
            (size.width as f32 * 0.5, size.height as f32 * 0.5)
        });
        let Some(hit) = self.renderer.as_ref().and_then(|renderer| {
            // A menu may open while the pointer is still at an old gameplay
            // position. Start from the screen centre until a button is focused.
            let (x, y) = if renderer
                .gui_hit_test(self.mouse_x as f32, self.mouse_y as f32)
                .is_some()
            {
                (self.mouse_x as f32, self.mouse_y as f32)
            } else {
                viewport_center.unwrap_or((640.0, 360.0))
            };
            renderer.gui_directional_hit(x, y, direction)
        }) else {
            return;
        };
        self.mouse_x = (hit.x + hit.w * 0.5) as f64;
        self.mouse_y = (hit.y + hit.h * 0.5) as f64;
        if let Some(renderer) = &mut self.renderer {
            renderer.set_gui_mouse_pos(self.mouse_x as f32, self.mouse_y as f32);
        }
        if let Some(window) = &self.window {
            let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                self.mouse_x,
                self.mouse_y,
            ));
        }
    }

    fn handle_gamepad_ui_press(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        match action {
            // A/Cross and RT activate the currently snapped target.
            Action::Jump | Action::Attack => {
                if self.inventory_open {
                    self.begin_gamepad_inventory_drag(MouseButton::Left);
                } else if let Some(button_id) = self.renderer.as_ref().and_then(|renderer| {
                    renderer.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32)
                }) {
                    self.handle_button_click(button_id, event_loop);
                }
            }
            // LT is the controller equivalent of a secondary inventory click.
            Action::Use if self.inventory_open => {
                self.begin_gamepad_inventory_drag(MouseButton::Right)
            }
            // B/Circle and Start back out of inventory or the current screen.
            Action::Sneak | Action::Pause => {
                if self.inventory_open {
                    self.close_inventory_screen(true);
                } else if matches!(self.state, GameState::Playing) {
                    self.state = GameState::Paused;
                } else if !matches!(self.state, GameState::MainMenu) {
                    self.return_to_previous_screen();
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle_keyboard_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        key_event: KeyEvent,
    ) {
        let PhysicalKey::Code(code) = key_event.physical_key else {
            return;
        };

        if matches!(self.state, GameState::Controls { .. }) && key_event.state.is_pressed() {
            self.control_device = ControlDevice::KeyboardMouse;
        }

        // Track Ctrl key state
        self.ctrl_held = match code {
            KeyCode::ControlLeft | KeyCode::ControlRight => key_event.state.is_pressed(),
            _ => self.ctrl_held && key_event.state.is_pressed(),
        };

        if matches!(self.state, GameState::Playing)
            && code == KeyCode::F3
            && key_event.state.is_pressed()
            && !key_event.repeat
        {
            if let Some(renderer) = &mut self.renderer {
                renderer.state.debug_overlay = !renderer.state.debug_overlay;
            }
            return;
        }

        // Minecraft 1.8.9: F1 toggles GameSettings.hideGUI. Keep world input
        // active while hiding the normal HUD, but leave interactive overlays
        // (inventory, death screen, sign editor, resource-pack prompt) usable.
        if matches!(self.state, GameState::Playing)
            && code == KeyCode::F1
            && key_event.state.is_pressed()
            && !key_event.repeat
        {
            self.hud_hidden = !self.hud_hidden;
            return;
        }

        if matches!(self.state, GameState::Controls { .. }) {
            if let Some(action) = self.rebinding_action {
                if key_event.state.is_pressed() {
                    if matches!(code, KeyCode::Escape | KeyCode::Backspace | KeyCode::Delete) {
                        self.keybinds.clear(action);
                    } else {
                        self.keybinds.set_key(action, code);
                    }
                    self.config.keybinds = self.keybinds.clone();
                    self.config.save_default();
                    self.rebinding_action = None;
                }
                return;
            }
        }

        // Menus use the same registered hitboxes as mouse input.  Direction
        // keys move a visible focus cursor; Enter activates the focused item.
        // Handle this before per-screen text/list shortcuts so every menu gets
        // consistent navigation.
        if key_event.state.is_pressed() && self.state.is_menu() {
            let direction = match code {
                KeyCode::ArrowUp => Some([0.0, -1.0]),
                KeyCode::ArrowDown => Some([0.0, 1.0]),
                KeyCode::ArrowLeft => Some([-1.0, 0.0]),
                KeyCode::ArrowRight => Some([1.0, 0.0]),
                _ => None,
            };
            if let Some(direction) = direction {
                self.move_menu_cursor(direction);
                return;
            }
            if code == KeyCode::Enter && !key_event.repeat {
                if let Some(button_id) = self.renderer.as_ref().and_then(|renderer| {
                    renderer.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32)
                }) {
                    self.handle_button_click(button_id, event_loop);
                    return;
                }
            }
        }

        let action = self.keybinds.action_for_key(code);

        if key_event.state.is_pressed() {
            if matches!(self.state, GameState::AltManager) {
                if self.entering_offline_name {
                    match code {
                        KeyCode::Escape => {
                            self.entering_offline_name = false;
                            self.offline_username_input.clear();
                        }
                        KeyCode::Enter => {
                            let name = self.offline_username_input.trim().to_string();
                            if !name.is_empty() && name.len() <= 16 {
                                self.create_offline_account(&name);
                            }
                            self.entering_offline_name = false;
                            self.offline_username_input.clear();
                        }
                        KeyCode::Backspace | KeyCode::Delete => {
                            self.offline_username_input.pop();
                        }
                        _ => {
                            if let Some(text) = key_event.text.as_deref() {
                                for ch in text.chars() {
                                    if !ch.is_control()
                                        && ch != '\r'
                                        && ch != '\n'
                                        && self.offline_username_input.len() < 16
                                    {
                                        self.offline_username_input.push(ch);
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                match code {
                    KeyCode::Escape => self.state = GameState::MainMenu,
                    KeyCode::ArrowUp => {
                        self.selected_account = self.selected_account.saturating_sub(1)
                    }
                    KeyCode::ArrowDown => {
                        self.selected_account =
                            (self.selected_account + 1).min(self.accounts.len().saturating_sub(1))
                    }
                    KeyCode::Enter if !self.accounts.is_empty() => {
                        self.handle_button_click(crate::ui::button_ids::ALT_USE, event_loop)
                    }
                    _ => {}
                }
                return;
            }
            if matches!(self.state, GameState::Modding { .. }) {
                match code {
                    KeyCode::Escape => self.return_to_previous_screen(),
                    KeyCode::ArrowUp => {
                        self.modding_selected = self.modding_selected.saturating_sub(1);
                    }
                    KeyCode::ArrowDown => {
                        self.modding_selected = (self.modding_selected + 1)
                            .min(self.scripts.mod_count().saturating_sub(1));
                    }
                    KeyCode::Enter if self.scripts.mod_count() > 0 => {
                        self.handle_button_click(crate::ui::button_ids::MODDING_TOGGLE, event_loop)
                    }
                    KeyCode::KeyR if self.scripts.mod_count() > 0 => {
                        self.handle_button_click(crate::ui::button_ids::MODDING_RELOAD, event_loop)
                    }
                    KeyCode::KeyC if self.scripts.mod_count() > 0 => self
                        .handle_button_click(crate::ui::button_ids::MODDING_CONFIGURE, event_loop),
                    _ => {}
                }
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.modding_scroll = self.modding_selected.saturating_sub(4);
                }
                return;
            }
            if matches!(self.state, GameState::ModConfig { .. }) {
                let entry_count = match &self.state {
                    GameState::ModConfig { mod_id, .. } => self
                        .scripts
                        .config_entries(mod_id)
                        .map_or(0, |entries| entries.len()),
                    _ => 0,
                };
                match code {
                    KeyCode::Escape => self.return_to_previous_screen(),
                    KeyCode::ArrowUp => {
                        self.mod_config_selected = self.mod_config_selected.saturating_sub(1);
                    }
                    KeyCode::ArrowDown => {
                        self.mod_config_selected =
                            (self.mod_config_selected + 1).min(entry_count.saturating_sub(1));
                    }
                    KeyCode::ArrowLeft if entry_count > 0 => self.handle_button_click(
                        crate::ui::button_ids::MOD_CONFIG_PREVIOUS_BASE
                            + self.mod_config_selected as u32,
                        event_loop,
                    ),
                    KeyCode::ArrowRight | KeyCode::Enter if entry_count > 0 => self
                        .handle_button_click(
                            crate::ui::button_ids::MOD_CONFIG_NEXT_BASE
                                + self.mod_config_selected as u32,
                            event_loop,
                        ),
                    KeyCode::KeyR if entry_count > 0 => self.handle_button_click(
                        crate::ui::button_ids::MOD_CONFIG_RESET_BASE
                            + self.mod_config_selected as u32,
                        event_loop,
                    ),
                    _ => {}
                }
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.mod_config_scroll = self.mod_config_selected.saturating_sub(4);
                }
                return;
            }
            if matches!(self.state, GameState::ServerEditor { .. }) {
                self.handle_server_editor_key(code, key_event.text.as_deref());
                return;
            }

            if matches!(self.state, GameState::DirectConnect) {
                match code {
                    KeyCode::Escape => {
                        self.state = GameState::Multiplayer;
                        if let Some(window) = &self.window {
                            window.set_ime_allowed(false);
                        }
                    }
                    KeyCode::Enter if !self.server_address.trim().is_empty() => {
                        self.handle_button_click(crate::ui::button_ids::CONNECT, event_loop);
                    }
                    KeyCode::Backspace | KeyCode::Delete => {
                        self.server_address.pop();
                    }
                    _ => self.append_direct_address(key_event.text.as_deref().unwrap_or("")),
                }
                return;
            }

            if matches!(self.state, GameState::Multiplayer) {
                match code {
                    KeyCode::Escape => self.state = GameState::MainMenu,
                    KeyCode::ArrowUp => {
                        self.selected_server = self.selected_server.saturating_sub(1);
                        self.keep_selected_server_visible();
                    }
                    KeyCode::ArrowDown => {
                        self.selected_server = (self.selected_server + 1)
                            .min(self.servers.servers.len().saturating_sub(1));
                        self.keep_selected_server_visible();
                    }
                    KeyCode::Enter if !self.servers.servers.is_empty() => {
                        if let Some(server) = self.servers.servers.get(self.selected_server) {
                            self.server_address = server.address.clone();
                        }
                        self.handle_button_click(crate::ui::button_ids::CONNECT, event_loop);
                    }
                    KeyCode::F5 => self.start_server_refresh(),
                    _ => {}
                }
                return;
            }

            if self.book_editor.is_some() {
                if let Some(window) = &self.window {
                    window.set_ime_allowed(true);
                }
                let ctrl =
                    matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) || self.ctrl_held;
                self.handle_book_key(code, key_event.text.as_deref(), ctrl);
                return;
            }

            if self.session.sign_editor.is_some() {
                // Enable IME for sign text input
                if let Some(window) = &self.window {
                    window.set_ime_allowed(true);
                }
                self.handle_sign_key(code, key_event.text.as_deref());
                return;
            }

            if self.chat_open {
                let ctrl =
                    matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight) || self.ctrl_held;
                self.handle_chat_key(code, key_event.text.as_deref(), ctrl);
                return;
            }

            let creative_search_open = self.inventory_open
                && self.session.gamemode == 1
                && self.renderer.as_ref().is_some_and(|renderer| {
                    renderer.state.creative_tab
                        == crate::render::hud::inventory::CREATIVE_TAB_SEARCH
                });
            if creative_search_open && !matches!(action, Some(Action::Pause | Action::Inventory)) {
                if let Some(renderer) = &mut self.renderer {
                    match code {
                        KeyCode::Backspace | KeyCode::Delete => {
                            renderer.state.creative_search.pop();
                        }
                        KeyCode::KeyV if self.ctrl_held => {
                            if let Some(text) = get_clipboard_text() {
                                append_creative_search(&mut renderer.state.creative_search, &text);
                            }
                        }
                        _ => {
                            if let Some(text) = key_event.text.as_deref() {
                                append_creative_search(&mut renderer.state.creative_search, text);
                            }
                        }
                    }
                    renderer.state.creative_scroll = 0.0;
                }
                return;
            }

            if let Some(act) = action {
                self.input.on_key_down(act);
                if self.dispatch_script_input_edge(
                    act,
                    crate::scripting::api::input::InputEdge::Pressed,
                    key_event.repeat,
                ) {
                    self.input.just_pressed.remove(&act);
                    return;
                }
            }
            if matches!(self.state, GameState::Playing) && code == KeyCode::Tab {
                self.player_list_open = true;
            }
            self.handle_key_press(event_loop, action);
        } else {
            if let Some(act) = action {
                self.input.on_key_up(act);
                if self.dispatch_script_input_edge(
                    act,
                    crate::scripting::api::input::InputEdge::Released,
                    false,
                ) {
                    self.input.just_released.remove(&act);
                }
            }
            if matches!(self.state, GameState::Playing) && code == KeyCode::Tab {
                self.player_list_open = false;
            }
        }
    }

    fn handle_key_press(&mut self, event_loop: &ActiveEventLoop, action: Option<Action>) {
        match self.state {
            GameState::Playing if action == Some(Action::Pause) => {
                if self.inventory_open {
                    self.close_inventory_screen(true);
                } else {
                    self.state = GameState::Paused;
                    self.mouse_captured = false;
                    self.set_cursor_captured(false);
                }
            }
            GameState::Playing if action == Some(Action::Inventory) => {
                self.toggle_inventory();
            }
            GameState::Playing if action == Some(Action::Chat) => {
                self.open_chat("");
            }
            GameState::Playing if action == Some(Action::Command) => {
                self.open_chat("/");
            }
            GameState::Playing if self.inventory_open && action == Some(Action::DropItem) => {
                let drop_stack = self.input.is_held(Action::Sprint);
                self.drop_hovered_inventory_slot(drop_stack);
            }
            GameState::Playing if !self.inventory_open => match action {
                Some(Action::Hotbar1) => self.select_hotbar(0),
                Some(Action::Hotbar2) => self.select_hotbar(1),
                Some(Action::Hotbar3) => self.select_hotbar(2),
                Some(Action::Hotbar4) => self.select_hotbar(3),
                Some(Action::Hotbar5) => self.select_hotbar(4),
                Some(Action::Hotbar6) => self.select_hotbar(5),
                Some(Action::Hotbar7) => self.select_hotbar(6),
                Some(Action::Hotbar8) => self.select_hotbar(7),
                Some(Action::Hotbar9) => self.select_hotbar(8),
                Some(Action::HotbarPrev) => self.select_hotbar((self.inventory.selected + 8) % 9),
                Some(Action::HotbarNext) => self.select_hotbar((self.inventory.selected + 1) % 9),
                Some(Action::DropItem) => {
                    let drop_stack = self.input.is_held(Action::Sprint);
                    self.spawn_predicted_dropped_item(drop_stack);
                    self.audio.play(crate::audio::SoundEvent {
                        name: "random.pop".to_string(),
                        category: crate::audio::SoundCategory::Players,
                        volume: 0.5,
                        pitch: (rand_f32() - rand_f32()) * 0.2 + 1.0,
                        position: None,
                    });
                    client::network::send_drop_selected_item(&self.connection, drop_stack);
                }
                _ => {}
            },
            GameState::Paused if action == Some(Action::Pause) => {
                self.state = GameState::Playing;
                self.mouse_captured = true;
                self.set_cursor_captured(true);
            }
            GameState::Options { .. }
            | GameState::VideoSettings { .. }
            | GameState::Controls { .. }
            | GameState::SkinCustomization { .. }
            | GameState::Language { .. }
            | GameState::AudioSettings { .. }
            | GameState::ChatSettings { .. }
            | GameState::ShaderPacks { .. }
            | GameState::Modding { .. }
                if action == Some(Action::Pause) =>
            {
                self.return_to_previous_screen();
            }
            GameState::Multiplayer | GameState::DirectConnect if action == Some(Action::Pause) => {
                self.state = GameState::MainMenu;
            }
            GameState::MainMenu if action == Some(Action::Pause) => {
                self.renderer = None;
                self.connection = None;
                event_loop.exit();
            }
            _ => {}
        }
    }

    pub(super) fn handle_ime_commit(&mut self, text: &str) {
        if self.book_editor.is_some() {
            self.append_book_text(text);
            return;
        }
        if self.session.sign_editor.is_some() {
            self.append_sign_text(text);
            return;
        }
        if self.chat_open {
            self.append_chat_text(text);
            return;
        }
        if self.inventory_open && self.session.gamemode == 1 {
            if let Some(renderer) = &mut self.renderer {
                if renderer.state.creative_tab == crate::render::hud::inventory::CREATIVE_TAB_SEARCH
                {
                    append_creative_search(&mut renderer.state.creative_search, text);
                    renderer.state.creative_scroll = 0.0;
                    return;
                }
            }
        }
        if matches!(self.state, GameState::DirectConnect) {
            self.append_direct_address(text);
            return;
        }
        if matches!(self.state, GameState::ServerEditor { .. }) {
            self.append_server_editor_text(text);
        }
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
        };

        let rows = scroll_lines.abs().ceil() as usize;

        // Chat scrolling (when chat is open)
        if self.chat_open {
            if let Some(renderer) = &mut self.renderer {
                let total = renderer.state.chat_lines.len();
                let max_visible = 15;
                let max_scroll = total.saturating_sub(max_visible);
                if scroll_lines > 0.0 {
                    renderer.state.chat_scroll =
                        (renderer.state.chat_scroll + rows).min(max_scroll);
                } else {
                    renderer.state.chat_scroll = renderer.state.chat_scroll.saturating_sub(rows);
                }
            }
            return;
        }

        if rows > 0 && matches!(self.state, GameState::Multiplayer) {
            if let Some(renderer) = &mut self.renderer {
                renderer.state.server_list_scroll = if scroll_lines > 0.0 {
                    renderer.state.server_list_scroll.saturating_sub(rows)
                } else {
                    renderer.state.server_list_scroll.saturating_add(rows)
                };
            }
            return;
        }
        if rows > 0 && matches!(self.state, GameState::Controls { .. }) {
            if let Some(renderer) = &mut self.renderer {
                renderer.state.controls_list_scroll = if scroll_lines > 0.0 {
                    renderer.state.controls_list_scroll.saturating_sub(rows)
                } else {
                    renderer.state.controls_list_scroll.saturating_add(rows)
                };
            }
            return;
        }
        if rows > 0 && matches!(self.state, GameState::ResourcePacks { .. }) {
            let window_width = self
                .window
                .as_ref()
                .map(|window| window.inner_size().width as f64)
                .unwrap_or(1280.0);
            if let Some(renderer) = &mut self.renderer {
                let scroll = if self.mouse_x < window_width * 0.5 {
                    &mut renderer.state.available_resource_pack_scroll
                } else {
                    &mut renderer.state.selected_resource_pack_scroll
                };
                *scroll = if scroll_lines > 0.0 {
                    scroll.saturating_sub(rows)
                } else {
                    scroll.saturating_add(rows)
                };
            }
            return;
        }
        if rows > 0 && matches!(self.state, GameState::ShaderPacks { .. }) {
            if let Some(renderer) = &mut self.renderer {
                renderer.state.shader_pack_scroll = if scroll_lines > 0.0 {
                    renderer.state.shader_pack_scroll.saturating_sub(rows)
                } else {
                    renderer.state.shader_pack_scroll.saturating_add(rows)
                };
            }
            return;
        }
        if rows > 0 && matches!(self.state, GameState::Modding { .. }) {
            if let Some(renderer) = &mut self.renderer {
                renderer.state.modding_scroll = if scroll_lines > 0.0 {
                    renderer.state.modding_scroll.saturating_sub(rows)
                } else {
                    renderer.state.modding_scroll.saturating_add(rows)
                };
            }
            return;
        }
        if rows > 0 && matches!(self.state, GameState::ModConfig { .. }) {
            if let Some(renderer) = &mut self.renderer {
                renderer.state.mod_config_scroll = if scroll_lines > 0.0 {
                    renderer.state.mod_config_scroll.saturating_sub(rows)
                } else {
                    renderer.state.mod_config_scroll.saturating_add(rows)
                };
            }
            return;
        }

        // Creative inventory scrolling
        if matches!(self.state, GameState::Playing)
            && self.inventory_open
            && self.session.gamemode == 1
        {
            if let Some(renderer) = &mut self.renderer {
                let items = renderer.creative_visible_entries();
                let max_scroll_rows =
                    crate::render::hud::inventory::creative_max_scroll_rows(items.len()) as f32;
                if max_scroll_rows > 0.0 {
                    renderer.state.creative_scroll = (renderer.state.creative_scroll
                        - scroll_lines / max_scroll_rows)
                        .clamp(0.0, 1.0);
                } else {
                    renderer.state.creative_scroll = 0.0;
                }
            }
            return;
        }

        // Hotbar scroll
        if matches!(self.state, GameState::Playing) && !self.inventory_open {
            let scroll = scroll_lines as i32;
            self.inventory.scroll(-scroll);
        }
    }

    pub(super) fn handle_mouse_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        button: MouseButton,
        pressed: bool,
    ) {
        if matches!(self.state, GameState::Controls { .. }) && pressed {
            self.control_device = ControlDevice::KeyboardMouse;
        }
        if button == MouseButton::Left && !pressed {
            // GuiOptionSlider.mouseReleased always stops a drag, including when
            // the pointer was released outside the control.
            self.active_slider = None;
            self.creative_scroll_dragging = false;
        }

        if button == MouseButton::Left && pressed && self.try_start_creative_scroll_drag() {
            return;
        }

        if matches!(self.state, GameState::Controls { .. }) {
            if let Some(action) = self.rebinding_action {
                if pressed {
                    self.keybinds.set_mouse(action, button);
                    self.config.keybinds = self.keybinds.clone();
                    self.config.save_default();
                    self.rebinding_action = None;
                }
                return;
            }
        }

        let mouse_action = self.keybinds.action_for_mouse(button);
        if let Some(action) = mouse_action {
            let gameplay_edge = matches!(self.state, GameState::Playing)
                && self.mouse_captured
                && !self.inventory_open
                && self.session.health > 0.0
                && !self
                    .session
                    .resource_pack
                    .as_ref()
                    .is_some_and(|pack| pack.status == "available");
            let release_of_observed_action = !pressed && self.input.is_held(action);
            if gameplay_edge || release_of_observed_action {
                if pressed {
                    self.input.on_key_down(action);
                } else {
                    self.input.on_key_up(action);
                }
                let edge = if pressed {
                    crate::scripting::api::input::InputEdge::Pressed
                } else {
                    crate::scripting::api::input::InputEdge::Released
                };
                let consumed = self.dispatch_script_input_edge(action, edge, false);
                if consumed {
                    if pressed {
                        self.input.just_pressed.remove(&action);
                        return;
                    }
                    self.input.just_released.remove(&action);
                }
            }
        }
        match (&self.state, button, pressed) {
            (GameState::Playing, MouseButton::Left, true) if self.session.health <= 0.0 => {
                let hit = self
                    .renderer
                    .as_ref()
                    .and_then(|r| r.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32));
                if let Some(btn_id) = hit {
                    self.handle_button_click(btn_id, event_loop);
                }
            }
            (GameState::Playing, MouseButton::Left, true)
                if self
                    .session
                    .resource_pack
                    .as_ref()
                    .is_some_and(|pack| pack.status == "available") =>
            {
                let hit = self
                    .renderer
                    .as_ref()
                    .and_then(|r| r.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32));
                if let Some(btn_id) = hit {
                    self.handle_button_click(btn_id, event_loop);
                }
            }
            (GameState::Playing, MouseButton::Left, true) if self.book_editor.is_some() => {
                self.handle_book_click();
            }
            (GameState::Playing, MouseButton::Left, true) if self.chat_open => {
                self.handle_chat_click();
            }
            (GameState::Playing, MouseButton::Left | MouseButton::Right, true)
                if self.inventory_open =>
            {
                self.handle_inventory_click(button);
            }
            (state, MouseButton::Left, true) if state.is_menu() => {
                let hit = self
                    .renderer
                    .as_ref()
                    .and_then(|r| r.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32));
                if let Some(btn_id) = hit {
                    self.handle_button_click(btn_id, event_loop);
                }
            }
            (GameState::Playing, _, true)
                if mouse_action == Some(Action::Attack) && !self.mouse_captured =>
            {
                self.mouse_captured = true;
                self.set_cursor_captured(true);
            }
            (GameState::Playing, _, _)
                if mouse_action == Some(Action::Attack) && self.mouse_captured =>
            {
                if pressed {
                    self.pending_attacks = self.pending_attacks.saturating_add(1);
                    if let Some(renderer) = &mut self.renderer {
                        renderer.trigger_hand_swing();
                    }
                    self.attack_held = true;
                } else {
                    self.attack_held = false;
                }
            }
            (GameState::Playing, _, true)
                if mouse_action == Some(Action::Use) && self.mouse_captured =>
            {
                self.use_held = true;
                self.use_presses_pending = self.use_presses_pending.saturating_add(1);
            }
            (GameState::Playing, _, false) if mouse_action == Some(Action::Use) => {
                self.use_held = false;
            }
            _ => {}
        }
    }

    fn try_start_creative_scroll_drag(&mut self) -> bool {
        if !matches!(self.state, GameState::Playing)
            || !self.inventory_open
            || self.session.gamemode != 1
        {
            return false;
        }

        let Some(renderer) = self.renderer.as_mut() else {
            return false;
        };
        if renderer.state.creative_tab == crate::render::hud::inventory::CREATIVE_TAB_INVENTORY {
            return false;
        }

        let item_count = renderer.creative_visible_entries().len();
        if crate::render::hud::inventory::creative_max_scroll_rows(item_count) == 0 {
            return false;
        }

        let size = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .unwrap_or(winit::dpi::PhysicalSize::new(1280, 720));
        let scrollbar = crate::render::hud::inventory::creative_scrollbar_geometry(
            size.width as f32,
            size.height as f32,
            renderer.state.gui_scale.max(1) as f32,
        );
        if !scrollbar.contains(self.mouse_x as f32, self.mouse_y as f32) {
            return false;
        }

        self.creative_scroll_dragging = true;
        renderer.state.creative_scroll = scrollbar.scroll_for_mouse_y(self.mouse_y as f32);
        true
    }

    pub(super) fn update_creative_scroll_drag(&mut self) {
        if !self.creative_scroll_dragging {
            return;
        }
        if !matches!(self.state, GameState::Playing)
            || !self.inventory_open
            || self.session.gamemode != 1
        {
            self.creative_scroll_dragging = false;
            return;
        }

        let Some(renderer) = self.renderer.as_mut() else {
            self.creative_scroll_dragging = false;
            return;
        };
        let size = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .unwrap_or(winit::dpi::PhysicalSize::new(1280, 720));
        let scrollbar = crate::render::hud::inventory::creative_scrollbar_geometry(
            size.width as f32,
            size.height as f32,
            renderer.state.gui_scale.max(1) as f32,
        );
        renderer.state.creative_scroll = scrollbar.scroll_for_mouse_y(self.mouse_y as f32);
    }

    fn handle_server_editor_key(&mut self, code: KeyCode, text: Option<&str>) {
        match code {
            KeyCode::Escape => self.close_server_editor(),
            KeyCode::Tab => {
                self.server_editor_address_focused = !self.server_editor_address_focused;
            }
            KeyCode::Enter => {
                self.save_server_editor();
            }
            KeyCode::Backspace | KeyCode::Delete => {
                if self.server_editor_address_focused {
                    self.server_editor_address.pop();
                } else {
                    self.server_editor_name.pop();
                }
            }
            _ => self.append_server_editor_text(text.unwrap_or("")),
        }
    }

    fn append_server_editor_text(&mut self, text: &str) {
        let (field, max_len) = if self.server_editor_address_focused {
            (&mut self.server_editor_address, 255)
        } else {
            (&mut self.server_editor_name, 128)
        };
        for ch in text.chars() {
            if !ch.is_control() && field.chars().count() < max_len {
                field.push(ch);
            }
        }
    }

    fn append_direct_address(&mut self, text: &str) {
        for ch in text.chars() {
            if !ch.is_control() && self.server_address.chars().count() < 255 {
                self.server_address.push(ch);
            }
        }
    }

    fn keep_selected_server_visible(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let window_height = self
            .window
            .as_ref()
            .map(|window| window.inner_size().height as f32)
            .unwrap_or(720.0);
        let scale = self.config.gui_scale.max(1) as f32;
        let visible_rows = ((window_height - 96.0 * scale).max(36.0 * scale) / (36.0 * scale))
            .floor()
            .max(1.0) as usize;
        if self.selected_server < renderer.state.server_list_scroll {
            renderer.state.server_list_scroll = self.selected_server;
        } else if self.selected_server >= renderer.state.server_list_scroll + visible_rows {
            renderer.state.server_list_scroll = self.selected_server + 1 - visible_rows;
        }
    }

    pub(super) fn open_chat(&mut self, prefix: &str) {
        if self.inventory_open {
            self.close_inventory_screen(false);
        }
        self.chat_open = true;
        self.chat_input.clear();
        self.chat_input.push_str(prefix);
        self.mouse_captured = false;
        self.set_cursor_captured(false);
        if let Some(window) = &self.window {
            window.set_ime_allowed(true);
        }
    }

    pub(super) fn close_chat(&mut self, recapture: bool) {
        self.chat_open = false;
        self.chat_input.clear();
        self.chat_history_index = None;
        self.chat_draft = None;
        if recapture && matches!(self.state, GameState::Playing) {
            self.mouse_captured = true;
            self.set_cursor_captured(true);
        }
        if let Some(window) = &self.window {
            window.set_ime_allowed(false);
        }
    }

    pub(super) fn handle_chat_click(&mut self) {
        let hit = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gui_hit_test(self.mouse_x as f32, self.mouse_y as f32));
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let max_scroll = renderer.state.chat_lines.len().saturating_sub(15);
        match hit {
            Some(crate::ui::button_ids::CHAT_SCROLL_UP) => {
                renderer.state.chat_scroll = (renderer.state.chat_scroll + 1).min(max_scroll);
            }
            Some(crate::ui::button_ids::CHAT_SCROLL_DOWN) => {
                renderer.state.chat_scroll = renderer.state.chat_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn handle_chat_key(&mut self, code: KeyCode, text: Option<&str>, ctrl: bool) {
        match code {
            KeyCode::Escape => {
                self.chat_history_index = None;
                self.close_chat(true);
            }
            KeyCode::Tab => self.complete_chat_input(),
            KeyCode::Enter => {
                let msg = self.chat_input.trim().to_string();
                if !msg.is_empty() {
                    // Save to history (avoid duplicating last entry)
                    if self.chat_history.last().map(|s| s.as_str()) != Some(&msg) {
                        self.chat_history.push(msg.clone());
                    }
                    self.chat_history_index = None;
                    if !self.handle_script_command(&msg) {
                        self.send_scripted_chat(&msg);
                    }
                }
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.chat_scroll = 0;
                }
                self.close_chat(true);
            }
            KeyCode::ArrowUp => {
                if self.chat_history.is_empty() {
                    return;
                }
                let idx = match self.chat_history_index {
                    Some(i) => {
                        if i + 1 < self.chat_history.len() {
                            i + 1
                        } else {
                            return; // already at oldest
                        }
                    }
                    None => {
                        // Save draft before navigating history
                        self.chat_draft = Some(self.chat_input.clone());
                        0
                    }
                };
                self.chat_history_index = Some(idx);
                self.chat_input = self.chat_history[self.chat_history.len() - 1 - idx].clone();
            }
            KeyCode::ArrowDown => {
                let idx = match self.chat_history_index {
                    Some(i) => {
                        if i == 0 {
                            // Back to draft
                            self.chat_history_index = None;
                            self.chat_input = self.chat_draft.take().unwrap_or_default();
                            return;
                        }
                        i - 1
                    }
                    None => return,
                };
                self.chat_history_index = Some(idx);
                self.chat_input = self.chat_history[self.chat_history.len() - 1 - idx].clone();
            }
            KeyCode::PageUp => {
                if let Some(renderer) = &mut self.renderer {
                    let total = renderer.state.chat_lines.len();
                    let max_visible = 15;
                    let max_scroll = total.saturating_sub(max_visible);
                    renderer.state.chat_scroll = (renderer.state.chat_scroll + 10).min(max_scroll);
                }
            }
            KeyCode::PageDown => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.state.chat_scroll = renderer.state.chat_scroll.saturating_sub(10);
                }
            }
            KeyCode::KeyV if ctrl => {
                // Paste from clipboard
                if let Some(text) = get_clipboard_text() {
                    for ch in text.chars() {
                        if ch == '\n' || ch == '\r' {
                            if self.chat_input.len() < 256 {
                                self.chat_input.push(' ');
                            }
                        } else if !ch.is_control() && self.chat_input.len() < 256 {
                            self.chat_input.push(ch);
                        }
                    }
                }
            }
            KeyCode::KeyC if ctrl => {
                if !self.chat_input.is_empty() {
                    set_clipboard_text(&self.chat_input);
                }
            }
            KeyCode::KeyA if ctrl => {}
            KeyCode::KeyW if ctrl => {
                if let Some(pos) = self.chat_input.trim_end().rfind(' ') {
                    self.chat_input.truncate(pos + 1);
                    self.chat_input = self.chat_input.trim_end().to_string();
                } else {
                    self.chat_input.clear();
                }
            }
            KeyCode::Backspace => {
                self.chat_input.pop();
            }
            _ => {
                if let Some(text) = text {
                    self.append_chat_text(text);
                }
            }
        }
    }

    fn handle_script_command(&mut self, message: &str) -> bool {
        let mut parts = message.split_whitespace();
        if parts.next() != Some("/mods") || parts.next() != Some("reload") {
            return false;
        }
        let result = if let Some(id) = parts.next() {
            self.scripts
                .reload(id)
                .map(|()| format!("Reloaded Lua mod '{id}'"))
                .map_err(|error| error.to_string())
        } else {
            let report = self.scripts.reload_all();
            if report.errors.is_empty() {
                Ok(format!("Reloaded {} Lua mod(s)", report.loaded.len()))
            } else {
                Err(report.errors.join("; "))
            }
        };
        match result {
            Ok(message) => self.session.push_system_line(message),
            Err(error) => self
                .session
                .push_system_line(format!("Lua mod reload failed: {error}")),
        }
        true
    }

    fn send_scripted_chat(&mut self, message: &str) {
        let packet = crate::net::dynamic_packet::DynamicPacket::v47_chat_message(message);
        let hooked = self
            .scripts
            .process_packet("network.packet.outbound", packet);
        let Some(packet) = hooked.packet else {
            return;
        };
        if let Err(error) = client::network::send_dynamic_packet(&self.connection, &packet) {
            self.session
                .push_system_line(format!("Lua packet validation failed: {error}"));
        }
    }

    fn handle_sign_key(&mut self, code: KeyCode, text: Option<&str>) {
        match code {
            KeyCode::Escape => {
                self.session.sign_editor = None;
                self.mouse_captured = true;
                self.set_cursor_captured(true);
                if let Some(window) = &self.window {
                    window.set_ime_allowed(false);
                }
            }
            KeyCode::Enter => {
                let should_submit = self
                    .session
                    .sign_editor
                    .as_ref()
                    .is_some_and(|editor| editor.active_line >= 3);
                if should_submit {
                    self.submit_sign_editor();
                } else if let Some(editor) = &mut self.session.sign_editor {
                    editor.active_line = (editor.active_line + 1).min(3);
                }
            }
            KeyCode::ArrowUp => {
                if let Some(editor) = &mut self.session.sign_editor {
                    editor.active_line = editor.active_line.saturating_sub(1);
                }
            }
            KeyCode::ArrowDown => {
                if let Some(editor) = &mut self.session.sign_editor {
                    editor.active_line = (editor.active_line + 1).min(3);
                }
            }
            KeyCode::Backspace => {
                if let Some(editor) = &mut self.session.sign_editor {
                    editor.lines[editor.active_line].pop();
                }
            }
            _ => {
                if let Some(text) = text {
                    self.append_sign_text(text);
                }
            }
        }
    }

    fn append_sign_text(&mut self, text: &str) {
        let Some(editor) = &mut self.session.sign_editor else {
            return;
        };
        let line = &mut editor.lines[editor.active_line];
        for ch in text.chars() {
            if ch == '\r' || ch == '\n' || ch.is_control() {
                continue;
            }
            if line.chars().count() < 15 {
                line.push(ch);
            }
        }
    }

    fn submit_sign_editor(&mut self) {
        let Some(editor) = self.session.sign_editor.take() else {
            return;
        };
        let lines = [
            editor.lines[0].as_str(),
            editor.lines[1].as_str(),
            editor.lines[2].as_str(),
            editor.lines[3].as_str(),
        ];
        client::network::send_update_sign(&self.connection, editor.pos, lines);
        self.mouse_captured = true;
        self.set_cursor_captured(true);
    }

    fn complete_chat_input(&mut self) {
        if let Some(first) = self.session.tab_complete_matches.first().cloned() {
            let prefix = if self.chat_input.starts_with('/') {
                "/"
            } else {
                ""
            };
            let before_word = self
                .chat_input
                .rfind(' ')
                .map(|idx| idx + 1)
                .unwrap_or(prefix.len());
            self.chat_input.truncate(before_word);
            self.chat_input.push_str(first.trim_start_matches('/'));
            if self.chat_input.len() < 256 {
                self.chat_input.push(' ');
            }
            self.session.tab_complete_matches.clear();
            return;
        }

        if self.chat_input.trim().is_empty() {
            return;
        }
        client::network::send_tab_complete(&self.connection, &self.chat_input, None);
    }

    fn append_chat_text(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\r' || ch == '\n' || ch.is_control() {
                continue;
            }
            if self.chat_input.len() < 256 {
                self.chat_input.push(ch);
            }
        }
    }

    fn select_hotbar(&mut self, slot: usize) {
        self.inventory.set_selected(slot);
    }

    fn create_offline_account(&mut self, name: &str) {
        // Generate MC-style offline UUID from username
        let uuid = crate::auth::service::AuthService::offline_uuid(name);
        let account = crate::auth::models::Account {
            microsoft_refresh_token: None,
            minecraft_access_token: None,
            minecraft_token_expiry: None,
            uuid: Some(uuid.clone()),
            username: Some(name.to_string()),
            skins: None,
            capes: None,
        };
        let _ = crate::auth::cache::save_account(&account);
        self.accounts = crate::auth::cache::load_accounts().unwrap_or_default();
        self.selected_account = self.accounts.len().saturating_sub(1);
        self.account = Some(account);
        self.username = name.to_string();
        self.config.username = name.to_string();
        self.config.save_default();
        self.local_skin = crate::assets::skin::PlayerSkin::default_steve();
        self.local_player_model = client::player_model::PlayerModel::steve();
        self.auth_status = format!("Offline account '{}' created", name);
    }
}

fn append_creative_search(search: &mut String, text: &str) {
    for ch in text.chars() {
        if !ch.is_control() && search.chars().count() < 15 {
            search.push(ch);
        }
    }
}

/// Get clipboard text via platform-specific clipboard tool.
pub(super) fn get_clipboard_text() -> Option<String> {
    let out = if cfg!(target_os = "windows") {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Clipboard"])
            .output()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("pbpaste").output()
    } else {
        // Linux: try wl-paste (Wayland) first, then xclip (X11)
        let result = std::process::Command::new("wl-paste").args(["-n"]).output();
        if result.as_ref().is_ok_and(|r| r.status.success()) {
            return result.ok().and_then(|out| {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            });
        }
        std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
    }
    .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// Set clipboard text via platform-specific clipboard tool.
fn set_clipboard_text(text: &str) {
    if cfg!(target_os = "windows") {
        let encoded = format!(
            "[System.Text.Encoding]::UTF8.GetBytes('{}') | Set-Clipboard",
            text.replace('\'', "''")
                .replace('\n', "`n")
                .replace('\r', "`r")
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "-NonInteractive", &encoded])
            .output();
    } else if cfg!(target_os = "macos") {
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .ok();
        if let Some(ref mut child) = child {
            use std::io::Write;
            let _ = child.stdin.take().map(|mut stdin| {
                let _ = stdin.write_all(text.as_bytes());
            });
            let _ = child.wait();
        }
    } else {
        // Linux: try wl-copy (Wayland) first, then xclip (X11)
        let mut child = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn();
        if child.as_ref().is_ok_and(|c| c.stdin.is_some()) {
            if let Some(ref mut child) = child.ok() {
                use std::io::Write;
                let _ = child.stdin.take().map(|mut stdin| {
                    let _ = stdin.write_all(text.as_bytes());
                });
                let _ = child.wait();
            }
            return;
        }
        let _ = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .ok()
            .map(|mut child| {
                use std::io::Write;
                let _ = child.stdin.take().map(|mut stdin| {
                    let _ = stdin.write_all(text.as_bytes());
                });
                let _ = child.wait();
            });
    }
}
