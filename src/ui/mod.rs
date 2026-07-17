//! UI system — menus, HUD, font rendering, i18n, GUI widgets.

pub mod button_ids;
pub mod font;
pub mod format;
pub mod i18n;
pub mod menu;
pub mod text;
pub mod widgets;

use font::FontRenderer;
use i18n::I18n;
use text::UiText;

/// Game UI state shared across all screens.
pub struct UiState {
    pub font: FontRenderer,
    pub i18n: I18n,
    pub text: UiText,
    pub fps: u32,
}

impl UiState {
    pub fn new(lang_code: &str) -> Self {
        let lang_path = format!("assets/minecraft/lang/{}.lang", lang_code);
        let fallback = "assets/minecraft/lang/en_US.lang";
        let i18n = I18n::load(&lang_path, Some(fallback));

        UiState {
            font: FontRenderer::new(),
            text: UiText::from_i18n(&i18n),
            i18n,
            fps: 0,
        }
    }

    pub fn t(&self, key: &str) -> String {
        self.i18n.t(key)
    }

    pub fn update_fps(&mut self, fps: u32) {
        self.fps = fps;
    }
}
