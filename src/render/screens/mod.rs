use super::gui::widgets::MenuMetrics;
use super::gui::GuiVertexBuilder;
use super::Renderer;
use crate::ui::button_ids as btn;
use crate::ui::format::format_text;

mod alt_manager;
mod controls;
mod main_menu;
mod mod_config;
mod modding;
mod multiplayer;
mod resource_packs;
mod scroll_list;
mod shader_packs;

impl Renderer {
    pub(super) fn draw_menu_screen(
        &mut self,
        menu: u32,
        in_world: bool,
        metrics: &MenuMetrics,
        overlay_gui: &mut GuiVertexBuilder,
        background_gui: &mut GuiVertexBuilder,
        widget_gui: &mut GuiVertexBuilder,
        inventory_gui: &mut GuiVertexBuilder,
        generic54_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
        block_gui: &mut GuiVertexBuilder,
        item_gui: &mut GuiVertexBuilder,
        icons_gui: &mut GuiVertexBuilder,
        creative_gui: &mut GuiVertexBuilder,
    ) {
        match (menu, in_world) {
            (0 | 1, _) => {}
            (7 | 13, true) => draw_default_background(metrics, background_gui),
            (_, true) => draw_world_overlay_background(metrics, overlay_gui),
            (_, false) => draw_default_background(metrics, background_gui),
        }
        match menu {
            0 => self.draw_main_menu(metrics, widget_gui, font_gui),
            1 => self.draw_ingame_hud(
                metrics,
                super::hud::HudBatches {
                    overlay: overlay_gui,
                    widget: widget_gui,
                    inventory: inventory_gui,
                    generic54: generic54_gui,
                    font: font_gui,
                    block: block_gui,
                    item: item_gui,
                    icons: icons_gui,
                    creative: creative_gui,
                },
            ),
            2 => self.draw_pause_menu(metrics, widget_gui, font_gui),
            3 => self.draw_options_menu(metrics, in_world, widget_gui, font_gui),
            4 => self.draw_multiplayer_menu(metrics, background_gui, widget_gui, font_gui),
            5 => self.draw_direct_connect_menu(metrics, background_gui, widget_gui, font_gui),
            6 => self.draw_video_settings_menu(metrics, widget_gui, font_gui),
            7 => self.draw_controls_menu(metrics, background_gui, widget_gui, font_gui),
            8 => self.draw_language_menu(metrics, widget_gui, font_gui),
            9 => self.draw_audio_menu(metrics, widget_gui, font_gui),
            10 => self.draw_skin_customization_menu(metrics, widget_gui, font_gui),
            11 => self.draw_connecting_screen(metrics, font_gui),
            12 => self.draw_loading_world_screen(metrics, font_gui),
            13 => self.draw_resource_packs_screen(metrics, widget_gui, font_gui),
            14 => self.draw_disconnected_screen(metrics, widget_gui, font_gui),
            15 => self.draw_server_editor_menu(metrics, background_gui, widget_gui, font_gui),
            16 => self.draw_alt_manager_screen(metrics, widget_gui, font_gui),
            17 => self.draw_modding_screen(metrics, widget_gui, font_gui),
            18 => self.draw_mod_config_screen(metrics, overlay_gui, widget_gui, font_gui),
            19 => self.draw_shader_packs_screen(metrics, widget_gui, font_gui),
            20 => self.draw_chat_settings_menu(metrics, widget_gui, font_gui),
            _ => {}
        }
    }

    fn draw_connecting_screen(&mut self, metrics: &MenuMetrics, font_gui: &mut GuiVertexBuilder) {
        let text = self.state.settings.ui_text();
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;

        // "Connecting to the server..."
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            sh / 2.0 - 20.0 * gs,
            text.get("multiplayer.connecting"),
            14.0 * gs,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );

