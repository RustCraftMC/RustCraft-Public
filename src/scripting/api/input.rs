//! Read-only input snapshots and controlled event-consumption requests.
//!
//! Lua sees logical actions, never raw device handles. The consume API can only
//! consume an action edge that is present in the current snapshot; it cannot
//! synthesize held movement, clicks, attacks, or mouse motion.

use mlua::{Lua, Table};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::client::keybind::{Action, InputState};
use crate::scripting::permissions::{Permission, PermissionSet};

pub const OBSERVE_PERMISSION: &str = "input.observe";
pub const CONSUME_PERMISSION: &str = "input.consume";
pub const MAX_CONSUME_REQUESTS_PER_SNAPSHOT: usize = 64;

pub type SharedInputState = Rc<RefCell<InputApiState>>;

/// Stable Lua names for every logical client action.
pub const ACTION_NAMES: &[(Action, &str)] = &[
    (Action::Forward, "forward"),
    (Action::Backward, "backward"),
    (Action::StrafeLeft, "strafe_left"),
    (Action::StrafeRight, "strafe_right"),
    (Action::Jump, "jump"),
    (Action::Sneak, "sneak"),
    (Action::Sprint, "sprint"),
    (Action::ToggleSprint, "toggle_sprint"),
    (Action::Attack, "attack"),
    (Action::Use, "use"),
    (Action::Inventory, "inventory"),
    (Action::DropItem, "drop_item"),
    (Action::Hotbar1, "hotbar_1"),
    (Action::Hotbar2, "hotbar_2"),
    (Action::Hotbar3, "hotbar_3"),
    (Action::Hotbar4, "hotbar_4"),
    (Action::Hotbar5, "hotbar_5"),
    (Action::Hotbar6, "hotbar_6"),
    (Action::Hotbar7, "hotbar_7"),
    (Action::Hotbar8, "hotbar_8"),
    (Action::Hotbar9, "hotbar_9"),
    (Action::HotbarNext, "hotbar_next"),
    (Action::HotbarPrev, "hotbar_previous"),
    (Action::Chat, "chat"),
    (Action::Command, "command"),
    (Action::PlayerList, "player_list"),
    (Action::Pause, "pause"),
    (Action::ToggleFlying, "toggle_flying"),
    (Action::TogglePerspective, "toggle_perspective"),
];

#[derive(Clone, Debug, Default)]
pub struct InputSnapshot {
    pub held: HashSet<Action>,
    pub just_pressed: HashSet<Action>,
    pub just_released: HashSet<Action>,
    pub mouse_delta: [f64; 2],
    pub cursor_captured: bool,
}

impl InputSnapshot {
    /// Copies the game's mutable input state into an API-safe owned snapshot.
    pub fn from_input_state(
        input: &InputState,
        mouse_delta: (f64, f64),
        cursor_captured: bool,
    ) -> Self {
        Self {
            held: true_actions(&input.held),
            just_pressed: true_actions(&input.just_pressed),
            just_released: true_actions(&input.just_released),
            mouse_delta: [mouse_delta.0, mouse_delta.1],
            cursor_captured,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEdge {
    Pressed,
    Released,
}

impl InputEdge {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pressed => "pressed",
            Self::Released => "released",
        }
    }
}

/// A request for the host to suppress one already-observed input edge.
///
/// There is intentionally no value/pressed field that could inject a new
/// action. The generation ties the request to exactly one published snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputConsumeRequest {
    pub requester: String,
    pub action: Action,
    pub edge: InputEdge,
    pub snapshot_generation: u64,
}

#[derive(Clone, Debug, Default)]
pub struct InputApiState {
    snapshot: InputSnapshot,
    generation: u64,
    consume_requests: Vec<InputConsumeRequest>,
}

impl InputApiState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes a new input snapshot and expires undrained requests from the
    /// previous one.
    pub fn set_snapshot(&mut self, mut snapshot: InputSnapshot) {
        for value in &mut snapshot.mouse_delta {
            if !value.is_finite() || value.abs() > 1_000_000.0 {
                *value = 0.0;
            }
        }
        self.snapshot = snapshot;
        self.generation = self.generation.wrapping_add(1).max(1);
        self.consume_requests.clear();
    }

    pub fn clear(&mut self) {
        self.set_snapshot(InputSnapshot::default());
    }

    pub fn snapshot(&self) -> &InputSnapshot {
        &self.snapshot
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Removes all requests so the app can apply them before normal input
    /// handling. Requests are bounded and refer only to observed edges.
    pub fn drain_consume_requests(&mut self) -> Vec<InputConsumeRequest> {
        std::mem::take(&mut self.consume_requests)
    }

    pub fn pending_consume_requests(&self) -> &[InputConsumeRequest] {
        &self.consume_requests
    }

    fn request_consumption(
        &mut self,
        requester: &str,
        action: Action,
        edge: InputEdge,
    ) -> Result<bool, &'static str> {
        let observed = match edge {
            InputEdge::Pressed => self.snapshot.just_pressed.contains(&action),
            InputEdge::Released => self.snapshot.just_released.contains(&action),
        };
        if !observed {
            return Ok(false);
        }
        if self.consume_requests.iter().any(|request| {
            request.requester == requester
                && request.action == action
                && request.edge == edge
                && request.snapshot_generation == self.generation
        }) {
            return Ok(false);
        }
        if self.consume_requests.len() >= MAX_CONSUME_REQUESTS_PER_SNAPSHOT {
            return Err("input consume request limit exceeded (64 per snapshot)");
        }
        self.consume_requests.push(InputConsumeRequest {
            requester: requester.into(),
            action,
            edge,
            snapshot_generation: self.generation,
        });
        Ok(true)
    }
}

