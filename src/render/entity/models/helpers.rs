use crate::render::entity::mesh::{ModelCuboid, PartType};

/// MC 1.8.9 ModelBox UV layout (verbatim from ModelBox.java / TexturedQuad.java).
/// Returns face_uvs in mesh.rs order: [-Z, +Z, +Y, -Y, -X, +X].
/// All coordinates are in pixels; normalized to [0,1] using tex_w × tex_h.
#[inline]
pub fn box_uvs(tx: u32, ty: u32, w: u32, h: u32, d: u32, tex_w: u32, tex_h: u32) -> [[f32; 4]; 6] {
    let tw = tex_w as f32;
    let th = tex_h as f32;
    #[inline]
    fn n(u1: u32, v1: u32, u2: u32, v2: u32, tw: f32, th: f32) -> [f32; 4] {
        [
            u1 as f32 / tw,
            v1 as f32 / th,
            u2 as f32 / tw,
            v2 as f32 / th,
        ]
    }
    [
        // [0] -Z (back):   u[tx+d,     tx+d+w],   v[ty+d,   ty+d+h]
        n(tx + d, ty + d, tx + d + w, ty + d + h, tw, th),
        // [1] +Z (front):  u[tx+w+2d,  tx+2w+2d], v[ty+d,   ty+d+h]
        n(
            tx + w + 2 * d,
            ty + d,
            tx + 2 * w + 2 * d,
            ty + d + h,
            tw,
            th,
        ),
        // [2] +Y (top):    u[tx+d,     tx+d+w],   v[ty,     ty+d]
        n(tx + d, ty, tx + d + w, ty + d, tw, th),
        // [3] -Y (bottom): u[tx+d+w,   tx+d+2w],  v[ty,     ty+d]
        n(tx + d + w, ty, tx + d + 2 * w, ty + d, tw, th),
        // [4] -X (left):   u[tx,       tx+d],     v[ty+d,   ty+d+h]
        n(tx, ty + d, tx + d, ty + d + h, tw, th),
        // [5] +X (right):  u[tx+w+d,   tx+w+2d],  v[ty+d,   ty+d+h]
        n(tx + w + d, ty + d, tx + w + 2 * d, ty + d + h, tw, th),
    ]
}

/// Helper shorthand for 64×32 textures (most mobs).
pub fn uv64x32(tx: u32, ty: u32, w: u32, h: u32, d: u32) -> [[f32; 4]; 6] {
    box_uvs(tx, ty, w, h, d, 64, 32)
}

/// Helper shorthand for 64×64 textures (zombie, villager, bat, etc.).
pub fn uv64x64(tx: u32, ty: u32, w: u32, h: u32, d: u32) -> [[f32; 4]; 6] {
    box_uvs(tx, ty, w, h, d, 64, 64)
}

/// Helper shorthand for 128×128 textures (horse, iron golem).
pub fn uv128(tx: u32, ty: u32, w: u32, h: u32, d: u32) -> [[f32; 4]; 6] {
    box_uvs(tx, ty, w, h, d, 128, 128)
}

/// Helper shorthand for 256×256 textures (ender dragon).
pub fn uv256(tx: u32, ty: u32, w: u32, h: u32, d: u32) -> [[f32; 4]; 6] {
    box_uvs(tx, ty, w, h, d, 256, 256)
}

/// Helper shorthand for 64×128 textures (witch).
pub fn uv64x128(tx: u32, ty: u32, w: u32, h: u32, d: u32) -> [[f32; 4]; 6] {
    box_uvs(tx, ty, w, h, d, 64, 128)
}

// ---------------------------------------------------------------------------
// Cuboid constructor helpers
// ---------------------------------------------------------------------------

/// Vanilla-faithful constructor. Takes MC pixel coords directly as found in
/// the decompiled `ModelX.java` (`setRotationPoint`, `addBox`, `mirror`,
/// `rotateAngle`) and converts to RustCraft block units (+Y up, feet at y=0).
///
/// `origin_y` is the MC-pixel y of the model's feet (where the ground sits);
/// 24 for bipeds/quadrupeds, varies for short mobs. `rotation_mc` is the base
/// rotation in MC space (+Y down); X and Z are negated when converting to RC,
/// Y is unchanged.
pub fn mc_cuboid(
    rp: [f32; 3],
    off: [f32; 3],
    size: [f32; 3],
    origin_y: f32,
    rotation_mc: [f32; 3],
    mirror: bool,
    face_uvs: [[f32; 4]; 6],
    part: PartType,
) -> ModelCuboid {
    ModelCuboid {
        rotation_point: [rp[0] / 16.0, (origin_y - rp[1]) / 16.0, rp[2] / 16.0],
        box_offset: [off[0] / 16.0, -(off[1] + size[1]) / 16.0, off[2] / 16.0],
        size: [size[0] / 16.0, size[1] / 16.0, size[2] / 16.0],
        rotation: [-rotation_mc[0], rotation_mc[1], -rotation_mc[2]],
        mirror,
        face_uvs,
        color: [1.0; 4],
        part_type: part,
    }
}

/// `mc_cuboid` shorthand: no mirror, no base rotation.
pub fn mc_part(
    rp: [f32; 3],
    off: [f32; 3],
    size: [f32; 3],
    origin_y: f32,
    face_uvs: [[f32; 4]; 6],
    part: PartType,
) -> ModelCuboid {
    mc_cuboid(rp, off, size, origin_y, [0.0; 3], false, face_uvs, part)
}

/// Apply a colour tint (used for dyed layers like sheep fleece).
pub fn tint(mut c: ModelCuboid, color: [f32; 4]) -> ModelCuboid {
    c.color = color;
    c
}

/// Compatibility constructor: box centred at `pivot` (RC block units), rotating
/// around that same point with no base rotation. Preserves the pre-refactor
/// behaviour for models not yet re-derived from vanilla.
pub fn cuboid(
    pivot: [f32; 3],
    size: [f32; 3],
    face_uvs: [[f32; 4]; 6],
    part: PartType,
) -> ModelCuboid {
    ModelCuboid {
        rotation_point: pivot,
        box_offset: [-size[0] / 2.0, -size[1] / 2.0, -size[2] / 2.0],
        size,
        rotation: [0.0; 3],
        mirror: false,
        face_uvs,
        color: [1.0; 4],
        part_type: part,
    }
}

/// Convert MC pixel rotation-point to a RustCraft block-unit pivot (RC +Y up),
/// for use with the `cuboid` compat helper. Feet end up at y=0 when `origin_y`
/// is the MC-pixel y of the feet.
pub fn mc_pivot(rx: f32, ry: f32, rz: f32, origin_y: f32) -> [f32; 3] {
    [rx / 16.0, (origin_y - ry) / 16.0, rz / 16.0]
}
