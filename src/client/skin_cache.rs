use crate::assets::skin::{PlayerSkin, SkinPreviewPixels};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct CachedSkin {
    pub skin: Arc<PlayerSkin>,
    pub preview: SkinPreviewPixels,
    pub face: [[u8; 4]; 64],
    pub slim: bool,
    pub source: SkinSource,
    pub content_hash: u64,
}

#[derive(Clone, Debug)]
pub struct CachedCape {
    pub pixels: Arc<Vec<u8>>,
    pub content_hash: u64,
}

#[derive(Clone, Debug)]
pub enum SkinSource {
    Default,
    File(PathBuf),
}

pub struct SkinCache {
    skins: HashMap<String, Arc<CachedSkin>>,
    default_skin: Arc<CachedSkin>,
    default_slim_skin: Arc<CachedSkin>,
    pending_downloads: HashMap<String, Vec<PendingSkinWaiter>>,
    retry_states: HashMap<String, SkinRetryState>,
    download_job_tx: mpsc::Sender<SkinDownloadJob>,
    download_tx: mpsc::Sender<SkinDownloadResult>,
    download_rx: mpsc::Receiver<SkinDownloadResult>,
    content_generation: u64,
    capes: HashMap<String, Arc<CachedCape>>,
    pending_cape_downloads: HashSet<String>,
    failed_cape_downloads: HashMap<String, Instant>,
    cape_job_tx: mpsc::Sender<CapeDownloadJob>,
    cape_rx: mpsc::Receiver<CapeDownloadResult>,
}

#[derive(Clone, Debug, Default)]
struct SkinProfile {
    texture_key: Option<String>,
    skin_url: Option<String>,
    cape_url: Option<String>,
    slim: bool,
}

struct SkinDownloadResult {
    texture_key: String,
    path: PathBuf,
    skin: Result<PlayerSkin, String>,
}

struct SkinDownloadJob {
    texture_key: String,
    skin_url: String,
    local_path: PathBuf,
}

struct CapeDownloadJob {
    texture_key: String,
    cape_url: String,
    local_path: PathBuf,
}

struct CapeDownloadResult {
    texture_key: String,
    cape: Result<Vec<u8>, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingSkinWaiter {
    cache_key: String,
    slim: bool,
}

#[derive(Clone, Debug)]
struct SkinRetryState {
    failures: u8,
    retry_after: Instant,
    in_flight: bool,
    ready_announced: bool,
}

impl SkinCache {
    pub fn new(default_skin: &PlayerSkin) -> Self {
        let (download_tx, download_rx) = mpsc::channel();
        let (download_job_tx, download_job_rx) = mpsc::channel::<SkinDownloadJob>();
        let download_job_rx = Arc::new(Mutex::new(download_job_rx));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        for worker_index in 0..4 {
            let jobs = Arc::clone(&download_job_rx);
            let results = download_tx.clone();
            let client = client.clone();
            std::thread::Builder::new()
                .name(format!("skin-download-worker-{worker_index}"))
                .spawn(move || loop {
                    let job = {
                        let Ok(receiver) = jobs.lock() else {
                            break;
                        };
                        let Ok(job) = receiver.recv() else {
                            break;
                        };
                        job
                    };
                    let skin = download_skin_texture(&client, &job.skin_url, &job.local_path);
                    let _ = results.send(SkinDownloadResult {
                        texture_key: job.texture_key,
                        path: job.local_path,
                        skin,
                    });
                })
                .expect("failed to start skin download worker");
        }
        let (cape_job_tx, cape_job_rx) = mpsc::channel::<CapeDownloadJob>();
        let (cape_tx, cape_rx) = mpsc::channel::<CapeDownloadResult>();
        let cape_job_rx = Arc::new(Mutex::new(cape_job_rx));
        for worker_index in 0..2 {
            let jobs = Arc::clone(&cape_job_rx);
            let results = cape_tx.clone();
            let client = client.clone();
            std::thread::Builder::new()
                .name(format!("cape-download-worker-{worker_index}"))
                .spawn(move || loop {
                    let job = {
                        let Ok(receiver) = jobs.lock() else {
                            break;
                        };
                        let Ok(job) = receiver.recv() else {
                            break;
                        };
                        job
                    };
                    let cape = download_cape_texture(&client, &job.cape_url, &job.local_path);
                    let _ = results.send(CapeDownloadResult {
                        texture_key: job.texture_key,
                        cape,
                    });
                })
                .expect("failed to start cape download worker");
        }
        let default_skin = Arc::new(CachedSkin::from_skin(
            default_skin.clone(),
            SkinSource::Default,
        ));
        let default_slim_skin = if default_skin.slim {
            Arc::clone(&default_skin)
        } else {
            Arc::new(CachedSkin::from_skin(
                default_skin.skin.as_ref().clone().with_slim_arms(true),
                SkinSource::Default,
            ))
        };
        Self {
            skins: HashMap::new(),
            default_skin,
            default_slim_skin,
            pending_downloads: HashMap::new(),
            retry_states: HashMap::new(),
            download_job_tx,
            download_tx,
            download_rx,
            content_generation: 0,
            capes: HashMap::new(),
            pending_cape_downloads: HashSet::new(),
            failed_cape_downloads: HashMap::new(),
            cape_job_tx,
            cape_rx,
        }
    }

