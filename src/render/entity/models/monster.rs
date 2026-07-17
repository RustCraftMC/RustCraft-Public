use super::helpers::*;
use crate::render::entity::mesh::{ModelCuboid, PartType};

// =========================================================================
// Creeper — 64×32 texture. Vanilla ModelCreeper: head rp(0,6,0) @ (0,0) 8×8×8,
// body rp(0,6,0) @ (16,16) 8×12×4, legs rp(±2,18,±4) @ (0,16) 4×6×4.
// Legs rp.y=18, height 6 → feet at y=24, so origin_y=24.
// Leg anim: leg1&leg4 phase A, leg2&leg3 phase B (diagonal trot).

pub fn creeper_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 8, 8, 8);
    let body = uv64x32(16, 16, 8, 12, 4);
    let leg = uv64x32(0, 16, 4, 6, 4);
    let oy = 24.0;
    let zero = [0.0; 3];

    vec![
        mc_part(
            [0.0, 6.0, 0.0],
            [-4.0, -8.0, -4.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 6.0, 0.0],
            [-4.0, 0.0, -2.0],
            [8.0, 12.0, 4.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-2.0, 18.0, 4.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [2.0, 18.0, 4.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [-2.0, 18.0, -4.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [2.0, 18.0, -4.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
    ]
}

// =========================================================================
// Spider — 64×32 texture. Vanilla ModelSpider: i=15.
// head rp(0,15,-3) @ (32,4) 8×8×8, neck rp(0,15,0) @ (0,0) 6×6×6,
// body rp(0,15,9) @ (0,12) 10×8×12. 8 legs @ (18,0) 16×2×2, each splayed by a
// static rotateAngleY/Z (set in setRotationAngles) — baked in as base rotation.
// Legs are Extra (static pose; spider walk anim not applied).

pub fn spider_model() -> Vec<ModelCuboid> {
    let head = uv64x32(32, 4, 8, 8, 8);
    let neck = uv64x32(0, 0, 6, 6, 6);
    let body = uv64x32(0, 12, 10, 8, 12);
    let leg = uv64x32(18, 0, 16, 2, 2);
    let oy = 24.0;
    let p4 = std::f32::consts::FRAC_PI_4;
    let p8 = std::f32::consts::FRAC_PI_8;
    // leg addBox offset: -x legs extend from -15, +x legs from -1.
    let off_l = [-15.0, -1.0, -1.0];
    let off_r = [-1.0, -1.0, -1.0];

    vec![
        mc_part(
            [0.0, 15.0, -3.0],
            [-4.0, -4.0, -8.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 15.0, 0.0],
            [-3.0, -3.0, -3.0],
            [6.0, 6.0, 6.0],
            oy,
            neck,
            PartType::Body,
        ),
        mc_part(
            [0.0, 15.0, 9.0],
            [-5.0, -4.0, -6.0],
            [10.0, 8.0, 12.0],
            oy,
            body,
            PartType::Body,
        ),
        // 8 legs: [rp, off, rotation_mc(rx,ry,rz)]
        mc_cuboid(
            [-4.0, 15.0, 2.0],
            off_l,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, p4, -p4],
            false,
            leg,
            PartType::SpiderLeg(0),
        ),
        mc_cuboid(
            [4.0, 15.0, 2.0],
            off_r,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, -p4, p4],
            false,
            leg,
            PartType::SpiderLeg(1),
        ),
        mc_cuboid(
            [-4.0, 15.0, 1.0],
            off_l,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, p8, -p4 * 0.74],
            false,
            leg,
            PartType::SpiderLeg(2),
        ),
        mc_cuboid(
            [4.0, 15.0, 1.0],
            off_r,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, -p8, p4 * 0.74],
            false,
            leg,
            PartType::SpiderLeg(3),
        ),
        mc_cuboid(
            [-4.0, 15.0, 0.0],
            off_l,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, -p8, -p4 * 0.74],
            false,
            leg,
            PartType::SpiderLeg(4),
        ),
        mc_cuboid(
            [4.0, 15.0, 0.0],
            off_r,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, p8, p4 * 0.74],
            false,
            leg,
            PartType::SpiderLeg(5),
        ),
        mc_cuboid(
            [-4.0, 15.0, -1.0],
            off_l,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, -p4, -p4],
            false,
            leg,
            PartType::SpiderLeg(6),
        ),
        mc_cuboid(
            [4.0, 15.0, -1.0],
            off_r,
            [16.0, 2.0, 2.0],
            oy,
            [0.0, p4, p4],
            false,
            leg,
            PartType::SpiderLeg(7),
        ),
    ]
}

