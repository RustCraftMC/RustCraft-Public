//! OptiFine custom sky support — reads `mcpatcher/sky/` from resource packs.
//!
//! Searches the legacy `mcpatcher/sky/world0/` and modern
//! `optifine/sky/world0/` locations, at either pack root or inside assets.
//!
//! Each `.properties` file defines one sky layer. The `source` key points to the
//! actual image (e.g. `source=./cloud1.png`). Time keys use **HH:MM** format —
//! e.g. `startFadeIn=18:00` → 18 000 ticks.
//!
//! Blending modes: `add`, `multiply`, `replace`. Rotation controlled by `speed`,
//! `axis`, and `rotate`.

use crate::assets::resolver::AssetResolver;

#[derive(Clone)]
pub struct SkyLayer {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub start_fade_in: Option<f32>,
    pub end_fade_in: Option<f32>,
    pub start_fade_out: Option<f32>,
    pub end_fade_out: Option<f32>,
    pub speed: f32,
    pub axis: [f32; 3],
    pub rotate: bool,
    pub blend: String,
}

#[derive(Clone)]
pub struct CustomSky {
    pub dimension: i8,
    pub layers: Vec<SkyLayer>,
    pub sun_pixels: Option<(Vec<u8>, u32, u32)>,
    pub moon_pixels: Option<(Vec<u8>, u32, u32)>,
}