        // Server address
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            sh / 2.0 + 10.0 * gs,
            &self.state.server_list.server_address(),
            metrics.font_sz * 0.75,
            [0.65, 0.65, 0.65, 1.0],
        );

        // Animated dots
        let t = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 500)
            % 4;
        let dots = ".".repeat(t as usize);
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            sh / 2.0 + 30.0 * gs,
            &dots,
            metrics.font_sz * 0.65,
            [0.55, 0.55, 0.55, 1.0],
        );
    }

    fn draw_loading_world_screen(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let text = self.state.settings.ui_text();
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;

        // Vanilla GuiDownloadTerrain only shows this centered status message; it
        // deliberately does not expose chunk-count progress or the server address.
        let loading_text = text.get("multiplayer.loadingWorld");
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            sh / 2.0 - 50.0 * gs,
            loading_text,
            14.0 * gs,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );
    }

    fn draw_disconnected_screen(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let text = self.state.settings.ui_text();
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;

        // Title
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            sh / 4.0 - 10.0 * gs,
            text.get("rustcraft.connection.disconnected"),
            18.0 * gs,
            [1.0, 0.3, 0.3, 1.0],
            gs,
        );

        // Disconnect reason
        let reason = &self.state.account.connection_status();
        if !reason.is_empty() {
            font_gui.draw_text_centered(
                &mut self.font,
                sw / 2.0,
                sh / 4.0 + 20.0 * gs,
                reason,
                metrics.font_sz * 0.85,
                [0.8, 0.8, 0.8, 1.0],
            );
        }

        // "Back to Title Screen" button
        let y = sh / 2.0 + 10.0 * gs;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::BACK_TO_GAME,
            [metrics.btn_x, y, metrics.btn_w, metrics.btn_h],
            text.get("menu.returnToMenu"),
        );
    }

    fn draw_pause_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let text = self.state.settings.ui_text();
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        // font_gui.fill_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.7]); // Removed to let panorama show
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            sh / 4.0 - 30.0 * gs,
            text.get("menu.game"),
            18.0 * gs,
            [1.0, 1.0, 1.0, 1.0],
            gs,
        );

        // Match GuiIngameMenu's two-column pause-menu rhythm. Entries unsupported
        // by this multiplayer-only client remain visible but disabled.
        let y = sh / 4.0 + 8.0 * gs;
        let half = (metrics.btn_w - 4.0 * gs) / 2.0;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::BACK_TO_GAME,
            [metrics.btn_x, y, metrics.btn_w, metrics.btn_h],
            text.get("menu.returnToGame"),
        );
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            0,
            [metrics.btn_x, y + 24.0 * gs, half, metrics.btn_h],
            text.get("gui.achievements"),
            false,
        );
        draw_button_enabled(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            0,
            [
                metrics.btn_x + half + 4.0 * gs,
                y + 24.0 * gs,
                half,
                metrics.btn_h,
            ],
            text.get("gui.stats"),
            false,
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::PAUSE_OPTIONS,
            [metrics.btn_x, y + 48.0 * gs, half, metrics.btn_h],
            text.get("menu.options"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::MODDING,
            [
                metrics.btn_x + half + 4.0 * gs,
                y + 48.0 * gs,
                half,
                metrics.btn_h,
            ],
            "Modding",
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DISCONNECT,
            [metrics.btn_x, y + 72.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("menu.disconnect"),
        );
    }

    fn draw_options_menu(
        &mut self,
        metrics: &MenuMetrics,
        in_world: bool,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        self.draw_standard_screen(
            metrics,
            font_gui,
            "options.title",
            sh / 4.0 - 30.0 * gs,
            24.0 * gs,
        );
        let text = self.state.settings.ui_text();

        let left_x = sw / 2.0 - metrics.btn_w - 4.0 * gs;
        let right_x = sw / 2.0 + 4.0 * gs;
        let row_y = sh / 4.0 + 20.0 * gs;
        let rows = [
            (
                (btn::VIDEO_SETTINGS, text.get("options.video")),
                (btn::CONTROLS, text.get("options.controls")),
            ),
            (
                (btn::LANGUAGE, text.get("options.language")),
                (btn::AUDIO_SETTINGS, text.get("options.sounds")),
            ),
            (
                (btn::CHAT_SETTINGS, text.get("options.chat.title")),
                (0, ""),
            ),
            (
                (
                    btn::SKIN_CUSTOMIZATION,
                    text.get("options.skinCustomisation"),
                ),
                (0, ""),
            ),
            (
                (btn::RESOURCE_PACKS, text.get("resourcePack.title")),
                (0, ""),
            ),
        ];
        for (row, (left, right)) in rows.iter().enumerate() {
            let y = row_y + row as f32 * (metrics.btn_h + metrics.btn_gap);
            draw_button(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                left.0,
                [left_x, y, metrics.btn_w, metrics.btn_h],
                left.1,
            );
            if right.0 != 0 {
                draw_button(
                    &mut self.font,
                    metrics,
                    widget_gui,
                    font_gui,
                    right.0,
                    [right_x, y, metrics.btn_w, metrics.btn_h],
                    right.1,
                );
            }
        }
        if in_world {
            let label = format!(
                "{}: {}",
                text.get("options.difficulty"),
                difficulty_label(&text, self.state.settings.difficulty())
            );
            // Multiplayer difficulty is supplied by the server. Vanilla displays it
            // in GuiOptions but disables the control outside singleplayer.
            draw_button_enabled(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                0,
                [
                    right_x,
                    row_y + 3.0 * (metrics.btn_h + metrics.btn_gap),
                    metrics.btn_w,
                    metrics.btn_h,
                ],
                &label,
                false,
            );
        }
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [
                metrics.btn_x,
                row_y + 5.0 * (metrics.btn_h + metrics.btn_gap) + 26.0 * gs,
                metrics.btn_w,
                metrics.btn_h,
            ],
            text.get("gui.done"),
        );
    }

    fn draw_chat_settings_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let gs = metrics.gs;

        self.draw_standard_screen(metrics, font_gui, "options.chat.title", 20.0 * gs, 18.0 * gs);
        let text = self.state.settings.ui_text();

        let left_x = sw / 2.0 - metrics.btn_w - 4.0 * gs;
        let right_x = sw / 2.0 + 4.0 * gs;
        let y = 52.0 * gs;

        // Row 1: Width slider, Height slider
        draw_slider(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CHAT_WIDTH_DOWN,
            [left_x, y, metrics.btn_w, metrics.btn_h],
            &format!(
                "{}: {:.0}%",
                text.get("options.chat.width"),
                self.state.hud.chat_width() * 100.0
            ),
            self.state.hud.chat_width().clamp(0.1, 1.0),
        );
        draw_slider(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CHAT_HEIGHT_DOWN,
            [right_x, y, metrics.btn_w, metrics.btn_h],
            &format!(
                "{}: {}",
                text.get("options.chat.height"),
                self.state.hud.chat_height()
            ),
            (self.state.hud.chat_height() as f32 - 1.0) / 29.0,
        );

        // Row 2: Background toggle, Chat Overlay toggle
        let row2_y = y + metrics.btn_h + metrics.btn_gap;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CHAT_BACKGROUND_TOGGLE,
            [left_x, row2_y, metrics.btn_w, metrics.btn_h],
            &format!(
                "{}: {}",
                text.get("options.chat.background"),
                if self.state.hud.chat_background() {
                    text.get("options.on")
                } else {
                    text.get("options.off")
                }
            ),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CHAT_OVERLAY_TOGGLE,
            [right_x, row2_y, metrics.btn_w, metrics.btn_h],
            &format!(
                "{}: {}",
                text.get("options.chat.overlay"),
                if self.state.hud.chat_overlay() {
                    text.get("options.on")
                } else {
                    text.get("options.off")
                }
            ),
        );

        // Row 3: Chat Avatars toggle, Tab Avatars toggle
        let row3_y = row2_y + metrics.btn_h + metrics.btn_gap;
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::CHAT_AVATARS_TOGGLE,
            [left_x, row3_y, metrics.btn_w, metrics.btn_h],
            &format!(
                "{}: {}",
                text.get("options.chat avatars"),
                if self.state.hud.chat_player_avatars() {
                    text.get("options.on")
                } else {
                    text.get("options.off")
                }
            ),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::TAB_AVATARS_TOGGLE,
            [right_x, row3_y, metrics.btn_w, metrics.btn_h],
            &format!(
                "{}: {}",
                text.get("options.chat.tabAvatars"),
                if self.state.hud.tab_player_avatars() {
                    text.get("options.on")
                } else {
                    text.get("options.off")
                }
            ),
        );

        // Done button
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [
                metrics.btn_x,
                row3_y + metrics.btn_h + metrics.btn_gap + 10.0 * gs,
                metrics.btn_w,
                metrics.btn_h,
            ],
            &text.get("gui.done"),
        );
    }

    fn draw_video_settings_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        self.draw_standard_screen(metrics, font_gui, "options.videoTitle", 24.0 * gs, 18.0 * gs);
        let text = self.state.settings.ui_text();
        let left_x = sw / 2.0 - metrics.btn_w - 4.0 * gs;
        let right_x = sw / 2.0 + 4.0 * gs;
        let rows = [
            (
                OptionControl::slider(
                    btn::GUI_SCALE_DOWN,
                    format!("{}: {}", text.get("options.guiScale"), self.state.settings.gui_scale()),
                    gui_scale_slider_value(self.state.settings.gui_scale()),
                ),
                OptionControl::slider(
                    btn::FOV_DOWN,
                    format!(
                        "{}: {}",
                        text.get("options.fov"),
                        fov_label(&text, self.state.settings.fov())
                    ),
                    (self.state.settings.fov() - 30.0) / 80.0,
                ),
            ),
            (
                OptionControl::slider(
                    btn::RENDER_DISTANCE_DOWN,
                    format!(
                        "{}: {}",
                        text.get("options.renderDistance"),
                        self.state.settings.render_distance()
                    ),
                    render_distance_slider_value(self.state.settings.render_distance()),
                ),
                OptionControl::slider(
                    btn::FRAMERATE_DOWN,
                    format!(
                        "{}: {}",
                        text.get("options.framerateLimit"),
                        framerate_label(&text, self.state.settings.max_framerate())
                    ),
                    framerate_slider_value(self.state.settings.max_framerate()),
                ),
            ),
            (
                OptionControl::single(
                    btn::SMOOTH_LIGHTING_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.ao"),
                        if self.state.settings.smooth_lighting() {
                            text.get("options.ao.max")
                        } else {
                            text.get("options.ao.off")
                        }
                    ),
                ),
                OptionControl::single(
                    btn::PARTICLES_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.particles"),
                        particle_label(&text, &self.state.settings.particles_label())
                    ),
                ),
            ),
            (
                OptionControl::single(btn::SHADER_PACKS, "Shader Packs...".to_string()),
                OptionControl::single(
                    0,
                    format!(
                        "FSR 3: {}",
                        if self.state.server_list.fsr3_available() {
                            "Available"
                        } else {
                            "SDK not linked"
                        }
                    ),
                ),
            ),
            (
                OptionControl::single(
                    btn::CLOUDS_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.renderClouds"),
                        text.bool_label(self.state.settings.clouds())
                    ),
                ),
                OptionControl::single(
                    btn::ENTITY_SHADOWS_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.entityShadows"),
                        text.bool_label(self.state.settings.entity_shadows())
                    ),
                ),
            ),
            (
                OptionControl::single(
                    btn::WEATHER_EFFECTS_TOGGLE,
                    format!(
                        "Weather Effects: {}",
                        text.bool_label(self.state.settings.weather_effects())
                    ),
                ),
                OptionControl::slider(
                    btn::BRIGHTNESS_DOWN,
                    format!(
                        "{}: {}",
                        text.get("options.gamma"),
                        gamma_label(&text, self.state.settings.gamma())
                    ),
                    self.state.settings.gamma().clamp(0.0, 1.0),
                ),
            ),
            (
                OptionControl::single(
                    btn::VIEW_BOBBING_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.viewBobbing"),
                        text.bool_label(self.state.settings.view_bobbing())
                    ),
                ),
                OptionControl::single(
                    btn::ADVANCED_TOOLTIPS_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.advancedTooltips"),
                        text.bool_label(self.state.settings.advanced_tooltips())
                    ),
                ),
            ),
            (
                OptionControl::single(
                    btn::BETTER_GRASS_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.betterGrass"),
                        text.bool_label(self.state.settings.better_grass())
                    ),
                ),
                OptionControl::single(
                    btn::CONNECTED_TEXTURES_TOGGLE,
                    format!(
                        "{}: {}",
                        text.get("options.connectedTextures"),
                        text.bool_label(self.state.settings.connected_textures())
                    ),
                ),
            ),
        ];
        for (row, (left, right)) in rows.iter().enumerate() {
            let y = 60.0 * gs + row as f32 * (metrics.btn_h + metrics.btn_gap);
            draw_option_button(&mut self.font, metrics, widget_gui, font_gui, left_x, y, left);
            draw_option_button(&mut self.font, metrics, widget_gui, font_gui, right_x, y, right);
        }
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [metrics.btn_x, sh - 46.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("gui.done"),
        );
    }

    fn draw_language_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        self.draw_standard_screen(metrics, font_gui, "options.language", 24.0 * gs, 18.0 * gs);
        let text = self.state.settings.ui_text();
        let current = format_text(
            text.get("rustcraft.language.current"),
            &[self.state.settings.language_name(), self.state.settings.language_code()],
        );
        font_gui.draw_text_shadowed(
            &mut self.font,
            sw / 2.0,
            64.0 * gs,
            &current,
            metrics.font_sz,
            [0.9, 0.9, 0.9, 1.0],
            gs,
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::LANGUAGE_TOGGLE,
            [metrics.btn_x, 96.0 * gs, metrics.btn_w, metrics.btn_h],
            if self.state.settings.language_code() == "zh_CN" {
                "English (US)"
            } else {
                "简体中文"
            },
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            130.0 * gs,
            text.get("rustcraft.language.reloadNote"),
            metrics.font_sz * 0.65,
            [0.65, 0.65, 0.65, 1.0],
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [metrics.btn_x, sh - 46.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("gui.done"),
        );
    }

    fn draw_audio_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        self.draw_standard_screen(metrics, font_gui, "options.sounds.title", 24.0 * gs, 18.0 * gs);
        let text = self.state.settings.ui_text();
        let rows = [
            (
                text.get("soundCategory.master"),
                self.state.settings.master_volume(),
                btn::MASTER_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.music"),
                self.state.settings.music_volume(),
                btn::MUSIC_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.block"),
                self.state.settings.blocks_volume(),
                btn::BLOCKS_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.hostile"),
                self.state.settings.hostile_volume(),
                btn::HOSTILE_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.neutral"),
                self.state.settings.friendly_volume(),
                btn::FRIENDLY_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.player"),
                self.state.settings.players_volume(),
                btn::PLAYERS_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.ambient"),
                self.state.settings.ambient_volume(),
                btn::AMBIENT_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.weather"),
                self.state.settings.weather_volume(),
                btn::WEATHER_VOLUME_DOWN,
            ),
            (
                text.get("soundCategory.ui"),
                self.state.settings.ui_volume(),
                btn::UI_VOLUME_DOWN,
            ),
        ];
        for (i, (label, value, id)) in rows.iter().enumerate() {
            let col = i % 2;
            let row = i / 2;
            let x = sw / 2.0 - metrics.btn_w - 4.0 * gs + col as f32 * (metrics.btn_w + 8.0 * gs);
            let y = 58.0 * gs + row as f32 * (metrics.btn_h + metrics.btn_gap);
            draw_slider(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                *id,
                [x, y, metrics.btn_w, metrics.btn_h],
                &format!("{}: {}%", label, (*value * 100.0).round()),
                *value,
            );
        }
        // Audio device selector
        let device_y = 58.0 * gs + 5.0 * (metrics.btn_h + metrics.btn_gap);
        let device_label = format!(
            "{}: {}",
            text.get("soundCategory.master"),
            self.state.settings.audio_device()
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::AUDIO_DEVICE_CYCLE,
            [metrics.btn_x, device_y, metrics.btn_w, metrics.btn_h],
            &device_label,
        );
        let null_text = if self.state.settings.audio_device() == "null" {
            text.get("rustcraft.audio.nullBackend")
        } else {
            ""
        };
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            device_y + metrics.btn_h + 4.0 * gs,
            null_text,
            metrics.font_sz * 0.62,
            [0.65, 0.65, 0.65, 1.0],
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [metrics.btn_x, sh - 46.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("gui.done"),
        );
    }

    fn draw_skin_customization_menu(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        self.draw_standard_screen(metrics, font_gui, "options.skinCustomisation.title", 24.0 * gs, 18.0 * gs);
        let text = self.state.settings.ui_text();
        let left_x = sw / 2.0 - metrics.btn_w - 4.0 * gs;
        let right_x = sw / 2.0 + 4.0 * gs;
        let rows = [
            (
                text.get("options.modelPart.cape"),
                0x01,
                btn::SKIN_CAPE_TOGGLE,
            ),
            (
                text.get("options.modelPart.jacket"),
                0x02,
                btn::SKIN_JACKET_TOGGLE,
            ),
            (
                text.get("options.modelPart.left_sleeve"),
                0x04,
                btn::SKIN_LEFT_SLEEVE_TOGGLE,
            ),
            (
                text.get("options.modelPart.right_sleeve"),
                0x08,
                btn::SKIN_RIGHT_SLEEVE_TOGGLE,
            ),
            (
                text.get("options.modelPart.left_pants_leg"),
                0x10,
                btn::SKIN_LEFT_PANTS_TOGGLE,
            ),
            (
                text.get("options.modelPart.right_pants_leg"),
                0x20,
                btn::SKIN_RIGHT_PANTS_TOGGLE,
            ),
            (
                text.get("options.modelPart.hat"),
                0x40,
                btn::SKIN_HAT_TOGGLE,
            ),
        ];
        for (i, (label, mask, id)) in rows.iter().enumerate() {
            let x = if i % 2 == 0 { left_x } else { right_x };
            let y = 58.0 * gs + (i / 2) as f32 * (metrics.btn_h + metrics.btn_gap);
            draw_button(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                *id,
                [x, y, metrics.btn_w, metrics.btn_h],
                &format!(
                    "{}: {}",
                    label,
                    text.bool_label(self.state.settings.skin_parts() & *mask != 0)
                ),
            );
        }
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::SKIN_ALL_TOGGLE,
            [
                metrics.btn_x,
                58.0 * gs + 4.0 * (metrics.btn_h + metrics.btn_gap),
                metrics.btn_w,
                metrics.btn_h,
            ],
            if self.state.settings.skin_parts() == 0x7f {
                text.get("gui.all")
            } else {
                text.get("rustcraft.skin.allCustom")
            },
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            58.0 * gs + 5.0 * (metrics.btn_h + metrics.btn_gap) + 6.0 * gs,
            text.get("rustcraft.skin.syncNote"),
            metrics.font_sz * 0.62,
            [0.65, 0.65, 0.65, 1.0],
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [metrics.btn_x, sh - 46.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("gui.done"),
        );
    }

    fn draw_resource_packs_screen_legacy(
        &mut self,
        metrics: &MenuMetrics,
        widget_gui: &mut GuiVertexBuilder,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let sw = self.swapchain.swapchain_extent.width as f32;
        let sh = self.swapchain.swapchain_extent.height as f32;
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let desc_sz = font_sz * 0.62;
        self.draw_standard_screen(metrics, font_gui, "resourcePack.title", 24.0 * gs, 18.0 * gs);
        let text = self.state.settings.ui_text();

        let col_w = 190.0 * gs;
        let col_gap = 8.0 * gs;
        let left_x = sw / 2.0 - col_w - col_gap / 2.0;
        let right_x = sw / 2.0 + col_gap / 2.0;
        let col_y = 58.0 * gs;
        let col_h = sh - 130.0 * gs;
        let row_h = 28.0 * gs;

        // Detail panel — shows description of hovered pack (right of columns)
        let detail_x = right_x + col_w + col_gap;
        let detail_w = sw - detail_x - 8.0 * gs;
        let detail_y = col_y;
        let detail_h = col_h;

        // Column backgrounds
        font_gui.fill_rect(left_x, col_y, col_w, col_h, [0.08, 0.08, 0.08, 0.92]);
        font_gui.fill_rect(right_x, col_y, col_w, col_h, [0.08, 0.08, 0.08, 0.92]);
        font_gui.fill_rect(
            detail_x,
            detail_y,
            detail_w,
            detail_h,
            [0.06, 0.06, 0.06, 0.92],
        );

        // Column headers
        let header_y = col_y - 16.0 * gs;
        font_gui.draw_text_centered(
            &mut self.font,
            left_x + col_w / 2.0,
            header_y,
            text.get("resourcePack.available.title"),
            font_sz * 0.9,
            [1.0, 1.0, 1.0, 1.0],
        );
        font_gui.draw_text_centered(
            &mut self.font,
            right_x + col_w / 2.0,
            header_y,
            text.get("resourcePack.selected.title"),
            font_sz * 0.9,
            [1.0, 1.0, 1.0, 1.0],
        );

        // Draw available packs (left column)
        let packs = self.state.server_list.available_resource_packs().clone();
        for (i, pack) in packs.iter().enumerate() {
            if i >= btn::RESOURCE_PACK_MAX {
                break;
            }
            let y = col_y + 4.0 * gs + i as f32 * row_h;
            let id = btn::RESOURCE_PACK_BASE + i as u32;
            let compatible = pack.pack_format == 0 || pack.pack_format == 1;
            let label = if compatible {
                pack.name.clone()
            } else {
                format!("{} §c(incompatible)", pack.name)
            };
            draw_button(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                id,
                [
                    left_x + 4.0 * gs + 14.0 * gs,
                    y,
                    col_w - 8.0 * gs - 14.0 * gs,
                    row_h - 2.0 * gs,
                ],
                &label,
            );
            // Pack icon
            if let Some(icon) = &pack.icon {
                draw_pack_icon(font_gui, left_x + 5.0 * gs, y + 1.0 * gs, 12.0 * gs, icon);
            }
            if !compatible {
                font_gui.fill_rect(
                    left_x + 4.0 * gs,
                    y,
                    3.0 * gs,
                    row_h - 2.0 * gs,
                    [0.8, 0.15, 0.15, 0.9],
                );
            }
        }

        // Draw selected packs (right column)
        let selected_packs = self.state.server_list.selected_resource_packs().clone();
        for (i, pack) in selected_packs.iter().enumerate() {
            if i >= btn::RESOURCE_PACK_SELECTED_MAX {
                break;
            }
            let y = col_y + 4.0 * gs + i as f32 * row_h;
            let id = btn::RESOURCE_PACK_SELECTED_BASE + i as u32;
            let compatible = pack.pack_format == 0 || pack.pack_format == 1;
            let label = if compatible {
                pack.name.clone()
            } else {
                format!("{} §c(incompatible)", pack.name)
            };
            draw_button(
                &mut self.font,
                metrics,
                widget_gui,
                font_gui,
                id,
                [
                    right_x + 4.0 * gs + 14.0 * gs,
                    y,
                    col_w - 8.0 * gs - 14.0 * gs,
                    row_h - 2.0 * gs,
                ],
                &label,
            );
            if let Some(icon) = &pack.icon {
                draw_pack_icon(font_gui, right_x + 5.0 * gs, y + 1.0 * gs, 12.0 * gs, icon);
            }
            if !compatible {
                font_gui.fill_rect(
                    right_x + 4.0 * gs,
                    y,
                    3.0 * gs,
                    row_h - 2.0 * gs,
                    [0.8, 0.15, 0.15, 0.9],
                );
            }
        }

        // Detail panel — show description of hovered pack
        let hovered_pack = self
            .gui_hit_test(metrics.mouse_pos[0], metrics.mouse_pos[1])
            .and_then(|id| {
                if id >= btn::RESOURCE_PACK_BASE
                    && id < btn::RESOURCE_PACK_BASE + btn::RESOURCE_PACK_MAX as u32
                {
                    let idx = (id - btn::RESOURCE_PACK_BASE) as usize;
                    self.state.server_list.available_resource_packs().get(idx)
                } else if id >= btn::RESOURCE_PACK_SELECTED_BASE
                    && id
                        < btn::RESOURCE_PACK_SELECTED_BASE + btn::RESOURCE_PACK_SELECTED_MAX as u32
                {
                    let idx = (id - btn::RESOURCE_PACK_SELECTED_BASE) as usize;
                    self.state.server_list.selected_resource_packs().get(idx)
                } else {
                    None
                }
            });

        if let Some(pack) = hovered_pack {
            let compatible = pack.pack_format == 0 || pack.pack_format == 1;
            let name_color = if compatible {
                [0.9, 0.9, 0.9, 1.0]
            } else {
                [0.9, 0.3, 0.3, 1.0]
            };
            font_gui.draw_text_centered(
                &mut self.font,
                detail_x + detail_w / 2.0,
                detail_y + 4.0 * gs,
                &pack.name,
                font_sz * 0.72,
                name_color,
            );
            if !compatible {
                font_gui.draw_text_centered(
                    &mut self.font,
                    detail_x + detail_w / 2.0,
                    detail_y + 18.0 * gs,
                    "§cIncompatible — requires MC 1.8.9",
                    desc_sz * 0.85,
                    [0.9, 0.3, 0.3, 1.0],
                );
            }
            if !pack.description.is_empty() {
                let desc_y = detail_y + if compatible { 24.0 } else { 36.0 } * gs;
                let wrap_w = detail_w - 12.0 * gs;
                let line_h = 10.0 * gs;
                let mut line_y = desc_y;
                for line in pack.description.split('\n') {
                    let text = line.trim();
                    if text.is_empty() {
                        line_y += line_h;
                        continue;
                    }
                    let line_w = self.font.text_width(text, desc_sz);
                    if line_w > wrap_w {
                        let mut cursor = 0;
                        let chars: Vec<char> = text.chars().collect();
                        while cursor < chars.len() {
                            let mut end = (cursor + 1).min(chars.len());
                            while end > cursor + 1
                                && self.font.text_width(
                                    &chars[cursor..end].iter().collect::<String>(),
                                    desc_sz,
                                ) > wrap_w
                            {
                                end -= 1;
                            }
                            let segment: String = chars[cursor..end].iter().collect();
                            font_gui.draw_text(
                                &mut self.font,
                                detail_x + 6.0 * gs,
                                line_y,
                                &segment,
                                desc_sz,
                                [0.7, 0.7, 0.7, 1.0],
                            );
                            cursor = end;
                            line_y += line_h;
                        }
                    } else {
                        font_gui.draw_text(
                            &mut self.font,
                            detail_x + 6.0 * gs,
                            line_y,
                            text,
                            desc_sz,
                            [0.7, 0.7, 0.7, 1.0],
                        );
                        line_y += line_h;
                    }
                }
            }
        }

        // Bottom buttons
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::RESOURCE_PACK_OPEN_FOLDER,
            [left_x, sh - 50.0 * gs, col_w, metrics.btn_h],
            text.get("resourcePack.openFolder"),
        );
        draw_button(
            &mut self.font,
            metrics,
            widget_gui,
            font_gui,
            btn::DONE,
            [right_x, sh - 50.0 * gs, col_w, metrics.btn_h],
            text.get("gui.done"),
        );
        font_gui.draw_text_centered(
            &mut self.font,
            sw / 2.0,
            sh - 26.0 * gs,
            text.get("resourcePack.folderInfo"),
            font_sz * 0.7,
            [0.65, 0.65, 0.65, 1.0],
        );
    }

    /// Draws the standard centered screen title by borrowing `ui_text` instead
    /// of cloning the full `HashMap` each frame. Callers should separately
    /// borrow `self.state.settings.ui_text()` for any body content.
    fn draw_standard_screen(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
        title_key: &'static str,
        title_y: f32,
        title_size: f32,
    ) {
        let text = self.state.settings.ui_text();
        font_gui.draw_text_shadowed(
            &mut self.font,
            metrics.sw / 2.0,
            title_y,
            text.get(title_key),
            title_size,
            [1.0, 1.0, 1.0, 1.0],
            metrics.gs,
        );
    }
}