// =========================================================================
// Enderman — 64×32 texture, extends ModelBiped with yOffset=-14
// head 8×8×8 @ (0,0) rot(0,-14,0), body 8×12×4 @ (32,16) rot(0,-14,0)
// arms 2×30×2 @ (56,0) rot(∓3,-12,0), legs 2×30×2 @ (56,0) rot(∓2,-2,0)

pub fn enderman_model() -> Vec<ModelCuboid> {
    // Vanilla ModelEnderman = ModelBiped(0, -14, 64, 32). Constructor rotation
    // points (the per-frame setRotationAngles shifts are ignored). Limbs 2×30×2.
    // Legs rp.y=-2, height 30 → feet at y=28, so origin_y=28.
    let head = uv64x32(0, 0, 8, 8, 8);
    let hat = uv64x32(0, 16, 8, 8, 8); // bipedHeadwear, inflate -0.5 → 7×7×7
    let body = uv64x32(32, 16, 8, 12, 4);
    let limb = uv64x32(56, 0, 2, 30, 2);
    let oy = 28.0;
    let zero = [0.0; 3];

    vec![
        mc_part(
            [0.0, -14.0, 0.0],
            [-4.0, -8.0, -4.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, -14.0, 0.0],
            [-3.5, -7.5, -3.5],
            [7.0, 7.0, 7.0],
            oy,
            hat,
            PartType::Head,
        ),
        mc_part(
            [0.0, -14.0, 0.0],
            [-4.0, 0.0, -2.0],
            [8.0, 12.0, 4.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-3.0, -12.0, 0.0],
            [-1.0, -2.0, -1.0],
            [2.0, 30.0, 2.0],
            oy,
            zero,
            false,
            limb,
            PartType::RightArm,
        ),
        mc_cuboid(
            [5.0, -12.0, 0.0],
            [-1.0, -2.0, -1.0],
            [2.0, 30.0, 2.0],
            oy,
            zero,
            true,
            limb,
            PartType::LeftArm,
        ),
        mc_cuboid(
            [-2.0, -2.0, 0.0],
            [-1.0, 0.0, -1.0],
            [2.0, 30.0, 2.0],
            oy,
            zero,
            false,
            limb,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [2.0, -2.0, 0.0],
            [-1.0, 0.0, -1.0],
            [2.0, 30.0, 2.0],
            oy,
            zero,
            true,
            limb,
            PartType::LeftLeg,
        ),
    ]
}

// =========================================================================
// Slime — 64×32 texture
// Outer shell: 8×8×8 @ (0,0)
// Inner body: 6×6×6 @ (0,16), eyes 2×2×2 @ (32,0), mouth 1×1×1 @ (32,8)

pub fn slime_model() -> Vec<ModelCuboid> {
    let outer = uv64x32(0, 0, 8, 8, 8);
    let inner = uv64x32(0, 16, 6, 6, 6);
    let eye = uv64x32(32, 0, 2, 2, 2);
    let mouth = uv64x32(32, 8, 1, 1, 1);

    let _oy = 16.0;
    vec![
        cuboid([0.0, 0.5, 0.0], [1.0, 1.0, 1.0], outer, PartType::Body),
        cuboid([0.0, 0.5, 0.0], [0.75, 0.75, 0.75], inner, PartType::Body),
        cuboid(
            [-0.25, 0.625, -0.375],
            [0.125, 0.125, 0.125],
            eye,
            PartType::Head,
        ),
        cuboid(
            [0.25, 0.625, -0.375],
            [0.125, 0.125, 0.125],
            eye,
            PartType::Head,
        ),
        cuboid(
            [0.0, 0.8125, -0.375],
            [0.0625, 0.0625, 0.0625],
            mouth,
            PartType::Head,
        ),
    ]
}