    /// Drain completed asynchronous downloads and return the skin-content revision.
    pub fn poll_content_generation(&mut self) -> u64 {
        self.drain_completed_downloads();
        let now = Instant::now();
        let mut retry_became_ready = false;
        for state in self.retry_states.values_mut() {
            if !state.in_flight && !state.ready_announced && now >= state.retry_after {
                state.ready_announced = true;
                retry_became_ready = true;
            }
        }
        if retry_became_ready {
            // Wake collect_pending_player_skins even when the entity roster itself did not change.
            self.content_generation = self.content_generation.wrapping_add(1);
        }
        self.content_generation
    }

    pub fn preview_for(
        &mut self,
        uuid: Option<&str>,
        name: Option<&str>,
        skin_property: Option<&str>,
    ) -> SkinPreviewPixels {
        self.lookup(uuid, name, skin_property).preview.clone()
    }

    pub fn face_for(
        &mut self,
        uuid: Option<&str>,
        name: Option<&str>,
        skin_property: Option<&str>,
    ) -> [[u8; 4]; 64] {
        self.lookup(uuid, name, skin_property).face
    }

    pub fn is_slim(
        &mut self,
        uuid: Option<&str>,
        name: Option<&str>,
        skin_property: Option<&str>,
    ) -> bool {
        self.lookup(uuid, name, skin_property).slim
    }

    /// Check whether a skin property contains a cape URL.
    pub fn has_cape(skin_property: Option<&str>) -> bool {
        skin_property
            .and_then(parse_skin_property)
            .is_some_and(|p| p.cape_url.is_some())
    }

    pub fn skin_for(
        &mut self,
        uuid: Option<&str>,
        name: Option<&str>,
        skin_property: Option<&str>,
    ) -> PlayerSkin {
        self.lookup(uuid, name, skin_property).skin.as_ref().clone()
    }

    pub fn snapshot_for(
        &mut self,
        uuid: Option<&str>,
        name: Option<&str>,
        skin_property: Option<&str>,
    ) -> Arc<CachedSkin> {
        self.lookup(uuid, name, skin_property)
    }

    pub fn cape_for_property(&mut self, skin_property: Option<&str>) -> Option<Arc<CachedCape>> {
        self.drain_completed_downloads();
        let cape_url = skin_property
            .and_then(parse_skin_property)
            .and_then(|profile| profile.cape_url)?;
        let texture_key = cape_url.rsplit('/').next()?.to_string();
        if let Some(cape) = self.capes.get(&texture_key) {
            return Some(Arc::clone(cape));
        }

        let local_path = PathBuf::from(format!("assets/capes/{texture_key}.png"));
        if let Some(pixels) = load_cape_pixels(&local_path) {
            let cape = Arc::new(CachedCape::new(pixels));
            self.capes.insert(texture_key, Arc::clone(&cape));
            return Some(cape);
        }

        let retry_blocked = self
            .failed_cape_downloads
            .get(&texture_key)
            .is_some_and(|retry_after| Instant::now() < *retry_after);
        if !retry_blocked && self.pending_cape_downloads.insert(texture_key.clone()) {
            let worker_unavailable = self
                .cape_job_tx
                .send(CapeDownloadJob {
                    texture_key: texture_key.clone(),
                    cape_url,
                    local_path,
                })
                .is_err();
            if worker_unavailable {
                self.pending_cape_downloads.remove(&texture_key);
            }
        }
        None
    }

