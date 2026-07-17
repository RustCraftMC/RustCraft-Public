//! Block model system — parses MC JSON models and bakes geometry.
//!
//! Supports:
//! - `elements` with `from`/`to` box definitions
//! - Per-face textures with `#variable` references
//! - Parent model inheritance
//! - Blockstate variant → model mapping with rotation
//!
//! MC model format: assets/minecraft/models/block/*.json
//! MC blockstate format: assets/minecraft/blockstates/*.json

use serde::Deserialize;
use std::collections::HashMap;

/// A baked face ready for rendering.
#[derive(Clone, Debug)]
pub struct BakedFace {
    /// 4 vertex positions (in 0..16 block-space)
    pub vertices: [[f32; 3]; 4],
    /// UV coordinates (in 0..1 range)
    pub uvs: [[f32; 2]; 4],
    /// Texture name (resolved to atlas tile index)
    pub texture: String,
    /// Face normal
    pub normal: [f32; 3],
    /// Whether this face should be culled against adjacent blocks
    pub cullface: Option<FaceDir>,
    /// Rotation around Y axis (0, 90, 180, 270)
    pub rotation_y: u16,
    /// Tint index for biome coloring (0 = grass, 1 = foliage, None = no tint)
    pub tintindex: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceDir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl FaceDir {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "down" => Some(FaceDir::Down),
            "up" => Some(FaceDir::Up),
            "north" => Some(FaceDir::North),
            "south" => Some(FaceDir::South),
            "west" => Some(FaceDir::West),
            "east" => Some(FaceDir::East),
            _ => None,
        }
    }

    pub fn normal(self) -> [f32; 3] {
        match self {
            FaceDir::Down => [0.0, -1.0, 0.0],
            FaceDir::Up => [0.0, 1.0, 0.0],
            FaceDir::North => [0.0, 0.0, -1.0],
            FaceDir::South => [0.0, 0.0, 1.0],
            FaceDir::West => [-1.0, 0.0, 0.0],
            FaceDir::East => [1.0, 0.0, 0.0],
        }
    }
}