// =========================================================================
// Ghast — 64×32 texture
// body 16×16×16 @ (0,0) rot(0,8,0)
// 9 tentacles 2×(8-14)×2 @ (0,0) arranged in grid

pub fn ghast_model() -> Vec<ModelCuboid> {
    let body = uv64x32(0, 0, 16, 16, 16);
    let tent = uv64x32(0, 0, 2, 10, 2); // average tentacle height

    let oy = 16.0;
    let mut parts = vec![cuboid(
        [0.0, 0.5, 0.0],
        [1.0, 1.0, 1.0],
        body,
        PartType::Body,
    )];
    // 9 tentacles in 3×3 grid at radius ~5 from center
    let offsets: [(f32, f32); 9] = [
        (-5.0, -5.0),
        (0.0, -5.0),
        (5.0, -5.0),
        (-5.0, 0.0),
        (0.0, 0.0),
        (5.0, 0.0),
        (-5.0, 5.0),
        (0.0, 5.0),
        (5.0, 5.0),
    ];
    for &(x, z) in &offsets {
        parts.push(cuboid(
            mc_pivot(x, 15.0, z, oy),
            [0.125, 0.625, 0.125],
            tent,
            PartType::Tentacle(parts.len() as u8),
        ));
    }
    parts
}

// =========================================================================
// Blaze — 64×32 texture
// head 8×8×8 @ (0,0), 12 sticks 2×8×2 @ (0,16) in 3 rings of 4

pub fn blaze_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 8, 8, 8);
    let rod = uv64x32(0, 16, 2, 8, 2);

    let _oy = 16.0;
    let mut parts = vec![cuboid(
        [0.0, 0.0, 0.0],
        [0.5, 0.5, 0.5],
        head,
        PartType::Head,
    )];
    // 12 rods in 3 rings of 4 — BlazeRod(idx) drives the per-ring spin anim.
    let radii = [0.5625, 0.4375, 0.3125]; // 9/16, 7/16, 5/16 blocks
    let y_offsets = [-0.125, 0.125, 0.6875]; // -2, +2, +11 pixels → relative to centre
    let mut rod_idx = 0u8;
    for (ring, &radius) in radii.iter().enumerate() {
        for i in 0..4 {
            let angle = (i as f32) * std::f32::consts::PI / 2.0
                + (ring as f32) * std::f32::consts::FRAC_PI_4;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            parts.push(cuboid(
                [x, y_offsets[ring], z],
                [0.125, 0.5, 0.125],
                rod,
                PartType::BlazeRod(rod_idx),
            ));
            rod_idx += 1;
        }
    }
    parts
}

// =========================================================================
// Silverfish — 64×32 texture
// 7 body segments with specific sizes, 3 wings
// Segment sizes (px): (3,2,2), (4,3,2), (6,4,3), (3,3,3), (2,2,3), (2,1,2), (1,1,2)

pub fn silverfish_model() -> Vec<ModelCuboid> {
    let seg = [
        (3u32, 2u32, 2u32, 0u32, 0u32), // seg 0
        (4, 3, 2, 0, 4),                // seg 1
        (6, 4, 3, 0, 9),                // seg 2
        (3, 3, 3, 0, 16),               // seg 3
        (2, 2, 3, 0, 22),               // seg 4
        (2, 1, 2, 11, 0),               // seg 5
        (1, 1, 2, 13, 4),               // seg 6
    ];

    let oy = 20.0;
    let mut parts = Vec::new();
    let mut z_pos: f32 = -3.5; // starting Z in pixels

    for (i, &(w, h, d, tx, ty)) in seg.iter().enumerate() {
        let uvs = uv64x32(tx, ty, w, h, d);
        parts.push(cuboid(
            mc_pivot(0.0, 24.0 - (h as f32) - (i as f32), z_pos, oy),
            [w as f32 / 16.0, h as f32 / 16.0, d as f32 / 16.0],
            uvs,
            PartType::Body,
        ));
        if i < seg.len() - 1 {
            let next_d = seg[i + 1].2 as f32;
            z_pos += (d as f32 + next_d) * 0.5;
        }
    }
    parts
}