    fn lookup(
        &mut self,
        uuid: Option<&str>,
        name: Option<&str>,
        skin_property: Option<&str>,
    ) -> Arc<CachedSkin> {
        self.drain_completed_downloads();

        // Fast path: hash skin_property string to avoid base64+JSON every frame
        let prop_hash = skin_property.map(|sp| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            sp.hash(&mut h);
            format!("{:x}", h.finish())
        });
        let id = uuid.or(name).unwrap_or("default");
        let fast_key = format!("{id}:{}", prop_hash.as_deref().unwrap_or("local"));
        if let Some(skin) = self.skins.get(&fast_key) {
            let retry_is_due = self
                .retry_states
                .get(&fast_key)
                .is_some_and(|state| !state.in_flight && Instant::now() >= state.retry_after);
            if !retry_is_due {
                return Arc::clone(skin);
            }
            // The cached value is only the Steve/Alex placeholder from a failed request.
            // Drop it once the backoff expires so this lookup schedules a fresh download.
            self.skins.remove(&fast_key);
        }
        // Cache miss — parse profile (expensive, only once per skin)
        let profile = match skin_property {
            Some(property) => parse_skin_property(property).unwrap_or_else(|| {
                log::warn!(
                    target: "rustcraft::skins",
                    "invalid player skin property; using default skin: player={id}"
                );
                SkinProfile::default()
            }),
            None => SkinProfile::default(),
        };
        let loaded = if let Some(ref tk) = profile.texture_key {
            // Mojang's texture key is already a content hash, so the shared flat cache is
            // canonical. Keep reading the short-lived per-player layout for compatibility.
            let local_path = PathBuf::from(format!("assets/skins/{tk}.png"));
            let per_player_path =
                PathBuf::from(format!("assets/skins/{}/{}.png", id.replace('-', ""), tk));
            [local_path.clone(), per_player_path]
                .into_iter()
                .find_map(|path| PlayerSkin::load(&path).ok().map(|skin| (path, skin)))
                .map(|(path, skin)| {
                    Arc::new(CachedSkin::from_skin(
                        skin.with_slim_arms(profile.slim),
                        SkinSource::File(path),
                    ))
                })
                .unwrap_or_else(|| {
                    self.schedule_skin_download(
                        tk.clone(),
                        profile.skin_url.clone().unwrap_or_default(),
                        local_path.clone(),
                        profile.slim,
                        fast_key.clone(),
                    );
                    if profile.slim {
                        Arc::clone(&self.default_slim_skin)
                    } else {
                        Arc::clone(&self.default_skin)
                    }
                })
        } else {
            self.candidate_paths(uuid, name, None::<&str>)
                .into_iter()
                .find_map(|p| {
                    let path = p.clone();
                    PlayerSkin::load(&p).ok().map(|skin| (path, skin))
                })
                .map(|(p, skin)| Arc::new(CachedSkin::from_skin(skin, SkinSource::File(p))))
                .unwrap_or_else(|| Arc::clone(&self.default_skin))
        };
        if matches!(&loaded.source, SkinSource::File(_)) {
            self.retry_states.remove(&fast_key);
        }
        self.skins.insert(fast_key, Arc::clone(&loaded));
        loaded
    }

