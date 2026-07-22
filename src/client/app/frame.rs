use super::App;
use crate::audio::AudioBackend;
use crate::client;
use crate::client::keybind::InputState;
use crate::client::state::GameState;
use crate::client::tick::TickTimer;
use winit::event_loop::ActiveEventLoop;

/// Pre-computed UI state flags shared across multiple sync sections in
/// `sync_renderer_state`. Constructed once per frame and passed by reference
/// to avoid repeating the same match expressions in each sub-method.
struct SyncUiFlags {
    controls_open: bool,
    alt_manager_open: bool,
    multiplayer_open: bool,
    server_editor_open: bool,
    connection_screen_open: bool,
    server_address_open: bool,
    modding_open: bool,
}

impl App {
    pub(super) fn handle_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let frame_started = std::time::Instant::now();
        let frame_interval_us = self
            .last_profile_frame_start
            .replace(frame_started)
            .map(|previous| frame_started.duration_since(previous).as_micros() as u64)
            .unwrap_or(0);
        let published = self.last_frame_profile;
        if let Some(renderer) = &mut self.renderer {
            renderer.state.frame_profile.set_frame_total_us(published.total_us);
            renderer.state.frame_profile.set_frame_interval_us(frame_interval_us);
            renderer.state.frame_profile.set_frame_outside_us(frame_interval_us.saturating_sub(published.total_us));
            renderer.state.frame_profile.set_frame_tasks_us(published.tasks_us);
            renderer.state.frame_profile.set_frame_network_us(published.network_us);
            renderer.state.frame_profile.set_frame_world_us(published.world_us);
            renderer.state.frame_profile.set_frame_tick_us(published.tick_us);
            renderer.state.frame_profile.set_frame_sync_us(published.sync_us);
            renderer.state.frame_profile.set_frame_render_us(published.render_us);
            renderer.state.frame_profile.set_frame_other_us(published.other_us);
            renderer.state.frame_profile.set_frame_script_us(published.script_us);
            renderer.state.frame_profile.set_frame_script_callbacks(published.script_callbacks);
            renderer.state.frame_profile.set_frame_script_slow_callbacks(published.script_slow_callbacks);
            renderer.state.frame_profile.set_frame_network_debug(published.network_debug);
            // Only the Playing state guarantees the window is focused (unfocus
            // auto-pauses), so it's the sole state that should feed 1% low.
            renderer.state.frame_profile.set_frame_interval_in_gameplay(matches!(self.state, GameState::Playing));
            renderer.state.snapshot_completed_frame_profile();
            renderer.state.reset_current_frame_profile();
        }

        let tasks_started = std::time::Instant::now();
        self.poll_background_tasks();
        let tasks_us = tasks_started.elapsed().as_micros() as u64;

        let network_started = std::time::Instant::now();
        let network_result = if self.net_ctrl.connection.is_some() {
            client::network::poll_network(
                &mut self.net_ctrl.connection,
                &mut self.scripts,
                &mut self.player,
                &mut self.net_ctrl.network_state,
                &mut self.world,
                &mut self.inventory,
                &mut self.session,
                &mut self.entities,
                &mut self.particles,
                &mut self.audio,
                Some(&self.ui.i18n),
            )
        } else {
            client::network::NetworkPollResult::default()
        };
        let network_debug = network_result.debug;
        let updated_meshes = network_result.meshes;
        let network_us = network_started.elapsed().as_micros() as u64;

        let world_started = std::time::Instant::now();
        if !updated_meshes.is_empty() {
            if let Some(renderer) = &mut self.renderer {
                renderer.upload_world_partial(&updated_meshes);
            }
        }

        // LoadingWorld → Playing: transition when server sends initial position
        if matches!(self.state, GameState::LoadingWorld) {
            if self.session.received_initial_position {
                let loaded = self.world.chunks.len();
                let rd = self.config.render_distance as i32;
                let expected = (rd * 2 + 1).pow(2) as usize;
                log::info!(
                    "world ready: loaded_chunks={}, expected_chunks={}, initial_position_received=true",
                    loaded, expected
                );
                self.world.defer_mesh_build = false;
                self.state = GameState::Playing;
                // Enqueue already-received chunks for incremental mesh building.
                // Prefer near-player columns so the camera is not staring at void
                // while the outer ring builds in the background.
                let pcx = (self.player.position.x / 16.0).floor() as i32;
                let pcz = (self.player.position.z / 16.0).floor() as i32;
                self.world.enqueue_all_chunks_for_mesh_near(pcx, pcz);
                self.set_cursor_captured(true);
                self.input_ctrl.mouse_captured = true;
            }
        }

        // Playing → Disconnected: detect connection loss
        if matches!(self.state, GameState::Playing) {
            let still_connected = self
                .net_ctrl
                .connection
                .as_ref()
                .map(|c| c.connected.load(std::sync::atomic::Ordering::SeqCst))
                .unwrap_or(false);
            if !still_connected && self.net_ctrl.connection.is_some() {
                let reason = self
                    .session
                    .last_disconnect_reason
                    .clone()
                    .unwrap_or_else(|| self.ui.t("rustcraft.connection.disconnected"));
                self.disconnect_world(&reason);
            }
        }
        if matches!(self.state, GameState::Playing) {
            for mesh in self
                .world
                .poll_finished_meshes()
                .into_iter()
                .chain(self.world.schedule_background_meshes())
            {
                self.pending_chunk_uploads.insert((mesh.cx, mesh.cz), mesh);
            }

            // Building GPU draw metadata and staging dense chunks happens on
            // this event/render thread. Cap both byte volume and wall-clock
            // time so a burst of dense chunks cannot hitch the frame.
            const CHUNK_UPLOAD_BUDGET: usize = 12 * 1024 * 1024;
            /// Even a single mesh larger than this is deferred to the next
            /// frame so one ultra-dense chunk can't monopolise a frame.
            const CHUNK_UPLOAD_SINGLE_MAX: usize = 4 * 1024 * 1024;
            /// Wall-clock budget for the upload-staging path. Anything that
            /// doesn't fit is left in `pending_chunk_uploads` for next frame.
            const CHUNK_UPLOAD_TIME_BUDGET_US: u64 = 16_000;
            let upload_started = std::time::Instant::now();
            let mut upload = Vec::new();
            let mut upload_bytes = 0usize;
            while let Some(coord) = self.pending_chunk_uploads.keys().next().copied() {
                // Peek the size without removing so we can defer oversize or
                // over-budget meshes without re-inserting them.
                let mesh_bytes = {
                    let mesh = self.pending_chunk_uploads.get(&coord).unwrap();
                    mesh.vertices.len() * std::mem::size_of::<crate::world::mesh::Vertex>()
                        + mesh.indices.len() * std::mem::size_of::<u32>()
                };
                if (!upload.is_empty() && mesh_bytes > CHUNK_UPLOAD_SINGLE_MAX)
                    || (!upload.is_empty() && upload_bytes + mesh_bytes > CHUNK_UPLOAD_BUDGET)
                    || (!upload.is_empty()
                        && upload_started.elapsed().as_micros() as u64
                            > CHUNK_UPLOAD_TIME_BUDGET_US)
                {
                    break;
                }
                let mesh = self.pending_chunk_uploads.remove(&coord).unwrap();
                upload_bytes += mesh_bytes;
                upload.push(mesh);
            }
            if !upload.is_empty() {
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_world_partial(&upload);
                }
            }