pub fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    state: SharedInputState,
    mod_id: &str,
) -> mlua::Result<()> {
    if !permissions.contains(Permission::InputObserve) {
        return Ok(());
    }

    let input = lua.create_table()?;

    input.set(
        "actions",
        lua.create_function(|lua, ()| {
            let actions = lua.create_table_with_capacity(ACTION_NAMES.len(), 0)?;
            for (index, (_, name)) in ACTION_NAMES.iter().enumerate() {
                actions.raw_set(index + 1, *name)?;
            }
            Ok(actions)
        })?,
    )?;

    let down_state = state.clone();
    input.set(
        "is_down",
        lua.create_function(move |_, name: String| {
            let action = action_from_name(&name)?;
            Ok(down_state.borrow().snapshot().held.contains(&action))
        })?,
    )?;

    let pressed_state = state.clone();
    input.set(
        "was_pressed",
        lua.create_function(move |_, name: String| {
            let action = action_from_name(&name)?;
            Ok(pressed_state
                .borrow()
                .snapshot()
                .just_pressed
                .contains(&action))
        })?,
    )?;

    let released_state = state.clone();
    input.set(
        "was_released",
        lua.create_function(move |_, name: String| {
            let action = action_from_name(&name)?;
            Ok(released_state
                .borrow()
                .snapshot()
                .just_released
                .contains(&action))
        })?,
    )?;

    let mouse_state = state.clone();
    input.set(
        "mouse",
        lua.create_function(move |lua, ()| {
            let snapshot = mouse_state.borrow().snapshot().clone();
            mouse_table(lua, &snapshot)
        })?,
    )?;

    let snapshot_state = state.clone();
    input.set(
        "snapshot",
        lua.create_function(move |lua, ()| {
            let state = snapshot_state.borrow();
            snapshot_table(lua, state.snapshot(), state.generation())
        })?,
    )?;

    if permissions.contains(Permission::InputConsume) {
        let requester: String = mod_id.chars().take(128).collect();
        input.set(
            "consume",
            lua.create_function(move |_, (name, edge): (String, Option<String>)| {
                let action = action_from_name(&name)?;
                let edge = input_edge(edge.as_deref().unwrap_or("pressed"))?;
                state
                    .borrow_mut()
                    .request_consumption(&requester, action, edge)
                    .map_err(|message| mlua::Error::RuntimeError(message.into()))
            })?,
        )?;
    }

    game.set("input", input)
}

pub fn canonical_action_name(action: Action) -> &'static str {
    ACTION_NAMES
        .iter()
        .find_map(|(candidate, name)| (*candidate == action).then_some(*name))
        .expect("every Action variant must have a canonical Lua name")
}

pub fn action_from_canonical_name(name: &str) -> Option<Action> {
    ACTION_NAMES
        .iter()
        .find_map(|(action, candidate)| (*candidate == name).then_some(*action))
}

fn action_from_name(name: &str) -> mlua::Result<Action> {
    action_from_canonical_name(name).ok_or_else(|| {
        mlua::Error::RuntimeError(format!(
            "unknown input action '{name}'; use game.input.actions() for canonical names"
        ))
    })
}

fn input_edge(value: &str) -> mlua::Result<InputEdge> {
    match value {
        "pressed" => Ok(InputEdge::Pressed),
        "released" => Ok(InputEdge::Released),
        _ => Err(mlua::Error::RuntimeError(
            "input consume edge must be 'pressed' or 'released'".into(),
        )),
    }
}

