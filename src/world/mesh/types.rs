/// Interleaved vertex: position, normal, UV, tint type, two lightmap channels,
/// and the vanilla ambient-occlusion color multiplier.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub block_type: f32,
    pub sky_light: f32,
    pub block_light: f32,
    pub ambient_occlusion: f32,
}

impl Vertex {
    pub const STRIDE: u32 = std::mem::size_of::<Vertex>() as u32; // 48
}

/// A mesh for one chunk, ready to upload to the GPU.
#[derive(Clone)]
pub struct ChunkMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub cx: i32,
    pub cz: i32,
    /// Index into `indices` where transparent geometry begins.
    /// Indices `[0, transparent_start)` are opaque; `[transparent_start, ..)` are transparent.
    pub transparent_start: u32,
    /// World-space bounding box computed in the background mesh builder so the
    /// render thread doesn't have to scan every vertex during upload.
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct MeshOptions {
    pub smooth_lighting: bool,
    /// Sky brightness factor (0.0 = night, 1.0 = day). Affects sky light in mesh.
    pub sky_brightness: f32,
    /// OptiFine-style Better Grass — grass block sides use top texture + tint.
    pub better_grass: bool,
    /// OptiFine-style Connected Textures — glass faces use seamless look.
    pub connected_textures: bool,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            smooth_lighting: true,
            sky_brightness: 1.0,
            better_grass: false,
            connected_textures: true,
        }
    }
}

impl ChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}
