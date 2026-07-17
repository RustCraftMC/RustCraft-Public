//! Keybinding system — configurable key mappings + input state.
//!
//! Single source of truth for all input handling.
//! Player code uses Action, never raw KeyCode.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Actions that can be bound to keys or mouse buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    // Movement
    Forward,
    Backward,
    StrafeLeft,
    StrafeRight,
    Jump,
    Sneak,
    Sprint,
    ToggleSprint,
    // Interaction
    Attack,
    Use,
    // Inventory
    Inventory,
    DropItem,
    // Hotbar
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    Hotbar9,
    HotbarNext,
    HotbarPrev,
    // Misc
    Chat,
    Command,
    PlayerList,
    Pause,
    ToggleFlying,
    TogglePerspective,
}

/// Which bindings are being edited in the controls screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlDevice {
    KeyboardMouse,
    Gamepad,
}

/// A normalized physical gamepad button.  Keeping this independent of gilrs
/// makes options.json portable between the supported platforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    LeftTrigger,
    RightTrigger,
    LeftTrigger2,
    RightTrigger2,
    LeftThumb,
    RightThumb,
    Start,
}

/// One configurable controller input.  Stick directions are bindings too, so
/// movement is not a hidden hard-coded exception.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadBinding {
    Button(GamepadButton),
    LeftStickUp,
    LeftStickDown,
    LeftStickLeft,
    LeftStickRight,
}

impl GamepadBinding {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Button(GamepadButton::South) => "A / Cross",
            Self::Button(GamepadButton::East) => "B / Circle",
            Self::Button(GamepadButton::West) => "X / Square",
            Self::Button(GamepadButton::North) => "Y / Triangle",
            Self::Button(GamepadButton::LeftTrigger) => "Left Bumper",
            Self::Button(GamepadButton::RightTrigger) => "Right Bumper",
            Self::Button(GamepadButton::LeftTrigger2) => "Left Trigger",
            Self::Button(GamepadButton::RightTrigger2) => "Right Trigger",
            Self::Button(GamepadButton::LeftThumb) => "Left Stick Click",
            Self::Button(GamepadButton::RightThumb) => "Right Stick Click",
            Self::Button(GamepadButton::Start) => "Menu / Start",
            Self::LeftStickUp => "Left Stick Up",
            Self::LeftStickDown => "Left Stick Down",
            Self::LeftStickLeft => "Left Stick Left",
            Self::LeftStickRight => "Left Stick Right",
        }
    }
}