// =========================================================================
// Endermite — 64×32 texture
// 4 body segments: (4,3,2), (6,4,5), (3,3,1), (1,2,1)

pub fn endermite_model() -> Vec<ModelCuboid> {
    let seg = [
        (4u32, 3u32, 2u32, 0u32, 0u32),
        (6, 4, 5, 0, 5),
        (3, 3, 1, 0, 14),
        (1, 2, 1, 0, 18),
    ];

    let oy = 20.0;
    let mut parts = Vec::new();
    let mut z_pos: f32 = -3.5;

    for (i, &(w, h, d, tx, ty)) in seg.iter().enumerate() {
        let uvs = uv64x32(tx, ty, w, h, d);
        parts.push(cuboid(
            mc_pivot(0.0, 24.0 - (h as f32) - (i as f32), z_pos, oy),
            [w as f32 / 16.0, h as f32 / 16.0, d as f32 / 16.0],
            uvs,
            PartType::Body,
        ));
        if i < seg.len() - 1 {
            let next_d = seg[i + 1].2 as f32;
            z_pos += (d as f32 + next_d) * 0.5;
        }
    }
    parts
}

// =========================================================================
// Guardian — 64×64 texture
// body: main 12×12×16 @ (0,0), side panels 2×12×12, top/bottom 12×2×12
// 12 spines 2×9×2, eye 2×2×1, tail 3 segments

pub fn guardian_model() -> Vec<ModelCuboid> {
    let body = uv64x64(0, 0, 12, 12, 16);
    let spine = uv64x64(0, 0, 2, 9, 2);
    let eye = uv64x64(8, 0, 2, 2, 1);
    let tail0 = uv64x64(40, 0, 4, 4, 8);
    let tail1 = uv64x64(0, 54, 3, 3, 7);

    let _oy = 20.0;
    vec![
        cuboid([0.0, 0.0, 0.0], [0.75, 0.75, 1.0], body, PartType::Body),
        cuboid(
            [0.0, 0.0, 5.0],
            [0.25, 0.25, 0.5],
            tail0,
            PartType::Tentacle(0),
        ),
        cuboid(
            [0.0, 0.0, 7.5],
            [0.1875, 0.1875, 0.4375],
            tail1,
            PartType::Tentacle(1),
        ),
        // Simplified: a few representative spines
        cuboid(
            [0.75, 0.0, 0.0],
            [0.125, 0.5625, 0.125],
            spine,
            PartType::Extra,
        ),
        cuboid(
            [-0.75, 0.0, 0.0],
            [0.125, 0.5625, 0.125],
            spine,
            PartType::Extra,
        ),
        cuboid(
            [0.0, 0.75, 0.0],
            [0.125, 0.5625, 0.125],
            spine,
            PartType::Extra,
        ),
        cuboid(
            [0.0, -0.75, 0.0],
            [0.125, 0.5625, 0.125],
            spine,
            PartType::Extra,
        ),
        cuboid(
            [0.0, 0.0, -1.0],
            [0.125, 0.125, 0.0625],
            eye,
            PartType::Head,
        ),
    ]
}

// =========================================================================
// Wither — 64×64 texture. Simplified vanilla silhouette: ribcage body +
// three heads (center 8³, two side 6³). No parent-child bones; each head is
// an independent part so the side heads can't track targets like vanilla, but
// the silhouette reads as a wither, not a ghast.
// Vanilla ModelWither: body rp(0,6,0) addBox(-10,-3,-3) 20×6×6 @ (12,16);
// heads rp(0,0,0)/(-7,2,0)/(7,2,0) @ (0,0)/(32,0) mirrored.

