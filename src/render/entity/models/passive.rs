use super::helpers::*;
use crate::entity::EntityVisualState;
use crate::render::entity::mesh::{ModelCuboid, PartType};

// =========================================================================
// Chicken — 64×32 texture. Vanilla ModelChicken: i=16.
// head rp(0,15,-4) @ (0,0) 4×6×3, bill/chin children of head.
// body rp(0,16,0) @ (0,9) 6×8×6, rotateAngleX=PI/2.
// legs rp(∓2/1,19,1) @ (26,0) 3×5×3. wings rp(±4,13,0) @ (24,13) 1×4×6.
// Feet at y=24 (leg rp.y 19 + height 5) → origin_y=24.

pub fn chicken_model() -> Vec<ModelCuboid> {
    let head = uv64x32(0, 0, 4, 6, 3);
    let bill = uv64x32(14, 0, 4, 2, 2);
    let chin = uv64x32(14, 4, 2, 2, 2);
    let body = uv64x32(0, 9, 6, 8, 6);
    let leg = uv64x32(26, 0, 3, 5, 3);
    let wing = uv64x32(24, 13, 1, 4, 6);
    let oy = 24.0;
    let body_rot = [std::f32::consts::FRAC_PI_2, 0.0, 0.0];
    let zero = [0.0; 3];

    vec![
        mc_part(
            [0.0, 15.0, -4.0],
            [-2.0, -6.0, -2.0],
            [4.0, 6.0, 3.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 15.0, -4.0],
            [-2.0, -4.0, -4.0],
            [4.0, 2.0, 2.0],
            oy,
            bill,
            PartType::Head,
        ),
        mc_part(
            [0.0, 15.0, -4.0],
            [-1.0, -2.0, -3.0],
            [2.0, 2.0, 2.0],
            oy,
            chin,
            PartType::Head,
        ),
        mc_cuboid(
            [0.0, 16.0, 0.0],
            [-3.0, -4.0, -3.0],
            [6.0, 8.0, 6.0],
            oy,
            body_rot,
            false,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-2.0, 19.0, 1.0],
            [-1.0, 0.0, -3.0],
            [3.0, 5.0, 3.0],
            oy,
            zero,
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [1.0, 19.0, 1.0],
            [-1.0, 0.0, -3.0],
            [3.0, 5.0, 3.0],
            oy,
            zero,
            false,
            leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [-4.0, 13.0, 0.0],
            [0.0, 0.0, -3.0],
            [1.0, 4.0, 6.0],
            oy,
            zero,
            false,
            wing,
            PartType::RightWing,
        ),
        mc_cuboid(
            [4.0, 13.0, 0.0],
            [-1.0, 0.0, -3.0],
            [1.0, 4.0, 6.0],
            oy,
            zero,
            false,
            wing,
            PartType::LeftWing,
        ),
    ]
}

// =========================================================================
// Squid — 64×32 texture
// body 12×16×12 @ (0,0) rot(0,8,0)
// 8 tentacles 2×18×2 @ (48,0) arranged in circle at radius 5

pub fn squid_model() -> Vec<ModelCuboid> {
    let body = uv64x32(0, 0, 12, 16, 12);
    let tent = uv64x32(48, 0, 2, 18, 2);

    let oy = 16.0;
    let mut parts = vec![cuboid(
        mc_pivot(0.0, 8.0, 0.0, oy),
        [0.75, 1.0, 0.75],
        body,
        PartType::Body,
    )];
    // 8 tentacles in a circle at radius 5 pixels from center
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI * 2.0 / 8.0;
        let x = angle.cos() * 5.0;
        let z = angle.sin() * 5.0;
        parts.push(cuboid(
            mc_pivot(x, 17.0, z, oy),
            [0.125, 1.125, 0.125],
            tent,
            PartType::Tentacle(i as u8),
        ));
    }
    parts
}