fn draw_button(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    rect: [f32; 4],
    label: &str,
) {
    super::gui::widgets::draw_button(
        metrics,
        widget_gui,
        font_gui,
        id,
        rect,
        label,
        font,
    );
}

fn draw_button_enabled(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    rect: [f32; 4],
    label: &str,
    enabled: bool,
) {
    if enabled {
        draw_button(font, metrics, widget_gui, font_gui, id, rect, label);
        return;
    }

    let [x, y, w, h] = rect;
    widget_gui.draw_button_rect_state(x, y, w, h, 2);
    font_gui.draw_text_shadowed(
        font,
        x + w / 2.0,
        y + (h - metrics.font_sz) / 2.0,
        label,
        metrics.font_sz,
        [0.63, 0.63, 0.63, 1.0],
        metrics.gs,
    );
}

fn draw_default_background(metrics: &MenuMetrics, background_gui: &mut GuiVertexBuilder) {
    let tile = 32.0 * metrics.gs;
    background_gui.add_quad(
        0.0,
        0.0,
        metrics.sw,
        metrics.sh,
        0.0,
        0.0,
        metrics.sw / tile,
        metrics.sh / tile,
        [0.25, 0.25, 0.25, 1.0],
    );
}

fn draw_world_overlay_background(metrics: &MenuMetrics, gui: &mut GuiVertexBuilder) {
    // Vanilla GuiScreen.drawWorldBackground: drawGradientRect(0, 0, w, h,
    // 0xC0101010, 0xD0101010).
    gui.fill_world_background(metrics.sw, metrics.sh);
}

