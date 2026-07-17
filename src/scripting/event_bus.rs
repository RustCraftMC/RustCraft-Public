use serde_json::Value;

use super::callback::CallbackId;
use super::manifest::ModId;

pub const HIGHEST: i32 = 1000;
pub const HIGH: i32 = 500;
pub const NORMAL: i32 = 0;
pub const LOW: i32 = -500;
pub const LOWEST: i32 = -1000;

#[derive(Clone, Debug)]
pub struct ScriptEvent {
    pub name: String,
    pub payload: Value,
}

impl ScriptEvent {
    pub fn new(name: impl Into<String>, payload: Value) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventOutcome {
    pub cancelled: bool,
    pub consumed: bool,
    pub result: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct PlannedCallback {
    pub mod_id: ModId,
    pub callback_id: CallbackId,
    pub priority: i32,
    pub load_order: usize,
}

#[derive(Default)]
pub struct ScriptEventBus;

impl ScriptEventBus {
    pub fn order(&self, callbacks: &mut [PlannedCallback]) {
        callbacks.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.load_order.cmp(&right.load_order))
                .then_with(|| left.callback_id.cmp(&right.callback_id))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_high_priority_first_and_keeps_load_order_stable() {
        let mut callbacks = vec![
            PlannedCallback {
                mod_id: ModId::parse("first").unwrap(),
                callback_id: 1,
                priority: NORMAL,
                load_order: 0,
            },
            PlannedCallback {
                mod_id: ModId::parse("second").unwrap(),
                callback_id: 1,
                priority: HIGH,
                load_order: 1,
            },
        ];
        ScriptEventBus.order(&mut callbacks);
        assert_eq!(callbacks[0].mod_id.as_str(), "second");
    }
}