impl Action {
    pub const fn all_bindable() -> &'static [Action] {
        &[
            Action::Attack,
            Action::Use,
            Action::Forward,
            Action::Backward,
            Action::StrafeLeft,
            Action::StrafeRight,
            Action::Jump,
            Action::Sneak,
            Action::Sprint,
            Action::Inventory,
            Action::DropItem,
            Action::Hotbar1,
            Action::Hotbar2,
            Action::Hotbar3,
            Action::Hotbar4,
            Action::Hotbar5,
            Action::Hotbar6,
            Action::Hotbar7,
            Action::Hotbar8,
            Action::Hotbar9,
            Action::HotbarNext,
            Action::HotbarPrev,
            Action::Chat,
            Action::Command,
            Action::PlayerList,
            Action::Pause,
            Action::TogglePerspective,
        ]
    }

    pub fn from_bindable_index(index: usize) -> Option<Self> {
        Self::all_bindable().get(index).copied()
    }

    pub fn bindable_index(self) -> Option<usize> {
        Self::all_bindable()
            .iter()
            .position(|action| *action == self)
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Forward => "Forward",
            Action::Backward => "Back",
            Action::StrafeLeft => "Left",
            Action::StrafeRight => "Right",
            Action::Jump => "Jump",
            Action::Sneak => "Sneak",
            Action::Sprint => "Sprint",
            Action::ToggleSprint => "Toggle Sprint",
            Action::Attack => "Attack/Destroy",
            Action::Use => "Use Item/Place Block",
            Action::Inventory => "Inventory",
            Action::DropItem => "Drop Item",
            Action::Hotbar1 => "Hotbar Slot 1",
            Action::Hotbar2 => "Hotbar Slot 2",
            Action::Hotbar3 => "Hotbar Slot 3",
            Action::Hotbar4 => "Hotbar Slot 4",
            Action::Hotbar5 => "Hotbar Slot 5",
            Action::Hotbar6 => "Hotbar Slot 6",
            Action::Hotbar7 => "Hotbar Slot 7",
            Action::Hotbar8 => "Hotbar Slot 8",
            Action::Hotbar9 => "Hotbar Slot 9",
            Action::HotbarNext => "Next Hotbar Slot",
            Action::HotbarPrev => "Previous Hotbar Slot",
            Action::Chat => "Open Chat",
            Action::Command => "Open Command",
            Action::PlayerList => "List Players",
            Action::Pause => "Menu",
            Action::ToggleFlying => "Toggle Flying",
            Action::TogglePerspective => "Toggle Perspective",
        }
    }

    pub fn translation_key(self) -> &'static str {
        match self {
            Action::Forward => "key.forward",
            Action::Backward => "key.back",
            Action::StrafeLeft => "key.left",
            Action::StrafeRight => "key.right",
            Action::Jump => "key.jump",
            Action::Sneak => "key.sneak",
            Action::Sprint | Action::ToggleSprint => "key.sprint",
            Action::Attack => "key.attack",
            Action::Use => "key.use",
            Action::Inventory => "key.inventory",
            Action::DropItem => "key.drop",
            Action::Hotbar1 => "key.hotbar.1",
            Action::Hotbar2 => "key.hotbar.2",
            Action::Hotbar3 => "key.hotbar.3",
            Action::Hotbar4 => "key.hotbar.4",
            Action::Hotbar5 => "key.hotbar.5",
            Action::Hotbar6 => "key.hotbar.6",
            Action::Hotbar7 => "key.hotbar.7",
            Action::Hotbar8 => "key.hotbar.8",
            Action::Hotbar9 => "key.hotbar.9",
            Action::HotbarNext => "key.hotbar.next",
            Action::HotbarPrev => "key.hotbar.previous",
            Action::Chat => "key.chat",
            Action::Command => "key.command",
            Action::PlayerList => "key.playerlist",
            Action::Pause => "key.pause",
            Action::ToggleFlying => "key.toggleFlying",
            Action::TogglePerspective => "key.togglePerspective",
        }
    }

    pub fn category_key(self) -> &'static str {
        match self {
            Action::Attack | Action::Use => "key.categories.gameplay",
            Action::Forward
            | Action::Backward
            | Action::StrafeLeft
            | Action::StrafeRight
            | Action::Jump
            | Action::Sneak
            | Action::Sprint
            | Action::ToggleSprint
            | Action::ToggleFlying => "key.categories.movement",
            Action::Inventory | Action::DropItem => "key.categories.inventory",
            Action::Hotbar1
            | Action::Hotbar2
            | Action::Hotbar3
            | Action::Hotbar4
            | Action::Hotbar5
            | Action::Hotbar6
            | Action::Hotbar7
            | Action::Hotbar8
            | Action::Hotbar9
            | Action::HotbarNext
            | Action::HotbarPrev => "key.categories.inventory",
            Action::Chat
            | Action::Command
            | Action::PlayerList
            | Action::Pause
            | Action::TogglePerspective => "key.categories.ui",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingInput {
    Key(KeyCode),
    Mouse(MouseButton),
    Unbound,
}

/// Keybinding manager.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub keys: HashMap<Action, KeyCode>,
    pub mouse: HashMap<Action, MouseButton>,
    pub gamepad: HashMap<Action, GamepadBinding>,
}

