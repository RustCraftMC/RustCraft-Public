//! Snapshot-only UI reads and a bounded queue of local UI commands.
//!
//! Lua never receives a window handle, widget pointer, renderer object, or a callback into
//! `App`. The host publishes owned snapshots and drains commands at a safe point on the client
//! thread. Commands in this module are deliberately local: none of them sends a packet or moves
//! the player.

use mlua::{Lua, LuaSerdeExt, Table};
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;

use crate::scripting::permissions::{Permission, PermissionSet};

pub const READ_PERMISSION: &str = "ui.read";
pub const MODIFY_PERMISSION: &str = "ui.modify";

pub const MAX_PENDING_UI_COMMANDS: usize = 128;
pub const MAX_LOCAL_MESSAGE_BYTES: usize = 4 * 1024;
pub const MAX_CHAT_INPUT_BYTES: usize = 256;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct UiSnapshot {
    pub screen: UiScreenSnapshot,
    pub chat: UiChatSnapshot,
    pub inventory: UiInventorySnapshot,
    pub gui: UiGuiSnapshot,
    pub window: UiWindowSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UiScreenSnapshot {
    /// Stable screen identifier such as `main_menu`, `game`, `pause`, or `chat`.
    pub id: String,
    pub title: Option<String>,
    pub in_game: bool,
    pub paused: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UiChatSnapshot {
    pub open: bool,
    /// Current local edit buffer. This is not a server message and contains no credentials.
    pub input: String,
    pub visible_messages: u32,
    pub unread_messages: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UiInventorySnapshot {
    pub open: bool,
    pub window_id: Option<i32>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub slot_count: u32,
    pub selected_hotbar_slot: u8,
    pub cursor_item: Option<UiItemSnapshot>,
    pub slots: Vec<UiItemSnapshot>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UiItemSnapshot {
    pub slot: i32,
    pub id: String,
    pub count: u32,
    pub damage: i32,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiGuiSnapshot {
    pub hud_visible: bool,
    pub crosshair_visible: bool,
    pub chat_visible: bool,
    pub debug_visible: bool,
    pub scale: f32,
    pub focused_widget: Option<String>,
}

impl Default for UiGuiSnapshot {
    fn default() -> Self {
        Self {
            hud_visible: true,
            crosshair_visible: true,
            chat_visible: true,
            debug_visible: false,
            scale: 1.0,
            focused_widget: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiWindowSnapshot {
    pub width: u32,
    pub height: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub scale_factor: f64,
    pub focused: bool,
    pub fullscreen: bool,
}

impl Default for UiWindowSnapshot {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            framebuffer_width: 0,
            framebuffer_height: 0,
            scale_factor: 1.0,
            focused: true,
            fullscreen: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiCommand {
    ShowSystemMessage { text: String },
    OpenChat { initial_text: String },
    CloseChat,
    SetHudVisible { visible: bool },
    SetCrosshairVisible { visible: bool },
}

/// Shared bridge between the client and all installed `game.ui` closures.
///
/// The client updates the snapshot once per logical tick and drains commands on that same thread.
/// A fresh Lua table is created for every read, so scripts cannot mutate host state by retaining it.
#[derive(Clone, Debug, Default)]
pub struct UiApiState {
    snapshot: Rc<RefCell<UiSnapshot>>,
    commands: Rc<RefCell<Vec<UiCommand>>>,
}

impl UiApiState {
    pub fn new(snapshot: UiSnapshot) -> Self {
        Self {
            snapshot: Rc::new(RefCell::new(snapshot)),
            commands: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn set_snapshot(&self, snapshot: UiSnapshot) {
        *self.snapshot.borrow_mut() = snapshot;
    }

    pub fn snapshot(&self) -> UiSnapshot {
        self.snapshot.borrow().clone()
    }

    pub fn drain_commands(&self) -> Vec<UiCommand> {
        self.commands.borrow_mut().drain(..).collect()
    }

    pub fn clear_commands(&self) {
        self.commands.borrow_mut().clear();
    }

    pub fn pending_command_count(&self) -> usize {
        self.commands.borrow().len()
    }

    fn push(&self, command: UiCommand) -> mlua::Result<()> {
        let mut commands = self.commands.borrow_mut();
        if commands.len() >= MAX_PENDING_UI_COMMANDS {
            return Err(mlua::Error::RuntimeError(format!(
                "pending local UI command limit exceeded ({MAX_PENDING_UI_COMMANDS})"
            )));
        }
        commands.push(command);
        Ok(())
    }
}

pub fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    state: UiApiState,
) -> mlua::Result<()> {
    let can_read = permissions.contains(Permission::UiRead);
    let can_modify = permissions.contains(Permission::UiModify);
    if !can_read && !can_modify {
        return Ok(());
    }

    let ui = lua.create_table()?;
    if can_read {
        install_snapshot_reads(lua, &ui, state.clone())?;
    }
    if can_modify {
        install_local_commands(lua, &ui, state)?;
    }
    game.set("ui", ui)
}

fn install_snapshot_reads(lua: &Lua, ui: &Table, state: UiApiState) -> mlua::Result<()> {
    let snapshot_state = state.clone();
    ui.set(
        "snapshot",
        lua.create_function(move |lua, ()| lua.to_value(&snapshot_state.snapshot()))?,
    )?;

    let screen_state = state.clone();
    ui.set(
        "screen",
        lua.create_function(move |lua, ()| lua.to_value(&screen_state.snapshot.borrow().screen))?,
    )?;

    let chat_state = state.clone();
    ui.set(
        "chat",
        lua.create_function(move |lua, ()| lua.to_value(&chat_state.snapshot.borrow().chat))?,
    )?;

    let inventory_state = state.clone();
    ui.set(
        "inventory",
        lua.create_function(move |lua, ()| {
            lua.to_value(&inventory_state.snapshot.borrow().inventory)
        })?,
    )?;

    let gui_state = state.clone();
    ui.set(
        "gui",
        lua.create_function(move |lua, ()| lua.to_value(&gui_state.snapshot.borrow().gui))?,
    )?;

    ui.set(
        "window",
        lua.create_function(move |lua, ()| lua.to_value(&state.snapshot.borrow().window))?,
    )
}

fn install_local_commands(lua: &Lua, ui: &Table, state: UiApiState) -> mlua::Result<()> {
    let message_state = state.clone();
    ui.set(
        "show_system_message",
        lua.create_function(move |_, text: String| {
            validate_text(
                "local system message",
                &text,
                MAX_LOCAL_MESSAGE_BYTES,
                false,
            )?;
            message_state.push(UiCommand::ShowSystemMessage { text })
        })?,
    )?;

    let open_chat_state = state.clone();
    ui.set(
        "open_chat",
        lua.create_function(move |_, initial_text: Option<String>| {
            let initial_text = initial_text.unwrap_or_default();
            validate_text("chat input", &initial_text, MAX_CHAT_INPUT_BYTES, true)?;
            open_chat_state.push(UiCommand::OpenChat { initial_text })
        })?,
    )?;

    let close_chat_state = state.clone();
    ui.set(
        "close_chat",
        lua.create_function(move |_, ()| close_chat_state.push(UiCommand::CloseChat))?,
    )?;

    let hud_state = state.clone();
    ui.set(
        "set_hud_visible",
        lua.create_function(move |_, visible: bool| {
            hud_state.push(UiCommand::SetHudVisible { visible })
        })?,
    )?;

    ui.set(
        "set_crosshair_visible",
        lua.create_function(move |_, visible: bool| {
            state.push(UiCommand::SetCrosshairVisible { visible })
        })?,
    )
}

fn validate_text(label: &str, text: &str, max_bytes: usize, allow_empty: bool) -> mlua::Result<()> {
    if (!allow_empty && text.trim().is_empty()) || text.len() > max_bytes {
        let empty_rule = if allow_empty {
            "at most"
        } else {
            "between 1 and"
        };
        return Err(mlua::Error::RuntimeError(format!(
            "{label} must contain {empty_rule} {max_bytes} UTF-8 bytes"
        )));
    }
    if text.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err(mlua::Error::RuntimeError(format!(
            "{label} contains unsupported control characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::permissions::PermissionPolicy;

    fn permissions(requested: &[Permission]) -> PermissionSet {
        PermissionSet::resolve("ui_test", requested, &PermissionPolicy::default())
    }

    fn install_for_test(lua: &Lua, requested: &[Permission], state: UiApiState) -> Table {
        let game = lua.create_table().unwrap();
        install(lua, &game, &permissions(requested), state).unwrap();
        lua.globals().set("game", game.clone()).unwrap();
        game
    }

    #[test]
    fn reads_owned_snapshots_without_exposing_mutable_host_state() {
        let lua = Lua::new();
        let state = UiApiState::new(UiSnapshot {
            screen: UiScreenSnapshot {
                id: "pause".into(),
                title: Some("Game menu".into()),
                in_game: true,
                paused: true,
            },
            chat: UiChatSnapshot {
                open: true,
                input: "hello".into(),
                visible_messages: 5,
                unread_messages: 2,
            },
            inventory: UiInventorySnapshot {
                open: true,
                window_id: Some(0),
                kind: Some("player".into()),
                title: Some("Inventory".into()),
                slot_count: 45,
                selected_hotbar_slot: 3,
                cursor_item: None,
                slots: vec![UiItemSnapshot {
                    slot: 36,
                    id: "minecraft:stone".into(),
                    count: 12,
                    damage: 0,
                    display_name: None,
                }],
            },
            gui: UiGuiSnapshot::default(),
            window: UiWindowSnapshot {
                width: 1280,
                height: 720,
                framebuffer_width: 2560,
                framebuffer_height: 1440,
                scale_factor: 2.0,
                focused: true,
                fullscreen: false,
            },
        });
        install_for_test(&lua, &[Permission::UiRead], state.clone());

        let values: (String, String, i64, i64, i64) = lua
            .load(
                r#"
                local first = game.ui.snapshot()
                first.screen.id = "forged"
                first.inventory.slots[1].count = 99
                local second = game.ui.snapshot()
                return second.screen.id, game.ui.chat().input,
                    game.ui.inventory().slots[1].count,
                    game.ui.window().framebuffer_width,
                    game.ui.gui().scale
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(values, ("pause".into(), "hello".into(), 12, 2560, 1));
        assert_eq!(state.snapshot().inventory.slots[0].count, 12);
    }

    #[test]
    fn permissions_prune_read_and_modify_functions_at_install_time() {
        let lua = Lua::new();
        let read_state = UiApiState::default();
        let read_game = install_for_test(&lua, &[Permission::UiRead], read_state);
        let read_ui: Table = read_game.get("ui").unwrap();
        assert!(read_ui.contains_key("snapshot").unwrap());
        assert!(!read_ui.contains_key("open_chat").unwrap());

        let lua = Lua::new();
        let modify_state = UiApiState::default();
        let modify_game = install_for_test(&lua, &[Permission::UiModify], modify_state);
        let modify_ui: Table = modify_game.get("ui").unwrap();
        assert!(!modify_ui.contains_key("snapshot").unwrap());
        assert!(modify_ui.contains_key("open_chat").unwrap());

        let lua = Lua::new();
        let game = install_for_test(&lua, &[], UiApiState::default());
        assert!(!game.contains_key("ui").unwrap());
    }

    #[test]
    fn modify_api_only_queues_bounded_local_ui_commands() {
        let lua = Lua::new();
        let state = UiApiState::default();
        install_for_test(&lua, &[Permission::UiModify], state.clone());
        lua.load(
            r#"
            game.ui.show_system_message("local only")
            game.ui.open_chat("draft")
            game.ui.close_chat()
            game.ui.set_hud_visible(false)
            game.ui.set_crosshair_visible(false)
            "#,
        )
        .exec()
        .unwrap();

        assert_eq!(
            state.drain_commands(),
            vec![
                UiCommand::ShowSystemMessage {
                    text: "local only".into()
                },
                UiCommand::OpenChat {
                    initial_text: "draft".into()
                },
                UiCommand::CloseChat,
                UiCommand::SetHudVisible { visible: false },
                UiCommand::SetCrosshairVisible { visible: false },
            ]
        );

        assert!(lua
            .load("game.ui.show_system_message(string.rep('x', 4097))")
            .exec()
            .is_err());
        assert!(lua
            .load("game.ui.open_chat(string.char(0))")
            .exec()
            .is_err());
        assert!(state.drain_commands().is_empty());
    }

    #[test]
    fn pending_command_limit_prevents_ui_spam() {
        let lua = Lua::new();
        let state = UiApiState::default();
        install_for_test(&lua, &[Permission::UiModify], state.clone());
        for _ in 0..MAX_PENDING_UI_COMMANDS {
            lua.load("game.ui.close_chat()").exec().unwrap();
        }
        assert!(lua.load("game.ui.close_chat()").exec().is_err());
        assert_eq!(state.pending_command_count(), MAX_PENDING_UI_COMMANDS);
    }
}