    fn drain_completed_downloads(&mut self) {
        while let Ok(result) = self.download_rx.try_recv() {
            let waiters = self
                .pending_downloads
                .remove(&result.texture_key)
                .unwrap_or_default();
            match result.skin {
                Ok(skin) => {
                    let mut variants: [Option<Arc<CachedSkin>>; 2] = [None, None];
                    let mut updated = false;
                    for waiter in waiters {
                        let index = usize::from(waiter.slim);
                        let cached = variants[index].get_or_insert_with(|| {
                            Arc::new(CachedSkin::from_skin(
                                skin.clone().with_slim_arms(waiter.slim),
                                SkinSource::File(result.path.clone()),
                            ))
                        });
                        self.retry_states.remove(&waiter.cache_key);
                        self.skins.insert(waiter.cache_key, Arc::clone(cached));
                        updated = true;
                    }
                    if updated {
                        self.content_generation = self.content_generation.wrapping_add(1);
                    }
                }
                Err(error) => {
                    self.mark_download_failed(&result.texture_key, waiters, &error);
                }
            }
        }
        while let Ok(result) = self.cape_rx.try_recv() {
            self.pending_cape_downloads.remove(&result.texture_key);
            match result.cape {
                Ok(pixels) => {
                    self.failed_cape_downloads.remove(&result.texture_key);
                    self.capes
                        .insert(result.texture_key, Arc::new(CachedCape::new(pixels)));
                    self.content_generation = self.content_generation.wrapping_add(1);
                }
                Err(error) => {
                    self.failed_cape_downloads.insert(
                        result.texture_key.clone(),
                        Instant::now() + Duration::from_secs(30),
                    );
                    log::warn!(
                        target: "rustcraft::skins",
                        "player cape download failed; retrying after 30s: texture={}, error={}",
                        result.texture_key,
                        error
                    );
                }
            }
        }
    }

    fn mark_download_failed(
        &mut self,
        texture_key: &str,
        waiters: Vec<PendingSkinWaiter>,
        error: &str,
    ) {
        let now = Instant::now();
        let mut longest_delay = Duration::from_secs(1);
        let waiter_count = waiters.len();
        for waiter in waiters {
            let state = self
                .retry_states
                .entry(waiter.cache_key)
                .or_insert(SkinRetryState {
                    failures: 0,
                    retry_after: now,
                    in_flight: false,
                    ready_announced: false,
                });
            state.failures = state.failures.saturating_add(1);
            let delay_secs = (1u64 << state.failures.saturating_sub(1).min(5)).min(30);
            let delay = Duration::from_secs(delay_secs);
            longest_delay = longest_delay.max(delay);
            state.retry_after = now + delay;
            state.in_flight = false;
            state.ready_announced = false;
        }
        log::warn!(
            target: "rustcraft::skins",
            "player skin download failed; retrying after {}s: texture={}, waiters={}, error={}",
            longest_delay.as_secs(),
            texture_key,
            waiter_count,
            error
        );
    }

    fn schedule_skin_download(
        &mut self,
        texture_key: String,
        skin_url: String,
        local_path: PathBuf,
        slim: bool,
        cache_key: String,
    ) {
        let waiter = PendingSkinWaiter { cache_key, slim };
        if let Some(waiters) = self.pending_downloads.get_mut(&texture_key) {
            if !waiters.contains(&waiter) {
                waiters.push(waiter.clone());
            }
            self.mark_download_in_flight(&waiter.cache_key);
            return;
        }
        self.pending_downloads
            .insert(texture_key.clone(), vec![waiter.clone()]);
        self.mark_download_in_flight(&waiter.cache_key);
        if self
            .download_job_tx
            .send(SkinDownloadJob {
                texture_key: texture_key.clone(),
                skin_url,
                local_path,
            })
            .is_err()
        {
            let waiters = self
                .pending_downloads
                .remove(&texture_key)
                .unwrap_or_default();
            self.mark_download_failed(&texture_key, waiters, "download worker is unavailable");
        }
    }

    fn mark_download_in_flight(&mut self, cache_key: &str) {
        let state = self
            .retry_states
            .entry(cache_key.to_string())
            .or_insert(SkinRetryState {
                failures: 0,
                retry_after: Instant::now(),
                in_flight: true,
                ready_announced: true,
            });
        state.in_flight = true;
        state.ready_announced = true;
    }