// =========================================================================
// Villager — 64×64 texture
// head 8×10×8 @ (0,0) rot(0,0,0), nose 2×4×2 @ (24,0) child of head
// body 8×12×6 @ (16,20) rot(0,0,0), robe 8×18×6 @ (0,38) child of body +0.5 inflate
// arms 4×8×4 @ (44,22) rot(0,2,0) — held together
// legs 4×12×4 @ (0,22) rot(±2,12,0)

pub fn villager_model() -> Vec<ModelCuboid> {
    // Vanilla ModelVillager (64×64). head rp(0,0,0) @ (0,0) 8×10×8, nose child
    // @ (24,0). body rp(0,0,0) @ (16,20) 8×12×6 + robe @ (0,38) 8×18×6 inflate 0.5.
    // arms (held together, rotateAngleX=-0.75) rp(0,3,-1): two side boxes @ (44,22)
    // + sleeve @ (40,38). legs rp(±2,12,0) @ (0,22) 4×12×4. Feet at y=24.
    let head = uv64x64(0, 0, 8, 10, 8);
    let nose = uv64x64(24, 0, 2, 4, 2);
    let body = uv64x64(16, 20, 8, 12, 6);
    let robe = uv64x64(0, 38, 8, 18, 6);
    let arm_side = uv64x64(44, 22, 4, 8, 4);
    let sleeve = uv64x64(40, 38, 8, 4, 4);
    let leg = uv64x64(0, 22, 4, 12, 4);
    let oy = 24.0;
    let arm_pose = [-0.75, 0.0, 0.0]; // arms folded forward (MC)
    let zero = [0.0; 3];

    vec![
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, -10.0, -4.0],
            [8.0, 10.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        // nose is a child of head; rotate around head's rp, offset = nose rp + addBox.
        mc_part(
            [0.0, 0.0, 0.0],
            [-1.0, -3.0, -6.0],
            [2.0, 4.0, 2.0],
            oy,
            nose,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 0.0, -3.0],
            [8.0, 12.0, 6.0],
            oy,
            body,
            PartType::Body,
        ),
        // robe is a child of body (inflate 0.5 → 9×19×7).
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.5, -0.5, -3.5],
            [9.0, 19.0, 7.0],
            oy,
            robe,
            PartType::Body,
        ),
        // arms (static forward-folded pose).
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

// =========================================================================
// Bat — 64×64 texture. Vanilla uses the same body boxes with two poses:
// hanging (upside-down) and flying. We keep that split via `visual.bat_hanging`.