struct OptionControl {
    primary_id: u32,
    secondary_id: u32,
    label: String,
    slider_value: Option<f32>,
}

impl OptionControl {
    fn single(id: u32, label: String) -> Self {
        Self {
            primary_id: id,
            secondary_id: 0,
            label,
            slider_value: None,
        }
    }

    fn button(primary_id: u32, secondary_id: u32, label: String) -> Self {
        Self {
            primary_id,
            secondary_id,
            label,
            slider_value: None,
        }
    }

    fn slider(id: u32, label: String, value: f32) -> Self {
        Self {
            primary_id: id,
            secondary_id: 0,
            label,
            slider_value: Some(value),
        }
    }
}

fn draw_slider(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    rect: [f32; 4],
    label: &str,
    value: f32,
) {
    let [x, y, w, h] = rect;
    let thumb_hovered = (metrics.mouse_pos[0] >= x
        && metrics.mouse_pos[0] <= x + w
        && metrics.mouse_pos[1] >= y
        && metrics.mouse_pos[1] <= y + h) as u32;
    widget_gui.draw_slider_rect_state(x, y, w, h, value, thumb_hovered);
    widget_gui.register_button(id, x, y, w, h);
    font_gui.draw_text_shadowed(
        font,
        x + w / 2.0,
        y + (h - metrics.font_sz) / 2.0,
        label,
        metrics.font_sz,
        [0.9, 0.9, 0.9, 1.0],
        metrics.gs,
    );
}

