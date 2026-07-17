//! Menu screens — main menu, pause menu, using GuiScreen system.

use super::i18n::I18n;
use super::widgets::{GuiBackground, GuiScreen, Widget};

/// Build the MC-style main menu screen.
pub fn build_main_menu(i18n: &I18n) -> GuiScreen {
    let mut screen = GuiScreen::new(GuiBackground::Panorama);

    // Minecraft title logo at top center
    screen.add_widget(Widget::label(
        0.0,
        -0.35,
        "RustCraft",
        32.0,
        [1.0, 1.0, 1.0, 1.0],
    ));

    // Subtitle
    screen.add_widget(Widget::label(
        0.0,
        -0.28,
        &format!("Minecraft 1.8.9 — {}", i18n.t("language.name")),
        14.0,
        [0.8, 0.8, 0.8, 1.0],
    ));

    // Buttons (MC style: centered, stacked vertically)
    let btn_w = 200.0;
    let btn_h = 20.0;
    let btn_y_start = -0.02;
    let btn_spacing = 0.06;

    screen.add_widget(Widget::centered_button(
        1,
        btn_y_start,
        btn_w,
        btn_h,
        &i18n.t("menu.singleplayer"),
    ));
    screen.add_widget(Widget::centered_button(
        4,
        btn_y_start - btn_spacing,
        btn_w,
        btn_h,
        &i18n.t("menu.multiplayer"),
    ));
    screen.add_widget(Widget::centered_button(
        2,
        btn_y_start - btn_spacing * 2.0,
        btn_w,
        btn_h,
        &i18n.t("menu.options"),
    ));
    screen.add_widget(Widget::centered_button(
        3,
        btn_y_start - btn_spacing * 3.0,
        btn_w,
        btn_h,
        &i18n.t("menu.quit"),
    ));

    screen
}

/// Build the MC-style pause menu.
pub fn build_pause_menu(i18n: &I18n) -> GuiScreen {
    let mut screen = GuiScreen::new(GuiBackground::Options);

    let btn_w = 200.0;
    let btn_h = 20.0;
    let btn_y_start = -0.15;
    let btn_spacing = 0.06;

    screen.add_widget(Widget::label(
        0.0,
        0.05,
        &i18n.t("menu.game"),
        20.0,
        [1.0, 1.0, 1.0, 1.0],
    ));
    screen.add_widget(Widget::centered_button(
        10,
        btn_y_start,
        btn_w,
        btn_h,
        &i18n.t("menu.returnToGame"),
    ));
    screen.add_widget(Widget::centered_button(
        11,
        btn_y_start - btn_spacing,
        btn_w,
        btn_h,
        &i18n.t("menu.options"),
    ));
    screen.add_widget(Widget::centered_button(
        12,
        btn_y_start - btn_spacing * 2.0,
        btn_w,
        btn_h,
        &i18n.t("menu.disconnect"),
    ));

    screen
}
