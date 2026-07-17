//! Native Vulkan shader-pack loading and runtime GLSL compilation.
//!
//! Packs live in `shaderpacks/` as either directories or zip files. Every pack
//! contains `rustcraft-shaders.json`; shader paths are relative to that file.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

pub const SHADER_PACK_DIR: &str = "shaderpacks";
pub const MANIFEST_NAME: &str = "rustcraft-shaders.json";
pub const SHADER_PACK_FORMAT: u32 = 1;
const MAX_PACK_FILES: usize = 4096;
const MAX_SHADER_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderCapabilities {
    pub ray_tracing: bool,
    pub fsr3: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderPackInfo {
    pub source_name: String,
    pub name: String,
    pub description: String,
    pub compatible: bool,
    pub requires_ray_tracing: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    WorldVertex,
    WorldFragment,
    EntityVertex,
    EntityFragment,
    SkyVertex,
    SkyFragment,
}

impl ShaderStage {
    fn from_manifest_key(key: &str) -> Option<Self> {
        Some(match key {
            "world.vertex" => Self::WorldVertex,
            "world.fragment" => Self::WorldFragment,
            "entity.vertex" => Self::EntityVertex,
            "entity.fragment" => Self::EntityFragment,
            "sky.vertex" => Self::SkyVertex,
            "sky.fragment" => Self::SkyFragment,
            _ => return None,
        })
    }

    fn shader_kind(self) -> shaderc::ShaderKind {
        match self {
            Self::WorldVertex | Self::EntityVertex | Self::SkyVertex => shaderc::ShaderKind::Vertex,
            Self::WorldFragment | Self::EntityFragment | Self::SkyFragment => {
                shaderc::ShaderKind::Fragment
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct ShaderPackManifest {
    format_version: u32,
    name: String,
    description: String,
    stages: BTreeMap<String, String>,
    defines: BTreeMap<String, String>,
    requires: ShaderPackRequirements,
}

impl Default for ShaderPackManifest {
    fn default() -> Self {
        Self {
            format_version: 0,
            name: String::new(),
            description: String::new(),
            stages: BTreeMap::new(),
            defines: BTreeMap::new(),
            requires: ShaderPackRequirements::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ShaderPackRequirements {
    ray_tracing: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ShaderPackShaders {
    stages: HashMap<ShaderStage, Vec<u32>>,
    pub active_name: Option<String>,
    pub error: Option<String>,
}

impl ShaderPackShaders {
    pub fn load_selected(selected: Option<&str>, capabilities: RenderCapabilities) -> Self {
        let Some(selected) = selected.filter(|name| !name.is_empty()) else {
            return Self::default();
        };
        match load_pack(selected, capabilities) {
            Ok(pack) => pack,
            Err(error) => {
                log::error!("shader pack '{selected}' could not be loaded: {error}");
                Self {
                    stages: HashMap::new(),
                    active_name: None,
                    error: Some(error),
                }
            }
        }
    }

    pub fn stage<'a>(&'a self, stage: ShaderStage, fallback: &'a [u8]) -> Vec<u32> {
        self.stages
            .get(&stage)
            .cloned()
            .unwrap_or_else(|| crate::render::spirv_words(fallback))
    }
}

pub fn discover_shader_packs(capabilities: RenderCapabilities) -> Vec<ShaderPackInfo> {
    let root = Path::new(SHADER_PACK_DIR);
    let _ = fs::create_dir_all(root);
    let mut packs = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return packs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir()
            && !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            continue;
        }
        let source_name = entry.file_name().to_string_lossy().into_owned();
        packs.push(inspect_pack(&source_name, capabilities));
    }
    packs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    packs
}

fn inspect_pack(source_name: &str, capabilities: RenderCapabilities) -> ShaderPackInfo {
    let fallback_name = source_name.trim_end_matches(".zip").to_string();
    match PackFiles::open(source_name).and_then(|files| parse_manifest(&files)) {
        Ok(manifest) => {
            let format_ok = manifest.format_version == SHADER_PACK_FORMAT;
            let rt_ok = !manifest.requires.ray_tracing || capabilities.ray_tracing;
            let error = if !format_ok {
                Some(format!(
                    "Unsupported format {} (expected {})",
                    manifest.format_version, SHADER_PACK_FORMAT
                ))
            } else if !rt_ok {
                Some("Requires VK_KHR_ray_tracing_pipeline".to_string())
            } else {
                None
            };
            ShaderPackInfo {
                source_name: source_name.to_string(),
                name: if manifest.name.is_empty() {
                    fallback_name
                } else {
                    manifest.name
                },
                description: manifest.description,
                compatible: error.is_none(),
                requires_ray_tracing: manifest.requires.ray_tracing,
                error,
            }
        }
        Err(error) => ShaderPackInfo {
            source_name: source_name.to_string(),
            name: fallback_name,
            description: String::new(),
            compatible: false,
            requires_ray_tracing: false,
            error: Some(error),
        },
    }
}

fn load_pack(
    source_name: &str,
    capabilities: RenderCapabilities,
) -> Result<ShaderPackShaders, String> {
    let files = PackFiles::open(source_name)?;
    let manifest = parse_manifest(&files)?;
    if manifest.format_version != SHADER_PACK_FORMAT {
        return Err(format!(
            "unsupported shader pack format {} (expected {})",
            manifest.format_version, SHADER_PACK_FORMAT
        ));
    }
    if manifest.requires.ray_tracing && !capabilities.ray_tracing {
        return Err("shader pack requires Vulkan ray tracing support".to_string());
    }

    let compiler = shaderc::Compiler::new().map_err(|error| error.to_string())?;
    let mut stages = HashMap::new();
    for (key, path) in &manifest.stages {
        let stage = ShaderStage::from_manifest_key(key)
            .ok_or_else(|| format!("unknown shader stage '{key}'"))?;
        let normalized = normalize_pack_path(path)?;
        let bytes = files
            .get(&normalized)
            .ok_or_else(|| format!("shader '{normalized}' does not exist"))?;
        let words = if normalized.ends_with(".spv") {
            bytes_to_words(bytes)?
        } else {
            compile_glsl(
                &compiler,
                &files,
                &normalized,
                bytes,
                stage,
                &manifest.defines,
            )?
        };
        stages.insert(stage, words);
    }
    log::info!(
        "loaded shader pack '{}' with {} overridden stages",
        manifest.name,
        stages.len()
    );
    Ok(ShaderPackShaders {
        stages,
        active_name: Some(if manifest.name.is_empty() {
            source_name.to_string()
        } else {
            manifest.name
        }),
        error: None,
    })
}

fn compile_glsl(
    compiler: &shaderc::Compiler,
    files: &PackFiles,
    path: &str,
    bytes: &[u8],
    stage: ShaderStage,
    defines: &BTreeMap<String, String>,
) -> Result<Vec<u32>, String> {
    let source =
        std::str::from_utf8(bytes).map_err(|_| format!("shader '{path}' is not valid UTF-8"))?;
    let mut options = shaderc::CompileOptions::new().map_err(|error| error.to_string())?;
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_3 as u32,
    );
    options.set_source_language(shaderc::SourceLanguage::GLSL);
    options.set_warnings_as_errors();
    for (name, value) in defines {
        options.add_macro_definition(name, (!value.is_empty()).then_some(value));
    }
    let include_files = files.files.clone();
    let requesting_file = path.to_string();
    options.set_include_callback(move |requested, include_type, requesting, _depth| {
        let base = if matches!(include_type, shaderc::IncludeType::Relative) {
            Path::new(requesting)
                .parent()
                .or_else(|| Path::new(&requesting_file).parent())
                .unwrap_or_else(|| Path::new(""))
                .join(requested)
        } else {
            PathBuf::from(requested)
        };
        let normalized = normalize_pack_path(&base.to_string_lossy())
            .map_err(|error| format!("invalid include '{requested}': {error}"))?;
        let content = include_files
            .get(&normalized)
            .ok_or_else(|| format!("include '{normalized}' does not exist"))?;
        let content = String::from_utf8(content.clone())
            .map_err(|_| format!("include '{normalized}' is not UTF-8"))?;
        Ok(shaderc::ResolvedInclude {
            resolved_name: normalized,
            content,
        })
    });
    compiler
        .compile_into_spirv(source, stage.shader_kind(), path, "main", Some(&options))
        .map(|artifact| artifact.as_binary().to_vec())
        .map_err(|error| format!("failed to compile '{path}': {error}"))
}

fn parse_manifest(files: &PackFiles) -> Result<ShaderPackManifest, String> {
    let bytes = files
        .get(MANIFEST_NAME)
        .ok_or_else(|| format!("missing {MANIFEST_NAME}"))?;
    serde_json::from_slice(bytes).map_err(|error| format!("invalid {MANIFEST_NAME}: {error}"))
}

fn bytes_to_words(bytes: &[u8]) -> Result<Vec<u32>, String> {
    if bytes.len() % 4 != 0 {
        return Err("SPIR-V byte length is not divisible by four".to_string());
    }
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    if words.first().copied() != Some(0x0723_0203) {
        return Err("shader is not valid little-endian SPIR-V".to_string());
    }
    Ok(words)
}

#[derive(Clone)]
struct PackFiles {
    files: HashMap<String, Vec<u8>>,
}

impl PackFiles {
    fn open(source_name: &str) -> Result<Self, String> {
        if Path::new(source_name).components().count() != 1
            || !matches!(
                Path::new(source_name).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err("shader pack name must not contain a path".to_string());
        }
        let root = Path::new(SHADER_PACK_DIR).join(source_name);
        let mut files = HashMap::new();
        if root.is_dir() {
            collect_directory(&root, &root, &mut files)?;
        } else {
            let data = fs::read(&root)
                .map_err(|error| format!("failed to read {}: {error}", root.display()))?;
            let mut archive = zip::ZipArchive::new(Cursor::new(data))
                .map_err(|error| format!("invalid shader pack zip: {error}"))?;
            if archive.len() > MAX_PACK_FILES {
                return Err(format!(
                    "shader pack contains more than {MAX_PACK_FILES} files"
                ));
            }
            for index in 0..archive.len() {
                let mut entry = archive
                    .by_index(index)
                    .map_err(|error| format!("invalid zip entry: {error}"))?;
                if entry.is_dir() {
                    continue;
                }
                if entry.size() > MAX_SHADER_FILE_BYTES {
                    return Err(format!("shader pack entry '{}' is too large", entry.name()));
                }
                let path = normalize_pack_path(entry.name())?;
                let mut data = Vec::new();
                entry
                    .read_to_end(&mut data)
                    .map_err(|error| format!("failed to read '{path}': {error}"))?;
                files.insert(path, data);
            }
            strip_single_root_directory(&mut files);
        }
        Ok(Self { files })
    }

    fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    output: &mut HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_directory(root, &path, output)?;
        } else {
            if output.len() >= MAX_PACK_FILES {
                return Err(format!(
                    "shader pack contains more than {MAX_PACK_FILES} files"
                ));
            }
            let file_size = entry
                .metadata()
                .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
                .len();
            if file_size > MAX_SHADER_FILE_BYTES {
                return Err(format!(
                    "shader pack file '{}' is too large",
                    path.display()
                ));
            }
            let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
            let normalized = normalize_pack_path(&relative.to_string_lossy())?;
            let data = fs::read(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            output.insert(normalized, data);
        }
    }
    Ok(())
}

fn strip_single_root_directory(files: &mut HashMap<String, Vec<u8>>) {
    if files.contains_key(MANIFEST_NAME) {
        return;
    }
    let Some(prefix) = files
        .keys()
        .find_map(|path| path.split_once('/').map(|(prefix, _)| prefix.to_string()))
    else {
        return;
    };
    let expected = format!("{prefix}/{MANIFEST_NAME}");
    if !files.contains_key(&expected)
        || files
            .keys()
            .any(|path| !path.starts_with(&format!("{prefix}/")))
    {
        return;
    }
    let old = std::mem::take(files);
    *files = old
        .into_iter()
        .map(|(path, data)| (path[prefix.len() + 1..].to_string(), data))
        .collect();
}

fn normalize_pack_path(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    let mut parts = Vec::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err("path escapes the shader pack".to_string());
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("absolute paths are not allowed".to_string())
            }
        }
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_cannot_escape_pack() {
        assert_eq!(
            normalize_pack_path("shaders/../common.glsl").unwrap(),
            "common.glsl"
        );
        assert!(normalize_pack_path("../secret").is_err());
        assert!(PackFiles::open("../outside.zip").is_err());
    }

    #[test]
    fn manifest_defaults_optional_fields() {
        let manifest: ShaderPackManifest = serde_json::from_str(
            r#"{"format_version":1,"name":"Test","stages":{"world.vertex":"world.vert"}}"#,
        )
        .unwrap();
        assert_eq!(manifest.format_version, SHADER_PACK_FORMAT);
        assert_eq!(manifest.name, "Test");
        assert!(!manifest.requires.ray_tracing);
    }

    #[test]
    fn runtime_compiler_resolves_relative_includes() {
        let files = PackFiles {
            files: HashMap::from([
                (
                    "shaders/main.vert".to_string(),
                    br#"#version 450
#include "common.glsl"
void main() { gl_Position = make_position(); }
"#
                    .to_vec(),
                ),
                (
                    "shaders/common.glsl".to_string(),
                    b"vec4 make_position() { return vec4(0.0); }\n".to_vec(),
                ),
            ]),
        };
        let compiler = shaderc::Compiler::new().unwrap();
        let words = compile_glsl(
            &compiler,
            &files,
            "shaders/main.vert",
            files.get("shaders/main.vert").unwrap(),
            ShaderStage::WorldVertex,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(words.first().copied(), Some(0x0723_0203));
    }
}