fn draw_title(
    font: &mut crate::ui::font::FontRenderer,
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    text: &str,
    size: f32,
    gs: f32,
) {
    font_gui.draw_text_shadowed(
        font,
        x,
        y,
        text,
        size,
        [1.0, 1.0, 1.0, 1.0],
        gs,
    );
}

fn draw_option_button(
    font: &mut crate::ui::font::FontRenderer,
    metrics: &MenuMetrics,
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    x: f32,
    y: f32,
    option: &OptionControl,
) {
    if let Some(value) = option.slider_value {
        draw_slider(
            font,
            metrics,
            widget_gui,
            font_gui,
            option.primary_id,
            [x, y, metrics.btn_w, metrics.btn_h],
            &option.label,
            value,
        );
    } else {
        draw_button(
            font,
            metrics,
            widget_gui,
            font_gui,
            option.primary_id,
            [x, y, metrics.btn_w, metrics.btn_h],
            &option.label,
        );
        if option.secondary_id != 0 {
            draw_option_increment(
                widget_gui,
                font_gui,
                option.secondary_id,
                x,
                y,
                metrics.btn_w,
                metrics.btn_h,
                metrics.font_sz,
                metrics.gs,
                font,
            );
        }
    }
}

fn draw_option_increment(
    widget_gui: &mut GuiVertexBuilder,
    font_gui: &mut GuiVertexBuilder,
    id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    font_sz: f32,
    gs: f32,
    font: &mut crate::ui::font::FontRenderer,
) {
    widget_gui.register_button(id, x + w - 34.0 * gs, y, 34.0 * gs, h);
    font_gui.fill_rect(
        x + w - 34.0 * gs,
        y + 2.0 * gs,
        1.0 * gs,
        h - 4.0 * gs,
        [0.0, 0.0, 0.0, 0.35],
    );
    font_gui.draw_text_shadowed(
        font,
        x + w - 17.0 * gs,
        y + (h - font_sz) / 2.0,
        ">",
        font_sz,
        [1.0, 1.0, 1.0, 1.0],
        gs,
    );
}

