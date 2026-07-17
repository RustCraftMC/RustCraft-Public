use std::collections::HashMap;
use std::time::Duration;

use super::manifest::ModId;

#[derive(Clone, Debug, Default)]
pub struct ModProfile {
    pub callback_count: u64,
    pub callback_time: Duration,
    pub slow_callback_count: u32,
    pub error_count: u32,
}

#[derive(Default)]
pub struct ScriptProfiler {
    profiles: HashMap<ModId, ModProfile>,
    slow_callback_threshold: Duration,
    frame_callback_time: Duration,
    frame_callback_count: u32,
    frame_slow_callback_count: u32,
}

impl ScriptProfiler {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            slow_callback_threshold: Duration::from_micros(200),
            frame_callback_time: Duration::ZERO,
            frame_callback_count: 0,
            frame_slow_callback_count: 0,
        }
    }

    pub fn record_callback(&mut self, mod_id: &ModId, elapsed: Duration, failed: bool) {
        let profile = self.profiles.entry(mod_id.clone()).or_default();
        profile.callback_count += 1;
        profile.callback_time += elapsed;
        self.frame_callback_time += elapsed;
        self.frame_callback_count = self.frame_callback_count.saturating_add(1);
        if elapsed > self.slow_callback_threshold {
            profile.slow_callback_count += 1;
            self.frame_slow_callback_count = self.frame_slow_callback_count.saturating_add(1);
        }
        if failed {
            profile.error_count += 1;
        }
    }

    pub fn profile(&self, mod_id: &ModId) -> Option<&ModProfile> {
        self.profiles.get(mod_id)
    }

    pub fn remove(&mut self, mod_id: &ModId) {
        self.profiles.remove(mod_id);
    }

    pub fn take_frame(&mut self) -> (u64, u32, u32) {
        let result = (
            self.frame_callback_time.as_micros() as u64,
            self.frame_callback_count,
            self.frame_slow_callback_count,
        );
        self.frame_callback_time = Duration::ZERO;
        self.frame_callback_count = 0;
        self.frame_slow_callback_count = 0;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_profile_accumulates_callbacks_and_resets_after_read() {
        let mod_id = ModId::parse("debug_profile").unwrap();
        let mut profiler = ScriptProfiler::new();
        profiler.record_callback(&mod_id, Duration::from_micros(125), false);
        profiler.record_callback(&mod_id, Duration::from_micros(250), false);

        assert_eq!(profiler.take_frame(), (375, 2, 1));
        assert_eq!(profiler.take_frame(), (0, 0, 0));
    }
}
