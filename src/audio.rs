use std::collections::{HashMap, VecDeque};
use std::io::Cursor;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::source::ChannelVolume;
use rodio::{Decoder, OutputStream, Sink, Source};

use crate::assets::index::AssetIndex;
use crate::assets::sound::SoundRegistry;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundCategory {
    Master,
    Music,
    Blocks,
    Weather,
    Hostile,
    Friendly,
    Players,
    Ambient,
    Ui,
}

impl SoundCategory {
    pub fn from_protocol_id(id: i32) -> Self {
        match id {
            0 => SoundCategory::Master,
            1 => SoundCategory::Music,
            2 => SoundCategory::Blocks,
            3 => SoundCategory::Weather,
            4 => SoundCategory::Hostile,
            5 => SoundCategory::Friendly,
            6 => SoundCategory::Players,
            7 => SoundCategory::Ambient,
            _ => SoundCategory::Master,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SoundEvent {
    pub name: String,
    pub category: SoundCategory,
    pub volume: f32,
    pub pitch: f32,
    pub position: Option<[f32; 3]>,
}

pub trait AudioBackend {
    fn play(&mut self, event: SoundEvent);
    fn set_volume(&mut self, category: SoundCategory, volume: f32);
    fn set_listener(&mut self, position: [f32; 3], yaw: f32);
    fn tick(&mut self);
    fn stop_all(&mut self);
}

// ---------------------------------------------------------------------------
// Spatial audio math
// ---------------------------------------------------------------------------

/// Vanilla linear attenuation starts at a 16-block radius.
const MAX_SOUND_DISTANCE: f32 = 16.0;

/// Listener state for spatial audio.
struct SpatialListener {
    position: [f32; 3],
    /// Renderer camera yaw in radians (0 = +X, matching `Camera::front`).
    yaw: f32,
}

impl SpatialListener {
    fn new() -> Self {
        Self {
            position: [0.0; 3],
            yaw: 0.0,
        }
    }

    /// Equal-power stereo balance in camera-local space.
    fn pan_to(&self, sound_pos: [f32; 3]) -> (f32, f32) {
        let dx = sound_pos[0] - self.position[0];
        let dz = sound_pos[2] - self.position[2];
        let horizontal = (dx * dx + dz * dz).sqrt();
        // At the player's own feet, direction is undefined.  Vanilla/OpenAL
        // keeps such a source centred rather than deriving pan from yaw.
        if horizontal < 0.25 {
            return (1.0, 1.0);
        }
        let right_x = -self.yaw.sin();
        let right_z = self.yaw.cos();
        let pan = ((dx * right_x + dz * right_z) / horizontal).clamp(-1.0, 1.0);
        // Equal-power panning retains perceived loudness as a source crosses
        // the centre line, unlike the previous one-sided attenuation curve.
        let left = ((1.0 - pan) * 0.5).sqrt() * std::f32::consts::SQRT_2;
        let right = ((1.0 + pan) * 0.5).sqrt() * std::f32::consts::SQRT_2;
        (left, right)
    }

    /// Distance between listener and a point.
    fn distance_to(&self, pos: [f32; 3]) -> f32 {
        let dx = self.position[0] - pos[0];
        let dy = self.position[1] - pos[1];
        let dz = self.position[2] - pos[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Compute distance attenuation (MC 1.8 style: linear falloff).
fn distance_attenuation(dist: f32, base_volume: f32) -> f32 {
    let max_distance = MAX_SOUND_DISTANCE * base_volume.max(1.0);
    (1.0 - dist / max_distance).clamp(0.0, 1.0)
}

#[cfg(test)]
mod spatial_tests {
    use super::*;

    #[test]
    fn listener_centres_nearby_self_sounds() {
        let listener = SpatialListener::new();
        assert_eq!(listener.pan_to([0.0, 1.62, 0.0]), (1.0, 1.0));
    }

    #[test]
    fn listener_uses_camera_right_vector() {
        let listener = SpatialListener::new(); // yaw 0 faces +X, right is +Z
        let (left, right) = listener.pan_to([0.0, 0.0, 4.0]);
        assert!(right > left);
    }
}

// ---------------------------------------------------------------------------
// Null backend
// ---------------------------------------------------------------------------

pub struct NullAudioBackend {
    volumes: HashMap<SoundCategory, f32>,
    played_count: u64,
    recent: VecDeque<SoundEvent>,
}

impl NullAudioBackend {
    pub fn new() -> Self {
        let mut volumes = HashMap::new();
        for cat in [
            SoundCategory::Master,
            SoundCategory::Music,
            SoundCategory::Blocks,
            SoundCategory::Weather,
            SoundCategory::Hostile,
            SoundCategory::Friendly,
            SoundCategory::Players,
            SoundCategory::Ambient,
            SoundCategory::Ui,
        ] {
            volumes.insert(cat, 1.0);
        }
        Self {
            volumes,
            played_count: 0,
            recent: VecDeque::with_capacity(16),
        }
    }
    pub fn played_count(&self) -> u64 {
        self.played_count
    }
    pub fn recent(&self) -> std::collections::vec_deque::Iter<'_, SoundEvent> {
        self.recent.iter()
    }
}

impl AudioBackend for NullAudioBackend {
    fn play(&mut self, event: SoundEvent) {
        self.played_count = self.played_count.saturating_add(1);
        if self.recent.len() >= 16 {
            self.recent.pop_front();
        }
        self.recent.push_back(event);
    }
    fn set_volume(&mut self, category: SoundCategory, volume: f32) {
        self.volumes.insert(category, volume.clamp(0.0, 1.0));
    }
    fn set_listener(&mut self, _position: [f32; 3], _yaw: f32) {}
    fn tick(&mut self) {}
    fn stop_all(&mut self) {}
}

// ---------------------------------------------------------------------------
// Active spatial sound tracking
// ---------------------------------------------------------------------------

/// A currently-playing positional sound whose volume is updated each tick.
struct ActiveSound {
    sink_index: usize,
    position: [f32; 3],
    category: SoundCategory,
    base_volume: f32,
}

// ---------------------------------------------------------------------------
// Rodio backend — real audio playback with spatial audio
// ---------------------------------------------------------------------------

const SINK_POOL_SIZE: usize = 48;
const MAX_ACTIVE_SPATIAL: usize = 64;

/// List available audio output device names.
pub fn list_audio_devices() -> Vec<String> {
    let host = rodio::cpal::default_host();
    let mut names: Vec<String> = match host.devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => vec![],
    };
    names.sort();
    names.dedup();
    if names.is_empty() {
        names.push("default".to_string());
    }
    names
}

pub struct RodioAudioBackend {
    _output_stream: OutputStream,
    device_name: String,
    sinks: Vec<Sink>,
    music_sink: Sink,
    volumes: HashMap<SoundCategory, f32>,
    sounds: SoundRegistry,
    index: AssetIndex,
    listener: SpatialListener,
    /// Spatial sounds currently playing — volumes updated each tick.
    active_sounds: Vec<ActiveSound>,
    played_count: u64,
    recent: VecDeque<SoundEvent>,
    next_sink: usize,
}

impl RodioAudioBackend {
    /// Create backend using the default system audio device.
    pub fn new(index: AssetIndex, sounds: SoundRegistry) -> Result<Self, String> {
        Self::with_device("default", index, sounds)
    }

    /// Create backend using a specific device name.
    /// Pass "default" to use the system default.
    pub fn with_device(
        device_name: &str,
        index: AssetIndex,
        sounds: SoundRegistry,
    ) -> Result<Self, String> {
        let (output_stream, output_handle) = if device_name == "default" || device_name.is_empty() {
            OutputStream::try_default()
                .map_err(|e| format!("Failed to open default audio output: {}", e))?
        } else {
            let host = rodio::cpal::default_host();
            let dev = host
                .devices()
                .map_err(|e| format!("Cannot enumerate audio devices: {}", e))?
                .find(|d| d.name().map_or(false, |n| n == device_name))
                .ok_or_else(|| format!("Audio device '{}' not found", device_name))?;
            OutputStream::try_from_device(&dev)
                .map_err(|e| format!("Failed to open audio device '{}': {}", device_name, e))?
        };

        let mut sinks = Vec::with_capacity(SINK_POOL_SIZE);
        for _ in 0..SINK_POOL_SIZE {
            sinks.push(
                Sink::try_new(&output_handle)
                    .map_err(|e| format!("Failed to create sink: {}", e))?,
            );
        }
        let music_sink = Sink::try_new(&output_handle)
            .map_err(|e| format!("Failed to create music sink: {}", e))?;

        let mut volumes = HashMap::new();
        for cat in [
            SoundCategory::Master,
            SoundCategory::Music,
            SoundCategory::Blocks,
            SoundCategory::Weather,
            SoundCategory::Hostile,
            SoundCategory::Friendly,
            SoundCategory::Players,
            SoundCategory::Ambient,
            SoundCategory::Ui,
        ] {
            volumes.insert(cat, 1.0);
        }

        log::info!(
            "Rodio audio backend ready: device='{}', sinks={}, sound_events={}",
            device_name,
            SINK_POOL_SIZE,
            sounds.len()
        );

        Ok(RodioAudioBackend {
            _output_stream: output_stream,
            device_name: device_name.to_string(),
            sinks,
            music_sink,
            volumes,
            sounds,
            index,
            listener: SpatialListener::new(),
            active_sounds: Vec::with_capacity(MAX_ACTIVE_SPATIAL),
            played_count: 0,
            recent: VecDeque::with_capacity(16),
            next_sink: 0,
        })
    }

    fn effective_volume(&self, category: SoundCategory) -> f32 {
        let master = self
            .volumes
            .get(&SoundCategory::Master)
            .copied()
            .unwrap_or(1.0);
        let cat = self.volumes.get(&category).copied().unwrap_or(1.0);
        master * cat
    }

    /// Compute the full spatial volume for a positional sound.
    fn spatial_volume(
        &self,
        sound_pos: [f32; 3],
        category: SoundCategory,
        base_volume: f32,
    ) -> f32 {
        let dist = self.listener.distance_to(sound_pos);
        let atten = distance_attenuation(dist, base_volume);
        let cat_vol = self.effective_volume(category);
        cat_vol * base_volume * atten
    }

    /// Compute stereo pan (left, right) for a positional sound.
    fn spatial_pan(&self, sound_pos: [f32; 3]) -> (f32, f32) {
        self.listener.pan_to(sound_pos)
    }

    fn alloc_sink(&mut self) -> usize {
        let idx = self.next_sink;
        self.next_sink = (self.next_sink + 1) % SINK_POOL_SIZE;
        self.sinks[idx].stop();
        idx
    }

    /// Play a sound with spatial panning applied via ChannelVolume.
    fn play_spatial(&mut self, event: &SoundEvent, sound_pos: [f32; 3]) {
        let files = self.sounds.resolve_files(&event.name);
        if files.is_empty() {
            return;
        }

        let non_streaming: Vec<_> = files.iter().filter(|(_, s)| !*s).cloned().collect();
        let pool = if non_streaming.is_empty() {
            files
        } else {
            non_streaming
        };
        let idx = (self.played_count as usize) % pool.len();
        let (resource_path, _is_streaming) = pool[idx].clone();

        let vol = self.spatial_volume(sound_pos, event.category, event.volume);
        if vol < 0.001 {
            return;
        }

        let Some(bytes) = self.index.read_bytes(&resource_path) else {
            return;
        };
        let cursor = Cursor::new(bytes);
        let Ok(decoder) = Decoder::new(cursor) else {
            return;
        };

        let (left_vol, right_vol) = self.spatial_pan(sound_pos);

        let sink_idx = self.alloc_sink();

        // ChannelVolume mixes any channel count to mono, then plays to each
        // output channel at the given volume — perfect for spatial panning.
        let panned = ChannelVolume::new(
            decoder.speed(event.pitch.clamp(0.01, 4.0)),
            vec![vol * left_vol, vol * right_vol],
        );
        self.sinks[sink_idx].append(panned);

        // Track for per-tick volume updates
        if self.active_sounds.len() < MAX_ACTIVE_SPATIAL {
            self.active_sounds.push(ActiveSound {
                sink_index: sink_idx,
                position: sound_pos,
                category: event.category,
                base_volume: event.volume,
            });
        }
    }
}

impl Drop for RodioAudioBackend {
    fn drop(&mut self) {
        // Ignore errors on shutdown (device may already be gone)
    }
}

impl AudioBackend for RodioAudioBackend {
    fn play(&mut self, event: SoundEvent) {
        self.played_count = self.played_count.saturating_add(1);
        if self.recent.len() >= 16 {
            self.recent.pop_front();
        }
        self.recent.push_back(event.clone());

        if let Some(pos) = event.position {
            self.play_spatial(&event, pos);
        } else {
            // Non-positional sound (UI, music, self) — no distance/pan
            let files = self.sounds.resolve_files(&event.name);
            if files.is_empty() {
                return;
            }

            let non_streaming: Vec<_> = files.iter().filter(|(_, s)| !*s).cloned().collect();
            let pool = if non_streaming.is_empty() {
                files
            } else {
                non_streaming
            };
            let idx = (self.played_count as usize) % pool.len();
            let (resource_path, is_streaming) = pool[idx].clone();

            let vol = self.effective_volume(event.category) * event.volume;
            if vol < 0.001 {
                return;
            }

            let Some(bytes) = self.index.read_bytes(&resource_path) else {
                return;
            };
            let cursor = Cursor::new(bytes);
            let Ok(decoder) = Decoder::new(cursor) else {
                return;
            };

            if is_streaming {
                self.music_sink.stop();
                self.music_sink
                    .append(decoder.speed(event.pitch.clamp(0.01, 4.0)));
                self.music_sink.set_volume(vol);
            } else {
                let sink_idx = self.alloc_sink();
                self.sinks[sink_idx].append(decoder.speed(event.pitch.clamp(0.01, 4.0)));
                self.sinks[sink_idx].set_volume(vol);
            }
        }
    }

    fn set_volume(&mut self, category: SoundCategory, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        self.volumes.insert(category, vol);
        if category == SoundCategory::Music || category == SoundCategory::Master {
            self.music_sink
                .set_volume(self.effective_volume(SoundCategory::Music));
        }
    }

    fn set_listener(&mut self, position: [f32; 3], yaw: f32) {
        self.listener.position = position;
        self.listener.yaw = yaw;
    }

    fn tick(&mut self) {
        // Collect indices of finished sounds, then remove them
        let mut to_remove = Vec::new();
        for (i, s) in self.active_sounds.iter().enumerate() {
            if self.sinks[s.sink_index].empty() {
                to_remove.push(i);
            } else {
                let vol = self.spatial_volume(s.position, s.category, s.base_volume);
                self.sinks[s.sink_index].set_volume(vol);
            }
        }
        // Remove in reverse order to preserve indices
        for i in to_remove.into_iter().rev() {
            self.active_sounds.swap_remove(i);
        }
    }

    fn stop_all(&mut self) {
        for sink in &self.sinks {
            sink.stop();
        }
        self.music_sink.stop();
        self.active_sounds.clear();
    }
}

// ---------------------------------------------------------------------------
// Unified backend wrapper
// ---------------------------------------------------------------------------

pub enum AudioBackendImpl {
    Null(NullAudioBackend),
    Rodio(RodioAudioBackend),
}

impl AudioBackendImpl {
    pub fn new(index: AssetIndex, sounds: SoundRegistry) -> Self {
        match RodioAudioBackend::new(index.clone(), sounds.clone()) {
            Ok(backend) => {
                log::info!("using Rodio audio backend with real playback");
                AudioBackendImpl::Rodio(backend)
            }
            Err(e) => {
                log::warn!("Rodio audio backend unavailable: {e}; using null audio backend");
                AudioBackendImpl::Null(NullAudioBackend::new())
            }
        }
    }

    pub fn reload_assets(&mut self, index: AssetIndex, sounds: SoundRegistry) {
        if let AudioBackendImpl::Rodio(backend) = self {
            backend.stop_all();
            backend.index = index;
            backend.sounds = sounds;
        }
    }
    pub fn new_null() -> Self {
        AudioBackendImpl::Null(NullAudioBackend::new())
    }

    /// Recreate the backend with a different audio device. Falls back to null on failure.
    /// Uses the current sounds/index from self if available.
    pub fn reinit(&mut self, device_name: &str) {
        let (index, sounds) = match self {
            AudioBackendImpl::Rodio(b) => (b.index.clone(), b.sounds.clone()),
            AudioBackendImpl::Null(_) => return,
        };
        match RodioAudioBackend::with_device(device_name, index, sounds) {
            Ok(backend) => {
                // Preserve volume settings
                let old_volumes = match self {
                    AudioBackendImpl::Rodio(b) => Some(std::mem::take(&mut b.volumes)),
                    AudioBackendImpl::Null(b) => {
                        let v = std::mem::take(&mut b.volumes);
                        Some(v)
                    }
                };
                *self = AudioBackendImpl::Rodio(backend);
                if let Some(vols) = old_volumes {
                    for (cat, vol) in vols {
                        self.set_volume(cat, vol);
                    }
                }
                log::info!("audio backend reinitialised with device '{device_name}'");
            }
            Err(e) => {
                log::error!("failed to reinitialise audio device '{device_name}': {e}");
                *self = AudioBackendImpl::Null(NullAudioBackend::new());
            }
        }
    }

    pub fn device_name(&self) -> &str {
        match self {
            AudioBackendImpl::Rodio(b) => &b.device_name,
            AudioBackendImpl::Null(_) => "null",
        }
    }

    pub fn played_count(&self) -> u64 {
        match self {
            AudioBackendImpl::Null(b) => b.played_count(),
            AudioBackendImpl::Rodio(b) => b.played_count,
        }
    }
    pub fn recent(&self) -> std::collections::vec_deque::Iter<'_, SoundEvent> {
        match self {
            AudioBackendImpl::Null(b) => b.recent(),
            AudioBackendImpl::Rodio(b) => b.recent.iter(),
        }
    }
}

impl AudioBackend for AudioBackendImpl {
    fn play(&mut self, event: SoundEvent) {
        match self {
            AudioBackendImpl::Null(b) => b.play(event),
            AudioBackendImpl::Rodio(b) => b.play(event),
        }
    }
    fn set_volume(&mut self, category: SoundCategory, volume: f32) {
        match self {
            AudioBackendImpl::Null(b) => b.set_volume(category, volume),
            AudioBackendImpl::Rodio(b) => b.set_volume(category, volume),
        }
    }
    fn set_listener(&mut self, position: [f32; 3], yaw: f32) {
        match self {
            AudioBackendImpl::Null(b) => b.set_listener(position, yaw),
            AudioBackendImpl::Rodio(b) => b.set_listener(position, yaw),
        }
    }
    fn tick(&mut self) {
        match self {
            AudioBackendImpl::Null(b) => b.tick(),
            AudioBackendImpl::Rodio(b) => b.tick(),
        }
    }
    fn stop_all(&mut self) {
        match self {
            AudioBackendImpl::Null(b) => b.stop_all(),
            AudioBackendImpl::Rodio(b) => b.stop_all(),
        }
    }
}