    fn candidate_paths(
        &self,
        uuid: Option<&str>,
        name: Option<&str>,
        texture_key: Option<&str>,
    ) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(texture_key) = texture_key {
            out.push(PathBuf::from(format!("assets/skins/{}.png", texture_key)));
        }
        for id in [uuid, name].into_iter().flatten() {
            let clean = id.replace('-', "");
            out.push(PathBuf::from(format!("assets/skins/{}.png", id)));
            if clean != id {
                out.push(PathBuf::from(format!("assets/skins/{}.png", clean)));
            }
        }
        out
    }
}

impl CachedSkin {
    fn from_skin(skin: PlayerSkin, source: SkinSource) -> Self {
        let preview = skin.preview_pixels();
        let face = skin.face_pixels();
        let slim = skin.slim_arms;
        let content_hash = skin_content_hash(&skin);
        Self {
            skin: Arc::new(skin),
            preview,
            face,
            slim,
            source,
            content_hash,
        }
    }
}

impl CachedCape {
    fn new(pixels: Vec<u8>) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        pixels.hash(&mut hasher);
        Self {
            pixels: Arc::new(pixels),
            content_hash: hasher.finish(),
        }
    }
}

pub(crate) fn skin_content_hash(skin: &PlayerSkin) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    skin.dimensions().hash(&mut hasher);
    std::mem::discriminant(&skin.layout).hash(&mut hasher);
    skin.pixels.as_raw().hash(&mut hasher);
    skin.slim_arms.hash(&mut hasher);
    hasher.finish()
}