pub fn bat_model(visual: EntityVisualState) -> Vec<ModelCuboid> {
    let head = uv64x64(0, 0, 6, 6, 6);
    let ear = uv64x64(24, 0, 3, 4, 1);
    let body = uv64x64(0, 16, 6, 12, 6);
    let body_flap = uv64x64(0, 34, 10, 6, 1);
    let wing_i = uv64x64(42, 0, 10, 16, 1);
    let wing_o = uv64x64(24, 16, 8, 12, 1);
    let oy = 24.0;

    if visual.bat_hanging {
        vec![
            mc_cuboid(
                [0.0, -2.0, 0.0],
                [-3.0, -3.0, -3.0],
                [6.0, 6.0, 6.0],
                oy,
                [0.0; 3],
                false,
                head,
                PartType::BatHangingHead,
            ),
            mc_cuboid(
                [0.0, -2.0, 0.0],
                [-4.0, -6.0, -2.0],
                [3.0, 4.0, 1.0],
                oy,
                [0.0; 3],
                false,
                ear,
                PartType::BatHangingHead,
            ),
            mc_cuboid(
                [0.0, -2.0, 0.0],
                [1.0, -6.0, -2.0],
                [3.0, 4.0, 1.0],
                oy,
                [0.0; 3],
                true,
                ear,
                PartType::BatHangingHead,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-3.0, 4.0, -3.0],
                [6.0, 12.0, 6.0],
                oy,
                [std::f32::consts::PI, 0.0, 0.0],
                false,
                body,
                PartType::Body,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-5.0, 16.0, 0.0],
                [10.0, 6.0, 1.0],
                oy,
                [std::f32::consts::PI, 0.0, 0.0],
                false,
                body_flap,
                PartType::Body,
            ),
            mc_cuboid(
                [-3.0, 0.0, 3.0],
                [-12.0, 1.0, 1.5],
                [10.0, 16.0, 1.0],
                oy,
                [-0.15707964, -std::f32::consts::PI * 2.0 / 5.0, 0.0],
                false,
                wing_i,
                PartType::Extra,
            ),
            mc_cuboid(
                [3.0, 0.0, 3.0],
                [2.0, 1.0, 1.5],
                [10.0, 16.0, 1.0],
                oy,
                [-0.15707964, std::f32::consts::PI * 2.0 / 5.0, 0.0],
                true,
                wing_i,
                PartType::Extra,
            ),
            mc_cuboid(
                [-15.0, 1.0, 4.5],
                [-8.0, 1.0, 0.0],
                [8.0, 12.0, 1.0],
                oy,
                [0.0, -1.7278761, 0.0],
                false,
                wing_o,
                PartType::Extra,
            ),
            mc_cuboid(
                [15.0, 1.0, 4.5],
                [0.0, 1.0, 0.0],
                [8.0, 12.0, 1.0],
                oy,
                [0.0, 1.7278761, 0.0],
                true,
                wing_o,
                PartType::Extra,
            ),
        ]
    } else {
        vec![
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-3.0, -3.0, -3.0],
                [6.0, 6.0, 6.0],
                oy,
                [0.0; 3],
                false,
                head,
                PartType::Head,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-4.0, -6.0, -2.0],
                [3.0, 4.0, 1.0],
                oy,
                [0.0; 3],
                false,
                ear,
                PartType::Head,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [1.0, -6.0, -2.0],
                [3.0, 4.0, 1.0],
                oy,
                [0.0; 3],
                true,
                ear,
                PartType::Head,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-3.0, 4.0, -3.0],
                [6.0, 12.0, 6.0],
                oy,
                [std::f32::consts::FRAC_PI_4, 0.0, 0.0],
                false,
                body,
                PartType::OscillatingBody,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-5.0, 16.0, 0.0],
                [10.0, 6.0, 1.0],
                oy,
                [std::f32::consts::FRAC_PI_4, 0.0, 0.0],
                false,
                body_flap,
                PartType::OscillatingBody,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [-12.0, 1.0, 1.5],
                [10.0, 16.0, 1.0],
                oy,
                [std::f32::consts::FRAC_PI_4, 0.0, 0.0],
                false,
                wing_i,
                PartType::BatRightWing,
            ),
            mc_cuboid(
                [0.0, 0.0, 0.0],
                [2.0, 1.0, 1.5],
                [10.0, 16.0, 1.0],
                oy,
                [std::f32::consts::FRAC_PI_4, 0.0, 0.0],
                true,
                wing_i,
                PartType::BatLeftWing,
            ),
            mc_cuboid(
                [-12.0, 1.0, 1.5],
                [-8.0, 1.0, 0.0],
                [8.0, 12.0, 1.0],
                oy,
                [std::f32::consts::FRAC_PI_4, 0.0, 0.0],
                false,
                wing_o,
                PartType::BatOuterRightWing,
            ),
            mc_cuboid(
                [12.0, 1.0, 1.5],
                [0.0, 1.0, 0.0],
                [8.0, 12.0, 1.0],
                oy,
                [std::f32::consts::FRAC_PI_4, 0.0, 0.0],
                true,
                wing_o,
                PartType::BatOuterLeftWing,
            ),
        ]
    }
}

