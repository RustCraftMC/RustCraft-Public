use super::helpers::*;
use crate::render::entity::mesh::{ModelCuboid, PartType};

// =========================================================================
// Pig — 64×32 texture. ModelPig = ModelQuadruped(legHeight=6) + snout child.
// head rp(0,12,-6) @ (0,0) 8×8×8, snout (child) @ (16,16) addBox(-2,0,-9) 4×3×1.
// body rp(0,11,2) @ (28,8) addBox(-5,-10,-7) 10×16×8, rotateAngleX=PI/2.
// legs rp(±3,18,±7/±5) @ (0,16) 4×6×4. Feet at y=24 → origin_y=24.

pub fn pig_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 8, 8, 8);
    let snout = uv64x32(16, 16, 4, 3, 1);
    let body = uv64x32(28, 8, 10, 16, 8);
    let leg = uv64x32(0, 16, 4, 6, 4);
    let oy = 24.0;
    let body_rot = [std::f32::consts::FRAC_PI_2, 0.0, 0.0];
    let zero = [0.0; 3];

    vec![
        mc_part(
            [0.0, 12.0, -6.0],
            [-4.0, -4.0, -8.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        // snout is a child of head; share head's rotation point.
        mc_part(
            [0.0, 12.0, -6.0],
            [-2.0, 0.0, -9.0],
            [4.0, 3.0, 1.0],
            oy,
            snout,
            PartType::Head,
        ),
        mc_cuboid(
            [0.0, 11.0, 2.0],
            [-5.0, -10.0, -7.0],
            [10.0, 16.0, 8.0],
            oy,
            body_rot,
            false,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-3.0, 18.0, 7.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [3.0, 18.0, 7.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [-3.0, 18.0, -5.0],
            [-2.0, 0.0, -2.0],
            [4.0, 6.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [3.0, 18.0, -5.0],
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
// Cow — 64×32 texture. ModelQuadruped(legHeight=12) with head/body overrides.
// head rp(0,4,-8) @ (0,0) 8×8×6, horns (children) @ (22,0) 1×3×1.
// body rp(0,5,2) @ (18,4) 12×18×10, rotateAngleX=PI/2; udder (child) @ (52,0).
// legs rp adjusted (±4,12,±7/∓6). Feet at y=24 → origin_y=24.

pub fn cow_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 8, 8, 6);
    let body = uv64x32(18, 4, 12, 18, 10);
    let leg = uv64x32(0, 16, 4, 12, 4);
    let horn = uv64x32(22, 0, 1, 3, 1);
    let udder = uv64x32(52, 0, 4, 6, 1);
    let oy = 24.0;
    let body_rot = [std::f32::consts::FRAC_PI_2, 0.0, 0.0];
    let zero = [0.0; 3];

    vec![
        mc_part(
            [0.0, 4.0, -8.0],
            [-4.0, -4.0, -6.0],
            [8.0, 8.0, 6.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 4.0, -8.0],
            [-5.0, -5.0, -4.0],
            [1.0, 3.0, 1.0],
            oy,
            horn,
            PartType::Head,
        ),
        mc_part(
            [0.0, 4.0, -8.0],
            [4.0, -5.0, -4.0],
            [1.0, 3.0, 1.0],
            oy,
            horn,
            PartType::Head,
        ),
        mc_cuboid(
            [0.0, 5.0, 2.0],
            [-6.0, -10.0, -7.0],
            [12.0, 18.0, 10.0],
            oy,
            body_rot,
            false,
            body,
            PartType::Body,
        ),
        // udder is a child of body; share body's rotation point + PI/2 pose.
        mc_cuboid(
            [0.0, 5.0, 2.0],
            [-2.0, 2.0, -8.0],
            [4.0, 6.0, 1.0],
            oy,
            body_rot,
            false,
            udder,
            PartType::Body,
        ),
        mc_cuboid(
            [-4.0, 12.0, 7.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [4.0, 12.0, 7.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [-4.0, 12.0, -6.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [4.0, 12.0, -6.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
    ]
}

// =========================================================================
// Sheep — 64×32 texture. Two layers: ModelSheep1 (fleece, inflated) tinted,
// and ModelSheep2 (bare skin). ModelQuadruped(legHeight=12): bare legs 12 tall,
// fleece legs 6 tall (wool-covered upper). Feet at y=24 → origin_y=24.
// Fleece body/head rotated PI/2 like the bare body.

pub fn sheep_model() -> Vec<ModelCuboid> {
    let fleece_head = uv64x32(0, 0, 6, 6, 6); // inflated 0.6 → 7.2
    let fleece_body = uv64x32(28, 8, 8, 16, 6); // inflated 1.75 → 11.5×19.5×9.5
    let fleece_leg = uv64x32(0, 16, 4, 6, 4); // inflated 0.5 → 5×7×5
    let bare_head = uv64x32(0, 0, 6, 6, 8);
    let bare_body = uv64x32(28, 8, 8, 16, 6);
    let bare_leg = uv64x32(0, 16, 4, 12, 4);
    let oy = 24.0;
    let body_rot = [std::f32::consts::FRAC_PI_2, 0.0, 0.0];
    let zero = [0.0; 3];
    let wool: [f32; 4] = [0.95, 0.95, 0.95, 1.0];

    vec![
        // Fleece layer (ModelSheep1) — head rp(0,6,-8), body rp(0,5,2), legs rp(±3,12,±7/±5).
        tint(
            mc_cuboid(
                [0.0, 6.0, -8.0],
                [-3.6, -4.6, -4.6],
                [7.2, 7.2, 7.2],
                oy,
                zero,
                false,
                fleece_head,
                PartType::Head,
            ),
            wool,
        ),
        tint(
            mc_cuboid(
                [0.0, 5.0, 2.0],
                [-5.75, -11.75, -8.75],
                [11.5, 19.5, 9.5],
                oy,
                body_rot,
                false,
                fleece_body,
                PartType::Body,
            ),
            wool,
        ),
        tint(
            mc_cuboid(
                [-3.0, 12.0, 7.0],
                [-2.5, -0.5, -2.5],
                [5.0, 7.0, 5.0],
                oy,
                zero,
                false,
                fleece_leg,
                PartType::RightLeg,
            ),
            wool,
        ),
        tint(
            mc_cuboid(
                [3.0, 12.0, 7.0],
                [-2.5, -0.5, -2.5],
                [5.0, 7.0, 5.0],
                oy,
                zero,
                false,
                fleece_leg,
                PartType::LeftLeg,
            ),
            wool,
        ),
        tint(
            mc_cuboid(
                [-3.0, 12.0, -5.0],
                [-2.5, -0.5, -2.5],
                [5.0, 7.0, 5.0],
                oy,
                zero,
                false,
                fleece_leg,
                PartType::LeftLeg,
            ),
            wool,
        ),
        tint(
            mc_cuboid(
                [3.0, 12.0, -5.0],
                [-2.5, -0.5, -2.5],
                [5.0, 7.0, 5.0],
                oy,
                zero,
                false,
                fleece_leg,
                PartType::RightLeg,
            ),
            wool,
        ),
        // Bare layer (ModelSheep2) — head rp(0,6,-8) 6×6×8, body rp(0,5,2) 8×16×6, legs 12 tall.
        mc_part(
            [0.0, 6.0, -8.0],
            [-3.0, -4.0, -6.0],
            [6.0, 6.0, 8.0],
            oy,
            bare_head,
            PartType::Head,
        ),
        mc_cuboid(
            [0.0, 5.0, 2.0],
            [-4.0, -10.0, -7.0],
            [8.0, 16.0, 6.0],
            oy,
            body_rot,
            false,
            bare_body,
            PartType::Body,
        ),
        mc_cuboid(
            [-3.0, 12.0, 7.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            bare_leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [3.0, 12.0, 7.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            bare_leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [-3.0, 12.0, -5.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            bare_leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [3.0, 12.0, -5.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            zero,
            false,
            bare_leg,
            PartType::RightLeg,
        ),
    ]
}

// =========================================================================
// Wolf — 64×32 texture
// head 6×6×4 @ (0,0) rot(-1,13.5,-7), snout 3×3×4 @ (0,10) child, ears 2×2×1 @ (16,14) child
// body 6×9×6 @ (18,14) rot(0,14,2), mane 8×6×7 @ (21,0) rot(-1,14,2)
// legs 2×8×2 @ (0,18) rot(±2.5,16,7)/(±2.5,16,-4)
// tail 2×8×2 @ (9,18) rot(-1,12,8)

pub fn wolf_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 6, 6, 4);
    let snout = uv64x32(0, 10, 3, 3, 4);
    let ear = uv64x32(16, 14, 2, 2, 1);
    let body = uv64x32(18, 14, 6, 9, 6);
    let mane = uv64x32(21, 0, 8, 6, 7);
    let leg = uv64x32(0, 18, 2, 8, 2);
    let tail = uv64x32(9, 18, 2, 8, 2);

    let oy = 18.0;
    vec![
        cuboid(
            mc_pivot(-1.0, -0.5, -15.0, oy),
            [0.375, 0.375, 0.25],
            head,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(-1.0, 0.0, -20.0, oy),
            [0.1875, 0.1875, 0.25],
            snout,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(-3.0, -5.0, -15.0, oy),
            [0.125, 0.125, 0.0625],
            ear,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(1.0, -5.0, -15.0, oy),
            [0.125, 0.125, 0.0625],
            ear,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(0.0, 0.0, 4.0, oy),
            [0.375, 0.5625, 0.375],
            body,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(-1.0, 0.0, 4.0, oy),
            [0.5, 0.375, 0.4375],
            mane,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(-2.5, 2.0, 7.0, oy),
            [0.125, 0.5, 0.125],
            leg,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(0.5, 2.0, 7.0, oy),
            [0.125, 0.5, 0.125],
            leg,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-2.5, 2.0, -4.0, oy),
            [0.125, 0.5, 0.125],
            leg,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(0.5, 2.0, -4.0, oy),
            [0.125, 0.5, 0.125],
            leg,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-1.0, -2.0, 8.0, oy),
            [0.125, 0.5, 0.125],
            tail,
            PartType::Tail,
        ),
    ]
}

// =========================================================================
// Ocelot — 64×32 texture
// head 5×4×5 @ (0,0) rot(0,15,-9), nose 3×2×2 @ (0,24), ears 1×1×2
// body 4×16×6 @ (20,0) rot(0,12,-10)
// tail 1×8×1 @ (0,15) rot(0,15,8), tail2 1×8×1 @ (4,15) rot(0,20,14)
// front legs 2×10×2 @ (40,0) rot(±1.2,13.8,-5)
// back legs 2×6×2 @ (8,13) rot(±1.1,18,5)

pub fn ocelot_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 5, 4, 5);
    let nose = uv64x32(0, 24, 3, 2, 2);
    let ear1 = uv64x32(0, 10, 1, 1, 2);
    let ear2 = uv64x32(6, 10, 1, 1, 2);
    let body = uv64x32(20, 0, 4, 16, 6);
    let tail = uv64x32(0, 15, 1, 8, 1);
    let tail2 = uv64x32(4, 15, 1, 8, 1);
    let fleg = uv64x32(40, 0, 2, 10, 2);
    let bleg = uv64x32(8, 13, 2, 6, 2);

    let oy = 18.0;
    vec![
        cuboid(
            mc_pivot(0.0, -1.0, -17.0, oy),
            [0.3125, 0.25, 0.3125],
            head,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(0.0, 1.0, -20.0, oy),
            [0.1875, 0.125, 0.125],
            nose,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(-2.0, -3.0, -14.0, oy),
            [0.0625, 0.0625, 0.125],
            ear1,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(2.0, -3.0, -14.0, oy),
            [0.0625, 0.0625, 0.125],
            ear2,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(0.0, 0.0, -6.0, oy),
            [0.25, 1.0, 0.375],
            body,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(0.0, -1.0, 8.0, oy),
            [0.0625, 0.5, 0.0625],
            tail,
            PartType::Tail,
        ),
        cuboid(
            mc_pivot(0.0, 2.0, 14.0, oy),
            [0.0625, 0.5, 0.0625],
            tail2,
            PartType::Tail,
        ),
        cuboid(
            mc_pivot(-1.2, 4.2, -13.0, oy),
            [0.125, 0.625, 0.125],
            fleg,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(1.2, 4.2, -13.0, oy),
            [0.125, 0.625, 0.125],
            fleg,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-1.1, 6.0, 5.0, oy),
            [0.125, 0.375, 0.125],
            bleg,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(1.1, 6.0, 5.0, oy),
            [0.125, 0.375, 0.125],
            bleg,
            PartType::RightLeg,
        ),
    ]
}

// =========================================================================
// Horse — 128×128 texture
// Simplified: body, neck, head, 4 legs (each with shin+hoof), tail, ears, mane
// body 10×10×24 @ (0,34) rot(0,11,9)
// head 5×5×7 @ (0,0) rot(0,4,-10)
// neck 4×14×8 @ (0,12) rot(0,4,-10)
// legs: back 4×9×5 @ (78,29), front 3×8×4 @ (44,29)

pub fn horse_model() -> Vec<ModelCuboid> {
    let body = uv128(0, 34, 10, 10, 24);
    let head = uv128(0, 0, 5, 5, 7);
    let neck = uv128(0, 12, 4, 14, 8);
    let bleg_u = uv128(78, 29, 4, 9, 5);
    let bleg_l = uv128(78, 43, 3, 5, 3);
    let bhoof = uv128(78, 51, 4, 3, 4);
    let fleg_u = uv128(44, 29, 3, 8, 4);
    let fleg_l = uv128(44, 41, 3, 5, 3);
    let fhoof = uv128(44, 51, 4, 3, 4);
    let tail = uv128(44, 0, 2, 2, 3);
    let ear = uv128(0, 0, 2, 3, 1);
    let mane = uv128(58, 0, 2, 16, 4);

    let oy = 20.0;
    vec![
        // Body: centre at (0, 11-8, 9) in MC px → (0, 3, 9) relative to origin
        cuboid(
            mc_pivot(0.0, 3.0, 9.0, oy),
            [0.625, 0.625, 1.5],
            body,
            PartType::Body,
        ),
        // Head
        cuboid(
            mc_pivot(0.0, -14.0, -6.0, oy),
            [0.3125, 0.3125, 0.4375],
            head,
            PartType::Head,
        ),
        // Neck
        cuboid(
            mc_pivot(0.0, -6.0, -6.0, oy),
            [0.25, 0.875, 0.5],
            neck,
            PartType::Body,
        ),
        // Ears
        cuboid(
            mc_pivot(0.45, -16.0, -6.0, oy),
            [0.125, 0.1875, 0.0625],
            ear,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(-2.45, -16.0, -6.0, oy),
            [0.125, 0.1875, 0.0625],
            ear,
            PartType::Head,
        ),
        // Mane
        cuboid(
            mc_pivot(-1.0, -7.5, -1.0, oy),
            [0.125, 1.0, 0.25],
            mane,
            PartType::Body,
        ),
        // Back left leg upper
        cuboid(
            mc_pivot(4.0, -2.0, 11.0, oy),
            [0.25, 0.5625, 0.3125],
            bleg_u,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(4.0, 4.0, 11.0, oy),
            [0.1875, 0.3125, 0.1875],
            bleg_l,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(4.0, 9.0, 11.0, oy),
            [0.25, 0.1875, 0.25],
            bhoof,
            PartType::LeftLeg,
        ),
        // Back right leg
        cuboid(
            mc_pivot(-4.0, -2.0, 11.0, oy),
            [0.25, 0.5625, 0.3125],
            bleg_u,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-4.0, 4.0, 11.0, oy),
            [0.1875, 0.3125, 0.1875],
            bleg_l,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-4.0, 9.0, 11.0, oy),
            [0.25, 0.1875, 0.25],
            bhoof,
            PartType::RightLeg,
        ),
        // Front left leg
        cuboid(
            mc_pivot(4.0, -1.0, -8.0, oy),
            [0.1875, 0.5, 0.25],
            fleg_u,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(4.0, 5.0, -8.0, oy),
            [0.1875, 0.3125, 0.1875],
            fleg_l,
            PartType::LeftLeg,
        ),
        cuboid(
            mc_pivot(4.0, 10.0, -8.0, oy),
            [0.25, 0.1875, 0.25],
            fhoof,
            PartType::LeftLeg,
        ),
        // Front right leg
        cuboid(
            mc_pivot(-4.0, -1.0, -8.0, oy),
            [0.1875, 0.5, 0.25],
            fleg_u,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-4.0, 5.0, -8.0, oy),
            [0.1875, 0.3125, 0.1875],
            fleg_l,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-4.0, 10.0, -8.0, oy),
            [0.25, 0.1875, 0.25],
            fhoof,
            PartType::RightLeg,
        ),
        // Tail
        cuboid(
            mc_pivot(0.0, -1.0, 14.0, oy),
            [0.125, 0.125, 0.1875],
            tail,
            PartType::Tail,
        ),
    ]
}

// =========================================================================
// Rabbit — 64×32 texture
// head 5×4×5 @ (32,0) rot(0,16,-1), nose 1×1×1 @ (32,9) child
// ears 2×5×1 @ (52,0)/(58,0) children of head
// body 6×5×10 @ (0,0) rot(0,19,8), tail 3×3×2 @ (52,6) rot(0,20,7)
// arms 2×7×2 @ (8,15)/(0,15), thighs 2×4×5, feet 2×1×7

pub fn rabbit_model() -> Vec<ModelCuboid> {
    let oy = 20.0;
    let mirror = true;
    vec![
        // rabbitLeftFoot: rp=(3, 17.5, 3.7), off=(-1, 5.5, -3.7), size=(2, 1, 7)
        mc_cuboid(
            [3.0, 17.5, 3.7],
            [-1.0, 5.5, -3.7],
            [2.0, 1.0, 7.0],
            oy,
            [0.0; 3],
            mirror,
            uv64x32(26, 24, 2, 1, 7),
            PartType::LeftLeg,
        ),
        // rabbitRightFoot: rp=(-3, 17.5, 3.7), off=(-1, 5.5, -3.7), size=(2, 1, 7)
        mc_cuboid(
            [-3.0, 17.5, 3.7],
            [-1.0, 5.5, -3.7],
            [2.0, 1.0, 7.0],
            oy,
            [0.0; 3],
            mirror,
            uv64x32(8, 24, 2, 1, 7),
            PartType::RightLeg,
        ),
        // rabbitLeftThigh: rp=(3, 17.5, 3.7), off=(-1, 0, 0), size=(2, 4, 5), rot=(-0.34906584, 0, 0)
        mc_cuboid(
            [3.0, 17.5, 3.7],
            [-1.0, 0.0, 0.0],
            [2.0, 4.0, 5.0],
            oy,
            [-0.34906584, 0.0, 0.0],
            mirror,
            uv64x32(30, 15, 2, 4, 5),
            PartType::LeftLeg,
        ),
        // rabbitRightThigh: rp=(-3, 17.5, 3.7), off=(-1, 0, 0), size=(2, 4, 5), rot=(-0.34906584, 0, 0)
        mc_cuboid(
            [-3.0, 17.5, 3.7],
            [-1.0, 0.0, 0.0],
            [2.0, 4.0, 5.0],
            oy,
            [-0.34906584, 0.0, 0.0],
            mirror,
            uv64x32(16, 15, 2, 4, 5),
            PartType::RightLeg,
        ),
        // rabbitBody: rp=(0, 19, 8), off=(-3, -2, -10), size=(6, 5, 10), rot=(-0.34906584, 0, 0)
        mc_cuboid(
            [0.0, 19.0, 8.0],
            [-3.0, -2.0, -10.0],
            [6.0, 5.0, 10.0],
            oy,
            [-0.34906584, 0.0, 0.0],
            mirror,
            uv64x32(0, 0, 6, 5, 10),
            PartType::Body,
        ),
        // rabbitLeftArm: rp=(3, 17, -1), off=(-1, 0, -1), size=(2, 7, 2), rot=(-0.17453292, 0, 0)
        mc_cuboid(
            [3.0, 17.0, -1.0],
            [-1.0, 0.0, -1.0],
            [2.0, 7.0, 2.0],
            oy,
            [-0.17453292, 0.0, 0.0],
            mirror,
            uv64x32(8, 15, 2, 7, 2),
            PartType::LeftArm,
        ),
        // rabbitRightArm: rp=(-3, 17, -1), off=(-1, 0, -1), size=(2, 7, 2), rot=(-0.17453292, 0, 0)
        mc_cuboid(
            [-3.0, 17.0, -1.0],
            [-1.0, 0.0, -1.0],
            [2.0, 7.0, 2.0],
            oy,
            [-0.17453292, 0.0, 0.0],
            mirror,
            uv64x32(0, 15, 2, 7, 2),
            PartType::RightArm,
        ),
        // rabbitHead: rp=(0, 16, -1), off=(-2.5, -4, -5), size=(5, 4, 5)
        mc_cuboid(
            [0.0, 16.0, -1.0],
            [-2.5, -4.0, -5.0],
            [5.0, 4.0, 5.0],
            oy,
            [0.0; 3],
            mirror,
            uv64x32(32, 0, 5, 4, 5),
            PartType::Head,
        ),
        // rabbitRightEar: rp=(0, 16, -1), off=(-2.5, -9, -1), size=(2, 5, 1), rot=(0, -0.2617994, 0)
        mc_cuboid(
            [0.0, 16.0, -1.0],
            [-2.5, -9.0, -1.0],
            [2.0, 5.0, 1.0],
            oy,
            [0.0, -0.2617994, 0.0],
            mirror,
            uv64x32(52, 0, 2, 5, 1),
            PartType::Head,
        ),
        // rabbitLeftEar: rp=(0, 16, -1), off=(0.5, -9, -1), size=(2, 5, 1), rot=(0, 0.2617994, 0)
        mc_cuboid(
            [0.0, 16.0, -1.0],
            [0.5, -9.0, -1.0],
            [2.0, 5.0, 1.0],
            oy,
            [0.0, 0.2617994, 0.0],
            mirror,
            uv64x32(58, 0, 2, 5, 1),
            PartType::Head,
        ),
        // rabbitTail: rp=(0, 20, 7), off=(-1.5, -1.5, 0), size=(3, 3, 2), rot=(-0.3490659, 0, 0)
        mc_cuboid(
            [0.0, 20.0, 7.0],
            [-1.5, -1.5, 0.0],
            [3.0, 3.0, 2.0],
            oy,
            [-0.3490659, 0.0, 0.0],
            mirror,
            uv64x32(52, 6, 3, 3, 2),
            PartType::Tail,
        ),
        // rabbitNose: rp=(0, 16, -1), off=(-0.5, -2.5, -5.5), size=(1, 1, 1)
        mc_cuboid(
            [0.0, 16.0, -1.0],
            [-0.5, -2.5, -5.5],
            [1.0, 1.0, 1.0],
            oy,
            [0.0; 3],
            mirror,
            uv64x32(32, 9, 1, 1, 1),
            PartType::Head,
        ),
    ]
}