fn parse_skin_property(value: &str) -> Option<SkinProfile> {
    let decoded = decode_base64(value)?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let textures = json.get("textures")?;
    let skin = textures.get("SKIN")?;
    let url = skin.get("url").and_then(|v| v.as_str());
    let skin_url = url.map(|s| s.to_string());
    let texture_key = url.and_then(|url| {
        url.rsplit('/')
            .next()
            .filter(|part| !part.is_empty())
            .map(str::to_string)
    });
    let slim = skin
        .get("metadata")
        .and_then(|v| v.get("model"))
        .and_then(|v| v.as_str())
        == Some("slim");
    let cape_url = textures
        .get("CAPE")
        .and_then(|c| c.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(SkinProfile {
        texture_key,
        skin_url,
        cape_url,
        slim,
    })
}

fn key_string(id: &str, texture_key: &str, slim: bool) -> String {
    format!(
        "{id}:{texture_key}:{}",
        if slim { "slim" } else { "classic" }
    )
}

fn download_skin_texture(
    client: &reqwest::blocking::Client,
    skin_url: &str,
    local_path: &std::path::Path,
) -> Result<PlayerSkin, String> {
    let url = if let Some(path) = skin_url.strip_prefix("http://textures.minecraft.net/") {
        format!("https://textures.minecraft.net/{path}")
    } else if skin_url.starts_with("http://") || skin_url.starts_with("https://") {
        skin_url.to_string()
    } else {
        return Err("profile contains an invalid skin URL".to_string());
    };
    let resp = client
        .get(&url)
        .send()
        .map_err(|error| format!("request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("server rejected request: {error}"))?;
    let bytes = resp
        .bytes()
        .map_err(|error| format!("failed to read response body: {error}"))?;
    if let Some(parent) = local_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let skin =
        PlayerSkin::from_bytes(&bytes).map_err(|error| format!("invalid skin image: {error}"))?;
    if let Err(error) = std::fs::write(local_path, &bytes) {
        log::debug!(
            target: "rustcraft::skins",
            "could not persist downloaded skin to {}: {}",
            local_path.display(),
            error
        );
    }
    Ok(skin)
}

fn download_cape_texture(
    client: &reqwest::blocking::Client,
    cape_url: &str,
    local_path: &std::path::Path,
) -> Result<Vec<u8>, String> {
    let url = if let Some(path) = cape_url.strip_prefix("http://textures.minecraft.net/") {
        format!("https://textures.minecraft.net/{path}")
    } else if cape_url.starts_with("http://") || cape_url.starts_with("https://") {
        cape_url.to_string()
    } else {
        return Err("profile contains an invalid cape URL".to_string());
    };
    let bytes = client
        .get(url)
        .send()
        .map_err(|error| format!("request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("server rejected request: {error}"))?
        .bytes()
        .map_err(|error| format!("failed to read response body: {error}"))?;
    let pixels = normalize_cape_image(
        image::load_from_memory(&bytes).map_err(|error| format!("invalid cape image: {error}"))?,
    );
    if let Some(parent) = local_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Some(image) = image::RgbaImage::from_raw(64, 32, pixels.clone()) {
        let _ = image.save(local_path);
    }
    Ok(pixels)
}

fn load_cape_pixels(path: &std::path::Path) -> Option<Vec<u8>> {
    image::open(path).ok().map(normalize_cape_image)
}

pub(crate) fn normalize_cape_image(image: image::DynamicImage) -> Vec<u8> {
    let rgba = image.to_rgba8();
    if rgba.dimensions() == (64, 32) {
        rgba.into_raw()
    } else {
        image::imageops::resize(&rgba, 64, 32, image::imageops::FilterType::Nearest).into_raw()
    }
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = [0u8; 4];
    let mut len = 0usize;

    for byte in input.bytes().filter(|b| !b.is_ascii_whitespace()) {
        if byte == b'=' {
            buf[len] = 64;
        } else {
            buf[len] = base64_value(byte)?;
        }
        len += 1;
        if len == 4 {
            push_base64_quad(&mut out, buf)?;
            len = 0;
        }
    }

    if len > 0 {
        for item in buf.iter_mut().skip(len) {
            *item = 64;
        }
        push_base64_quad(&mut out, buf)?;
    }

    Some(out)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        // Accept both the standard and URL-safe alphabets: offline-mode
        // server plugins (SkinsRestorer-style NPCs) emit URL-safe values.
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

fn push_base64_quad(out: &mut Vec<u8>, quad: [u8; 4]) -> Option<()> {
    if quad[0] == 64 || quad[1] == 64 {
        return None;
    }
    out.push((quad[0] << 2) | (quad[1] >> 4));
    if quad[2] != 64 {
        out.push(((quad[1] & 0x0f) << 4) | (quad[2] >> 2));
    }
    if quad[3] != 64 {
        out.push(((quad[2] & 0x03) << 6) | quad[3]);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_base64(input: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in input.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
            let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
            let triple = (b0 << 16) | (b1 << 8) | b2;
            out.push(ALPHABET[(triple >> 18) as usize & 63] as char);
            out.push(ALPHABET[(triple >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                ALPHABET[(triple >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                ALPHABET[triple as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    fn texture_property(texture_key: &str, slim: bool) -> String {
        let metadata = if slim {
            r#","metadata":{"model":"slim"}"#
        } else {
            ""
        };
        let json = format!(
            concat!(
                r#"{{"timestamp":1752581279000,"profileId":"1bcfc1727687415c997d111eb55a2959","#,
                r#""profileName":"TestUser","signatureRequired":true,"textures":{{"SKIN":"#,
                r#"{{"url":"http://textures.minecraft.net/texture/{key}"{metadata}}}}}}}"#
            ),
            key = texture_key,
            metadata = metadata
        );
        encode_base64(json.as_bytes())
    }

    #[test]
    fn parse_skin_property_accepts_real_mojang_profiles() {
        let padded = texture_property("57616107ff2a19288", false);
        let profile = parse_skin_property(&padded).expect("padded base64 must parse");
        assert_eq!(profile.texture_key.as_deref(), Some("57616107ff2a19288"));
        assert!(!profile.slim);

        let unpadded = padded.trim_end_matches('=').to_string();
        let profile = parse_skin_property(&unpadded).expect("unpadded base64 must parse");
        assert_eq!(profile.texture_key.as_deref(), Some("57616107ff2a19288"));

        let slim = texture_property("57616107ff2a19288", true);
        let profile = parse_skin_property(&slim).expect("slim profile must parse");
        assert!(profile.slim);
    }

    #[test]
    fn parse_skin_property_accepts_url_safe_base64() {
        let standard = texture_property("57616107ff2a19288", false);
        let url_safe = standard.replace('+', "-").replace('/', "_");
        let profile = parse_skin_property(&url_safe).expect("url-safe base64 must parse");
        assert_eq!(profile.texture_key.as_deref(), Some("57616107ff2a19288"));
    }

    #[test]
    fn lookup_uses_disk_cached_texture_from_profile_property() {
        let texture_key = format!("rustcraft-test-{}", std::process::id());
        let dir = std::path::Path::new("assets/skins");
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{texture_key}.png"));
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([1, 2, 3, 255]));
        img.save(&path).unwrap();

        let property = texture_property(&texture_key, false);
        let mut cache = SkinCache::new(&PlayerSkin::default_steve());
        let snapshot = cache.snapshot_for(
            Some("11111111-2222-3333-4444-555555555555"),
            Some("TestUser"),
            Some(&property),
        );
        std::fs::remove_file(&path).ok();

        assert!(
            matches!(snapshot.source, SkinSource::File(_)),
            "expected file-backed skin, got {:?}",
            snapshot.source
        );
        assert_eq!(snapshot.skin.sample(0, 0), [1, 2, 3, 255]);
    }

    #[test]
    fn snapshot_hits_share_cached_skin_and_pixels() {
        let mut cache = SkinCache::new(&PlayerSkin::default_steve());

        let first = cache.snapshot_for(None, None, None);
        let second = cache.snapshot_for(None, None, None);

        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first.skin, &second.skin));
        assert_eq!(first.content_hash, second.content_hash);
    }

    #[test]
    fn compatibility_skin_copy_does_not_mutate_snapshot() {
        let mut cache = SkinCache::new(&PlayerSkin::default_steve());
        let before = cache.snapshot_for(None, None, None);
        let original_pixel = before.skin.sample(0, 0);

        let mut owned = cache.skin_for(None, None, None);
        owned.pixels.put_pixel(0, 0, image::Rgba([1, 2, 3, 4]));

        let after = cache.snapshot_for(None, None, None);
        assert!(Arc::ptr_eq(&before, &after));
        assert_eq!(after.skin.sample(0, 0), original_pixel);
    }

    #[test]
    fn content_hash_includes_skin_model() {
        let classic = CachedSkin::from_skin(PlayerSkin::default_steve(), SkinSource::Default);
        let slim = CachedSkin::from_skin(
            PlayerSkin::default_steve().with_slim_arms(true),
            SkinSource::Default,
        );

        assert_ne!(classic.content_hash, slim.content_hash);
    }

    #[test]
    fn shared_texture_download_fills_classic_and_slim_waiters() {
        let mut cache = SkinCache::new(&PlayerSkin::default_steve());
        let texture_key = "shared-texture".to_string();
        let classic_key = "classic-cache-key".to_string();
        let slim_key = "slim-cache-key".to_string();

        cache.pending_downloads.insert(
            texture_key.clone(),
            vec![
                PendingSkinWaiter {
                    cache_key: classic_key.clone(),
                    slim: false,
                },
                PendingSkinWaiter {
                    cache_key: slim_key.clone(),
                    slim: true,
                },
            ],
        );

        let mut downloaded = PlayerSkin::default_steve();
        downloaded
            .pixels
            .put_pixel(0, 0, image::Rgba([12, 34, 56, 255]));
        cache
            .download_tx
            .send(SkinDownloadResult {
                texture_key: texture_key.clone(),
                path: PathBuf::from("shared-texture.png"),
                skin: Ok(downloaded),
            })
            .unwrap();

        assert_eq!(cache.poll_content_generation(), 1);
        assert_eq!(cache.poll_content_generation(), 1);

        assert!(!cache.pending_downloads.contains_key(&texture_key));
        let classic = cache.skins.get(&classic_key).unwrap();
        let slim = cache.skins.get(&slim_key).unwrap();
        assert!(!classic.slim);
        assert!(!classic.skin.slim_arms);
        assert!(slim.slim);
        assert!(slim.skin.slim_arms);
        assert_eq!(classic.skin.sample(0, 0), [12, 34, 56, 255]);
        assert_eq!(slim.skin.sample(0, 0), [12, 34, 56, 255]);
        assert_eq!(classic.skin.pixels, slim.skin.pixels);
        assert!(!Arc::ptr_eq(classic, slim));
        assert!(!Arc::ptr_eq(&classic.skin, &slim.skin));
        assert_ne!(classic.content_hash, slim.content_hash);
    }
}