// =========================================================================
// Snowman — 64×64 texture
// head 8×8×8 @ (0,0) -0.5 inflate, rot(0,4,0)
// body 10×10×10 @ (0,16) -0.5 inflate, rot(0,13,0)
// bottom 12×12×12 @ (0,36) -0.5 inflate, rot(0,24,0)
// arms 12×2×2 @ (32,0) -0.5 inflate, rot(0,6,0)

pub fn snowman_model() -> Vec<ModelCuboid> {
    let head = uv64x64(0, 0, 8, 8, 8);
    let body = uv64x64(0, 16, 10, 10, 10);
    let btm = uv64x64(0, 36, 12, 12, 12);
    let arm = uv64x64(32, 0, 12, 2, 2);

    let oy = 28.0;
    vec![
        cuboid(
            mc_pivot(0.0, -4.0, 0.0, oy),
            [0.4375, 0.4375, 0.4375],
            head,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(0.0, 7.0, 0.0, oy),
            [0.5625, 0.5625, 0.5625],
            body,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(0.0, 18.0, 0.0, oy),
            [0.6875, 0.6875, 0.6875],
            btm,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(5.0, 2.0, 0.0, oy),
            [0.6875, 0.125, 0.125],
            arm,
            PartType::LeftArm,
        ),
        cuboid(
            mc_pivot(-5.0, 2.0, 0.0, oy),
            [0.6875, 0.125, 0.125],
            arm,
            PartType::RightArm,
        ),
    ]
}

// =========================================================================
// Iron Golem — 128×128 texture
// head 8×10×8 @ (0,0) rot(0,-7,-2), nose 2×4×2 @ (24,0) child
// body 18×12×11 @ (0,40) rot(0,-7,0), belly 9×5×6 @ (0,70) child +0.5
// right arm 4×30×6 @ (60,21) rot(0,-7,0), left arm 4×30×6 @ (60,51) rot(0,-7,0)
// left leg 6×16×5 @ (37,0) rot(-4,11,0), right leg 6×16×5 @ (60,0) rot(5,11,0) mirror

pub fn iron_golem_model() -> Vec<ModelCuboid> {
    let head = uv128(0, 0, 8, 10, 8);
    let nose = uv128(24, 0, 2, 4, 2);
    let body = uv128(0, 40, 18, 12, 11);
    let belly = uv128(0, 70, 9, 5, 6);
    let rarm = uv128(60, 21, 4, 30, 6);
    let larm = uv128(60, 58, 4, 30, 6);
    let rleg = uv128(60, 0, 6, 16, 5);
    let lleg = uv128(37, 0, 6, 16, 5);

    let oy = 34.0;
    vec![
        cuboid(
            mc_pivot(0.0, -19.0, -4.0, oy),
            [0.5, 0.625, 0.5],
            head,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(-1.0, -24.0, -5.5, oy),
            [0.125, 0.25, 0.125],
            nose,
            PartType::Head,
        ),
        cuboid(
            mc_pivot(0.0, -9.0, 0.0, oy),
            [1.125, 0.75, 0.6875],
            body,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(0.0, 1.0, 0.0, oy),
            [0.5625, 0.3125, 0.375],
            belly,
            PartType::Body,
        ),
        cuboid(
            mc_pivot(13.0, -9.0, 0.0, oy),
            [0.25, 1.875, 0.375],
            rarm,
            PartType::RightArm,
        ),
        cuboid(
            mc_pivot(-13.0, -9.0, 0.0, oy),
            [0.25, 1.875, 0.375],
            larm,
            PartType::LeftArm,
        ),
        cuboid(
            mc_pivot(5.0, 11.0, 0.0, oy),
            [0.375, 1.0, 0.3125],
            rleg,
            PartType::RightLeg,
        ),
        cuboid(
            mc_pivot(-4.0, 11.0, 0.0, oy),
            [0.375, 1.0, 0.3125],
            lleg,
            PartType::LeftLeg,
        ),
    ]
}