impl KeyBindings {
    pub fn defaults() -> Self {
        let mut keys = HashMap::new();
        keys.insert(Action::Forward, KeyCode::KeyW);
        keys.insert(Action::Backward, KeyCode::KeyS);
        keys.insert(Action::StrafeLeft, KeyCode::KeyA);
        keys.insert(Action::StrafeRight, KeyCode::KeyD);
        keys.insert(Action::Jump, KeyCode::Space);
        keys.insert(Action::Sneak, KeyCode::ShiftLeft);
        keys.insert(Action::Sprint, KeyCode::ControlLeft);
        keys.insert(Action::Inventory, KeyCode::KeyE);
        keys.insert(Action::DropItem, KeyCode::KeyQ);
        keys.insert(Action::Hotbar1, KeyCode::Digit1);
        keys.insert(Action::Hotbar2, KeyCode::Digit2);
        keys.insert(Action::Hotbar3, KeyCode::Digit3);
        keys.insert(Action::Hotbar4, KeyCode::Digit4);
        keys.insert(Action::Hotbar5, KeyCode::Digit5);
        keys.insert(Action::Hotbar6, KeyCode::Digit6);
        keys.insert(Action::Hotbar7, KeyCode::Digit7);
        keys.insert(Action::Hotbar8, KeyCode::Digit8);
        keys.insert(Action::Hotbar9, KeyCode::Digit9);
        keys.insert(Action::Chat, KeyCode::KeyT);
        keys.insert(Action::Command, KeyCode::Slash);
        keys.insert(Action::PlayerList, KeyCode::Tab);
        keys.insert(Action::Pause, KeyCode::Escape);
        keys.insert(Action::TogglePerspective, KeyCode::F5);

        let mut mouse = HashMap::new();
        mouse.insert(Action::Attack, MouseButton::Left);
        mouse.insert(Action::Use, MouseButton::Right);

        let mut gamepad = HashMap::new();
        gamepad.insert(Action::Forward, GamepadBinding::LeftStickUp);
        gamepad.insert(Action::Backward, GamepadBinding::LeftStickDown);
        gamepad.insert(Action::StrafeLeft, GamepadBinding::LeftStickLeft);
        gamepad.insert(Action::StrafeRight, GamepadBinding::LeftStickRight);
        gamepad.insert(Action::Jump, GamepadBinding::Button(GamepadButton::South));
        gamepad.insert(Action::Sneak, GamepadBinding::Button(GamepadButton::East));
        gamepad.insert(
            Action::Inventory,
            GamepadBinding::Button(GamepadButton::West),
        );
        gamepad.insert(
            Action::DropItem,
            GamepadBinding::Button(GamepadButton::North),
        );
        gamepad.insert(
            Action::Attack,
            GamepadBinding::Button(GamepadButton::RightTrigger2),
        );
        gamepad.insert(
            Action::Use,
            GamepadBinding::Button(GamepadButton::LeftTrigger2),
        );
        gamepad.insert(
            Action::HotbarPrev,
            GamepadBinding::Button(GamepadButton::LeftTrigger),
        );
        gamepad.insert(
            Action::HotbarNext,
            GamepadBinding::Button(GamepadButton::RightTrigger),
        );
        gamepad.insert(
            Action::Sprint,
            GamepadBinding::Button(GamepadButton::LeftThumb),
        );
        gamepad.insert(
            Action::TogglePerspective,
            GamepadBinding::Button(GamepadButton::RightThumb),
        );
        gamepad.insert(Action::Pause, GamepadBinding::Button(GamepadButton::Start));

        KeyBindings {
            keys,
            mouse,
            gamepad,
        }
    }