fn snapshot_table(lua: &Lua, snapshot: &InputSnapshot, generation: u64) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("generation", generation)?;
    let actions = lua.create_table_with_capacity(0, ACTION_NAMES.len())?;
    for &(action, name) in ACTION_NAMES {
        let action_state = lua.create_table()?;
        action_state.set("down", snapshot.held.contains(&action))?;
        action_state.set("pressed", snapshot.just_pressed.contains(&action))?;
        action_state.set("released", snapshot.just_released.contains(&action))?;
        actions.set(name, action_state)?;
    }
    table.set("actions", actions)?;
    table.set("mouse", mouse_table(lua, snapshot)?)?;
    Ok(table)
}

fn mouse_table(lua: &Lua, snapshot: &InputSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("delta_x", snapshot.mouse_delta[0])?;
    table.set("delta_y", snapshot.mouse_delta[1])?;
    table.set("captured", snapshot.cursor_captured)?;
    Ok(table)
}

fn true_actions(values: &std::collections::HashMap<Action, bool>) -> HashSet<Action> {
    values
        .iter()
        .filter_map(|(action, active)| active.then_some(*action))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::permissions::PermissionPolicy;

    fn permissions(requested: &[Permission]) -> PermissionSet {
        PermissionSet::resolve("input-test", requested, &PermissionPolicy::default())
    }

    fn installed(requested: &[Permission], snapshot: InputSnapshot) -> (Lua, SharedInputState) {
        let lua = Lua::new();
        let game = lua.create_table().unwrap();
        let state = Rc::new(RefCell::new(InputApiState::new()));
        state.borrow_mut().set_snapshot(snapshot);
        install(
            &lua,
            &game,
            &permissions(requested),
            state.clone(),
            "input-test",
        )
        .unwrap();
        lua.globals().set("game", game).unwrap();
        (lua, state)
    }

    #[test]
    fn input_module_requires_observe_permission() {
        let (lua, _state) = installed(&[], InputSnapshot::default());
        lua.load("assert(game.input == nil)").exec().unwrap();
    }

    #[test]
    fn observes_canonical_actions_and_mouse_snapshot() {
        let mut snapshot = InputSnapshot {
            mouse_delta: [3.5, -2.0],
            cursor_captured: true,
            ..InputSnapshot::default()
        };
        snapshot.held.insert(Action::Forward);
        snapshot.just_pressed.insert(Action::Attack);
        snapshot.just_released.insert(Action::Use);
        let (lua, _state) = installed(&[Permission::InputObserve], snapshot);
        lua.load(
            r#"
                assert(game.input.is_down("forward"))
                assert(not game.input.is_down("backward"))
                assert(game.input.was_pressed("attack"))
                assert(game.input.was_released("use"))
                local mouse = game.input.mouse()
                assert(mouse.delta_x == 3.5 and mouse.delta_y == -2)
                assert(mouse.captured == true)
                local snapshot = game.input.snapshot()
                assert(snapshot.actions.forward.down == true)
                assert(snapshot.actions.attack.pressed == true)
                assert(not pcall(game.input.is_down, "not_an_action"))
                assert(game.input.consume == nil)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn consume_only_queues_an_observed_edge_once() {
        let mut snapshot = InputSnapshot::default();
        snapshot.just_pressed.insert(Action::Attack);
        let (lua, state) = installed(
            &[Permission::InputObserve, Permission::InputConsume],
            snapshot,
        );
        lua.load(
            r#"
                assert(game.input.consume("attack", "pressed") == true)
                assert(game.input.consume("attack", "pressed") == false)
                assert(game.input.consume("forward", "pressed") == false)
                assert(game.input.consume("attack", "released") == false)
                assert(not pcall(game.input.consume, "attack", "held"))
            "#,
        )
        .exec()
        .unwrap();

        let requests = state.borrow_mut().drain_consume_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].requester, "input-test");
        assert_eq!(requests[0].action, Action::Attack);
        assert_eq!(requests[0].edge, InputEdge::Pressed);
        assert_eq!(requests[0].snapshot_generation, 1);
    }

    #[test]
    fn publishing_next_snapshot_expires_stale_consumption() {
        let mut snapshot = InputSnapshot::default();
        snapshot.just_pressed.insert(Action::Inventory);
        let (lua, state) = installed(
            &[Permission::InputObserve, Permission::InputConsume],
            snapshot,
        );
        lua.load("assert(game.input.consume('inventory'))")
            .exec()
            .unwrap();
        assert_eq!(state.borrow().pending_consume_requests().len(), 1);
        state.borrow_mut().set_snapshot(InputSnapshot::default());
        assert!(state.borrow().pending_consume_requests().is_empty());
        assert_eq!(state.borrow().generation(), 2);
    }

    #[test]
    fn every_client_action_has_a_round_trip_canonical_name() {
        for &(action, name) in ACTION_NAMES {
            assert_eq!(action_from_canonical_name(name), Some(action));
            assert_eq!(canonical_action_name(action), name);
        }
    }
}