impl CustomSky {
    pub fn load(resolver: &mut AssetResolver, dimension: i8) -> Option<Self> {
        let world_dir = match dimension {
            0 => "world0",
            -1 => "world-1",
            1 => "world1",
            _ => return None,
        };

        // Collect all .png data and .properties text from both possible locations.
        let mut png_data: std::collections::BTreeMap<String, Vec<u8>> =
            std::collections::BTreeMap::new();
        let mut props_texts: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();

        // Legacy MCPatcher and newer OptiFine source directories.
        let dirs = [
            format!("mcpatcher/sky/{}", world_dir),
            format!("assets/minecraft/mcpatcher/sky/{}", world_dir),
            format!("optifine/sky/{}", world_dir),
            format!("assets/minecraft/optifine/sky/{}", world_dir),
        ];

        for dir in &dirs {
            for entry in resolver.list_pack_dir(dir) {
                let full = format!("{}/{}", dir, entry);
                if entry.ends_with(".png") {
                    if let Some(data) = resolver.read_raw(&full) {
                        png_data.entry(entry.clone()).or_insert(data);
                    }
                } else if entry.ends_with(".properties") {
                    if let Some(bytes) = resolver.read_raw(&full) {
                        if let Ok(text) = String::from_utf8(bytes) {
                            props_texts.entry(entry.clone()).or_insert(text);
                        }
                    }
                }
            }
        }

        // Also scan the local disk `mcpatcher/sky/` directory.
        for base in [
            std::path::PathBuf::from("mcpatcher")
                .join("sky")
                .join(world_dir),
            std::path::PathBuf::from("assets")
                .join("minecraft")
                .join("mcpatcher")
                .join("sky")
                .join(world_dir),
        ] {
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".png") && !png_data.contains_key(&name) {
                        if let Ok(data) = std::fs::read(&entry.path()) {
                            png_data.insert(name, data);
                        }
                    } else if name.ends_with(".properties") && !props_texts.contains_key(&name) {
                        if let Ok(text) = std::fs::read_to_string(entry.path()) {
                            props_texts.insert(name, text);
                        }
                    }
                }
            }
        }

        // Custom sun / moon textures (read from packs or local disk).
        let read_png = |files: &std::collections::BTreeMap<String, Vec<u8>>,
                        name: &str|
         -> Option<(Vec<u8>, u32, u32)> {
            files.get(name).and_then(|data| {
                let img = image::load_from_memory(data).ok()?;
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                Some((rgba.into_raw(), w, h))
            })
        };

        let sun_pixels = read_png(&png_data, "sun.png");
        let moon_pixels = read_png(&png_data, "moon_phases.png");

        // Build layers from .properties files. OptiFine defaults a missing
        // source in skyN.properties to skyN.png.
        let mut layers: Vec<(String, SkyLayer)> = Vec::new();

        for (props_name, text) in &props_texts {
            let parsed = parse_properties(text);
            let default_source = props_name
                .strip_suffix(".properties")
                .map(|name| format!("{name}.png"));
            let source = parsed.source.as_deref().or(default_source.as_deref());
            let Some(source) = source else { continue };
            // source paths are relative, e.g. "./cloud1.png" → "cloud1.png"
            let img_name = source.trim_start_matches("./");

            let Some(data) = png_data.get(img_name) else {
                log::debug!("custom sky layer '{props_name}' references missing '{img_name}'");
                continue;
            };
            let Ok(img) = image::load_from_memory(data) else {
                log::debug!("custom sky layer '{props_name}' has an invalid '{img_name}' image");
                continue;
            };
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();

            layers.push((
                props_name.clone(),
                SkyLayer {
                    pixels: rgba.into_raw(),
                    width: w,
                    height: h,
                    start_fade_in: parsed.start_fade_in,
                    end_fade_in: parsed.end_fade_in,
                    start_fade_out: parsed.start_fade_out,
                    end_fade_out: parsed.end_fade_out,
                    speed: parsed.speed,
                    axis: parsed.axis,
                    rotate: parsed.rotate || parsed.speed.abs() > 0.001,
                    blend: parsed.blend,
                },
            ));
        }

        // Also load any remaining PNGs (e.g. `cloud1.png`) without a matching
        // .properties file as simple always-visible sky layers.
        for (name, data) in png_data.iter() {
            if !name.ends_with(".png") {
                continue;
            }
            if name == "sun.png" || name == "moon_phases.png" {
                continue;
            }
            let base = name.trim_end_matches(".png");
            let props_key = format!("{}.properties", base);
            if props_texts.contains_key(&props_key) {
                continue;
            }

            let img = image::load_from_memory(data).ok()?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            layers.push((
                name.clone(),
                SkyLayer {
                    pixels: rgba.into_raw(),
                    width: w,
                    height: h,
                    start_fade_in: None,
                    end_fade_in: None,
                    start_fade_out: None,
                    end_fade_out: None,
                    speed: 0.0,
                    axis: [0.0, 1.0, 0.0],
                    rotate: false,
                    blend: "replace".to_string(),
                },
            ));
        }

        if layers.is_empty() {
            return None;
        }

        // Sort by properties file name so that lower numbers = higher priority.
        // In OptiFine the lowest-numbered properties file is drawn first (back),
        // so higher-numbered files overlay on top.
        layers.sort_by(|a, b| a.0.cmp(&b.0));

        Some(CustomSky {
            dimension,
            layers: layers.into_iter().map(|(_, l)| l).collect(),
            sun_pixels,
            moon_pixels,
        })
    }

    pub fn layer_alpha(layer: &SkyLayer, time_of_day: f32) -> f32 {
        if layer.start_fade_in.is_none()
            && layer.end_fade_in.is_none()
            && layer.start_fade_out.is_none()
            && layer.end_fade_out.is_none()
        {
            return 1.0;
        }
        let ticks = time_of_day % 24000.0;
        let sf_in = layer.start_fade_in.unwrap_or(0.0);
        let ef_in = layer.end_fade_in.unwrap_or(6000.0);
        let sf_out = layer.start_fade_out.unwrap_or(18000.0);
        let ef_out = layer.end_fade_out.unwrap_or(24000.0);
        let elapsed = |start: f32| (ticks - start).rem_euclid(24000.0);
        let fade_in_span = (ef_in - sf_in).rem_euclid(24000.0);
        if fade_in_span > 1.0 && elapsed(sf_in) <= fade_in_span {
            return elapsed(sf_in) / fade_in_span;
        }
        let full_span = (sf_out - ef_in).rem_euclid(24000.0);
        if elapsed(ef_in) < full_span {
            return 1.0;
        }
        let fade_out_span = (ef_out - sf_out).rem_euclid(24000.0);
        if fade_out_span > 1.0 && elapsed(sf_out) <= fade_out_span {
            return 1.0 - elapsed(sf_out) / fade_out_span;
        }
        0.0
    }
}