fn difficulty_label(text: &crate::ui::text::UiText, value: u8) -> &str {
    match value {
        0 => text.get("options.difficulty.peaceful"),
        1 => text.get("options.difficulty.easy"),
        2 => text.get("options.difficulty.normal"),
        3 => text.get("options.difficulty.hard"),
        _ => text.get("options.difficulty.normal"),
    }
}

fn framerate_label(text: &crate::ui::text::UiText, value: u32) -> String {
    if value >= crate::client::config::UNLIMITED_FRAMERATE {
        text.get("options.framerateLimit.max").to_string()
    } else {
        format!("{} fps", value)
    }
}

fn framerate_slider_value(value: u32) -> f32 {
    if value >= crate::client::config::UNLIMITED_FRAMERATE {
        1.0
    } else {
        ((value.saturating_sub(30)) as f32
            / (crate::client::config::UNLIMITED_FRAMERATE - 30) as f32)
            .clamp(0.0, 1.0)
    }
}

fn gui_scale_slider_value(value: u32) -> f32 {
    (value.saturating_sub(1).min(3) as f32) / 3.0
}

fn render_distance_slider_value(value: u8) -> f32 {
    (value.saturating_sub(2).min(14) as f32) / 14.0
}

fn fov_label(text: &crate::ui::text::UiText, value: f32) -> String {
    if value <= 30.0 {
        text.get("options.fov.min").to_string()
    } else if value >= 110.0 {
        text.get("options.fov.max").to_string()
    } else {
        format!("{:.0}", value)
    }
}

