//! Cross-platform gamepad polling. `gilrs` normalizes controller layouts
//! across Windows, Linux/BSD and macOS, including hot-plugging.

use crate::client::keybind::{Action, GamepadBinding, GamepadButton, KeyBindings};
use gilrs::{Axis, Button, Event, GamepadId, Gilrs};
use std::collections::HashSet;

const STICK_DEADZONE: f32 = 0.18;
const REBIND_STICK_THRESHOLD: f32 = 0.65;

/// The state to merge into the game's action input for this frame.
#[derive(Default)]
pub struct GamepadFrame {
    pub held: HashSet<Action>,
    pub pressed: HashSet<Action>,
    pub released: HashSet<Action>,
    /// Newly actuated physical bindings, used by the controls screen.
    pub binding_pressed: Vec<GamepadBinding>,
    /// True only when a controller generated meaningful input this frame.
    pub used: bool,
    pub look_x: f32,
    pub look_y: f32,
    /// Edge-triggered D-pad/left-stick navigation for menus and inventories.
    pub navigation: Option<[f32; 2]>,
}

/// Cross-platform gamepad source. Failure to initialize is non-fatal so a
/// missing native gamepad backend never prevents keyboard/mouse play.
pub struct GamepadInput {
    gilrs: Option<Gilrs>,
    active_gamepad: Option<GamepadId>,
    previous_held: HashSet<Action>,
    previous_physical_held: HashSet<GamepadBinding>,
    previous_navigation: Option<[f32; 2]>,
}

impl GamepadInput {
    pub fn new() -> Self {
        let gilrs = match Gilrs::new() {
            Ok(gilrs) => Some(gilrs),
            Err(error) => {
                log::warn!("gamepad support unavailable: {error}");
                None
            }
        };
        Self {
            gilrs,
            active_gamepad: None,
            previous_held: HashSet::new(),
            previous_physical_held: HashSet::new(),
            previous_navigation: None,
        }
    }

    /// Poll the most recently used connected controller using the saved
    /// bindings rather than a hard-coded layout.
    pub fn poll(&mut self, bindings: &KeyBindings) -> GamepadFrame {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return GamepadFrame::default();
        };

        let mut received_event = false;
        while let Some(Event { id, .. }) = gilrs.next_event() {
            received_event = true;
            if gilrs.gamepad(id).is_connected() {
                self.active_gamepad = Some(id);
            }
        }

        let id = self
            .active_gamepad
            .filter(|id| gilrs.gamepad(*id).is_connected())
            .or_else(|| {
                gilrs
                    .gamepads()
                    .find_map(|(id, pad)| pad.is_connected().then_some(id))
            });
        self.active_gamepad = id;
        let Some(id) = id else {
            self.previous_held.clear();
            self.previous_physical_held.clear();
            self.previous_navigation = None;
            return GamepadFrame::default();
        };

        let gamepad = gilrs.gamepad(id);
        let left_x = deadzone(gamepad.value(Axis::LeftStickX));
        let left_y = deadzone(gamepad.value(Axis::LeftStickY));
        let mut frame = GamepadFrame {
            look_x: deadzone(gamepad.value(Axis::RightStickX)),
            // `process_mouse` subtracts Y, while gamepad up is positive.
            look_y: -deadzone(gamepad.value(Axis::RightStickY)),
            used: received_event || left_x != 0.0 || left_y != 0.0,
            ..GamepadFrame::default()
        };

        let physical_held = physical_bindings(&gamepad, left_x, left_y);
        let navigation = navigation_direction(&gamepad, left_x, left_y);
        frame.navigation = (navigation != self.previous_navigation)
            .then_some(navigation)
            .flatten();
        self.previous_navigation = navigation;
        frame.used |= !physical_held.is_empty() || frame.look_x != 0.0 || frame.look_y != 0.0;
        frame.binding_pressed = physical_held
            .difference(&self.previous_physical_held)
            .copied()
            .collect();
        self.previous_physical_held = physical_held;

        for action in Action::all_bindable() {
            if let Some(binding) = bindings.gamepad_binding_for(*action) {
                if binding_is_active(binding, &gamepad, left_x, left_y) {
                    frame.held.insert(*action);
                }
            }
        }
        frame.pressed = frame
            .held
            .difference(&self.previous_held)
            .copied()
            .collect();
        frame.released = self
            .previous_held
            .difference(&frame.held)
            .copied()
            .collect();
        self.previous_held = frame.held.clone();
        frame
    }
}