#[derive(Default)]
struct ParsedProperties {
    source: Option<String>,
    start_fade_in: Option<f32>,
    end_fade_in: Option<f32>,
    start_fade_out: Option<f32>,
    end_fade_out: Option<f32>,
    speed: f32,
    axis: [f32; 3],
    rotate: bool,
    blend: String,
}

/// Parse an OptiFine `.properties` file.
///
/// Time keys accept either raw ticks (`6000`) or HH:MM notation (`6:00`).
fn parse_properties(text: &str) -> ParsedProperties {
    let mut p = ParsedProperties {
        blend: "replace".to_string(),
        axis: [0.0, 1.0, 0.0],
        ..Default::default()
    };

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some(kv) => (kv.0.trim().to_lowercase(), kv.1.trim().to_string()),
            None => continue,
        };

        match key.as_str() {
            "source" => p.source = Some(value),
            "startfadein" => p.start_fade_in = parse_optifine_time(&value),
            "endfadein" => p.end_fade_in = parse_optifine_time(&value),
            "startfadeout" => p.start_fade_out = parse_optifine_time(&value),
            "endfadeout" => p.end_fade_out = parse_optifine_time(&value),
            "speed" => {
                if let Ok(s) = value.parse::<f32>() {
                    p.speed = s;
                    if s.abs() > 0.001 {
                        p.rotate = true;
                    }
                }
            }
            "rotate" => p.rotate = value.parse().unwrap_or(false),
            "axis" => {
                let parts: Vec<f32> = value
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if parts.len() == 3 {
                    p.axis = [parts[0], parts[1], parts[2]];
                }
            }
            "blend" => p.blend = value.to_lowercase(),
            _ => {}
        }
    }

    p
}

/// Parse an OptiFine time value — either raw ticks (`18000`) or HH:MM (`18:00`).
fn parse_optifine_time(value: &str) -> Option<f32> {
    if let Ok(ticks) = value.parse::<f32>() {
        return Some(ticks);
    }
    let (h, m) = value.split_once(':')?;
    let hours: f32 = h.parse().ok()?;
    let minutes: f32 = m.parse().ok()?;
    Some((((hours - 6.0).rem_euclid(24.0) * 1000.0) + minutes * (1000.0 / 60.0)).round())
}

#[cfg(test)]
mod tests {
    use super::{parse_optifine_time, CustomSky, SkyLayer};

    #[test]
    fn parse_ticks() {
        assert_eq!(parse_optifine_time("6000"), Some(6000.0));
    }

    #[test]
    fn parse_hhmm() {
        assert_eq!(parse_optifine_time("18:00"), Some(12000.0));
        assert_eq!(parse_optifine_time("18:45"), Some(12750.0));
        // 18:50 = 12000 + 50*16.67 ≈ 12833
        let v = parse_optifine_time("18:50").unwrap();
        assert!((v - 12833.0).abs() < 2.0);
    }

    #[test]
    fn layers_without_fades_stay_visible_all_day() {
        let layer = SkyLayer {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            start_fade_in: None,
            end_fade_in: None,
            start_fade_out: None,
            end_fade_out: None,
            speed: 0.0,
            axis: [0.0; 3],
            rotate: false,
            blend: String::new(),
        };
        assert_eq!(CustomSky::layer_alpha(&layer, 0.0), 1.0);
        assert_eq!(CustomSky::layer_alpha(&layer, 18000.0), 1.0);
    }
}