pub fn wither_model() -> Vec<ModelCuboid> {
    let b1 = uv64x64(0, 16, 20, 3, 3);
    let b2 = uv64x64(0, 22, 3, 10, 3);
    let b3 = uv64x64(24, 22, 3, 6, 3);
    let head = uv64x64(0, 0, 8, 8, 8);
    let side = uv64x64(32, 0, 6, 6, 6);
    let oy = 24.0;
    let zero = [0.0; 3];

    vec![
        mc_cuboid(
            [0.0, 0.0, 0.0],
            [-10.0, 3.9, -0.5],
            [20.0, 3.0, 3.0],
            oy,
            zero,
            false,
            b1,
            PartType::OscillatingBody,
        ),
        mc_cuboid(
            [0.0, 0.0, 0.0],
            [-1.5, 6.9, -0.5],
            [3.0, 10.0, 3.0],
            oy,
            zero,
            false,
            b2,
            PartType::OscillatingBody,
        ),
        mc_cuboid(
            [0.0, 0.0, 0.0],
            [-1.5, 9.8, -0.5],
            [3.0, 6.0, 3.0],
            oy,
            zero,
            false,
            b3,
            PartType::OscillatingBody,
        ),
        mc_cuboid(
            [0.0, 0.0, 0.0],
            [-4.0, -4.0, -4.0],
            [8.0, 8.0, 8.0],
            oy,
            zero,
            false,
            head,
            PartType::Head,
        ),
        mc_cuboid(
            [-8.0, 4.0, 0.0],
            [-3.0, -3.0, -3.0],
            [6.0, 6.0, 6.0],
            oy,
            zero,
            false,
            side,
            PartType::Head,
        ),
        mc_cuboid(
            [8.0, 4.0, 0.0],
            [-3.0, -3.0, -3.0],
            [6.0, 6.0, 6.0],
            oy,
            zero,
            true,
            side,
            PartType::Head,
        ),
    ]
}

// =========================================================================
// Ender Dragon — 256×256 texture. Simplified silhouette: body + head + jaw +
// two wings with tips + two tail segments. No parent-child bones or body
// curve; each part is independently positioned. UV regions are laid out to
// fit within 256×256 without overflow or overlap (vanilla's exact texture
// offsets are not used — this is a simplified model).

pub fn ender_dragon_model() -> Vec<ModelCuboid> {
    let head = uv256(0, 0, 24, 24, 24);
    let jaw = uv256(0, 48, 12, 8, 16);
    let body = uv256(0, 72, 24, 24, 48);
    let wing = uv256(96, 0, 40, 8, 40);
    let wing_tip = uv256(96, 148, 40, 4, 40);
    let tail0 = uv256(0, 192, 16, 16, 16);
    let tail1 = uv256(0, 224, 12, 12, 12);
    let oy = 24.0;
    let zero = [0.0; 3];

    vec![
        mc_cuboid(
            [0.0, 12.0, 0.0],
            [-12.0, -12.0, -24.0],
            [24.0, 24.0, 48.0],
            oy,
            zero,
            false,
            body,
            PartType::OscillatingBody,
        ),
        mc_cuboid(
            [0.0, 12.0, -36.0],
            [-12.0, -12.0, -12.0],
            [24.0, 24.0, 24.0],
            oy,
            zero,
            false,
            head,
            PartType::Head,
        ),
        mc_cuboid(
            [0.0, 18.0, -36.0],
            [-6.0, -4.0, -8.0],
            [12.0, 8.0, 16.0],
            oy,
            zero,
            false,
            jaw,
            PartType::Head,
        ),
        mc_cuboid(
            [-18.0, 12.0, 0.0],
            [-20.0, -4.0, -20.0],
            [40.0, 8.0, 40.0],
            oy,
            zero,
            false,
            wing,
            PartType::DragonRightWing,
        ),
        mc_cuboid(
            [18.0, 12.0, 0.0],
            [-20.0, -4.0, -20.0],
            [40.0, 8.0, 40.0],
            oy,
            zero,
            true,
            wing,
            PartType::DragonLeftWing,
        ),
        mc_cuboid(
            [-40.0, 12.0, 0.0],
            [-20.0, -2.0, -20.0],
            [40.0, 4.0, 40.0],
            oy,
            zero,
            false,
            wing_tip,
            PartType::DragonOuterRightWing,
        ),
        mc_cuboid(
            [40.0, 12.0, 0.0],
            [-20.0, -2.0, -20.0],
            [40.0, 4.0, 40.0],
            oy,
            zero,
            true,
            wing_tip,
            PartType::DragonOuterLeftWing,
        ),
        mc_cuboid(
            [0.0, 12.0, 24.0],
            [-8.0, -8.0, -8.0],
            [16.0, 16.0, 16.0],
            oy,
            zero,
            false,
            tail0,
            PartType::Tentacle(0),
        ),
        mc_cuboid(
            [0.0, 12.0, 40.0],
            [-6.0, -6.0, -6.0],
            [12.0, 12.0, 12.0],
            oy,
            zero,
            false,
            tail1,
            PartType::Tentacle(1),
        ),
    ]
}

