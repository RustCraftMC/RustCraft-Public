//! Independent tick system — 20 ticks/second, uses real wall-clock time.
//!
//! NOT tied to frame rate. Uses Instant to track real time.
//! At 3000fps: ticks happen every ~151 frames.
//! At 60fps: ticks happen every ~1 frame.
//! Always exactly 20 ticks/sec regardless of GPU speed.

use std::time::Instant;

pub const TICKS_PER_SECOND: f32 = 20.0;
pub const SECONDS_PER_TICK: f32 = 1.0 / TICKS_PER_SECOND; // 0.05

pub struct TickTimer {
    last_tick: Instant,
    accumulated: f64,
    tick_count: u64,
}

impl TickTimer {
    pub fn new() -> Self {
        TickTimer {
            last_tick: Instant::now(),
            accumulated: 0.0,
            tick_count: 0,
        }
    }

    /// Call once per frame. Returns number of ticks to execute.
    /// Uses real wall-clock time, not frame delta.
    pub fn update(&mut self) -> u32 {
        let now = Instant::now();
        let elapsed = (now - self.last_tick).as_secs_f64();
        self.last_tick = now;

        self.advance_elapsed(elapsed)
    }

    fn advance_elapsed(&mut self, elapsed: f64) -> u32 {
        // Minecraft 1.8.9 Timer caps elapsedTicks at 10. Preserve every tick
        // admitted by that cap so the caller executes matching physics and
        // movement-packet updates instead of silently consuming them.
        self.accumulated += elapsed.min(0.5); // max 10 ticks catch-up

        let tick_seconds = 1.0_f64 / TICKS_PER_SECOND as f64;
        let ticks = (self.accumulated / tick_seconds).floor() as u32;
        self.accumulated -= ticks as f64 * tick_seconds;
        ticks
    }

    pub fn begin_tick(&mut self) {
        self.tick_count += 1;
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Interpolation factor (0.0-1.0) for smooth rendering between ticks.
    pub fn alpha(&self) -> f32 {
        (self.accumulated / SECONDS_PER_TICK as f64).min(1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::TickTimer;

    #[test]
    fn catch_up_preserves_all_ten_vanilla_ticks() {
        let mut timer = TickTimer::new();
        let ticks = timer.advance_elapsed(0.5);
        assert_eq!(ticks, 10);
        assert_eq!(timer.tick_count(), 0);
        for _ in 0..ticks {
            timer.begin_tick();
        }
        assert_eq!(timer.tick_count(), 10);
    }
}