            // Evict chunks outside render distance to cap memory
            let rd = self.config.render_distance;
            let pcx = (self.player.position.x / 16.0).floor() as i32;
            let pcz = (self.player.position.z / 16.0).floor() as i32;
            let evicted = self.world.unload_distant_chunks(pcx, pcz, rd as u32);
            if !evicted.is_empty() {
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_world_partial(&evicted);
                }
            }

            // Trim snapshot tombstones every 100 frames
            self.frame_count = self.frame_count.wrapping_add(1);
            if self.frame_count % 100 == 0 {
                self.world.trim_snapshot_tombstones();
            }
        }
        let world_us = world_started.elapsed().as_micros() as u64;

        if self.inventory.window_just_opened {
            self.inventory.window_just_opened = false;
            self.inventory_open = true;
            self.input_ctrl.mouse_captured = false;
            self.set_cursor_captured(false);
        }
        // Auto-close inventory UI when the server closes our container window
        // (open_window_id drops from non-zero to 0), but NOT when user opens
        // their own player inventory (window_id=0).
        if self.inventory_open
            && self.inventory.open_window_id == 0
            && self.inventory.had_server_window
        {
            // The server initiated the close (S2E CloseWindow).  We must still
            // decrement the local chest viewer count so the lid closes; the
            // S2E handler cannot do this because it has no World access.
            if let Some(position) = self.inventory.open_chest_position.take() {
                self.world.close_chest_for_local_viewer(position);
            }
            self.inventory_open = false;
            self.inventory.had_server_window = false;
            self.inventory.cursor = crate::client::inventory::ItemStack::EMPTY;
            self.input_ctrl.mouse_captured = true;
            self.set_cursor_captured(true);
        }
        if self.inventory.open_window_id != 0 {
            self.inventory.had_server_window = true;
        }

        if matches!(self.state, GameState::Playing)
            && self.input_ctrl.mouse_captured
            && self
                .session
                .resource_pack
                .as_ref()
                .is_some_and(|pack| pack.status == "available")
        {
            self.input_ctrl.mouse_captured = false;
            self.set_cursor_captured(false);
        }

        let tick_started = std::time::Instant::now();
        self.update_fps_and_title();
        self.tick_gameplay(event_loop);
        let tick_us = tick_started.elapsed().as_micros() as u64;
        // Camera FOV and view-bobbing are set from config / script overrides.
        // The tick loop only runs when tick_timer advances, so updating them
        // here ensures they are fresh every frame — otherwise a FOV change in
        // the options menu takes effect only on the next tick (up to 50ms lag).
        self.player.camera.fov = self.script_effective_fov();
        self.player.camera.view_bobbing = self.script_effective_view_bobbing();
        let t_sync = std::time::Instant::now();
        self.sync_renderer_state();
        let sync_us = t_sync.elapsed().as_micros() as u64;

        let hurt_cam = self.script_effective_hurt_cam();
        let fov_change = self.script_effective_fov_change();

        let mut render_us = 0;
        if let Some(renderer) = &mut self.renderer {
            let eye = self.player.camera.position;
            let underwater = matches!(
                self.world
                    .get_block(eye.x as i32, eye.y as i32, eye.z as i32),
                crate::world::block::Block::FlowingWater | crate::world::block::Block::StillWater
            );
            renderer.state.settings.set_underwater(underwater);
            renderer.state.settings.set_underwater_yaw(self.player.camera.yaw);
            renderer.state.settings.set_underwater_pitch(self.player.camera.pitch);
            self.player.camera.hurt_cam_enabled = hurt_cam;
            self.player.camera.fov_change_enabled = fov_change;
            let mut fov_mod = 1.0f32;
            if self.player.camera.fov_change_enabled {
                if underwater {
                    fov_mod *= 60.0 / 70.0;
                }
                if self.player.flying {
                    fov_mod *= 1.1;
                }
                if self.player.sprinting {
                    fov_mod *= 1.15;
                }
                if self.item_use_active {
                    let held_id = self.inventory.selected_item().item_id;
                    if held_id == 261 {
                        let charge = (self.item_use_timer / 1.0).min(1.0);
                        fov_mod *= 1.0 - charge * charge * 0.15;
                    }
                }
            }
            // Apply as target; actual smoothing happens in physics tick
            self.fov_target = fov_mod;
            let render_started = std::time::Instant::now();
            self.player.camera.partial_tick = self.tick_timer.alpha();
            renderer.draw_frame(
                &self.player.camera,
                self.state.menu_id(),
                underwater,
                self.state.has_world_background(),
            );
            render_us = render_started.elapsed().as_micros() as u64;
        }

        let (script_us, script_callbacks, script_slow_callbacks) =
            self.scripts.take_frame_profile();
        let total_us = frame_started.elapsed().as_micros() as u64;
        let accounted_us = tasks_us
            .saturating_add(network_us)
            .saturating_add(world_us)
            .saturating_add(tick_us)
            .saturating_add(sync_us)
            .saturating_add(render_us);
        self.last_frame_profile = super::AppFrameProfile {
            total_us,
            tasks_us,
            network_us,
            world_us,
            tick_us,
            sync_us,
            render_us,
            other_us: total_us.saturating_sub(accounted_us),
            script_us,
            script_callbacks,
            script_slow_callbacks,
            network_debug,
        };
    }

    fn poll_background_tasks(&mut self) {
        let authenticated = self.auth_task.as_ref().and_then(|task| task.try_finish());
        if let Some(result) = authenticated {
            self.auth_task = None;
            match result {
                Ok(account) => {
                    self.username = account
                        .username
                        .clone()
                        .unwrap_or_else(|| self.username.clone());
                    self.config.username = self.username.clone();
                    self.config.save_default();
                    self.auth_status = "Microsoft account authenticated".to_string();
                    self.account = Some(account);
                    self.update_local_skin();
                    self.accounts = crate::auth::cache::load_accounts().unwrap_or_default();
                    self.selected_account = self
                        .accounts
                        .iter()
                        .position(|saved| {
                            self.account
                                .as_ref()
                                .is_some_and(|active| active.uuid == saved.uuid)
                        })
                        .unwrap_or(0);
                }
                Err(error) => self.auth_status = format!("Login failed: {error}"),
            }
        }
        let refreshed = self
            .net_ctrl
            .server_refresh_task
            .as_ref()
            .and_then(|task| task.try_finish());
        if let Some(servers) = refreshed {
            self.servers = servers;
            self.selected_server = self
                .selected_server
                .min(self.servers.servers.len().saturating_sub(1));
            self.net_ctrl.server_refresh_task = None;
            self.session
                .push_system_line(self.ui.t("rustcraft.server.refreshed"));
        }

        let connected = self.net_ctrl.connect_task.as_ref().and_then(|task| {
            task.try_finish()
                .map(|result| (task.address.clone(), result))
        });
        if let Some((addr, result)) = connected {
            self.net_ctrl.connect_task = None;
            match result {
                Ok(conn) => {
                    log::info!("connection task completed successfully: server={addr}");
                    self.selected_server = self.servers.upsert(addr.clone(), addr.clone());
                    self.net_ctrl.connection = Some(conn);
                    self.scripts.set_connection_active(true);
                    self.net_ctrl.network_state = crate::client::network::ClientNetworkState::new();
                    self.session.push_system_line(format!(
                        "{}: {}",
                        self.ui.t("rustcraft.connection.connected"),
                        addr
                    ));
                    if matches!(self.state, GameState::Connecting) {
                        self.state = GameState::LoadingWorld;
                        self.session.received_initial_position = false;
                        self.world.defer_mesh_build = true;
                        self.world.set_smooth_lighting(self.config.smooth_lighting);
                        let day_time = self.session.day_time as f32;
                        self.world.set_sky_brightness(
                            crate::render::sky::SkyGradient::daylight_factor(day_time),
                        );
                        self.tick_timer = TickTimer::new();
                    }
                }
                Err(err) => {
                    log::error!("connection task failed: server={addr}, error={err}");
                    self.session.push_system_line(self.ui.i18n.tf(
                        "rustcraft.connection.failedWithReason",
                        &[&addr, &err.to_string()],
                    ));
                    self.attack_held = false;
                    self.use_held = false;
                    self.use_presses_pending = 0;
                    self.pending_attacks = 0;
                    self.dig.cancel();
                    self.pending_dig_cancel = None;
                    self.block_hit_delay = 0;
                    // Ensure no half-open connection lingers after a failed login.
                    if let Some(conn) = self.net_ctrl.connection.take() {
                        conn.close();
                        drop(conn);
                    }
                    self.scripts.set_connection_active(false);
                    self.state = GameState::Multiplayer;
                    self.input_ctrl.mouse_captured = false;
                    self.set_cursor_captured(false);
                }
            }
        }
    }

    fn update_fps_and_title(&mut self) {
        self.fps_frames += 1;
        if self.fps_timer.elapsed().as_secs_f32() < 1.0 {
            return;
        }

        self.ui.update_fps(self.fps_frames);
        if let Some(renderer) = &mut self.renderer {
            renderer.state.hud.set_fps_count(self.fps_frames);
        }
        if let Some(window) = &self.window {
            window.set_title(super::WINDOW_TITLE);
        }
        self.fps_frames = 0;
        self.fps_timer = std::time::Instant::now();
    }

    fn tick_gameplay(&mut self, event_loop: &ActiveEventLoop) {
        // Gilrs is polling-based, unlike winit's keyboard/mouse event stream.
        // Poll once per rendered frame so controller input stays responsive even
        // when the game has no fixed simulation tick in that frame.
        self.poll_gamepad(event_loop);
        let now = std::time::Instant::now();
        let api_frame_delta = now
            .duration_since(self.last_script_api_frame)
            .as_secs_f32()
            .min(0.5);
        self.last_script_api_frame = now;
        self.publish_script_snapshots(api_frame_delta, false);
        self.apply_script_commands();
        if matches!(self.state, GameState::Playing) && self.session.health <= 0.0 {
            self.attack_held = false;
            self.use_held = false;
            self.use_presses_pending = 0;
            self.pending_attacks = 0;
            self.dig.cancel();
            self.pending_dig_cancel = None;
            self.block_hit_delay = 0;
            if self.input_ctrl.mouse_captured {
                self.input_ctrl.mouse_captured = false;
                self.set_cursor_captured(false);
            }
        }
        if !matches!(self.state, GameState::Playing) {
            // Keep the world alive when menus are open — skip player input only
            self.input_ctrl.mouse_dx = 0.0;
            self.input_ctrl.mouse_dy = 0.0;
        }

        // Mouse input — only when the game has focus
        if self.input_ctrl.mouse_captured {
            self.player.process_mouse(
                self.input_ctrl.mouse_dx as f32,
                self.input_ctrl.mouse_dy as f32,
                self.config.mouse_sensitivity,
                self.config.invert_mouse,
            );
            self.input_ctrl.mouse_dx = 0.0;
            self.input_ctrl.mouse_dy = 0.0;
        }

        let physics_ticks = self.tick_timer.update();
        let ticks = physics_ticks;
        let mut interaction_meshes = Vec::new();
        for tick_i in 0..ticks {
            self.tick_timer.begin_tick();
            self.publish_script_snapshots(1.0 / 20.0, true);
            self.scripts.dispatch_json(
                "client.tick",
                serde_json::json!({
                    "tick": self.tick_timer.tick_count(),
                    "delta_time": 1.0 / 20.0,
                    "playing": matches!(self.state, GameState::Playing),
                }),
            );
            self.apply_script_commands();
            for (mod_id, packet) in self.scripts.take_active_packets() {
                let hooked = self
                    .scripts
                    .process_packet("network.packet.outbound", packet);
                let Some(packet) = hooked.packet else {
                    continue;
                };
                if let Err(error) = client::network::send_dynamic_packet(&self.net_ctrl.connection, &packet)
                {
                    log::warn!(
                        target: "rustcraft::lua",
                        "mod '{mod_id}' active packet rejected: {error}"
                    );
                }
            }
            // Smooth FOV — save previous for frame interpolation, then LERP
            self.player.camera.prev_fov_modifier = self.player.camera.fov_modifier;
            let target = self.fov_target.clamp(0.1, 1.5);
            self.player.camera.fov_modifier += (target - self.player.camera.fov_modifier) * 0.5;
            // Save prev use_timer for smooth interpolation
            self.prev_item_use_timer = self.item_use_timer;
            // S19 status 9 is handled by the network thread before the next
            // client tick in vanilla. Clear using-item state before movement
            // so the first tick after server confirmation is not slowed.
            if self.player.take_item_use_finished() {
                self.item_use_active = false;
                self.item_use_timer = 0.0;
                self.food_cooldown = 4;
                if self.net_ctrl.connection.is_none() {
                    self.inventory.remove_selected_one();
                }
            }
            // Vanilla PlayerControllerMP.updateController synchronizes the
            // selected hotbar slot before any C07/C08 interaction packet.
            client::network::sync_held_item(
                &self.net_ctrl.connection,
                &mut self.net_ctrl.network_state,
                self.inventory.selected,
            );
            // A render-frame release/use edge may have queued ABORT. Vanilla
            // emits it before the next interaction packet and before movement.
            self.flush_pending_dig_cancel();
            // Vanilla GuiContainer.mouseClicked sends C0E during
            // currentScreen.handleMouseInput() inside runTick, before
            // clickMouse.  Flush any clicks the render-frame callback queued.
            if let Some(connection) = &self.net_ctrl.connection {
                for data in self.pending_click_windows.drain(..) {
                    connection.send_play_packet(0x0E, &data);
                }
            }
            // Minecraft.runTick drains attack presses instead of dispatching
            // them when the tick starts with an item already in use.
            let using_item_at_tick_start = self.item_use_active;
            if using_item_at_tick_start {
                self.pending_attacks = 0;
            }
            // In the non-using branch vanilla processes clickMouse before its
            // right-click queue. This matters when both edges occur in one
            // tick: the attack must not be sent after eating/blocking starts.
            if self.is_swing_in_progress {
                self.swing_progress_int += 1;
                if self.swing_progress_int >= 6 {
                    self.is_swing_in_progress = false;
                    self.swing_progress_int = 0;
                }
            }
            if self.pending_attacks > 0 {
                self.pending_attacks -= 1;
                if self.attack_targeted_entity() {
                    self.attack_held = false;
                }
            }
            // Consume queued right-click input after clickMouse and before
            // continuous mining, matching Minecraft.runTick.
            self.tick_item_use_input();

            // Minecraft.sendClickBlockToController runs before world entities
            // update, so digging packets use the pre-movement position/yaw.
            interaction_meshes.extend(self.tick_block_interaction());
            // Player movement: always tick for gravity/friction.
            // When the mouse isn't captured (menu open), pass neutral input
            // so the player doesn't move but still falls/lands naturally.
            self.player.using_item = self.item_use_active;
            if self.input_ctrl.mouse_captured {
                self.player.tick(
                    &self.input_ctrl.input,
                    &self.world,
                    &self.entities,
                    self.session.entity_id,
                );
            } else {
                self.player.tick(
                    &InputState::new(),
                    &self.world,
                    &self.entities,
                    self.session.entity_id,
                );
            }
            self.tick_climbing_sound();
            // Update spatial audio listener position from player camera
            let pos = self.player.camera.position;
            let yaw = self.player.camera.yaw;
            self.audio
                .set_listener([pos[0] as f32, pos[1] as f32, pos[2] as f32], yaw);
            self.world.tick_chests();
            self.entities.tick_all(1.0 / 20.0, &self.world);
            // Spawn entity-specific particles (mob walking, fire, spell effects, etc.)
            self.entities
                .spawn_entity_particles(&mut self.particles, &self.world);
            if self.tick_timer.tick_count() % 4 == 0 {
                for effect in &self.player.active_effects {
                    if !effect.hide_particles {
                        self.particles.spawn_mob_spell(
                            nalgebra::Point3::new(
                                self.player.position.x as f32,
                                self.player.position.y as f32 + 0.5,
                                self.player.position.z as f32,
                            ),
                            crate::entity::potion_effect_color(effect.effect_id),
                            false,
                        );
                    }
                }
            }
            // Spawn environment particles (water/lava drips, underwater, etc.)
            self.spawn_environment_particles();
            self.particles.tick_in_world(1.0 / 20.0, &self.world);
            // Tick item use animation at game-tick rate (eating, drinking, bow, sword)
            if self.item_use_active || self.use_held {
                self.tick_item_use(1.0 / 20.0);
            }
            // Auto-continue eating after cooldown (vanilla: auto-starts next food if held)
            if self.food_cooldown > 0 {
                self.food_cooldown -= 1;
            }
            self.session.tick_title(1);
            self.session.tick_action_bar(1);
            let ability_flags = (if self.player.flying { 0x02 } else { 0 })
                | (if self.player.allow_flying { 0x04 } else { 0 });
            let new_abilities = (
                ability_flags,
                self.session.flying_speed,
                self.session.walking_speed,
            );
            if self.last_sent_abilities != Some(new_abilities) {
                client::network::send_player_abilities(
                    &self.net_ctrl.connection,
                    ability_flags,
                    self.session.flying_speed,
                    self.session.walking_speed,
                );
                self.last_sent_abilities = Some(new_abilities);
            }
            client::network::send_player_tick(
                &self.net_ctrl.connection,
                &mut self.player,
                &mut self.net_ctrl.network_state,
                self.session.entity_id,
            );
            self.use_release_pending = false;
            self.input_ctrl.input.tick_reset();
        }

        if !interaction_meshes.is_empty() {
            if let Some(renderer) = &mut self.renderer {
                renderer.upload_world_partial(&interaction_meshes);
            }
        }

        self.apply_script_visual_overrides();
        self.player
            .update_render_position(self.tick_timer.alpha(), Some(&self.world));
        self.entities
            .update_render_positions(self.tick_timer.alpha());
    }

    /// Spawn environment particles (water/lava drips, underwater bubbles, etc.)
    fn spawn_environment_particles(&mut self) {
        use crate::world::block::Block;

        let cam_position = self.player.camera.position;
        let px = cam_position.x as i32;
        let py = cam_position.y as i32;
        let pz = cam_position.z as i32;

        self.spawn_rain_particles();

        // Only spawn environment particles occasionally (every 8 ticks)
        if self.tick_timer.tick_count() % 8 != 0 {
            return;
        }

        // Check blocks around the camera for water/lava drips
        for dx in -4..=4 {
            for dy in -3..=3 {
                for dz in -4..=4 {
                    let bx = px + dx;
                    let by = py + dy;
                    let bz = pz + dz;
                    let block = self.world.get_block(bx, by, bz);

                    // Water drip from ceiling
                    if block == Block::StillWater || block == Block::FlowingWater {
                        // Check if there's air above
                        let above = self.world.get_block(bx, by + 1, bz);
                        if above == Block::Air {
                            // Spawn water drip particles below
                            if crate::client::particles::particle_pos_hash(
                                (bx as f32 * 0.37 + bz as f32 * 0.11 + by as f32 * 0.53) * 10.0,
                            ) < 0.02
                            {
                                self.particles.spawn_drip(
                                    nalgebra::Point3::new(
                                        bx as f32 + 0.5,
                                        by as f32,
                                        bz as f32 + 0.5,
                                    ),
                                    false,
                                );
                            }
                        }
                    }

                    // Lava drip from ceiling
                    if block == Block::FlowingLava || block == Block::StillLava {
                        let above = self.world.get_block(bx, by + 1, bz);
                        if above == Block::Air {
                            if crate::client::particles::particle_pos_hash(
                                (bx as f32 * 0.19 + bz as f32 * 0.53 + by as f32 * 0.37) * 10.0,
                            ) < 0.02
                            {
                                self.particles.spawn_drip(
                                    nalgebra::Point3::new(
                                        bx as f32 + 0.5,
                                        by as f32,
                                        bz as f32 + 0.5,
                                    ),
                                    true,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Underwater particles (suspended, bubbles)
        let eye_block = self.world.get_block(px, py, pz);
        if eye_block == Block::StillWater || eye_block == Block::FlowingWater {
            // Spawn suspended particles
            for i in 0..2 {
                let seed = self.tick_timer.tick_count() as f32 + i as f32;
                let ox = crate::client::particles::particle_pos_hash(seed) * 6.0 - 3.0;
                let oy = crate::client::particles::particle_pos_hash(seed + 1.0) * 4.0 - 2.0;
                let oz = crate::client::particles::particle_pos_hash(seed + 2.0) * 6.0 - 3.0;
                self.particles.spawn(crate::client::particles::Particle {
                    kind: crate::client::particles::ParticleKind::Suspended,
                    position: nalgebra::Point3::new(
                        cam_position.x + ox,
                        cam_position.y + oy,
                        cam_position.z + oz,
                    ),
                    velocity: nalgebra::Vector3::new(0.0, 0.01, 0.0),
                    age: 0.0,
                    lifetime: 1.5,
                    size: 0.04,
                    color: [0.40, 0.60, 0.90, 0.25],
                    rotation: 0.0,
                    texture_jitter: [0.0, 0.0],
                    on_ground: false,
                });
            }

            // Spawn bubbles near the player's head
            if self.tick_timer.tick_count() % 16 == 0 {
                let seed = self.tick_timer.tick_count() as f32;
                self.particles.spawn(crate::client::particles::Particle {
                    kind: crate::client::particles::ParticleKind::Bubble,
                    position: nalgebra::Point3::new(
                        cam_position.x + crate::client::particles::particle_pos_hash(seed) * 0.4
                            - 0.2,
                        cam_position.y,
                        cam_position.z
                            + crate::client::particles::particle_pos_hash(seed + 3.0) * 0.4
                            - 0.2,
                    ),
                    velocity: nalgebra::Vector3::new(0.0, 0.05, 0.0),
                    age: 0.0,
                    lifetime: 0.5,
                    size: 0.06,
                    color: [0.40, 0.65, 1.0, 0.50],
                    rotation: 0.0,
                    texture_jitter: [0.0, 0.0],
                    on_ground: false,
                });
            }
        }
    }

    /// Local precipitation follows the player's camera instead of being a HUD
    /// indicator. The world collision pass stops drops under ceilings.
    fn spawn_rain_particles(&mut self) {
        if !self.config.weather_effects || !self.session.game_state.raining {
            return;
        }

        let camera = &self.player.camera;
        let tick = self.tick_timer.tick_count() as f32;
        let strength = self.session.game_state.rain_level.max(0.1).clamp(0.0, 1.0);
        for index in 0..8 {
            let seed = tick * 17.0 + index as f32 * 5.0;
            let x =
                camera.position.x + crate::client::particles::particle_pos_hash(seed) * 16.0 - 8.0;
            let z = camera.position.z
                + crate::client::particles::particle_pos_hash(seed + 1.0) * 16.0
                - 8.0;
            let y = camera.position.y
                + 6.0
                + crate::client::particles::particle_pos_hash(seed + 2.0) * 4.0;
            let bx = x.floor() as i32;
            let bz = z.floor() as i32;
            let blocked_above = ((camera.position.y.floor() as i32)..=y.ceil() as i32)
                .any(|by| self.world.get_block(bx, by, bz).is_solid());
            if blocked_above {
                continue;
            }
            self.particles.spawn(crate::client::particles::Particle {
                kind: crate::client::particles::ParticleKind::Rain,
                position: nalgebra::Point3::new(x, y, z),
                velocity: nalgebra::Vector3::new(0.0, -18.0, 0.0),
                age: 0.0,
                lifetime: 0.55,
                size: 0.08,
                color: [0.75, 0.85, 1.0, 0.25 + strength * 0.35],
                rotation: 0.0,
                texture_jitter: [0.0, 0.0],
                on_ground: false,
            });
        }
    }

    fn sync_renderer_state(&mut self) {
        self.sync_session_locale();

        let target = if matches!(self.state, GameState::Playing)
            && self.input_ctrl.mouse_captured
            && !self.inventory_open
            && !self.chat_open
        {
            client::interaction::target_block(&self.world, &self.player.camera, 4.5)
        } else {
            None
        };

        if self.renderer.is_none() {
            return;
        }

        // Pre-compute UI state flags shared across multiple sync sections.
        let ui_flags = SyncUiFlags {
            controls_open: matches!(&self.state, GameState::Controls { .. }),
            alt_manager_open: matches!(&self.state, GameState::AltManager),
            multiplayer_open: matches!(&self.state, GameState::Multiplayer),
            server_editor_open: matches!(&self.state, GameState::ServerEditor { .. }),
            connection_screen_open: matches!(
                &self.state,
                GameState::Connecting | GameState::LoadingWorld | GameState::Disconnected { .. }
            ),
            server_address_open: matches!(
                &self.state,
                GameState::DirectConnect | GameState::Connecting | GameState::LoadingWorld
            ),
            modding_open: matches!(&self.state, GameState::Modding { .. }),
        };

        // Static world render metadata and downloaded skin completions only
        // change on the 20 Hz client tick. Re-hashing signs, skull NBT and the
        // entire player roster at render frequency wastes hundreds of passes
        // per second without making the result any fresher than vanilla.
        let static_render_tick = self.tick_timer.tick_count();
        let refresh_static_render_state = self.last_static_render_tick != static_render_tick;
        let (skull_hash, player_skin_update) = if refresh_static_render_state {
            self.last_static_render_tick = static_render_tick;
            let hash = self.compute_skull_hash();
            (Some(hash), self.collect_pending_player_skins(hash))
        } else {
            (None, None)
        };
        let entity_hash = self.compute_entity_hash();

        let active_config_id = match &self.state {
            GameState::ModConfig { mod_id, .. } => Some(mod_id.clone()),
            _ => None,
        };
        let mod_config_view = active_config_id.as_deref().map(|mod_id| {
            let mod_info = self
                .scripts
                .loaded_mods()
                .into_iter()
                .find(|info| info.id == mod_id);
            let title = mod_info
                .as_ref()
                .map(|info| format!("{} Configuration", info.name))
                .unwrap_or_else(|| format!("{mod_id} Configuration"));
            let locked = self.net_ctrl.connection.is_some()
                && mod_info
                    .as_ref()
                    .is_some_and(|info| info.protocol_translator);
            let rows = self
                .scripts
                .config_entries(mod_id)
                .unwrap_or_default()
                .into_iter()
                .map(mod_config_row)
                .collect::<Vec<_>>();
            self.mod_config_selected = self.mod_config_selected.min(rows.len().saturating_sub(1));
            (title, rows, locked)
        });

        if self.renderer.is_none() {
            return;
        }

        if let Some(renderer) = &mut self.renderer {
            renderer.state.frame_profile.set_entity_state_hash(entity_hash);
        }

        self.sync_chest_entries();

        if let Some(renderer) = &mut self.renderer {
            if let Some((pending, content_hash, layout_hash)) = player_skin_update {
                if renderer.state.hud.player_skin_content_hash() != content_hash
                    || renderer.state.hud.player_skin_layout_hash() != layout_hash
                {
                    renderer.state.hud.set_pending_player_skins(pending);
                    renderer.state.hud.set_player_skin_content_hash(content_hash);
                    renderer.state.hud.set_player_skin_layout_hash(layout_hash);
                }
            }
        }

        self.sync_hotbar();
        self.sync_sign_entries(refresh_static_render_state);
        self.sync_inventory_window();
        self.sync_block_selection(&target);
        self.sync_render_settings();
        self.sync_ui_text(&ui_flags);
        self.sync_menu_state(&ui_flags, &mod_config_view);
        self.sync_hud_gameplay(&target, refresh_static_render_state);
        self.sync_player_list(refresh_static_render_state);
        self.sync_entity_billboards(entity_hash, refresh_static_render_state);
        self.sync_skull_and_particles(skull_hash);
        self.sync_local_player_and_hand(&target, refresh_static_render_state);
        self.sync_inventory_slots();
    }

    /// Sync session locale and client settings that must be updated every frame
    /// regardless of renderer availability.
    fn sync_session_locale(&mut self) {
        if self.last_ui_text_hash == 0 || self.session.locale != self.config.language {
            self.session.locale.clone_from(&self.config.language);
            self.session.text.inventory_rejected = self.ui.t("rustcraft.inventory.rejected");
            self.session.text.resource_pack_offered = self.ui.t("rustcraft.resourcePack.offered");
            self.session.text.opened_window = self.ui.t("rustcraft.inventory.openedWindow");
        }
        self.session.view_distance = self.config.render_distance;
        self.session.skin_parts = self.config.skin_parts;
    }

    fn sync_chest_entries(&mut self) {
        let chest_alpha = self.tick_timer.alpha();
        let renderer = self.renderer.as_mut().unwrap();
        renderer.state.hud.set_chest_entries(self
            .world
            .chests
            .iter()
            .filter_map(|(&(x, y, z), chest)| {
                let block = self.world.get_block(x, y, z);
                if !crate::world::is_chest_block(block) {
                    return None;
                }
                let same = |nx, ny, nz| self.world.get_block(nx, ny, nz) == block;
                if block != crate::world::block::Block::EnderChest
                    && (same(x - 1, y, z) || same(x, y, z - 1))
                {
                    return None;
                }
                let double_x = block != crate::world::block::Block::EnderChest && same(x + 1, y, z);
                let double_z = block != crate::world::block::Block::EnderChest && same(x, y, z + 1);
                let mut lid_angle =
                    chest.prev_lid_angle + (chest.lid_angle - chest.prev_lid_angle) * chest_alpha;
                if double_x {
                    if let Some(adjacent) = self.world.chests.get(&(x + 1, y, z)) {
                        lid_angle = lid_angle.max(
                            adjacent.prev_lid_angle
                                + (adjacent.lid_angle - adjacent.prev_lid_angle) * chest_alpha,
                        );
                    }
                } else if double_z {
                    if let Some(adjacent) = self.world.chests.get(&(x, y, z + 1)) {
                        lid_angle = lid_angle.max(
                            adjacent.prev_lid_angle
                                + (adjacent.lid_angle - adjacent.prev_lid_angle) * chest_alpha,
                        );
                    }
                }
                let light = self.world.light_at_world(x, y, z);
                Some(crate::render::ChestRenderEntry {
                    position: [x, y, z],
                    block,
                    metadata: self.world.get_block_metadata(x, y, z),
                    lid_angle,
                    double_x,
                    double_z,
                    sky_light: light.sky,
                    block_light: light.block,
                })
            })
            .collect());
        renderer.state.hud.chest_entries_mut()
            .sort_unstable_by_key(|entry| entry.position);
    }

    fn sync_hotbar(&mut self) {
        let renderer = self.renderer.as_mut().unwrap();
        for i in 0..9 {
            let slot = &self.inventory.slots[i];
            renderer.state.inventory.hotbar_slots_mut()[i] = (slot.item_id, slot.count, slot.damage);
        }
        renderer.state.inventory.set_hotbar_selected(self.inventory.selected);
        renderer.state.inventory.set_inventory_open(self.inventory_open);
    }

    fn sync_sign_entries(&mut self, refresh_static: bool) {
        let renderer = self.renderer.as_mut().unwrap();
        if !refresh_static {
            return;
        }
        // Build from the current block state rather than hashing the raw
        // tile-entity map. Tile data can arrive before its chunk and stale
        // entries can survive an unload, so only real, currently loaded
        // sign blocks are allowed into render state.
        let mut sign_entries: Vec<_> = self
            .session
            .sign_data
            .iter()
            .filter_map(|(&pos, lines)| {
                let block = self.world.get_block(pos.0, pos.1, pos.2);
                let wall_mounted = block == crate::world::block::Block::WallSign;
                if !wall_mounted && block != crate::world::block::Block::StandingSign {
                    return None;
                }
                Some(crate::render::hud::entities::SignEntry {
                    position: [pos.0, pos.1, pos.2],
                    // Network decoding already converts the JSON component
                    // to plain text. Parsing it again loses nested content.
                    lines: lines.to_vec(),
                    wall_mounted,
                    metadata: self.world.get_block_metadata(pos.0, pos.1, pos.2),
                })
            })
            .collect();
        sign_entries.sort_unstable_by_key(|entry| entry.position);

        let sign_hash = {
            use std::hash::Hasher;
            let mut h = fnv::FnvHasher::default();
            h.write_usize(sign_entries.len());
            for sign in &sign_entries {
                for position in sign.position {
                    h.write_i32(position);
                }
                h.write_u8(sign.wall_mounted as u8);
                h.write_u8(sign.metadata);
                for line in &sign.lines {
                    h.write(line.as_bytes());
                    h.write_u8(0);
                }
            }
            h.finish()
        };
        if sign_hash != self.last_sign_hash {
            self.last_sign_hash = sign_hash;
            renderer.state.hud.set_sign_entries(sign_entries);
        }
    }

    fn sync_inventory_window(&mut self) {
        if !self.inventory_open {
            return;
        }
        let renderer = self.renderer.as_mut().unwrap();
        renderer.state.inventory.set_inventory_window_id(self.inventory.open_window_id);
        renderer.state.inventory.inventory_window_type_mut().clone_from(&self.inventory.open_window_type);
        renderer.state.inventory.inventory_window_title_mut().clone_from(&self.inventory.open_window_title);
        renderer.state.inventory.set_inventory_window_slot_count(self.inventory.open_window_slot_count);
        renderer.state.inventory.inventory_window_slots_mut().clear();
        renderer.state.inventory.inventory_window_slots_mut().extend(
            (0..self.inventory.open_window_slot_count)
                .map(|slot| self.inventory.item_view_for_protocol_slot(slot as i16)),
        );
        renderer.state.inventory.inventory_window_properties_mut().clone_from(&self.inventory.open_window_properties);
    }

    fn sync_block_selection(&mut self, target: &Option<client::interaction::TargetBlock>) {
        let renderer = self.renderer.as_mut().unwrap();
        let new_boxes = target
            .as_ref()
            .map(|target| target.boxes.clone())
            .unwrap_or_default();
        if new_boxes.len() != renderer.state.hud.block_selection_boxes().len()
            || new_boxes
                .iter()
                .zip(renderer.state.hud.block_selection_boxes().iter())
                .any(|(a, b)| a.min != b.min || a.max != b.max)
        {
            renderer.block_dirty = true;
        }
        renderer.state.hud.set_block_selection_boxes(new_boxes);
        let new_dig_progress = self.dig.progress();
        if (new_dig_progress - renderer.state.hud.dig_progress()).abs() > 0.001 {
            renderer.block_dirty = true;
        }
        renderer.state.hud.set_dig_progress(new_dig_progress);
        let new_dig_pos = self.dig.active_pos().map(|p| [p.0, p.1, p.2]);
        if new_dig_pos != renderer.state.hud.dig_position() {
            renderer.block_dirty = true;
        }
        renderer.state.hud.set_dig_position(new_dig_pos);
    }

    fn sync_render_settings(&mut self) {
        let renderer = self.renderer.as_mut().unwrap();
        renderer.state.settings.set_render_distance(self.config.render_distance);
        renderer.state.settings.set_smooth_lighting(self.config.smooth_lighting);
        renderer.state.frame_profile.set_chunk_count_loaded(self.world.chunks.len());
        let sky_b = crate::render::sky::SkyGradient::daylight_factor(self.session.day_time as f32);
        if (sky_b - renderer.state.frame_profile.sky_brightness_cached()).abs() > 0.01 {
            renderer.state.frame_profile.set_sky_brightness_cached(sky_b);
            self.world.set_sky_brightness(sky_b);
        }
        let particles_label = self.config.particles.label();
        if *renderer.state.settings.particles_label() != particles_label {
            renderer.state.settings.particles_label_mut().clear();
            renderer.state.settings.particles_label_mut().push_str(particles_label);
        }
        renderer.state.settings.set_particles_enabled(self.config.particles.enabled());
        renderer.state.settings.set_master_volume(self.config.master_volume);
        renderer.state.settings.set_music_volume(self.config.music_volume);
        renderer.state.settings.set_blocks_volume(self.config.blocks_volume);
        renderer.state.settings.set_hostile_volume(self.config.hostile_volume);
        renderer.state.settings.set_friendly_volume(self.config.friendly_volume);
        renderer.state.settings.set_players_volume(self.config.players_volume);
        renderer.state.settings.set_ambient_volume(self.config.ambient_volume);
        renderer.state.settings.set_weather_volume(self.config.weather_volume);
        renderer.state.settings.set_ui_volume(self.config.ui_volume);
        let audio_device = self.audio.device_name();
        if *renderer.state.settings.audio_device() != audio_device {
            renderer.state.settings.audio_device_mut().clear();
            renderer.state.settings.audio_device_mut().push_str(audio_device);
        }
        renderer.state.settings.set_fov(self.config.fov);
        renderer.state.settings.set_gamma(self.config.gamma);
        renderer.state.settings.set_max_framerate(self.config.max_framerate);
        renderer.state.settings.set_clouds(self.config.clouds);
        renderer.state.settings.set_weather_effects(self.config.weather_effects);
        renderer.state.settings.set_entity_shadows(self.config.entity_shadows);
        renderer.state.settings.set_view_bobbing(self.config.view_bobbing);
        renderer.state.settings.set_advanced_tooltips(self.config.advanced_tooltips);
        renderer.state.hud.set_chat_width(self.config.chat_width.clamp(0.1, 1.0));
        renderer.state.hud.set_chat_height(self.config.chat_height.clamp(1, 30));
        renderer.state.hud.set_chat_background(self.config.chat_background);
        renderer.state.hud.set_chat_overlay(self.config.chat_overlay);
        renderer.state.hud.set_chat_player_avatars(self.config.chat_player_avatars);
        renderer.state.hud.set_tab_player_avatars(self.config.tab_player_avatars);
        renderer.state.settings.set_better_grass(self.config.better_grass);
        renderer.state.settings.set_connected_textures(self.config.connected_textures);
        // Difficulty is server/world state, not a client-configurable preference.
        renderer.state.settings.set_difficulty(self.session.difficulty);
        renderer.state.settings.set_skin_parts(self.config.skin_parts);
        let ui_text_hash = {
            use std::hash::Hasher;
            let mut h = fnv::FnvHasher::default();
            h.write(self.config.language.as_bytes());
            h.finish()
        };
        if ui_text_hash != self.last_ui_text_hash {
            self.last_ui_text_hash = ui_text_hash;
            renderer.state.settings.language_code_mut().clone_from(&self.config.language);
            renderer.state.settings.set_language_name(self.ui.t("language.name"));
            renderer.state.settings.set_ui_text(self.ui.text.clone());
        }
    }

    fn sync_ui_text(&mut self, flags: &SyncUiFlags) {
        let renderer = self.renderer.as_mut().unwrap();
        if flags.controls_open {
            renderer.state.settings.set_control_bindings(self.input_ctrl.keybinds.control_rows_for_device(
                self.input_ctrl.control_device,
                self.input_ctrl.rebinding_action,
                &self.ui.i18n,
            ));
            renderer.state.settings.set_rebinding_action(self.input_ctrl.rebinding_action);
            renderer.state.settings.set_controls_gamepad(matches!(
                self.input_ctrl.control_device,
                crate::client::keybind::ControlDevice::Gamepad
            ));
        }
        renderer.state.settings.set_mouse_sensitivity(self.config.mouse_sensitivity);
        renderer.state.settings.set_invert_mouse(self.config.invert_mouse);
        renderer.state.settings.set_gamepad_look_sensitivity(self.config.gamepad_look_sensitivity);
        renderer.state.settings.set_gamepad_cursor_speed(self.config.gamepad_cursor_speed);
    }

    fn sync_menu_state(
        &mut self,
        flags: &SyncUiFlags,
        mod_config_view: &Option<(String, Vec<crate::render::ModConfigRow>, bool)>,
    ) {
        let renderer = self.renderer.as_mut().unwrap();
        if flags.server_address_open {
            renderer.state.server_list.server_address_mut().clone_from(&self.server_address);
            renderer.state.account.username_mut().clone_from(&self.username);
        }
        if flags.alt_manager_open {
            renderer.state.account.set_account_name(self
                .account
                .as_ref()
                .and_then(|account| account.username.clone())
                .unwrap_or_else(|| "No Microsoft account selected".to_string()));
            renderer.state.account.set_account_status(if self.auth_task.is_some() {
                "Waiting for Microsoft sign-in...".to_string()
            } else {
                self.auth_status.clone()
            });
            renderer.state.account.set_entering_offline_name(self.entering_offline_name);
            renderer.state.account.offline_username_input_mut().clone_from(&self.offline_username_input);
            renderer.state.account.set_account_list(self
                .accounts
                .iter()
                .map(|account| {
                    let active = self
                        .account
                        .as_ref()
                        .is_some_and(|selected| selected.uuid == account.uuid);
                    (
                        account
                            .username
                            .clone()
                            .unwrap_or_else(|| "Unknown account".to_string()),
                        account.uuid.clone().unwrap_or_default(),
                        active,
                    )
                })
                .collect());
            renderer.state.account.set_selected_account(self.selected_account);
        }
        if flags.connection_screen_open {
            renderer.state.account.set_connection_status(match &self.state {
                GameState::Disconnected { reason } => reason.clone(),
                _ => {
                    if let Some(task) = &self.net_ctrl.connect_task {
                        self.ui
                            .i18n
                            .tf("rustcraft.connection.connecting", &[&task.address])
                    } else if self.net_ctrl.connection.is_some() {
                        if self
                            .net_ctrl
                            .connection
                            .as_ref()
                            .map(|connection| {
                                connection
                                    .connected
                                    .load(std::sync::atomic::Ordering::SeqCst)
                            })
                            .unwrap_or(false)
                        {
                            self.ui.t("rustcraft.connection.connected")
                        } else {
                            self.ui.t("rustcraft.connection.disconnected")
                        }
                    } else {
                        self.ui.t("rustcraft.connection.notConnected")
                    }
                }
            });
        }
        if flags.multiplayer_open {
            renderer.state.account.set_server_refreshing(self.net_ctrl.server_refresh_task.is_some());
            // Only rebuild when count or key fields changed.
            if renderer.state.server_list.server_list().len() != self.servers.servers.len()
                || self
                    .servers
                    .servers
                    .iter()
                    .zip(renderer.state.server_list.server_list().iter())
                    .any(|(a, b)| {
                        a.address != b.address
                            || a.status.ping_ms != b.ping_ms
                            || a.status.online != b.online
                            || a.status.players_online != b.players_online
                    })
            {
                renderer.state.server_list.set_server_list(self
                    .servers
                    .servers
                    .iter()
                    .map(|server| crate::render::ServerListRow {
                        name: server.name.clone(),
                        address: server.address.clone(),
                        online: server.status.online,
                        ping_ms: server.status.ping_ms,
                        players_online: server.status.players_online,
                        players_max: server.status.players_max,
                        version_name: server.status.version_name.clone(),
                        description: server.status.description.clone(),
                        error: server.status.error.clone(),
                        favicon_pixels: server
                            .status
                            .favicon
                            .as_ref()
                            .and_then(|f| crate::client::server_list::decode_favicon(f)),
                    })
                    .collect());
            }
            renderer.state.server_list.set_selected_server(self.selected_server);
        }
        if flags.server_editor_open {
            renderer.state.server_list.server_editor_name_mut().clone_from(&self.server_editor_name);
            renderer.state.server_list.server_editor_address_mut().clone_from(&self.server_editor_address);
            renderer.state.server_list.set_server_editor_address_focused(self.server_editor_address_focused);
        }
        if flags.modding_open {
            let modding_rows = self
                .scripts
                .loaded_mods()
                .into_iter()
                .map(|mod_info| crate::render::ModManagerRow {
                    id: mod_info.id,
                    name: mod_info.name,
                    version: mod_info.version,
                    enabled: mod_info.enabled,
                    protocol_translator: mod_info.protocol_translator,
                    config_entries: mod_info.config_entries,
                    granted_permissions: mod_info.granted_permissions,
                    denied_permissions: mod_info.denied_permissions,
                })
                .collect::<Vec<_>>();
            self.modding_selected = self
                .modding_selected
                .min(modding_rows.len().saturating_sub(1));
            renderer.state.server_list.set_modding_rows(modding_rows);
            renderer.state.server_list.set_modding_selected(self.modding_selected);
            renderer.state.server_list.modding_status_mut().clone_from(&self.modding_status);
            renderer.state.server_list.set_modding_connection_active(self.net_ctrl.connection.is_some());
        }
        if let Some((title, rows, locked)) = mod_config_view {
            renderer.state.server_list.set_mod_config_title(Some(title.clone()));
            renderer.state.server_list.set_mod_config_rows(rows.clone());
            renderer.state.server_list.set_mod_config_selected(self.mod_config_selected);
            renderer.state.server_list.set_mod_config_status(self.mod_config_status.clone());
            renderer.state.server_list.set_mod_config_locked(*locked);
        } else if renderer.state.server_list.mod_config_title_mut().take().is_some() {
            renderer.state.server_list.mod_config_rows_mut().clear();
            renderer.state.server_list.set_mod_config_locked(false);
        }
    }

    fn sync_hud_gameplay(
        &mut self,
        target: &Option<client::interaction::TargetBlock>,
        refresh_static: bool,
    ) {
        let renderer = self.renderer.as_mut().unwrap();
        renderer.state.hud.set_max_players(self.session.max_players);
        renderer.state.hud.set_world_time(self.session.world_time);
        renderer.state.hud.set_day_time(self.session.day_time);
        renderer.state.hud.set_dimension(self.session.dimension);
        renderer.state.hud.set_debug_pos([
            self.player.position.x,
            self.player.position.y,
            self.player.position.z,
        ]);
        renderer.state.hud.set_debug_yaw(self.player.camera.yaw);
        renderer.state.hud.set_debug_pitch(self.player.camera.pitch);
        renderer.state.hud.set_debug_entity_count(self.entities.count());
        renderer.state.hud.set_debug_looking_at(target.as_ref().map(|t| {
            [t.hit.pos.0, t.hit.pos.1, t.hit.pos.2]
        }));
        let bx = self.player.position.x.floor() as i32;
        let by = self.player.position.y.floor() as i32;
        let bz = self.player.position.z.floor() as i32;
        renderer.state.hud.set_debug_biome_id(self.world.biome_at(bx, bz));
        let light = self.world.light_at_world(bx, by, bz);
        let combined = light.sky.max(light.block);
        renderer.state.hud.set_debug_light_combined(combined);
        renderer.state.hud.set_debug_light_sky(light.sky);
        renderer.state.hud.set_debug_light_block(light.block);
        let looking_block = target.as_ref().map(|t| {
            let block = self.world.get_block(t.hit.pos.0, t.hit.pos.1, t.hit.pos.2);
            format!("minecraft:{}", block.properties().name)
        });
        renderer
            .state
            .hud
            .set_debug_looking_block(looking_block);
        renderer.state.hud.set_raining(self.session.game_state.raining);
        renderer.state.hud.set_rain_level(self.session.game_state.rain_level);
        renderer.state.hud.set_thunder_level(self.session.game_state.thunder_level);
        renderer.state.hud.set_gamemode(self.session.gamemode);
        if *renderer.state.hud.level_type() != self.session.level_type {
            renderer.state.hud.level_type_mut().clone_from(&self.session.level_type);
        }
        renderer.state.hud.set_spawn_position(self.session.spawn_position);

        // Health animation
        let current_health = self.session.health;
        if (current_health - renderer.state.hud.health()).abs() > 0.01 {
            renderer.state.hud.set_prev_health(renderer.state.hud.health());
            renderer.state.hud.set_health_timer(20);
        }
        renderer.state.hud.set_health(current_health);

        // Food animation
        if renderer.state.hud.food() != self.session.food {
            renderer.state.hud.set_prev_food(renderer.state.hud.food());
            renderer.state.hud.set_food_timer(10);
        }

        // Armor from inventory
        renderer.state.hud.set_armor_points(crate::client::armor::total_armor_points(&self.inventory));

        // Absorption from potion effect
        renderer.state.hud.set_absorption(self
            .player
            .active_effects
            .iter()
            .find(|e| e.effect_id == 22)
            .map(|e| (e.amplifier as f32 + 1.0) * 4.0)
            .unwrap_or(0.0));

        renderer.state.hud.set_food(self.session.food);
        renderer.state.hud.set_saturation(self.session.saturation);

        // Decay animation timers
        if renderer.state.hud.health_timer() > 0 {
            renderer.state.hud.set_health_timer(renderer.state.hud.health_timer() - 1);
        }
        if renderer.state.hud.food_timer() > 0 {
            renderer.state.hud.set_food_timer(renderer.state.hud.food_timer() - 1);
        }
        renderer.state.hud.set_experience_bar(self.session.experience_bar);
        renderer.state.hud.set_experience_level(self.session.experience_level);
        renderer.state.hud.set_experience_total(self.session.experience_total);
        renderer.state.hud.set_chat_open(self.chat_open);
        if self.chat_open {
            renderer.state.hud.chat_input_mut().clone_from(&self.chat_input);
        }
        let localized_chat_lines = self
            .session
            .chat_lines
            .iter()
            .zip(&self.session.chat_json)
            .map(|(text, json)| {
                json.as_deref()
                    .and_then(|json| {
                        crate::client::session::localized_chat_text(json, &self.ui.i18n)
                    })
                    .unwrap_or_else(|| text.clone())
            })
            .collect::<Vec<_>>();
        if self.chat_open || *renderer.state.hud.chat_lines() != localized_chat_lines {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            if self.chat_open {
                renderer.state.hud.set_chat_visible_time(now);
            }
            if renderer.state.hud.chat_lines() != &localized_chat_lines {
                renderer.state.hud.chat_lines_mut().clone_from(&localized_chat_lines);
                renderer.state.hud.set_chat_faces(self
                    .session
                    .chat_senders
                    .iter()
                    .map(|sender| {
                        sender.as_deref().and_then(|uuid| {
                            let player = self.session.player_list.get(uuid)?;
                            Some(self.skin_cache.face_for(
                                Some(uuid),
                                Some(&player.name),
                                player.skin_property.as_deref(),
                            ))
                        })
                    })
                    .collect());
                // Keep messages visible only when a new chat state arrives.
                renderer.state.hud.set_chat_last_message_time(now);
            }
        }
        let sign_editor_was_open = renderer.state.hud.sign_editor_open();
        renderer.state.hud.set_sign_editor_open(self.session.sign_editor.is_some());
        if let Some(editor) = &self.session.sign_editor {
            renderer.state.hud.set_sign_editor_lines(editor.lines.clone());
            renderer.state.hud.set_sign_editor_active_line(editor.active_line);
        } else if sign_editor_was_open {
            renderer.state.hud.set_sign_editor_lines(Default::default());
            renderer.state.hud.set_sign_editor_active_line(0);
        }
        renderer.state.hud.set_book_editor_open(self.book_editor.is_some());
        if let Some(editor) = &self.book_editor {
            renderer.state.hud.book_pages_mut().clone_from(&editor.pages);
            renderer.state.hud.set_book_page(editor.page);
            renderer.state.hud.set_book_signing(editor.signing);
            renderer.state.hud.book_title_mut().clone_from(&editor.title);
        } else {
            renderer.state.hud.book_pages_mut().clear();
            renderer.state.hud.set_book_page(0);
            renderer.state.hud.set_book_signing(false);
            renderer.state.hud.book_title_mut().clear();
        }
        renderer.state.hud.set_player_list_open(self.player_list_open);
        if self.player_list_open {
            renderer.state.hud.tab_header_mut().clone_from(&self.session.tab_header);
            renderer.state.hud.tab_footer_mut().clone_from(&self.session.tab_footer);
        }
        if *renderer.state.hud.action_bar() != self.session.action_bar {
            renderer.state.hud.action_bar_mut().clone_from(&self.session.action_bar);
        }
        if *renderer.state.hud.title_text() != self.session.title.title {
            renderer.state.hud.title_text_mut().clone_from(&self.session.title.title);
        }
        if *renderer.state.hud.subtitle_text() != self.session.title.subtitle {
            renderer.state.hud.subtitle_text_mut().clone_from(&self.session.title.subtitle);
        }
        renderer.state.hud.set_title_alpha(self.session.title.alpha());
        if self.last_scoreboard_generation != self.session.scoreboard_generation {
            self.last_scoreboard_generation = self.session.scoreboard_generation;
            let (sidebar_title, sidebar_lines) = self.session.sidebar_lines();
            renderer.state.hud.set_sidebar_title(sidebar_title);
            renderer.state.hud.set_sidebar_lines(sidebar_lines
                .into_iter()
                .map(|line| crate::render::SidebarLine {
                    display: line.display,
                    score: line.score,
                })
                .collect());
        }
        renderer.state.hud.set_world_border_center([
            self.session.world_border.center_x,
            self.session.world_border.center_z,
        ]);
        renderer.state.hud.set_world_border_diameter(self.session.world_border.diameter);
        renderer.state.hud.set_world_border_warning_blocks(self.session.world_border.warning_blocks);
        if *renderer.state.server_list.server_brand() != self.session.server_brand {
            renderer.state.server_list.server_brand_mut().clone_from(&self.session.server_brand);
        }
        renderer.state.server_list.set_resource_pack_status(self
            .session
            .resource_pack
            .as_ref()
            .map(|pack| format!("{} {}", pack.status, pack.hash)));

        let _ = refresh_static; // used by caller for player list sync
    }

    fn sync_player_list(&mut self, refresh_static: bool) {
        let renderer = self.renderer.as_mut().unwrap();
        if self.last_player_list_generation != self.session.player_list_generation
            || self.config.tab_player_avatars
        {
            self.last_player_list_generation = self.session.player_list_generation;
            let mut rows: Vec<_> = self
                .session
                .player_list
                .values()
                .map(|player| {
                    (
                        self.session.player_display_name(player),
                        player.ping,
                        player.gamemode,
                        self.config.tab_player_avatars.then(|| {
                            self.skin_cache.face_for(
                                Some(&player.uuid),
                                Some(&player.name),
                                player.skin_property.as_deref(),
                            )
                        }),
                    )
                })
                .collect();
            rows.sort_by_cached_key(|entry| entry.0.to_lowercase());
            renderer.state.hud.set_player_list(rows
                .iter()
                .map(|(name, ping, gamemode, _)| (name.clone(), *ping, *gamemode))
                .collect());
            renderer.state.hud.set_player_list_faces(rows
                .into_iter()
                .filter_map(|(_, _, _, face)| face)
                .collect());
        }
        renderer.state.frame_profile.set_entity_count(self.entities.count());
        let sound_event_count = self.audio.played_count();
        if renderer.state.hud.sound_event_count() != sound_event_count {
            renderer.state.hud.set_sound_event_count(sound_event_count);
            renderer.state.hud.set_recent_sounds(self
                .audio
                .recent()
                .rev()
                .take(3)
                .map(|event| format!("{} {:.1}x {:.2}", event.name, event.volume, event.pitch))
                .collect());
        }
        let _ = refresh_static;
    }

    fn sync_entity_billboards(
        &mut self,
        entity_hash: u64,
        refresh_static: bool,
    ) {
        let renderer = self.renderer.as_mut().unwrap();
        // Only rebuild entity billboards when entity state actually changed.
        if entity_hash != self.last_entity_hash {
            self.last_entity_hash = entity_hash;
            renderer.state.frame_profile.set_entity_billboard_generation(
                renderer.state.frame_profile.entity_billboard_generation().wrapping_add(1));
            renderer.state.hud.set_entity_billboards(self
                .entities
                .iter()
                .into_iter()
                .map(|(_, entity)| {
                    entity_billboard(
                        &entity,
                        &self.session,
                        &self.world,
                        self.player_skin_slims
                            .get(&entity.entity_id)
                            .copied()
                            .unwrap_or(false),
                        self.player_cape_ready.contains(&entity.entity_id),
                        self.tick_timer.alpha(),
                    )
                })
                .collect());
        }
        // Light changes when the world around an entity changes (torch placed,
        // block broken, etc.). Re-sampling every frame would invalidate the
        // cached entity mesh hashes and force a full rebuild of every entity,
        // starving the rest of the rendering pipeline and making scoreboard /
        // UI updates feel unresponsive. A once-per-tick update matches vanilla's
        // 20 Hz lightmap refresh and lets the mesh cache stay hit-warm.
        if refresh_static {
            for billboard in renderer.state.hud.entity_billboards_mut() {
                let bx = billboard.position[0].floor() as i32;
                let by = billboard.position[1].floor() as i32;
                let bz = billboard.position[2].floor() as i32;
                let light = self.world.light_at_world(bx, by, bz);
                billboard.sky_light = light.sky;
                billboard.block_light = light.block;
            }
        }
    }

    fn sync_skull_and_particles(
        &mut self,
        skull_hash: Option<u64>,
    ) {
        let renderer = self.renderer.as_mut().unwrap();
        if let Some(skull_hash) = skull_hash {
            if skull_hash != self.last_skull_hash {
                self.last_skull_hash = skull_hash;
                renderer.state.hud.set_skull_entries(self
                    .world
                    .skulls
                    .iter()
                    .map(|(&(x, y, z), skull)| crate::render::SkullRenderEntry {
                        position: [x, y, z],
                        block_metadata: self.world.get_block_metadata(x, y, z),
                        skull_type: skull.skull_type,
                        rotation: skull.rotation,
                        skin_key: skull_skin_key(skull),
                    })
                    .collect());
            }
        }
        if self.config.particles.enabled() {
            renderer.set_particles(self.particles.particles(), self.particles.generation());
        } else {
            renderer.set_particles(&[], self.particles.generation());
        }
    }

    fn sync_local_player_and_hand(
        &mut self,
        _target: &Option<client::interaction::TargetBlock>,
        _refresh_static: bool,
    ) {
        let renderer = self.renderer.as_mut().unwrap();
        if self.player.camera_mode != 0 {
            let mut local_equipment: [Option<(u16, u16)>; 5] = Default::default();
            if let Some(slot) = self.inventory.slots.get(self.inventory.selected) {
                local_equipment[0] = Some((slot.item_id, slot.damage));
            }
            // S04 equipment slots are 1=boots, 2=leggings, 3=chest, 4=helmet.
            for (armor_index, slot) in self.inventory.armor.iter().enumerate() {
                if !slot.is_empty() {
                    local_equipment[4 - armor_index] = Some((slot.item_id, slot.damage));
                }
            }
            let local_billboard = crate::render::EntityBillboard {
                entity_id: i32::MIN,
                position: [
                    self.player.render_position.x,
                    self.player.render_position.y,
                    self.player.render_position.z,
                ],
                sky_light: {
                    let light = self.world.light_at_world(
                        self.player.render_position.x.floor() as i32,
                        self.player.render_position.y.floor() as i32,
                        self.player.render_position.z.floor() as i32,
                    );
                    light.sky
                },
                block_light: {
                    let light = self.world.light_at_world(
                        self.player.render_position.x.floor() as i32,
                        self.player.render_position.y.floor() as i32,
                        self.player.render_position.z.floor() as i32,
                    );
                    light.block
                },
                height: 1.8,
                width: 0.6,
                name: None,
                kind: crate::render::EntityBillboardKind::Player,
                entity_type: crate::entity::EntityType::Player,
                health: Some((self.session.health, 20.0)),
                held_item: local_equipment[0].map(|e| e.0),
                equipment: local_equipment,
                item_id: None,
                item_damage: local_equipment[0].map(|e| e.1),
                item_nbt: None,
                swing_progress: renderer.state.hud.hand_swing_progress(),
                skin_key: Some("player/local".to_string()),
                slim: self.local_skin.slim_arms || self.local_player_model.slim_arms,
                skin_parts_mask: self.session.skin_parts,
                has_cape: self.local_cape_pixels.is_some(),
                cape_rotation: cape_rotation(
                    self.player.render_chasing_position,
                    self.player.render_position,
                    self.player.body_yaw,
                    self.player.prev_distance_walked_modified,
                    self.player.distance_walked_modified,
                    self.player.prev_camera_yaw,
                    self.player.camera_yaw,
                    self.tick_timer.alpha(),
                    self.player.sneaking,
                ),
                yaw: self.player.body_yaw,
                pitch: self.player.camera.mc_pitch_degrees(),
                head_yaw: self.player.camera.mc_yaw_degrees(),
                limb_swing: self.player.limb_swing,
                limb_swing_amount: self.player.limb_swing_amount,
                sneaking: self.player.sneaking,
                blocking: self.item_use_active
                    && matches!(
                        local_equipment[0].map(|item| item.0),
                        Some(267 | 268 | 272 | 276 | 283)
                    ),
                invisible: self
                    .player
                    .active_effects
                    .iter()
                    .any(|effect| effect.effect_id == 14),
                riding: self.player.vehicle_id.is_some(),
                name_visible: false,
                age_ticks: self.tick_timer.tick_count() as f32,
                hover_start: 0.0,
                velocity: [0.0, 0.0, 0.0],
                hurt_alpha: 0.0,
                death_alpha: 0.0,
                swing_alpha: renderer.state.hud.hand_swing_progress(),
                critical_alpha: 0.0,
                visual: crate::entity::EntityVisualState::default(),
            };
            renderer.state.hud.entity_billboards_mut().retain(|billboard| billboard.entity_id != i32::MIN);
            renderer.state.hud.entity_billboards_mut().push(local_billboard.clone());
            renderer.state.hud.set_local_player_billboard(Some(local_billboard));
        } else {
            renderer.state.hud.entity_billboards_mut().retain(|billboard| billboard.entity_id != i32::MIN);
            renderer.state.hud.set_local_player_billboard(None);
        }

        renderer.state.settings.set_camera_mode(self.player.camera_mode);
        renderer.state.hud.set_first_person_arm_yaw(self.player.render_arm_yaw);
        renderer.state.hud.set_first_person_arm_pitch(self.player.render_arm_pitch);
        renderer.state.hud.set_first_person_prev_arm_yaw(self.player.prev_render_arm_yaw);
        renderer.state.hud.set_first_person_prev_arm_pitch(self.player.prev_render_arm_pitch);
        renderer.state.settings.set_local_model_parts(self.local_player_model.parts.len());
        let (skin_w, skin_h) = self.local_skin.dimensions();
        renderer.state.settings.set_local_skin_size([skin_w, skin_h]);
        let new_slim = self.local_skin.slim_arms || self.local_player_model.slim_arms;
        if self.local_skin_dirty {
            self.local_skin_dirty = false;
            renderer.state.settings.set_local_skin_slim(new_slim);
            renderer.state.settings.set_local_skin_face(self.local_skin.face_pixels());
            renderer.state.settings.set_local_skin_preview(self.local_skin.preview_pixels());
            renderer.state.settings.set_local_skin(self.local_skin.clone());
            renderer.update_skin_gpu();
            if let Some(ref pixels) = self.local_cape_pixels {
                renderer.upload_cape_to_atlas(pixels);
            }
        }

        // First-person hand state
        let selected_item = self.inventory.slots[self.inventory.selected];

        // Trigger once on the attack edge, then re-trigger during continuous mining
        if self.attack_held && !self.prev_attack_held {
            renderer.trigger_hand_swing();
        }
        self.prev_attack_held = self.attack_held;

        renderer.update_hand_state(
            selected_item.item_id,
            // Bow icon variants are keyed by damage 1-3 (pulling frames), so
            // durability wear must not leak into the held-item texture.
            if selected_item.item_id == 261 {
                0
            } else {
                selected_item.damage
            },
            self.inventory.slot_meta[self.inventory.selected]
                .nbt
                .clone(),
        );
        // Continuous mining swing: re-trigger when previous swing completes
        if self.attack_held
            && renderer.state.hud.hand_swing_timer() <= 0.0
            && (self.dig.active_pos().is_some()
                || (self.item_use_active
                    && super::block_interaction::is_sword(selected_item.item_id)))
        {
            renderer.trigger_hand_swing();
        }
        let held_id = selected_item.item_id;
        let previous_hand_use_kind = renderer.state.hud.hand_use_kind();
        renderer.state.hud.set_hand_use_kind(if self.item_use_active {
            match held_id {
                267 | 268 | 272 | 276 | 283 => 1, // swords: BLOCK
                373 => 3,                         // potion: DRINK
                261 => 4,                         // bow
                _ => 2,                           // food: EAT
            }
        } else {
            0
        });

        if held_id == 261 && self.item_use_active {
            let pull = (self.item_use_timer / 1.0).min(1.0);
            let bow_damage = if pull >= 0.9 {
                3
            } else if pull >= 0.65 {
                2
            } else {
                1
            };
            renderer.state.hud.set_hand_item_damage(bow_damage);
        } else if held_id == 261 {
            // Not drawing: standby icon. Durability wear must not select a
            // pulling frame (those are keyed by damage 1-3).
            renderer.state.hud.set_hand_item_damage(0);
        }
        let alpha = self.tick_timer.alpha();
        renderer.state.hud.set_hand_use_progress(self.prev_item_use_timer + (self.item_use_timer - self.prev_item_use_timer) * alpha);
        renderer.state.hud.active_potion_effects_mut().clone_from(&self.player.active_effects);
        renderer.state.frame_profile.set_time(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f32());
        let use_kind = renderer.state.hud.hand_use_kind();
        let now = std::time::Instant::now();
        const SCRIPT_ANIMATION_INTERVAL: std::time::Duration =
            std::time::Duration::from_micros(8_333); // 120 Hz
                                                     // A use-action transition must dispatch immediately. Otherwise the
                                                     // render frame that enters/leaves BLOCK can bake a mesh with the
                                                     // previous script matrix and vanilla flags, then visibly snap once the
                                                     // 120 Hz script throttle catches up.
        if renderer.state.hud.hand_use_kind() != previous_hand_use_kind
            || now.duration_since(self.last_script_frame) >= SCRIPT_ANIMATION_INTERVAL
        {
            self.last_script_frame = now;

            // Lua animation callbacks are decoupled from the uncapped world
            // renderer. 120 Hz preserves smooth partial-tick hand animation,
            // while cached matrices can be reused by intervening GPU frames.
            let animation_context = crate::render::first_person::FirstPersonAnimationContext {
                hand: crate::render::first_person::Hand::MainHand,
                item_id: crate::render::first_person::item_resource_id(
                    held_id,
                    selected_item.damage,
                ),
                numeric_item_id: held_id,
                item_type: crate::render::first_person::ItemType::classify(held_id, use_kind),
                use_action: crate::render::first_person::UseAction::from_use_kind(use_kind),
                equip_progress: 1.0 - renderer.hand_equip_progress,
                previous_equip_progress: 1.0 - renderer.hand_equip_progress,
                swing_progress: renderer.state.hud.hand_swing_progress(),
                previous_swing_progress: renderer.state.hud.hand_swing_progress(),
                swinging: renderer.state.hud.hand_swing_timer() > 0.0
                    || renderer.state.hud.hand_swing_progress() > 0.0,
                swing_duration_ticks: 6,
                use_progress: renderer.state.hud.hand_use_progress(),
                use_ticks: (self.item_use_timer.max(0.0) * 20.0).floor() as u32,
                remaining_use_ticks: self.item_use_timer.max(0.0).floor() as u32,
                max_use_ticks: match use_kind {
                    1 => 0,  // block is indefinite
                    2 => 32, // eat ~32 ticks (1.6s)
                    3 => 32, // drink ~32 ticks
                    4 => 72, // bow charge
                    _ => 0,
                },
                attack_cooldown: 1.0,
                using_item: use_kind != 0,
                blocking: use_kind == 1,
                attack_pressed: self.attack_held && !self.prev_attack_held,
                attack_held: self.attack_held,
                use_pressed: self.use_held,
                use_held: self.use_held,
                sneaking: self.player.sneaking,
                yaw: self.player.camera.mc_yaw_degrees(),
                pitch: self.player.camera.mc_pitch_degrees(),
                partial_tick: alpha,
                fov: self.player.camera.fov,
                aspect_ratio: self.player.camera.aspect,
            };
            let animation_transforms = self.scripts.dispatch_first_person(&animation_context);
            renderer.state.hud.set_first_person_arm_transform(animation_transforms.combined_arm());
            renderer.state.hud.set_first_person_item_transform(animation_transforms.combined_item());
            renderer.state.hud.set_fp_vanilla_flags(animation_transforms.vanilla_flags.clone());
        }

        const SCRIPT_HUD_INTERVAL: std::time::Duration = std::time::Duration::from_micros(16_667); // 60 Hz
        if !self.hud_hidden && now.duration_since(self.last_script_hud_frame) >= SCRIPT_HUD_INTERVAL
        {
            let script_delta = now
                .duration_since(self.last_script_hud_frame)
                .as_secs_f32()
                .min(0.25);
            self.last_script_hud_frame = now;

            // HUD input indicators remain responsive at display refresh speed,
            // while expensive Lua text/layout work is not repeated at 1000 Hz.
            let viewport = self
                .window
                .as_ref()
                .map(|window| window.inner_size())
                .unwrap_or(winit::dpi::PhysicalSize::new(1280, 720));
            let script_frame = crate::render::hooks::ScriptFrameContext {
                delta_time: script_delta,
                viewport_width: viewport.width,
                viewport_height: viewport.height,
            };
            renderer.state.hud.set_script_hud_before_commands(self
                .scripts
                .dispatch_render("render.hud.before", script_frame));
            let remaining_commands =
                4096usize.saturating_sub(renderer.state.hud.script_hud_before_commands().len());
            renderer.state.hud.set_script_hud_commands(self
                .scripts
                .dispatch_render("render.hud.after", script_frame));
            renderer.state.hud.script_hud_commands_mut().truncate(remaining_commands);
        } else if self.hud_hidden
            && (!renderer.state.hud.script_hud_before_commands().is_empty()
                || !renderer.state.hud.script_hud_commands().is_empty())
        {
            // Hiding the vanilla HUD must also hide mod-rendered HUD commands,
            // and avoids retaining a stale draw list until F1 is pressed again.
            renderer.state.hud.script_hud_before_commands_mut().clear();
            renderer.state.hud.script_hud_commands_mut().clear();
        }
    }

    fn sync_inventory_slots(&mut self) {
        if !self.inventory_open {
            return;
        }
        let renderer = self.renderer.as_mut().unwrap();
        for i in 0..36 {
            renderer.state.inventory.inventory_slots_mut()[i] =
                self.inventory.slots[i].view_with_meta(Some(&self.inventory.slot_meta[i]));
        }
        for i in 0..4 {
            renderer.state.inventory.inventory_armor_slots_mut()[i] =
                self.inventory.armor[i].view_with_meta(Some(&self.inventory.armor_meta[i]));
        }
        for i in 0..5 {
            renderer.state.inventory.inventory_crafting_slots_mut()[i] = self.inventory.crafting[i]
                .view_with_meta(Some(&self.inventory.crafting_meta[i]));
        }
        renderer.state.inventory.set_inventory_cursor_slot(self
            .inventory
            .cursor
            .view_with_meta(Some(&self.inventory.cursor_meta)));
    }

    fn collect_pending_player_skins(
        &mut self,
        skull_hash: u64,
    ) -> Option<(Vec<crate::render::PendingPlayerSkin>, u64, u64)> {
        let cache_generation = self.skin_cache.poll_content_generation();
        let roster_hash = self.compute_player_skin_roster_hash();
        let skin_generation = self.session.player_skin_generation;
        if self.last_player_skin_roster_hash == roster_hash
            && self.last_player_skin_generation == skin_generation
            && self.last_player_skin_skull_hash == skull_hash
            && self.last_skin_cache_generation == cache_generation
        {
            return None;
        }

        self.last_player_skin_roster_hash = roster_hash;
        self.last_player_skin_generation = skin_generation;
        self.last_player_skin_skull_hash = skull_hash;
        self.last_skin_cache_generation = cache_generation;

        let mut pending = Vec::new();
        self.player_skin_slims.clear();
        self.player_cape_ready.clear();
        pending.push(crate::render::PendingPlayerSkin {
            key: "player/local".to_string(),
            skin: std::sync::Arc::new(self.local_skin.clone()),
            content_hash: crate::client::skin_cache::skin_content_hash(&self.local_skin),
            cape_pixels: self
                .local_cape_pixels
                .as_ref()
                .map(|pixels| std::sync::Arc::new(pixels.clone())),
            cape_content_hash: self.local_cape_hash,
        });
        let session = &self.session;
        let skin_cache = &mut self.skin_cache;
        let skin_profiles = &mut self.player_skin_profiles;
        let entities = &self.entities;

        for (_, entity) in entities.iter() {
            let crate::entity::EntityData::Player {
                name,
                skin_property: entity_skin_property,
                ..
            } = &entity.data
            else {
                continue;
            };
            let identity = if let Some(uuid) = entity.uuid.as_deref() {
                format!("uuid:{uuid}")
            } else {
                format!("name:{name}")
            };
            let listed_profile = entity
                .uuid
                .as_deref()
                .and_then(|uuid| session.player_list.get(uuid));
            let listed_property = listed_profile.map(|player| player.skin_property.as_deref());
            let profile = skin_profiles.entry(entity.entity_id).or_insert_with(|| {
                super::PlayerSkinProfileCache::new(
                    identity.clone(),
                    listed_property,
                    entity_skin_property.as_deref(),
                )
            });
            profile.update(identity, listed_property, entity_skin_property.as_deref());
            let snapshot = skin_cache.snapshot_for(
                entity.uuid.as_deref(),
                Some(name),
                profile.skin_property.as_deref(),
            );
            self.player_skin_slims
                .insert(entity.entity_id, snapshot.slim);
            let cape = skin_cache.cape_for_property(profile.skin_property.as_deref());
            if cape.is_some() {
                self.player_cape_ready.insert(entity.entity_id);
            }
            pending.push(crate::render::PendingPlayerSkin {
                key: player_skin_key(&entity),
                skin: std::sync::Arc::clone(&snapshot.skin),
                content_hash: snapshot.content_hash,
                cape_pixels: cape
                    .as_ref()
                    .map(|cape| std::sync::Arc::clone(&cape.pixels)),
                cape_content_hash: cape.as_ref().map_or(0, |cape| cape.content_hash),
            });
        }
        skin_profiles.retain(|entity_id, _| {
            entities
                .get(*entity_id)
                .is_some_and(|entity| matches!(&entity.data, crate::entity::EntityData::Player { .. }))
        });

        for skull in self
            .world
            .skulls
            .values()
            .filter(|skull| skull.skull_type == 3)
        {
            let snapshot = skin_cache.snapshot_for(
                skull.owner_uuid.as_deref(),
                skull.owner_name.as_deref(),
                skull.skin_property.as_deref(),
            );
            pending.push(crate::render::PendingPlayerSkin {
                key: skull_skin_key(skull),
                skin: std::sync::Arc::clone(&snapshot.skin),
                content_hash: snapshot.content_hash,
                cape_pixels: None,
                cape_content_hash: 0,
            });
        }

        pending.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.content_hash.cmp(&b.content_hash))
        });
        pending.dedup_by(|a, b| a.key == b.key);
        pending.truncate(crate::render::entity::atlas::PLAYER_SKIN_CAPACITY);

        use std::hash::Hasher;
        let mut layout_hasher = fnv::FnvHasher::default();
        let mut content_hasher = fnv::FnvHasher::default();
        layout_hasher.write_usize(pending.len());
        content_hasher.write_usize(pending.len());
        for skin in &pending {
            layout_hasher.write(skin.key.as_bytes());
            layout_hasher.write_u8(0);
            let (width, height) = skin.skin.dimensions();
            layout_hasher.write_u32(width);
            layout_hasher.write_u32(height);
            content_hasher.write(skin.key.as_bytes());
            content_hasher.write_u8(0);
            content_hasher.write_u64(skin.content_hash);
            content_hasher.write_u64(skin.cape_content_hash);
        }

        Some((pending, content_hasher.finish(), layout_hasher.finish()))
    }

    fn compute_player_skin_roster_hash(&self) -> u64 {
        use std::hash::Hasher;
        let mut sum = 0u64;
        let mut xor = 0u64;
        let mut count = 0usize;

        for (_, entity) in self.entities.iter() {
            let crate::entity::EntityData::Player { name, .. } = &entity.data else {
                continue;
            };
            let mut entry = fnv::FnvHasher::default();
            entry.write_i32(entity.entity_id);
            entry.write_u8(entity.uuid.is_some() as u8);
            entry.write(entity.uuid.as_deref().unwrap_or(name).as_bytes());
            let value = entry.finish();
            sum = sum.wrapping_add(value);
            xor ^= value.rotate_left((value >> 58) as u32);
            count += 1;
        }

        let mut hasher = fnv::FnvHasher::default();
        hasher.write_usize(count);
        hasher.write_u64(sum);
        hasher.write_u64(xor);
        hasher.finish()
    }

    fn compute_skull_hash(&self) -> u64 {
        use std::hash::Hasher;
        let mut sum = 0u64;
        let mut xor = 0u64;
        for (&(x, y, z), skull) in &self.world.skulls {
            let mut entry = fnv::FnvHasher::default();
            entry.write_i32(x);
            entry.write_i32(y);
            entry.write_i32(z);
            entry.write_u16(self.world.get_block_state(x, y, z));
            entry.write_u8(skull.skull_type);
            entry.write_u8(skull.rotation);
            if skull.skull_type == 3 {
                if let Some(owner) = skull.owner_uuid.as_deref().or(skull.owner_name.as_deref()) {
                    entry.write(owner.as_bytes());
                }
                entry.write_u8(0);
                if let Some(property) = skull.skin_property.as_deref() {
                    entry.write(property.as_bytes());
                }
            }
            let value = entry.finish();
            sum = sum.wrapping_add(value);
            xor ^= value.rotate_left((value >> 58) as u32);
        }

        let mut hasher = fnv::FnvHasher::default();
        hasher.write_usize(self.world.skulls.len());
        hasher.write_u64(sum);
        hasher.write_u64(xor);
        hasher.finish()
    }

    fn compute_entity_hash(&self) -> u64 {
        use std::hash::Hasher;
        let mut h = fnv::FnvHasher::default();
        h.write_usize(self.entities.count());
        h.write_u64(self.session.player_profile_generation);
        for (_, entity) in self.entities.iter() {
            h.write_i32(entity.entity_id);
            h.write_u32(entity.entity_type as u32);
            h.write_i64((entity.render_position.x * 20.0) as i64);
            h.write_i64((entity.render_position.y * 20.0) as i64);
            h.write_i64((entity.render_position.z * 20.0) as i64);
            h.write_i64((entity.render_chasing_position.x * 100.0) as i64);
            h.write_i64((entity.render_chasing_position.y * 100.0) as i64);
            h.write_i64((entity.render_chasing_position.z * 100.0) as i64);
            h.write_i32((entity.distance_walked_modified * 100.0) as i32);
            h.write_i32((entity.camera_yaw * 1000.0) as i32);
            h.write_i32((entity.yaw * 10.0) as i32);
            h.write_i32((entity.body_yaw * 10.0) as i32);
            h.write_i32((entity.head_yaw * 10.0) as i32);
            h.write_i32((entity.pitch * 10.0) as i32);
            h.write_u32((entity.limb_swing * 20.0) as u32);
            h.write_u32((entity.limb_swing_amount * 20.0) as u32);
            h.write_u32((entity.hurt_time * 5.0) as u32);
            h.write_u32(entity.metadata.len() as u32);
            // Visual variants drive atlas_name_for_entity (wolf angry, horse
            // color, villager profession, …). Omitting them left billboards
            // stale after EntityMetadata packets and scrambled skins.
            h.write_u32(entity.visual.is_child as u32);
            h.write_u8(entity.visual.skeleton_type);
            h.write_u32(entity.visual.zombie_villager as u32);
            h.write_u32(entity.visual.zombie_converting as u32);
            h.write_u32(entity.visual.wolf_tamed as u32);
            h.write_u32(entity.visual.wolf_angry as u32);
            h.write_u8(entity.visual.wolf_collar);
            h.write_u8(entity.visual.ocelot_skin);
            h.write_u8(entity.visual.horse_type);
            h.write_u32(entity.visual.horse_variant);
            h.write_u8(entity.visual.horse_armor);
            h.write_u32(entity.visual.guardian_elder as u32);
            h.write_u8(entity.visual.slime_size);
            h.write_u8(entity.visual.villager_profession);
            h.write_u8(entity.visual.sheep_color);
            h.write_u8(entity.visual.rabbit_type);
            h.write_u32(entity.visual.pig_saddled as u32);
            h.write_u32(entity.visual.creeper_charged as u32);
            h.write_u8(entity.visual.armor_stand_flags);
            h.write_u8(entity.skin_parts);
            for slot in &entity.equipment {
                match slot {
                    Some(s) => {
                        h.write_u8(1);
                        h.write_i16(s.item_id);
                        h.write_i16(s.damage);
                    }
                    None => h.write_u8(0),
                }
            }
            h.write_u32(
                self.player_skin_slims
                    .get(&entity.entity_id)
                    .copied()
                    .unwrap_or(false) as u32,
            );
            h.write_u32(self.player_cape_ready.contains(&entity.entity_id) as u32);
            if let crate::entity::EntityData::Player { name, .. } = &entity.data {
                if let Some(uuid) = entity.uuid.as_deref() {
                    h.write(uuid.as_bytes());
                }
                h.write_u8(0);
                h.write(name.as_bytes());
                h.write_u8(0);
            }
        }
        h.finish()
    }
}

impl App {
    /// Play the local climbing cadence from vanilla's ladder/vine sound type.
    /// Server movement packets do not carry this client-side movement sound.
    fn tick_climbing_sound(&mut self) {
        let x = self.player.position.x.floor() as i32;
        let y = self.player.position.y.floor() as i32;
        let z = self.player.position.z.floor() as i32;
        let block = self.world.get_block(x, y, z);
        let climbing = matches!(
            block,
            crate::world::block::Block::Ladder | crate::world::block::Block::Vine
        );
        if self.ladder_sound_cooldown > 0 {
            self.ladder_sound_cooldown -= 1;
        }
        if climbing && self.player.velocity.y.abs() > 0.01 && self.ladder_sound_cooldown == 0 {
            let sound = block.sound_type();
            self.audio.play(crate::audio::SoundEvent {
                name: sound.step_event().to_string(),
                category: crate::audio::SoundCategory::Players,
                volume: sound.volume() * 0.15,
                pitch: sound.pitch(),
                // Player-local movement sounds use attenuation NONE in vanilla
                // PositionedSoundRecord, so they must remain centred.
                position: None,
            });
            self.ladder_sound_cooldown = 4;
        }
    }
}

fn entity_billboard(
    entity: &crate::entity::Entity,
    session: &crate::client::session::SessionState,
    world: &crate::world::World,
    slim: bool,
    cape_ready: bool,
    partial_tick: f32,
) -> crate::render::EntityBillboard {
    use crate::entity::{EntityData, EntityType};
    let (width, height) = entity.entity_type.bounding_box();
    let kind = match entity.entity_type {
        EntityType::Player => crate::render::EntityBillboardKind::Player,
        EntityType::ArmorStand => crate::render::EntityBillboardKind::Other,
        EntityType::Item => crate::render::EntityBillboardKind::Item,
        EntityType::XPOrb => crate::render::EntityBillboardKind::XpOrb,
        EntityType::Arrow
        | EntityType::Snowball
        | EntityType::ThrownEgg
        | EntityType::Fireball
        | EntityType::SmallFireball
        | EntityType::EnderPearl
        | EntityType::ThrownPotion
        | EntityType::ThrownExpBottle
        | EntityType::EnderEye
        | EntityType::WitherSkull
        | EntityType::FireworkRocket => crate::render::EntityBillboardKind::Projectile,
        ty if ty.is_mob() && !ty.is_passive() => crate::render::EntityBillboardKind::Hostile,
        ty if ty.is_passive() => crate::render::EntityBillboardKind::Passive,
        _ => crate::render::EntityBillboardKind::Other,
    };

    let custom_name = entity.metadata.iter().find_map(|entry| {
        (entry.index == 2)
            .then_some(&entry.value)
            .and_then(|value| match value {
                crate::net::metadata::MetadataValue::String(name) if !name.is_empty() => {
                    Some(name.clone())
                }
                _ => None,
            })
    });
    let (name, health) = match &entity.data {
        EntityData::Player { name, .. } => {
            let display = entity
                .uuid
                .as_deref()
                .and_then(|uuid| session.player_list.get(uuid))
                .map(|player| session.player_display_name(player))
                .unwrap_or_else(|| session.decorate_player_name(name));
            (Some(display), None)
        }
        EntityData::Mob { health, max_health }
        | EntityData::Living {
            health, max_health, ..
        } => (custom_name.clone(), Some((*health, *max_health))),
        EntityData::Item {
            item_id,
            count,
            damage,
            ..
        } => (Some(format!("{}x{}:{}", count, item_id, damage)), None),
        EntityData::XPOrb { value } => (Some(format!("{} XP", value)), None),
        EntityData::None => (custom_name.clone(), None),
    };
    let skin_key =
        matches!(entity.entity_type, EntityType::Player).then(|| player_skin_key(entity));

    let base_invisible = entity.metadata.iter().any(|entry| {
        entry.index == 0
            && matches!(entry.value, crate::net::metadata::MetadataValue::Byte(flags) if flags as u8 & 0x20 != 0)
    });
    let always_render_name = entity.metadata.iter().any(|entry| {
        entry.index == 3
            && matches!(entry.value, crate::net::metadata::MetadataValue::Byte(value) if value != 0)
    });
    let name_visible = match &entity.data {
        EntityData::Player { name: _name, .. } => {
            !base_invisible
                && !entity.active_effects.iter().any(|e| e.effect_id == 14)
                && entity.vehicle_id.is_none()
        }
        _ => custom_name.is_some() && always_render_name,
    };

    let light = world.light_at_world(
        entity.render_position.x.floor() as i32,
        entity.render_position.y.floor() as i32,
        entity.render_position.z.floor() as i32,
    );
    crate::render::EntityBillboard {
        entity_id: entity.entity_id,
        position: [
            entity.render_position.x,
            entity.render_position.y,
            entity.render_position.z,
        ],
        sky_light: light.sky,
        block_light: light.block,
        height,
        width,
        name,
        kind,
        entity_type: entity.entity_type,
        health,
        held_item: entity.current_item.map(|id| id.max(0) as u16),
        equipment: {
            let mut eq: [Option<(u16, u16)>; 5] = Default::default();
            for (i, slot) in entity.equipment.iter().enumerate().take(5) {
                eq[i] = slot
                    .as_ref()
                    .map(|s| (s.item_id.max(0) as u16, s.damage.max(0) as u16));
            }
            eq
        },
        item_id: if let crate::entity::EntityData::Item { item_id, .. } = &entity.data {
            Some(*item_id)
        } else {
            None
        },
        item_damage: if let crate::entity::EntityData::Item { damage, .. } = &entity.data {
            Some(*damage)
        } else {
            None
        },
        item_nbt: if let crate::entity::EntityData::Item { nbt, .. } = &entity.data {
            nbt.clone()
        } else {
            None
        },
        swing_progress: (entity.swing_time / 0.35).clamp(0.0, 1.0),
        skin_key,
        slim,
        skin_parts_mask: entity.skin_parts,
        has_cape: cape_ready,
        cape_rotation: cape_rotation(
            entity.render_chasing_position,
            entity.render_position,
            entity.body_yaw,
            entity.prev_distance_walked_modified,
            entity.distance_walked_modified,
            entity.prev_camera_yaw,
            entity.camera_yaw,
            1.0,
            entity.metadata.iter().any(|entry| {
                entry.index == 0
                    && matches!(entry.value, crate::net::metadata::MetadataValue::Byte(flags) if flags as u8 & 0x02 != 0)
            }),
        ),
        yaw: entity.body_yaw,
        pitch: entity.pitch,
        head_yaw: entity.head_yaw,
        limb_swing: entity.prev_limb_swing
            + (entity.limb_swing - entity.prev_limb_swing) * partial_tick,
        limb_swing_amount: entity.prev_limb_swing_amount
            + (entity.limb_swing_amount - entity.prev_limb_swing_amount) * partial_tick,
        sneaking: entity.metadata.iter().any(|entry| {
            entry.index == 0
                && matches!(entry.value, crate::net::metadata::MetadataValue::Byte(flags) if flags as u8 & 0x02 != 0)
        }),
        blocking: entity.using_item
            && matches!(entity.current_item, Some(267 | 268 | 272 | 276 | 283)),
        invisible: base_invisible
            || entity.active_effects.iter().any(|effect| effect.effect_id == 14),
        riding: entity.vehicle_id.is_some(),
        name_visible,
        age_ticks: entity.ticks_alive as f32,
        hover_start: entity.hover_start(),
        velocity: [entity.velocity.x, entity.velocity.y, entity.velocity.z],
        hurt_alpha: (entity.hurt_time / 0.45).clamp(0.0, 1.0),
        // `death_time` counts down, while the model needs elapsed time. Status
        // 3 is terminal, so retain the completed corpse pose until the server
        // finally destroys the tracked entity.
        death_alpha: (entity.last_status == Some(3))
            .then(|| 1.0 - entity.death_time.clamp(0.0, 1.0))
            .unwrap_or(0.0),
        swing_alpha: (entity.swing_time / 0.35).clamp(0.0, 1.0),
        critical_alpha: (entity.critical_time / 0.45).clamp(0.0, 1.0),
        visual: entity.visual,
    }
}
fn player_skin_key(entity: &crate::entity::Entity) -> String {
    let name = match &entity.data {
        crate::entity::EntityData::Player { name, .. } => Some(name.as_str()),
        _ => None,
    };
    format!(
        "player/{}",
        entity.uuid.as_deref().or(name).unwrap_or("default")
    )
}

#[allow(clippy::too_many_arguments)]
fn cape_rotation(
    chasing: nalgebra::Point3<f32>,
    position: nalgebra::Point3<f32>,
    body_yaw: f32,
    prev_distance: f32,
    distance: f32,
    prev_camera_yaw: f32,
    camera_yaw: f32,
    partial_tick: f32,
    sneaking: bool,
) -> [f32; 3] {
    let delta = chasing - position;
    let yaw = body_yaw.to_radians();
    let sin_yaw = yaw.sin();
    let neg_cos_yaw = -yaw.cos();
    let vertical = (delta.y * 10.0).clamp(-6.0, 32.0);
    let forward = ((delta.x * sin_yaw + delta.z * neg_cos_yaw) * 100.0).max(0.0);
    let sideways = (delta.x * neg_cos_yaw - delta.z * sin_yaw) * 100.0;
    let walk_distance = prev_distance + (distance - prev_distance) * partial_tick;
    let camera_yaw = prev_camera_yaw + (camera_yaw - prev_camera_yaw) * partial_tick;
    let walk = (walk_distance * 6.0).sin() * 32.0 * camera_yaw;
    let x = 6.0 + forward * 0.5 + vertical + walk + if sneaking { 25.0 } else { 0.0 };

    [
        -x.to_radians(),
        std::f32::consts::PI - (sideways * 0.5).to_radians(),
        -(sideways * 0.5).to_radians(),
    ]
}

fn skull_skin_key(skull: &crate::world::SkullBlockEntity) -> String {
    if skull.skull_type == 3 {
        format!(
            "player/skull/{}/{}",
            skull
                .owner_uuid
                .as_deref()
                .or(skull.owner_name.as_deref())
                .unwrap_or("default"),
            skull
                .skin_property
                .as_deref()
                .map(stable_short_hash)
                .unwrap_or(0)
        )
    } else {
        match skull.skull_type {
            1 => "wither_skeleton",
            2 => "zombie",
            4 => "creeper",
            _ => "skeleton",
        }
        .to_string()
    }
}

fn stable_short_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn mod_config_row(entry: crate::scripting::ConfigEntrySnapshot) -> crate::render::ModConfigRow {
    use crate::scripting::{ConfigEntryKind, ConfigValue};

    let is_default = entry.value == entry.default_value;
    let (value, can_previous, can_next) = match (&entry.kind, &entry.value) {
        (ConfigEntryKind::Boolean, ConfigValue::Boolean(value)) => {
            (if *value { "On" } else { "Off" }.to_string(), true, true)
        }
        (ConfigEntryKind::Number { min, max, .. }, ConfigValue::Number(value)) => (
            format_config_number(*value),
            *value > *min + f64::EPSILON,
            *value < *max - f64::EPSILON,
        ),
        (ConfigEntryKind::Choice { options }, ConfigValue::Choice(value)) => {
            let label = options
                .iter()
                .find(|option| option.value == *value)
                .map(|option| option.label.clone())
                .unwrap_or_else(|| value.clone());
            (label, options.len() > 1, options.len() > 1)
        }
        _ => ("Invalid value".to_string(), false, false),
    };
    crate::render::ModConfigRow {
        key: entry.key,
        label: entry.label,
        description: entry.description,
        value,
        is_default,
        can_previous,
        can_next,
    }
}

fn format_config_number(value: f64) -> String {
    let formatted = format!("{value:.4}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

impl App {
    /// Reload the local player's skin from the current authenticated account.
    pub(super) fn update_local_skin(&mut self) {
        self.local_skin_dirty = true;
        self.last_skin_cache_generation = u64::MAX;
        if let Some(ref account) = self.account {
            let uuid_key = account
                .uuid
                .as_deref()
                .unwrap_or("default")
                .replace('-', "");
            let skin_info = account.skins.as_ref().and_then(|s| s.first());
            let texture_key = skin_info.and_then(|s| s.url.rsplit('/').next());
            let skin_url = skin_info.map(|s| s.url.as_str());
            let is_slim = skin_info.map_or(false, |s| s.variant == "SLIM");
            self.local_player_model.slim_arms = is_slim;

            // Load cape pixels for atlas upload
            if let Some(cape) = load_local_cape(account) {
                let raw = cape;
                let mut h = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash_slice(&raw, &mut h);
                let hash = std::hash::Hasher::finish(&h);
                if self.local_cape_hash != hash {
                    self.local_cape_hash = hash;
                    self.local_cape_pixels = Some(raw);
                }
            }

            if let Some(tk) = texture_key {
                let path = format!("assets/skins/{}/{}.png", uuid_key, tk);
                if let Ok(skin) = crate::assets::skin::PlayerSkin::load(&path) {
                    self.local_skin = skin;
                    return;
                }
                // Fallback old format
                let old_path = format!("assets/skins/{}.png", tk);
                if let Ok(skin) = crate::assets::skin::PlayerSkin::load(&old_path) {
                    self.local_skin = skin;
                    return;
                }
                if let Some(url) = skin_url {
                    if let Ok(resp) = reqwest::blocking::get(url) {
                        if let Ok(bytes) = resp.bytes() {
                            let parent_dir = std::path::Path::new(&path)
                                .parent()
                                .unwrap_or_else(|| std::path::Path::new("assets/skins"));
                            let _ = std::fs::create_dir_all(parent_dir);
                            let _ = std::fs::write(&path, &bytes);
                            if let Ok(skin) = crate::assets::skin::PlayerSkin::load(&path) {
                                self.local_skin = skin;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Download and cache the first cape PNG for an account, returning raw RGBA pixels.
fn load_local_cape(account: &crate::auth::models::Account) -> Option<Vec<u8>> {
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