fn gamma_label(text: &crate::ui::text::UiText, value: f32) -> String {
    if value <= 0.0 {
        text.get("options.gamma.min").to_string()
    } else if value >= 1.0 {
        text.get("options.gamma.max").to_string()
    } else {
        format!("{:.0}%", (value * 100.0).round())
    }
}

fn particle_label(text: &crate::ui::text::UiText, value: &str) -> String {
    match value {
        "Decreased" => text.get("options.particles.decreased").to_string(),
        "Minimal" => text.get("options.particles.minimal").to_string(),
        _ => text.get("options.particles.all").to_string(),
    }
}

fn server_status_label(text: &crate::ui::text::UiText, server: &super::ServerListRow) -> String {
    if server.online {
        match (server.players_online, server.players_max, server.ping_ms) {
            (Some(online), Some(max), Some(ping)) => format!("{}/{}  {}ms", online, max, ping),
            (_, _, Some(ping)) => {
                format!("{}  {}ms", text.get("rustcraft.multiplayer.online"), ping)
            }
            _ => text.get("rustcraft.multiplayer.online").to_string(),
        }
    } else if server.error.is_some() {
        text.get("rustcraft.multiplayer.offline").to_string()
    } else {
        text.get("rustcraft.multiplayer.unknown").to_string()
    }
}

/// Draw a 32×32 RGBA pack icon at (x,y) scaled to `size` pixels on screen.
pub(super) fn draw_pack_icon(
    gui: &mut crate::render::gui::GuiVertexBuilder,
    x: f32,
    y: f32,
    size: f32,
    pixels: &[u8; 4096],
) {
    let ps = size / 32.0;
    for py in 0..32 {
        for px in 0..32 {
            let idx = (py * 32 + px) * 4;
            let r = pixels[idx] as f32 / 255.0;
            let g = pixels[idx + 1] as f32 / 255.0;
            let b = pixels[idx + 2] as f32 / 255.0;
            let a = pixels[idx + 3] as f32 / 255.0;
            if a < 0.01 {
                continue;
            }
            gui.fill_rect(x + px as f32 * ps, y + py as f32 * ps, ps, ps, [r, g, b, a]);
        }
    }
}