    pub fn merged_with_defaults(mut self) -> Self {
        let defaults = Self::defaults();
        for action in Action::all_bindable() {
            if !self.keys.contains_key(action) && !self.mouse.contains_key(action) {
                if let Some(key) = defaults.keys.get(action) {
                    self.keys.insert(*action, *key);
                }
                if let Some(mouse) = defaults.mouse.get(action) {
                    self.mouse.insert(*action, *mouse);
                }
            }
            if !self.gamepad.contains_key(action) {
                if let Some(binding) = defaults.gamepad.get(action) {
                    self.gamepad.insert(*action, *binding);
                }
            }
        }
        self
    }

    pub fn action_for_key(&self, key: KeyCode) -> Option<Action> {
        Action::all_bindable()
            .iter()
            .copied()
            .find(|action| self.keys.get(action).copied() == Some(key))
    }

    pub fn action_for_mouse(&self, button: MouseButton) -> Option<Action> {
        Action::all_bindable()
            .iter()
            .copied()
            .find(|action| self.mouse.get(action).copied() == Some(button))
    }

    pub fn binding_for(&self, action: Action) -> BindingInput {
        if let Some(key) = self.keys.get(&action) {
            BindingInput::Key(*key)
        } else if let Some(mouse) = self.mouse.get(&action) {
            BindingInput::Mouse(*mouse)
        } else {
            BindingInput::Unbound
        }
    }

    pub fn gamepad_binding_for(&self, action: Action) -> Option<GamepadBinding> {
        self.gamepad.get(&action).copied()
    }

    pub fn set_key(&mut self, action: Action, key: KeyCode) {
        self.mouse.remove(&action);
        self.keys.insert(action, key);
    }

    pub fn set_mouse(&mut self, action: Action, button: MouseButton) {
        self.keys.remove(&action);
        self.mouse.insert(action, button);
    }

    pub fn clear(&mut self, action: Action) {
        self.keys.remove(&action);
        self.mouse.remove(&action);
    }

    pub fn clear_gamepad(&mut self, action: Action) {
        self.gamepad.remove(&action);
    }

    pub fn set_gamepad(&mut self, action: Action, binding: GamepadBinding) {
        self.gamepad.insert(action, binding);
    }

    pub fn reset_action(&mut self, action: Action) {
        let defaults = Self::defaults();
        match defaults.binding_for(action) {
            BindingInput::Key(key) => self.set_key(action, key),
            BindingInput::Mouse(button) => self.set_mouse(action, button),
            BindingInput::Unbound => self.clear(action),
        }
    }

    pub fn reset_gamepad_action(&mut self, action: Action) {
        match Self::defaults().gamepad_binding_for(action) {
            Some(binding) => self.set_gamepad(action, binding),
            None => self.clear_gamepad(action),
        }
    }

    pub fn has_conflict(&self, action: Action) -> bool {
        let binding = self.binding_for(action);
        if binding == BindingInput::Unbound {
            return false;
        }
        Action::all_bindable()
            .iter()
            .filter(|candidate| self.binding_for(**candidate) == binding)
            .count()
            > 1
    }

    pub fn binding_label(&self, action: Action) -> String {
        match self.binding_for(action) {
            BindingInput::Key(key) => key_label(key),
            BindingInput::Mouse(button) => mouse_button_label(button),
            BindingInput::Unbound => "NONE".to_string(),
        }
    }

    pub fn gamepad_binding_label(&self, action: Action) -> String {
        self.gamepad_binding_for(action)
            .map(|binding| binding.label().to_string())
            .unwrap_or_else(|| "NONE".to_string())
    }

    pub fn has_gamepad_conflict(&self, action: Action) -> bool {
        let Some(binding) = self.gamepad_binding_for(action) else {
            return false;
        };
        Action::all_bindable()
            .iter()
            .filter(|candidate| self.gamepad_binding_for(**candidate) == Some(binding))
            .count()
            > 1
    }

    pub fn control_rows(
        &self,
        rebinding_action: Option<Action>,
        i18n: &crate::ui::i18n::I18n,
    ) -> Vec<crate::render::ControlBindingRow> {
        self.control_rows_for_device(ControlDevice::KeyboardMouse, rebinding_action, i18n)
    }