// =========================================================================
// Witch — 64×128 texture (extended for hat)
// Uses ModelWitch which extends ModelBiped with hat and nose overlay
// head 8×8×8 @ (0,0), hat brim 30×1×22 @ (0,64+0) with hat top
// nose 1×2×4 @ (24,0) child of head

pub fn witch_model() -> Vec<ModelCuboid> {
    let head = uv64x128(0, 0, 8, 10, 8);
    let nose = uv64x128(24, 0, 2, 4, 2);
    let body = uv64x128(16, 20, 8, 12, 6);
    let robe = uv64x128(0, 38, 8, 18, 6);
    let arm_side = uv64x128(44, 22, 4, 8, 4);
    let sleeve = uv64x128(40, 38, 8, 4, 4);
    let leg = uv64x128(0, 22, 4, 12, 4);

    let hat0 = uv64x128(0, 64, 10, 2, 10);
    let hat1 = uv64x128(0, 76, 7, 4, 7);
    let hat2 = uv64x128(0, 87, 4, 4, 4);
    let hat3 = uv64x128(0, 95, 1, 2, 1);

    let oy = 24.0;
    let arm_pose = [-0.75, 0.0, 0.0];
    let zero = [0.0; 3];

    vec![
        // Head & Nose
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, -10.0, -4.0],
            [8.0, 10.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-1.0, -3.0, -6.0],
            [2.0, 4.0, 2.0],
            oy,
            nose,
            PartType::Head,
        ),
        // Hat
        mc_cuboid(
            [-5.0, -10.0, -5.0],
            [0.0, 0.0, 0.0],
            [10.0, 2.0, 10.0],
            oy,
            [-0.05, 0.0, 0.02],
            false,
            hat0,
            PartType::Head,
        ),
        mc_cuboid(
            [-3.25, -14.0, -3.0],
            [0.0, 0.0, 0.0],
            [7.0, 4.0, 7.0],
            oy,
            [-0.1, 0.0, 0.04],
            false,
            hat1,
            PartType::Head,
        ),
        mc_cuboid(
            [-1.5, -18.0, -1.0],
            [0.0, 0.0, 0.0],
            [4.0, 4.0, 4.0],
            oy,
            [-0.2, 0.0, 0.08],
            false,
            hat2,
            PartType::Head,
        ),
        mc_cuboid(
            [0.25, -20.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 1.0],
            oy,
            [-0.4, 0.0, 0.16],
            false,
            hat3,
            PartType::Head,
        ),
        // Body & Robe
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 0.0, -3.0],
            [8.0, 12.0, 6.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 0.0, -3.0],
            [8.0, 18.0, 6.0],
            oy,
            robe,
            PartType::Body,
        ), // Robe
        // Arms (Villager style)
        mc_cuboid(
            [0.0, 3.0, -1.0],
            [-8.0, -2.0, -2.0],
            [4.0, 8.0, 4.0],
            oy,
            arm_pose,
            false,
            arm_side,
            PartType::Extra,
        ),
        mc_cuboid(
            [0.0, 3.0, -1.0],
            [4.0, -2.0, -2.0],
            [4.0, 8.0, 4.0],
            oy,
            arm_pose,
            false,
            arm_side,
            PartType::Extra,
        ),
        mc_cuboid(
            [0.0, 3.0, -1.0],
            [-4.0, 2.0, -2.0],
            [8.0, 4.0, 4.0],
            oy,
            arm_pose,
            false,
            sleeve,
            PartType::Extra,
        ),
        // Legs
        mc_cuboid(
            [-2.0, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [2.0, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            true,
            leg,
            PartType::LeftLeg,
        ),
    ]
}