fn navigation_direction(
    gamepad: &gilrs::Gamepad<'_>,
    left_x: f32,
    left_y: f32,
) -> Option<[f32; 2]> {
    if gamepad.is_pressed(Button::DPadUp) {
        Some([0.0, -1.0])
    } else if gamepad.is_pressed(Button::DPadDown) {
        Some([0.0, 1.0])
    } else if gamepad.is_pressed(Button::DPadLeft) {
        Some([-1.0, 0.0])
    } else if gamepad.is_pressed(Button::DPadRight) {
        Some([1.0, 0.0])
    } else if left_y >= REBIND_STICK_THRESHOLD {
        Some([0.0, -1.0])
    } else if left_y <= -REBIND_STICK_THRESHOLD {
        Some([0.0, 1.0])
    } else if left_x <= -REBIND_STICK_THRESHOLD {
        Some([-1.0, 0.0])
    } else if left_x >= REBIND_STICK_THRESHOLD {
        Some([1.0, 0.0])
    } else {
        None
    }
}

fn physical_bindings(
    gamepad: &gilrs::Gamepad<'_>,
    left_x: f32,
    left_y: f32,
) -> HashSet<GamepadBinding> {
    let mut held = HashSet::new();
    for button in GAMEPAD_BUTTONS {
        if gamepad.is_pressed(to_gilrs_button(button)) {
            held.insert(GamepadBinding::Button(button));
        }
    }
    if left_y >= REBIND_STICK_THRESHOLD {
        held.insert(GamepadBinding::LeftStickUp);
    }
    if left_y <= -REBIND_STICK_THRESHOLD {
        held.insert(GamepadBinding::LeftStickDown);
    }
    if left_x <= -REBIND_STICK_THRESHOLD {
        held.insert(GamepadBinding::LeftStickLeft);
    }
    if left_x >= REBIND_STICK_THRESHOLD {
        held.insert(GamepadBinding::LeftStickRight);
    }
    held
}

fn binding_is_active(
    binding: GamepadBinding,
    gamepad: &gilrs::Gamepad<'_>,
    left_x: f32,
    left_y: f32,
) -> bool {
    match binding {
        GamepadBinding::Button(button) => gamepad.is_pressed(to_gilrs_button(button)),
        GamepadBinding::LeftStickUp => left_y > 0.0,
        GamepadBinding::LeftStickDown => left_y < 0.0,
        GamepadBinding::LeftStickLeft => left_x < 0.0,
        GamepadBinding::LeftStickRight => left_x > 0.0,
    }
}

const GAMEPAD_BUTTONS: [GamepadButton; 11] = [
    GamepadButton::South,
    GamepadButton::East,
    GamepadButton::West,
    GamepadButton::North,
    GamepadButton::LeftTrigger,
    GamepadButton::RightTrigger,
    GamepadButton::LeftTrigger2,
    GamepadButton::RightTrigger2,
    GamepadButton::LeftThumb,
    GamepadButton::RightThumb,
    GamepadButton::Start,
];

fn to_gilrs_button(button: GamepadButton) -> Button {
    match button {
        GamepadButton::South => Button::South,
        GamepadButton::East => Button::East,
        GamepadButton::West => Button::West,
        GamepadButton::North => Button::North,
        GamepadButton::LeftTrigger => Button::LeftTrigger,
        GamepadButton::RightTrigger => Button::RightTrigger,
        GamepadButton::LeftTrigger2 => Button::LeftTrigger2,
        GamepadButton::RightTrigger2 => Button::RightTrigger2,
        GamepadButton::LeftThumb => Button::LeftThumb,
        GamepadButton::RightThumb => Button::RightThumb,
        GamepadButton::Start => Button::Start,
    }
}

fn deadzone(value: f32) -> f32 {
    if value.abs() <= STICK_DEADZONE {
        0.0
    } else {
        value.signum() * ((value.abs() - STICK_DEADZONE) / (1.0 - STICK_DEADZONE))
    }
}

#[cfg(test)]
mod tests {
    use super::deadzone;
    use crate::client::keybind::{Action, GamepadBinding, GamepadButton, KeyBindings};

    #[test]
    fn bedrock_defaults_keep_face_buttons_triggers_and_stick_movement() {
        let bindings = KeyBindings::defaults();
        assert_eq!(
            bindings.gamepad_binding_for(Action::Jump),
            Some(GamepadBinding::Button(GamepadButton::South))
        );
        assert_eq!(
            bindings.gamepad_binding_for(Action::Attack),
            Some(GamepadBinding::Button(GamepadButton::RightTrigger2))
        );
        assert_eq!(
            bindings.gamepad_binding_for(Action::Forward),
            Some(GamepadBinding::LeftStickUp)
        );
    }

    #[test]
    fn stick_deadzone_removes_drift_and_preserves_range() {
        assert_eq!(deadzone(0.18), 0.0);
        assert_eq!(deadzone(-0.18), 0.0);
        assert_eq!(deadzone(1.0), 1.0);
        assert_eq!(deadzone(-1.0), -1.0);
    }
}