    pub fn control_rows_for_device(
        &self,
        device: ControlDevice,
        rebinding_action: Option<Action>,
        i18n: &crate::ui::i18n::I18n,
    ) -> Vec<crate::render::ControlBindingRow> {
        let defaults = Self::defaults();
        Action::all_bindable()
            .iter()
            .copied()
            .map(|action| crate::render::ControlBindingRow {
                action,
                label: i18n.t(action.translation_key()),
                binding: match device {
                    ControlDevice::KeyboardMouse => self.binding_label(action),
                    ControlDevice::Gamepad => self.gamepad_binding_label(action),
                },
                category: i18n.t(action.category_key()),
                conflict: match device {
                    ControlDevice::KeyboardMouse => self.has_conflict(action),
                    ControlDevice::Gamepad => self.has_gamepad_conflict(action),
                },
                listening: rebinding_action == Some(action),
                is_default: match device {
                    ControlDevice::KeyboardMouse => {
                        self.binding_for(action) == defaults.binding_for(action)
                    }
                    ControlDevice::Gamepad => {
                        self.gamepad_binding_for(action) == defaults.gamepad_binding_for(action)
                    }
                },
            })
            .collect()
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::defaults()
    }
}

pub fn key_label(key: KeyCode) -> String {
    let label = match key {
        KeyCode::KeyA => "A",
        KeyCode::KeyB => "B",
        KeyCode::KeyC => "C",
        KeyCode::KeyD => "D",
        KeyCode::KeyE => "E",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyI => "I",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyM => "M",
        KeyCode::KeyN => "N",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyR => "R",
        KeyCode::KeyS => "S",
        KeyCode::KeyT => "T",
        KeyCode::KeyU => "U",
        KeyCode::KeyV => "V",
        KeyCode::KeyW => "W",
        KeyCode::KeyX => "X",
        KeyCode::KeyY => "Y",
        KeyCode::KeyZ => "Z",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Numpad0 => "Num 0",
        KeyCode::Numpad1 => "Num 1",
        KeyCode::Numpad2 => "Num 2",
        KeyCode::Numpad3 => "Num 3",
        KeyCode::Numpad4 => "Num 4",
        KeyCode::Numpad5 => "Num 5",
        KeyCode::Numpad6 => "Num 6",
        KeyCode::Numpad7 => "Num 7",
        KeyCode::Numpad8 => "Num 8",
        KeyCode::Numpad9 => "Num 9",
        KeyCode::Space => "Space",
        KeyCode::ShiftLeft => "Left Shift",
        KeyCode::ShiftRight => "Right Shift",
        KeyCode::ControlLeft => "Left Ctrl",
        KeyCode::ControlRight => "Right Ctrl",
        KeyCode::AltLeft => "Left Alt",
        KeyCode::AltRight => "Right Alt",
        KeyCode::SuperLeft => "Left Super",
        KeyCode::SuperRight => "Right Super",
        KeyCode::Enter => "Enter",
        KeyCode::NumpadEnter => "Num Enter",
        KeyCode::Escape => "Esc",
        KeyCode::Tab => "Tab",
        KeyCode::Backspace => "Backspace",
        KeyCode::Delete => "Delete",
        KeyCode::Insert => "Insert",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "Page Up",
        KeyCode::PageDown => "Page Down",
        KeyCode::ArrowUp => "Up",
        KeyCode::ArrowDown => "Down",
        KeyCode::ArrowLeft => "Left",
        KeyCode::ArrowRight => "Right",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Backslash => "\\",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::Backquote => "`",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::CapsLock => "Caps Lock",
        KeyCode::PrintScreen => "Print Screen",
        KeyCode::ScrollLock => "Scroll Lock",
        KeyCode::Pause => "Pause",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::F13 => "F13",
        KeyCode::F14 => "F14",
        KeyCode::F15 => "F15",
        KeyCode::F16 => "F16",
        KeyCode::F17 => "F17",
        KeyCode::F18 => "F18",
        KeyCode::F19 => "F19",
        KeyCode::F20 => "F20",
        KeyCode::F21 => "F21",
        KeyCode::F22 => "F22",
        KeyCode::F23 => "F23",
        KeyCode::F24 => "F24",
        KeyCode::NumpadAdd => "Num +",
        KeyCode::NumpadSubtract => "Num -",
        KeyCode::NumpadMultiply => "Num *",
        KeyCode::NumpadDivide => "Num /",
        KeyCode::NumpadDecimal => "Num .",
        _ => return format!("{:?}", key),
    };
    label.to_string()
}

