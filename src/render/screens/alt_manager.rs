use super::scroll_list::GuiScrollList;
use super::{draw_button, draw_button_enabled, draw_title};
use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;
use crate::ui::button_ids as btn;

impl Renderer {
    pub(super) fn draw_alt_manager_screen(
        &mut self,
        metrics: &MenuMetrics,
        widgets: &mut GuiVertexBuilder,
        font: &mut GuiVertexBuilder,
    ) {
        let gs = metrics.gs;
        let text = self.state.ui_text.clone();
        draw_title(
            self,
            font,
            metrics.sw / 2.0,
            18.0 * gs,
            text.get("rustcraft.altmanager.title"),
            14.0 * gs,
            gs,
        );
        let list = GuiScrollList::new(
            0.0,
            38.0 * gs,
            metrics.sw,
            (metrics.sh - 92.0 * gs).max(40.0 * gs),
            34.0 * gs,
            self.state.account_list.len(),
            self.state.selected_account.saturating_sub(4),
        );
        list.draw_background(font);
        if self.state.account_list.is_empty() {
            font.draw_text_centered(
                &mut self.font,
                metrics.sw / 2.0,
                72.0 * gs,
                text.get("rustcraft.altmanager.empty"),
                metrics.font_sz,
                [0.65, 0.65, 0.65, 1.0],
            );
        }
        let width = (300.0 * gs).min(metrics.sw - 24.0 * gs);
        let x = (metrics.sw - width) / 2.0;
        for index in list.visible_range() {
            let (name, uuid, active) = self.state.account_list[index].clone();
            let y = list.row_y(index);
            if index == self.state.selected_account {
                font.fill_rect(x, y, width, 32.0 * gs, [0.25, 0.25, 0.25, 0.95]);
            }
            widgets.register_button(
                btn::ALT_ACCOUNT_ROW_BASE + index as u32,
                x,
                y,
                width,
                32.0 * gs,
            );

            // Load face from cache or download
            let key = uuid.replace('-', "");
            let face = self.load_account_face(index, &key);
            let face_size = 24.0 * gs;
            let face_x = x + 4.0 * gs;
            let face_y = y + (32.0 * gs - face_size) / 2.0;
            let px = face_size / 8.0;
            for fy in 0..8 {
                for fx in 0..8 {
                    let pixel = face[fy * 8 + fx];
                    if pixel[3] == 0 {
                        continue;
                    }
                    font.fill_rect(
                        face_x + fx as f32 * px,
                        face_y + fy as f32 * px,
                        px,
                        px,
                        [
                            pixel[0] as f32 / 255.0,
                            pixel[1] as f32 / 255.0,
                            pixel[2] as f32 / 255.0,
                            1.0,
                        ],
                    );
                }
            }

            font.draw_text(
                &mut self.font,
                x + 34.0 * gs,
                y + 5.0 * gs,
                &name,
                metrics.font_sz,
                if active {
                    [0.45, 1.0, 0.45, 1.0]
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                },
            );
            font.draw_text(
                &mut self.font,
                x + 34.0 * gs,
                y + 18.0 * gs,
                &uuid,
                metrics.font_sz * 0.65,
                [0.55, 0.55, 0.55, 1.0],
            );
        }
        list.draw_scrollbar(font, gs);
        let y = metrics.sh - 48.0 * gs;
        let has = !self.state.account_list.is_empty()
            && self.state.selected_account < self.state.account_list.len();
        draw_button(
            self,
            metrics,
            widgets,
            font,
            btn::ALT_LOGIN,
            [metrics.sw / 2.0 - 154.0 * gs, y, 100.0 * gs, metrics.btn_h],
            text.get("rustcraft.altmanager.addAccount"),
        );
        draw_button(
            self,
            metrics,
            widgets,
            font,
            btn::ALT_OFFLINE,
            [metrics.sw / 2.0 - 50.0 * gs, y, 100.0 * gs, metrics.btn_h],
            text.get("rustcraft.altmanager.addOffline"),
        );
        draw_button_enabled(
            self,
            metrics,
            widgets,
            font,
            btn::ALT_USE,
            [metrics.sw / 2.0 + 54.0 * gs, y, 100.0 * gs, metrics.btn_h],
            text.get("rustcraft.altmanager.useAccount"),
            has,
        );
        draw_button_enabled(
            self,
            metrics,
            widgets,
            font,
            btn::ALT_LOGOUT,
            [metrics.sw / 2.0 + 158.0 * gs, y, 100.0 * gs, metrics.btn_h],
            text.get("rustcraft.altmanager.remove"),
            has,
        );
        draw_button(
            self,
            metrics,
            widgets,
            font,
            btn::ALT_BACK,
            [metrics.btn_x, y + 24.0 * gs, metrics.btn_w, metrics.btn_h],
            text.get("gui.done"),
        );
        font.draw_text_centered(
            &mut self.font,
            metrics.sw / 2.0,
            y - 13.0 * gs,
            &self.state.account_status,
            metrics.font_sz * 0.7,
            [0.7, 0.7, 0.7, 1.0],
        );

        // Offline username input overlay
        if self.state.entering_offline_name {
            let overlay_y = (metrics.sh - 50.0 * gs) / 2.0;
            font.fill_rect(0.0, 0.0, metrics.sw, metrics.sh, [0.0, 0.0, 0.0, 0.52]);
            font.fill_rect(
                (metrics.sw - 260.0 * gs) / 2.0,
                overlay_y,
                260.0 * gs,
                50.0 * gs,
                [0.12, 0.12, 0.12, 0.95],
            );
            font.draw_text_centered(
                &mut self.font,
                metrics.sw / 2.0,
                overlay_y + 8.0 * gs,
                text.get("rustcraft.altmanager.enterUsername"),
                metrics.font_sz * 0.8,
                [0.9, 0.9, 0.9, 1.0],
            );
            let hint = text.get("rustcraft.altmanager.usernameHint").to_string();
            let input_text: &str = if self.state.offline_username_input.is_empty() {
                &hint
            } else {
                &self.state.offline_username_input
            };
            let color = if self.state.offline_username_input.is_empty() {
                [0.4, 0.4, 0.4, 1.0]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            font.draw_text_centered(
                &mut self.font,
                metrics.sw / 2.0,
                overlay_y + 28.0 * gs,
                input_text,
                metrics.font_sz,
                color,
            );
            font.draw_text_centered(
                &mut self.font,
                metrics.sw / 2.0,
                overlay_y + 56.0 * gs,
                text.get("rustcraft.altmanager.confirmHint"),
                metrics.font_sz * 0.55,
                [0.55, 0.55, 0.55, 1.0],
            );
        }
    }

    fn load_account_face(&mut self, index: usize, key: &str) -> [[u8; 4]; 64] {
        if let Some(&face) = self.state.account_faces.get(key) {
            return face;
        }
        let (_, uuid, _) = &self.state.account_list[index];

        // Get account data from auth cache
        let accounts = crate::auth::cache::load_accounts().unwrap_or_default();
        let account = accounts
            .iter()
            .find(|a| a.uuid.as_deref() == Some(uuid.as_str()));
        let skin_info = account
            .and_then(|a| a.skins.as_ref())
            .and_then(|s| s.first());
        let texture_key = skin_info.and_then(|s| s.url.rsplit('/').next());
        let skin_url = skin_info.map(|s| s.url.as_str());

        let skin = texture_key
            .and_then(|tk| {
                let path = format!("assets/skins/{}/{}.png", key, tk);
                crate::assets::skin::PlayerSkin::load(&path).ok()
            })
            .or_else(|| {
                crate::assets::skin::PlayerSkin::load(format!("assets/skins/{}.png", key)).ok()
            })
            .or_else(|| {
                skin_url.and_then(|url| {
                    let tk = texture_key.unwrap_or("unknown");
                    let bytes = reqwest::blocking::get(url).ok()?.bytes().ok()?;
                    let path = format!("assets/skins/{}.png", tk);
                    let _ = std::fs::write(&path, &bytes);
                    crate::assets::skin::PlayerSkin::load(&path).ok()
                })
            })
            .unwrap_or_else(crate::assets::skin::PlayerSkin::default_steve);

        let face = skin.face_pixels();
        self.state.account_faces.insert(key.to_string(), face);
        face
    }
}