// --- JSON deserialization types ---

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct JsonModel {
    pub parent: Option<String>,
    pub textures: HashMap<String, String>,
    pub elements: Vec<JsonElement>,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct JsonElement {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub rotation: Option<JsonRotation>,
    pub faces: HashMap<String, JsonFace>,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct JsonRotation {
    pub origin: [f32; 3],
    pub axis: String,
    pub angle: f32,
    pub rescale: bool,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct JsonFace {
    pub uv: Option<[f32; 4]>,
    pub texture: String,
    pub cullface: Option<String>,
    pub rotation: Option<u16>,
    pub tintindex: Option<i32>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct JsonBlockstate {
    pub variants: HashMap<String, JsonBlockstateVariantList>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum JsonBlockstateVariantList {
    One(JsonBlockstateVariant),
    Many(Vec<JsonBlockstateVariant>),
}

impl JsonBlockstateVariantList {
    fn into_vec(self) -> Vec<JsonBlockstateVariant> {
        match self {
            Self::One(variant) => vec![variant],
            Self::Many(variants) => variants,
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct JsonBlockstateVariant {
    pub model: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    pub uvlock: Option<bool>,
}

/// A resolved block model with baked faces.
#[derive(Clone, Debug)]
pub struct BlockModel {
    pub faces: Vec<BakedFace>,
    pub is_full_block: bool,
}

/// Model registry — loads and caches all block models.
pub struct ModelRegistry {
    pub models: HashMap<String, BlockModel>,
    /// Raw JSON models for parent chain resolution (not baked yet)
    raw_models: HashMap<String, JsonModel>,
    blockstates: HashMap<String, HashMap<String, Vec<JsonBlockstateVariant>>>,
    /// Texture name → atlas tile index
    pub texture_map: HashMap<String, usize>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        ModelRegistry {
            models: HashMap::new(),
            raw_models: HashMap::new(),
            blockstates: HashMap::new(),
            texture_map: HashMap::new(),
        }
    }

    /// Load all models and blockstates from a resource pack directory.
    pub fn load_from_pack(&mut self, base: &str) {
        // Load blockstate definitions
        let bs_dir = format!("{}/blockstates", base);
        if let Ok(entries) = std::fs::read_dir(&bs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(bs) = serde_json::from_str::<JsonBlockstate>(&text) {
                            let mut variants = HashMap::new();
                            for (key, value) in bs.variants {
                                variants.insert(key, value.into_vec());
                            }
                            self.blockstates.insert(name, variants);
                        }
                    }
                }
            }
        }

        // Load raw block models (don't bake yet — need full parent chain)
        let model_dir = format!("{}/models/block", base);
        if let Ok(entries) = std::fs::read_dir(&model_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(json) = serde_json::from_str::<JsonModel>(&text) {
                            self.raw_models.insert(name, json);
                        }
                    }
                }
            }
        }

        log::info!(
            "vanilla model registry loaded: raw_models={}, blockstates={}",
            self.raw_models.len(),
            self.blockstates.len()
        );
    }

    /// Load vanilla model names while resolving each file through the active
    /// resource-pack stack, so overrides replace the matching vanilla JSON.
    pub fn load_with_resolver(&mut self, resolver: &mut crate::assets::resolver::AssetResolver) {
        for file in resolver.list_resource_files("blockstates", "json") {
            let name = file.trim_end_matches(".json").to_string();
            let resource = format!("minecraft/blockstates/{}", file);
            if let Some(text) = resolver.read_string(&resource) {
                if let Ok(bs) = serde_json::from_str::<JsonBlockstate>(&text) {
                    let variants = bs
                        .variants
                        .into_iter()
                        .map(|(key, value)| (key, value.into_vec()))
                        .collect();
                    self.blockstates.insert(name, variants);
                }
            }
        }

        for file in resolver.list_resource_files("models/block", "json") {
            let name = file.trim_end_matches(".json").to_string();
            let resource = format!("minecraft/models/block/{}", file);
            if let Some(text) = resolver.read_string(&resource) {
                if let Ok(model) = serde_json::from_str::<JsonModel>(&text) {
                    self.raw_models.insert(name, model);
                }
            }
        }

        log::info!(
            "resolved model registry loaded: models={}, blockstates={}",
            self.raw_models.len(),
            self.blockstates.len()
        );
    }

    /// Bake a model by name, resolving the full parent chain recursively.
    pub fn bake_model_by_name(&mut self, name: &str) -> Option<BlockModel> {
        if let Some(cached) = self.models.get(name) {
            return Some(cached.clone());
        }
        // Clone the raw model to avoid borrow issues
        let raw = self.raw_models.get(name)?.clone();
        let baked = self.resolve_and_bake(&raw, name, &mut Vec::new());
        self.models.insert(name.to_string(), baked.clone());
        Some(baked)
    }

    /// Recursively resolve parent chain and bake elements.
    fn resolve_and_bake(
        &self,
        json: &JsonModel,
        name: &str,
        chain: &mut Vec<String>,
    ) -> BlockModel {
        // Prevent infinite loops
        if chain.contains(&name.to_string()) {
            return BlockModel {
                faces: Vec::new(),
                is_full_block: false,
            };
        }
        chain.push(name.to_string());

        // Walk up the full parent chain, collecting textures and finding elements.
        // Textures: child overrides parent (like vanilla MC).
        // Elements: use the first (deepest) ancestor that defines them.
        let mut textures = HashMap::new();
        let mut elements = None;

        // Collect all models in the chain (from leaf to root), with depth limit
        let mut chain_models: Vec<&JsonModel> = Vec::new();
        let mut cur = Some(json);
        let mut depth = 0;
        while let Some(m) = cur {
            chain_models.push(m);
            depth += 1;
            if depth > 16 {
                break;
            }
            if let Some(ref parent_name) = m.parent {
                let parent_key = normalize_model_name(parent_name);
                cur = self.raw_models.get(&parent_key);
            } else {
                cur = None;
            }
        }

        // The first model from the leaf upward that defines elements owns the
        // geometry. Parent models only provide geometry when the child does not.
        for model in &chain_models {
            if !model.elements.is_empty() {
                elements = Some(model.elements.clone());
                break;
            }
        }

        // Walk from root to leaf: root textures first, then child overrides.
        for model in chain_models.iter().rev() {
            for (k, v) in &model.textures {
                textures.insert(k.clone(), v.clone());
            }
        }

        let elements = elements.unwrap_or_default();
        self.bake_elements(&elements, &textures)
    }

    /// Resolve a texture reference from a textures map.
    fn resolve_texture_from_map(
        &self,
        tex_ref: &str,
        textures: &HashMap<String, String>,
    ) -> String {
        if tex_ref.starts_with('#') {
            let var = &tex_ref[1..];
            if let Some(resolved) = textures.get(var) {
                return self.resolve_texture_from_map(resolved, textures);
            }
            // Fallback: return "blocks/stone" for unresolved variables
            return "blocks/stone".to_string();
        }
        normalize_texture_name(tex_ref)
    }

    /// Bake elements into faces using the merged textures.
    fn bake_elements(
        &self,
        elements: &[JsonElement],
        textures: &HashMap<String, String>,
    ) -> BlockModel {
        let mut faces = Vec::new();
        let is_full_block = elements.len() == 1
            && elements[0].from == [0.0, 0.0, 0.0]
            && elements[0].to == [16.0, 16.0, 16.0];

        for elem in elements {
            let from = elem.from;
            let to = elem.to;

            let rot = elem.rotation.as_ref();

            for (face_name, face) in &elem.faces {
                let dir = FaceDir::from_str(face_name);
                let normal = dir.map(|d| d.normal()).unwrap_or([0.0, 1.0, 0.0]);

                let tex = self.resolve_texture(&face.texture, &textures);

                // --- face UVs (0..1) ---
                let uvs: [[f32; 2]; 4] = if let Some(uv) = &face.uv {
                    // MC convention: uv[1]=min_v (top), uv[3]=max_v (bottom)
                    // Vertex order: V0=y_min(bottom), V1=y_min, V2=y_max(top), V3=y_max
                    // So V0/V1 get uv[3]/16 (bottom of texture area), V2/V3 get uv[1]/16 (top)
                    [
                        [uv[0] / 16.0, uv[3] / 16.0],
                        [uv[2] / 16.0, uv[3] / 16.0],
                        [uv[2] / 16.0, uv[1] / 16.0],
                        [uv[0] / 16.0, uv[1] / 16.0],
                    ]
                } else {
                    // Auto UV: project the element onto the texture plane
                    // VERTICAL faces: element bottom (y=from[1]) → texture bottom (V=1.0),
                    //                 element top (y=to[1]) → texture top (V=0.0).
                    // This matches auto_element_uv in builder.rs.
                    match dir {
                        Some(FaceDir::Up) => [
                            [from[0] / 16.0, (16.0 - to[2]) / 16.0],
                            [to[0] / 16.0, (16.0 - to[2]) / 16.0],
                            [to[0] / 16.0, (16.0 - from[2]) / 16.0],
                            [from[0] / 16.0, (16.0 - from[2]) / 16.0],
                        ],
                        Some(FaceDir::Down) => [
                            [from[0] / 16.0, from[2] / 16.0],
                            [to[0] / 16.0, from[2] / 16.0],
                            [to[0] / 16.0, to[2] / 16.0],
                            [from[0] / 16.0, to[2] / 16.0],
                        ],
                        Some(FaceDir::North) => [
                            [(16.0 - to[0]) / 16.0, (16.0 - from[1]) / 16.0],
                            [(16.0 - from[0]) / 16.0, (16.0 - from[1]) / 16.0],
                            [(16.0 - from[0]) / 16.0, (16.0 - to[1]) / 16.0],
                            [(16.0 - to[0]) / 16.0, (16.0 - to[1]) / 16.0],
                        ],
                        Some(FaceDir::South) => [
                            [from[0] / 16.0, (16.0 - from[1]) / 16.0],
                            [to[0] / 16.0, (16.0 - from[1]) / 16.0],
                            [to[0] / 16.0, (16.0 - to[1]) / 16.0],
                            [from[0] / 16.0, (16.0 - to[1]) / 16.0],
                        ],
                        Some(FaceDir::West) => [
                            [from[2] / 16.0, (16.0 - from[1]) / 16.0],
                            [to[2] / 16.0, (16.0 - from[1]) / 16.0],
                            [to[2] / 16.0, (16.0 - to[1]) / 16.0],
                            [from[2] / 16.0, (16.0 - to[1]) / 16.0],
                        ],
                        Some(FaceDir::East) => [
                            [(16.0 - to[2]) / 16.0, (16.0 - from[1]) / 16.0],
                            [(16.0 - from[2]) / 16.0, (16.0 - from[1]) / 16.0],
                            [(16.0 - from[2]) / 16.0, (16.0 - to[1]) / 16.0],
                            [(16.0 - to[2]) / 16.0, (16.0 - to[1]) / 16.0],
                        ],
                        None => [[0.0; 2]; 4],
                    }
                };

                // Apply face UV rotation (0/90/180/270)
                let uvs = if let Some(rot_angle) = face.rotation {
                    if rot_angle != 0 {
                        rotate_uvs_90(uvs, rot_angle)
                    } else {
                        uvs
                    }
                } else {
                    uvs
                };

                // --- vertex positions for this face (0..16) ---
                let raw_verts: [[f32; 3]; 4] = match dir {
                    Some(FaceDir::Up) => [
                        [from[0], to[1], to[2]],
                        [to[0], to[1], to[2]],
                        [to[0], to[1], from[2]],
                        [from[0], to[1], from[2]],
                    ],
                    Some(FaceDir::Down) => [
                        [from[0], from[1], from[2]],
                        [to[0], from[1], from[2]],
                        [to[0], from[1], to[2]],
                        [from[0], from[1], to[2]],
                    ],
                    Some(FaceDir::North) => [
                        [to[0], from[1], from[2]],
                        [from[0], from[1], from[2]],
                        [from[0], to[1], from[2]],
                        [to[0], to[1], from[2]],
                    ],
                    Some(FaceDir::South) => [
                        [from[0], from[1], to[2]],
                        [to[0], from[1], to[2]],
                        [to[0], to[1], to[2]],
                        [from[0], to[1], to[2]],
                    ],
                    Some(FaceDir::West) => [
                        [from[0], from[1], from[2]],
                        [from[0], from[1], to[2]],
                        [from[0], to[1], to[2]],
                        [from[0], to[1], from[2]],
                    ],
                    Some(FaceDir::East) => [
                        [to[0], from[1], to[2]],
                        [to[0], from[1], from[2]],
                        [to[0], to[1], from[2]],
                        [to[0], to[1], to[2]],
                    ],
                    None => [[0.0; 3]; 4],
                };

                // Apply element-level rotation (X/Y/Z axis)
                let (verts, rotated_normal) = if let Some(r) = rot {
                    rotate_element_verts(raw_verts, normal, r)
                } else {
                    (raw_verts, normal)
                };

                // Faces without cullface are vanilla general quads and must
                // remain visible regardless of neighboring blocks.
                // Element rotation affects the baked vertices, not cullface.
                // ModelBakery only rotates cullface with the blockstate model rotation.
                let cullface = face.cullface.as_deref().and_then(FaceDir::from_str);

                faces.push(BakedFace {
                    vertices: verts,
                    uvs,
                    texture: tex,
                    normal: rotated_normal,
                    cullface,
                    rotation_y: 0,
                    tintindex: face.tintindex,
                });
            }
        }

        BlockModel {
            faces,
            is_full_block,
        }
    }

    fn resolve_texture(&self, tex_ref: &str, textures: &HashMap<String, String>) -> String {
        if tex_ref.starts_with('#') {
            let var = &tex_ref[1..];
            if let Some(resolved) = textures.get(var) {
                return self.resolve_texture(resolved, textures);
            }
            // Fallback for unresolved texture variables
            return "blocks/stone".to_string();
        }
        normalize_texture_name(tex_ref)
    }

    /// Get the atlas tile index for a texture name.
    pub fn texture_index(&self, name: &str) -> usize {
        self.texture_map.get(name).copied().unwrap_or(1) // fallback: stone
    }

    /// Bake a model for a specific blockstate variant, applying x/y rotation.
    /// This is called by BlockModelCache when building the cache.
    pub fn bake_variant_model(
        &mut self,
        model_name: &str,
        _variant_key: &str,
    ) -> Option<BlockModel> {
        self.bake_model_by_name(model_name)
    }

    /// Load a specific blockstate JSON and return all variants with their rotation.
    pub fn load_blockstate_variants(&self, block_name: &str) -> Vec<(String, String, f32, f32)> {
        let Some(variants) = self.blockstates.get(block_name) else {
            return Vec::new();
        };
        let mut keys: Vec<_> = variants.keys().collect();
        keys.sort_unstable();
        keys.into_iter()
            .flat_map(|key| {
                variants[key].iter().map(move |variant| {
                    (
                        key.clone(),
                        normalize_model_name(&variant.model),
                        variant.x,
                        variant.y,
                    )
                })
            })
            .collect()
    }

    /// Bake a model with x/y rotation applied (rotates vertex positions + normals).
    pub fn bake_model_with_rotation(
        &mut self,
        model_name: &str,
        x_rot: f32,
        y_rot: f32,
    ) -> Option<BlockModel> {
        let mut model = self.bake_variant_model(model_name, "")?;

        if x_rot == 0.0 && y_rot == 0.0 {
            return Some(model);
        }

        // Apply rotation to each face's vertices, UVs, and normals
        for face in &mut model.faces {
            for v in &mut face.vertices {
                *v = rotate_vertex(*v, x_rot, y_rot);
            }
            face.normal = rotate_normal(face.normal, x_rot, y_rot);
            face.cullface = face.cullface.map(|d| rotate_facedir(d, x_rot, y_rot));
        }

        Some(model)
    }
}

fn normalize_model_name(name: &str) -> String {
    name.strip_prefix("minecraft:")
        .unwrap_or(name)
        .strip_prefix("block/")
        .unwrap_or_else(|| name.strip_prefix("minecraft:").unwrap_or(name))
        .to_string()
}

fn normalize_texture_name(name: &str) -> String {
    name.strip_prefix("minecraft:").unwrap_or(name).to_string()
}

/// Rotate a vertex position (in 0..16 block space) around the block center (8,8,8).
fn rotate_vertex(v: [f32; 3], x_rot_deg: f32, y_rot_deg: f32) -> [f32; 3] {
    let mut p = [v[0] - 8.0, v[1] - 8.0, v[2] - 8.0];

    // Y rotation (around Y axis)
    if y_rot_deg != 0.0 {
        // ModelRotation applies blockstate rotations with a negative angle.
        // This sign is observable on asymmetric textures such as curved rails.
        let rad = (-y_rot_deg).to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let (x, z) = (p[0], p[2]);
        p[0] = x * cos + z * sin;
        p[2] = -x * sin + z * cos;
    }

    // X rotation (around X axis)
    if x_rot_deg != 0.0 {
        let rad = (-x_rot_deg).to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let (y, z) = (p[1], p[2]);
        p[1] = y * cos - z * sin;
        p[2] = y * sin + z * cos;
    }

    [p[0] + 8.0, p[1] + 8.0, p[2] + 8.0]
}

/// Rotate a normal vector.
fn rotate_normal(n: [f32; 3], x_rot_deg: f32, y_rot_deg: f32) -> [f32; 3] {
    let mut p = n;

    if y_rot_deg != 0.0 {
        let rad = (-y_rot_deg).to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let (x, z) = (p[0], p[2]);
        p[0] = x * cos + z * sin;
        p[2] = -x * sin + z * cos;
    }

    if x_rot_deg != 0.0 {
        let rad = (-x_rot_deg).to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let (y, z) = (p[1], p[2]);
        p[1] = y * cos - z * sin;
        p[2] = y * sin + z * cos;
    }

    p
}

/// Rotate a face direction.
fn rotate_facedir(d: FaceDir, x_rot_deg: f32, y_rot_deg: f32) -> FaceDir {
    let n = d.normal();
    let rotated = rotate_normal(n, x_rot_deg, y_rot_deg);
    let abs = [rotated[0].abs(), rotated[1].abs(), rotated[2].abs()];
    if abs[0] >= abs[1] && abs[0] >= abs[2] {
        if rotated[0] > 0.0 {
            FaceDir::East
        } else {
            FaceDir::West
        }
    } else if abs[1] >= abs[0] && abs[1] >= abs[2] {
        if rotated[1] > 0.0 {
            FaceDir::Up
        } else {
            FaceDir::Down
        }
    } else {
        if rotated[2] > 0.0 {
            FaceDir::South
        } else {
            FaceDir::North
        }
    }
}

// ── helpers used by bake_elements ──

fn rotate_uvs_90(uvs: [[f32; 2]; 4], amount: u16) -> [[f32; 2]; 4] {
    let steps = (amount / 90) % 4;
    if steps == 0 {
        return uvs;
    }
    let mut result = uvs;
    for _ in 0..steps {
        let prev = result;
        for i in 0..4 {
            result[i] = [1.0 - prev[i][1], prev[i][0]];
        }
    }
    result
}

fn rotate_element_verts(
    mut verts: [[f32; 3]; 4],
    mut normal: [f32; 3],
    rot: &JsonRotation,
) -> ([[f32; 3]; 4], [f32; 3]) {
    let origin = rot.origin;
    let angle = rot.angle;
    if angle == 0.0 {
        return (verts, normal);
    }
    let rad = angle.to_radians();
    let (s, c) = rad.sin_cos();

    match rot.axis.as_str() {
        "x" => {
            for v in &mut verts {
                let dy = v[1] - origin[1];
                let dz = v[2] - origin[2];
                v[1] = origin[1] + dy * c - dz * s;
                v[2] = origin[2] + dy * s + dz * c;
            }
            let (ny, nz) = (normal[1], normal[2]);
            normal[1] = ny * c - nz * s;
            normal[2] = ny * s + nz * c;
        }
        "y" => {
            for v in &mut verts {
                let dx = v[0] - origin[0];
                let dz = v[2] - origin[2];
                v[0] = origin[0] + dx * c + dz * s;
                v[2] = origin[2] - dx * s + dz * c;
            }
            let (nx, nz) = (normal[0], normal[2]);
            normal[0] = nx * c + nz * s;
            normal[2] = -nx * s + nz * c;
        }
        "z" => {
            for v in &mut verts {
                let dx = v[0] - origin[0];
                let dy = v[1] - origin[1];
                v[0] = origin[0] + dx * c - dy * s;
                v[1] = origin[1] + dx * s + dy * c;
            }
            let (nx, ny) = (normal[0], normal[1]);
            normal[0] = nx * c - ny * s;
            normal[1] = nx * s + ny * c;
        }
        _ => {}
    }
    (verts, normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blockstate_accepts_vanilla_weighted_variant_arrays() {
        let json =
            r#"{"variants":{"normal":[{"model":"stone"},{"model":"stone_mirrored","y":180}]}}"#;
        let parsed: JsonBlockstate = serde_json::from_str(json).unwrap();
        let variants = parsed.variants.into_iter().next().unwrap().1.into_vec();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].model, "stone");
        assert_eq!(variants[1].y, 180.0);
    }

    #[test]
    fn namespaced_model_references_are_normalized() {
        assert_eq!(normalize_model_name("minecraft:block/stone"), "stone");
        assert_eq!(
            normalize_texture_name("minecraft:blocks/stone"),
            "blocks/stone"
        );
    }

    #[test]
    fn faces_without_cullface_remain_general_quads() {
        let model: JsonModel = serde_json::from_str(
            r##"{
                "textures":{"all":"blocks/stone"},
                "elements":[{
                    "from":[0,0,0],"to":[16,16,16],
                    "faces":{
                        "north":{"texture":"#all"},
                        "south":{"texture":"#all","cullface":"south"}
                    }
                }]
            }"##,
        )
        .unwrap();
        let registry = ModelRegistry::new();
        let baked = registry.bake_elements(&model.elements, &model.textures);

        let north = baked
            .faces
            .iter()
            .find(|face| face.normal == FaceDir::North.normal())
            .unwrap();
        let south = baked
            .faces
            .iter()
            .find(|face| face.normal == FaceDir::South.normal())
            .unwrap();
        assert_eq!(north.cullface, None);
        assert_eq!(south.cullface, Some(FaceDir::South));
    }

    #[test]
    fn element_rotation_does_not_rotate_cullface() {
        let model: JsonModel = serde_json::from_str(
            r##"{
                "textures":{"all":"blocks/stone"},
                "elements":[{
                    "from":[0,0,0],"to":[16,16,16],
                    "rotation":{"origin":[8,8,8],"axis":"y","angle":90},
                    "faces":{"north":{"texture":"#all","cullface":"north"}}
                }]
            }"##,
        )
        .unwrap();
        let registry = ModelRegistry::new();
        let baked = registry.bake_elements(&model.elements, &model.textures);

        assert_eq!(baked.faces.len(), 1);
        assert_eq!(baked.faces[0].cullface, Some(FaceDir::North));
    }

    #[test]
    fn blockstate_rotation_uses_vanilla_negative_angles() {
        let assert_vec3_close = |actual: [f32; 3], expected: [f32; 3]| {
            for axis in 0..3 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 1.0e-5,
                    "axis {axis}: expected {}, got {}",
                    expected[axis],
                    actual[axis]
                );
            }
        };

        assert_vec3_close(rotate_vertex([16.0, 8.0, 8.0], 0.0, 90.0), [8.0, 8.0, 16.0]);
        assert_vec3_close(rotate_normal([1.0, 0.0, 0.0], 0.0, 90.0), [0.0, 0.0, 1.0]);
        assert_vec3_close(rotate_normal([0.0, 1.0, 0.0], 90.0, 0.0), [0.0, 0.0, -1.0]);
    }
}