pub fn mouse_button_label(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "Button 1".to_string(),
        MouseButton::Right => "Button 2".to_string(),
        MouseButton::Middle => "Button 3".to_string(),
        MouseButton::Back => "Button 4".to_string(),
        MouseButton::Forward => "Button 5".to_string(),
        MouseButton::Other(index) => format!("Button {}", index + 1),
    }
}

/// Tracks the state of all actions each tick.
/// This is the single source of truth for input — Player reads from here.
#[derive(Default)]
pub struct InputState {
    /// Which actions are currently held down.
    pub held: HashMap<Action, bool>,
    /// Keyboard/mouse state, retained separately so releasing one input source
    /// cannot release an action still held on a gamepad.
    physical_held: HashMap<Action, bool>,
    gamepad_held: HashSet<Action>,
    /// Which actions were just pressed this frame (cleared each tick).
    pub just_pressed: HashMap<Action, bool>,
    /// Which actions were just released this frame (cleared each tick).
    pub just_released: HashMap<Action, bool>,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call when a key or mouse button is pressed.
    pub fn on_key_down(&mut self, action: Action) {
        self.physical_held.insert(action, true);
        self.refresh_held(action);
    }

    /// Call when a key or mouse button is released.
    pub fn on_key_up(&mut self, action: Action) {
        self.physical_held.insert(action, false);
        self.refresh_held(action);
    }

    /// Merge held gamepad actions with keyboard and mouse state.  Edge events
    /// are generated only when the combined action state changes.
    pub fn set_gamepad_held(&mut self, actions: HashSet<Action>) {
        let changed: HashSet<Action> = self.gamepad_held.union(&actions).copied().collect();
        self.gamepad_held = actions;
        for action in changed {
            self.refresh_held(action);
        }
    }

    fn refresh_held(&mut self, action: Action) {
        let was_held = self.is_held(action);
        let is_held = *self.physical_held.get(&action).unwrap_or(&false)
            || self.gamepad_held.contains(&action);
        self.held.insert(action, is_held);
        if is_held && !was_held {
            self.just_pressed.insert(action, true);
        } else if !is_held && was_held {
            self.just_released.insert(action, true);
        }
    }

    /// Call once per tick to clear just_pressed/just_released.
    pub fn tick_reset(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }

    /// Is the action currently held?
    pub fn is_held(&self, action: Action) -> bool {
        *self.held.get(&action).unwrap_or(&false)
    }

    /// Was the action just pressed this tick?
    pub fn is_just_pressed(&self, action: Action) -> bool {
        *self.just_pressed.get(&action).unwrap_or(&false)
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, GamepadBinding, GamepadButton, KeyBindings};

    #[test]
    fn gamepad_bindings_round_trip_through_options_json() {
        let mut bindings = KeyBindings::defaults();
        bindings.set_gamepad(
            Action::Jump,
            GamepadBinding::Button(GamepadButton::RightThumb),
        );

        let json = serde_json::to_string(&bindings).unwrap();
        let loaded: KeyBindings = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.gamepad_binding_for(Action::Jump),
            Some(GamepadBinding::Button(GamepadButton::RightThumb))
        );
    }
}
