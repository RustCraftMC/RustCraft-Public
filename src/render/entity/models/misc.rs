use super::helpers::*;
use crate::render::entity::mesh::{ModelCuboid, PartType};

// =========================================================================
// Armor Stand — 64×64 texture, thin rod geometry
// head 2×7×2 @ (0,0), body 12×3×3 @ (0,26)
// arms 2×12×2 @ (24,0)/(32,16), legs 2×11×2 @ (8,0)/(40,16)
// side bars 2×7×2, waist 8×2×2, base 12×1×12

pub fn armor_stand_model(flags: u8) -> Vec<ModelCuboid> {
    let head = uv64x64(0, 0, 2, 7, 2);
    let body = uv64x64(0, 26, 12, 3, 3);
    let arm_r = uv64x64(24, 0, 2, 12, 2);
    let arm_l = uv64x64(32, 16, 2, 12, 2);
    let leg_r = uv64x64(8, 0, 2, 11, 2);
    let leg_l = uv64x64(40, 16, 2, 11, 2);
    let side = uv64x64(16, 0, 2, 7, 2);
    let waist = uv64x64(0, 48, 8, 2, 2);
    let base = uv64x64(0, 32, 12, 1, 12);

    let oy = 24.0;
    let mut parts = vec![
        mc_part(
            [0.0, 0.0, 0.0],
            [-1.0, -7.0, -1.0],
            [2.0, 7.0, 2.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-6.0, 0.0, -1.5],
            [12.0, 3.0, 3.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-1.9, 12.0, 0.0],
            [-1.0, 0.0, -1.0],
            [2.0, 11.0, 2.0],
            oy,
            [0.0; 3],
            false,
            leg_r,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [1.9, 12.0, 0.0],
            [-1.0, 0.0, -1.0],
            [2.0, 11.0, 2.0],
            oy,
            [0.0; 3],
            true,
            leg_l,
            PartType::LeftLeg,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-3.0, 3.0, -1.0],
            [2.0, 7.0, 2.0],
            oy,
            side,
            PartType::Body,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [1.0, 3.0, -1.0],
            [2.0, 7.0, 2.0],
            oy,
            uv64x64(48, 16, 2, 7, 2),
            PartType::Body,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 10.0, -1.0],
            [8.0, 2.0, 2.0],
            oy,
            waist,
            PartType::Body,
        ),
    ];
    if flags & 0x04 != 0 {
        parts.push(mc_cuboid(
            [-5.0, 2.0, 0.0],
            [-2.0, -2.0, -1.0],
            [2.0, 12.0, 2.0],
            oy,
            [0.0; 3],
            false,
            arm_r,
            PartType::RightArm,
        ));
        parts.push(mc_cuboid(
            [5.0, 2.0, 0.0],
            [0.0, -2.0, -1.0],
            [2.0, 12.0, 2.0],
            oy,
            [0.0; 3],
            true,
            arm_l,
            PartType::LeftArm,
        ));
    }
    if flags & 0x08 == 0 {
        parts.push(mc_part(
            [0.0, 12.0, 0.0],
            [-6.0, 11.0, -6.0],
            [12.0, 1.0, 12.0],
            oy,
            base,
            PartType::Extra,
        ));
    }
    parts
}

// =========================================================================
// Item / XPOrb / Projectile models (small 3D shapes)

/// Item entity — small rotating cube
pub fn item_model() -> Vec<ModelCuboid> {
    let face = uv64x32(0, 0, 16, 16, 16);
    vec![cuboid(
        [0.0, 0.25, 0.0],
        [0.25, 0.25, 0.25],
        face,
        PartType::Extra,
    )]
}

/// XP orb — small glowing diamond using experience_orb.png (64x64, 4x4 grid of 16x16 sprites)
pub fn xp_orb_model() -> Vec<ModelCuboid> {
    // Map all 6 faces to sprite index 0 (top-left 16x16 cell in 64x64 texture)
    // Sprite 0 UVs: u=[0, 16/64=0.25], v=[0, 16/64=0.25]
    let orb_uv = [0.0, 0.0, 0.25, 0.25];
    vec![ModelCuboid {
        rotation_point: [0.0, 0.25, 0.0],
        box_offset: [-0.15, -0.15, -0.15],
        size: [0.3, 0.3, 0.3],
        rotation: [0.0; 3],
        mirror: false,
        face_uvs: [orb_uv; 6],
        color: [1.0, 1.0, 1.0, 1.0],
        part_type: PartType::Extra,
    }]
}

/// Arrow — flat crossed-quad with shaft and arrowhead
pub fn arrow_model() -> Vec<ModelCuboid> {
    let face = uv64x32(0, 0, 16, 16, 16);
    vec![
        cuboid([0.0, 0.0, 0.0], [0.12, 0.8, 0.12], face, PartType::Extra),
        cuboid([0.0, 0.7, 0.0], [0.2, 0.15, 0.2], face, PartType::Extra),
    ]
}

/// Small projectile (snowball, ender pearl, egg, potion, exp bottle)
pub fn projectile_model() -> Vec<ModelCuboid> {
    let face = uv64x32(0, 0, 16, 16, 16);
    vec![cuboid(
        [0.0, 0.15, 0.0],
        [0.12, 0.12, 0.12],
        face,
        PartType::Extra,
    )]
}

/// Falling block / TNT — block shape
pub fn falling_block_model() -> Vec<ModelCuboid> {
    let face = uv64x32(0, 0, 16, 16, 16);
    vec![cuboid(
        [0.0, 0.5, 0.0],
        [1.0, 1.0, 1.0],
        face,
        PartType::Body,
    )]
}

/// Boat — simple boat shape
pub fn boat_model() -> Vec<ModelCuboid> {
    let face = uv64x32(0, 0, 16, 16, 16);
    vec![cuboid(
        [0.0, 0.0, 0.0],
        [1.2, 0.4, 0.6],
        face,
        PartType::Body,
    )]
}

/// Minecart — simple box
pub fn minecart_model() -> Vec<ModelCuboid> {
    let face = uv64x32(0, 0, 16, 16, 16);
    vec![cuboid(
        [0.0, 0.25, 0.0],
        [0.98, 0.7, 0.98],
        face,
        PartType::Body,
    )]
}

/// Generate armor overlay cuboids for a set of equipment.
/// `equipment` is [held, boots(1), leggings(2), chestplate(3), helmet(4)].
/// Returns two vecs: (layer_1_cuboids, layer_2_cuboids) — layer_1 uses the
/// armor layer 1 texture (helmet, chestplate, boots), layer_2 uses layer 2 (leggings).
pub fn armor_overlay_cuboids(
    equipment: &[Option<(u16, u16)>; 5],
    base_model: &[ModelCuboid],
    slim: bool,
) -> (Vec<ModelCuboid>, Vec<ModelCuboid>) {
    let mut layer1 = Vec::new();
    let mut layer2 = Vec::new();

    // Equipment protocol order: [held(0), boots(1), leggings(2), chestplate(3), helmet(4)]
    let _ = slim;

    // Find base cuboid data for each part type
    let head_data = find_part(base_model, PartType::Head);
    let body_data = find_part(base_model, PartType::Body);
    let rarm_data = find_part(base_model, PartType::RightArm);
    let larm_data = find_part(base_model, PartType::LeftArm);
    let rleg_data = find_part(base_model, PartType::RightLeg);
    let lleg_data = find_part(base_model, PartType::LeftLeg);

    let infl = 0.5 / 16.0;

    // --- Layer 1: Helmet, Chestplate (body+arms), Boots (legs) ---
    // Helmet
    let has_helmet = equipment.get(4).and_then(|s| s.as_ref()).is_some();
    if has_helmet {
        if let Some(h) = &head_data {
            let uv = uv64x32(0, 0, 8, 8, 8);
            layer1.push(ModelCuboid {
                rotation_point: h.rotation_point,
                box_offset: [
                    h.box_offset[0] - infl,
                    h.box_offset[1] - infl,
                    h.box_offset[2] - infl,
                ],
                size: [
                    h.size[0] + infl * 2.0,
                    h.size[1] + infl * 2.0,
                    h.size[2] + infl * 2.0,
                ],
                rotation: h.rotation,
                mirror: h.mirror,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::Head,
            });
        }
    }

    // Chestplate: body + arms
    let has_chestplate = equipment.get(3).and_then(|s| s.as_ref()).is_some();
    if has_chestplate {
        if let Some(b) = &body_data {
            let uv = uv64x32(16, 16, 8, 12, 4);
            layer1.push(ModelCuboid {
                rotation_point: b.rotation_point,
                box_offset: [
                    b.box_offset[0] - infl,
                    b.box_offset[1] - infl,
                    b.box_offset[2] - infl,
                ],
                size: [
                    b.size[0] + infl * 2.0,
                    b.size[1] + infl * 2.0,
                    b.size[2] + infl * 2.0,
                ],
                rotation: b.rotation,
                mirror: b.mirror,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::Body,
            });
        }
        if let Some(ra) = &rarm_data {
            let uv = uv64x32(40, 16, 4, 12, 4);
            layer1.push(ModelCuboid {
                rotation_point: ra.rotation_point,
                box_offset: [
                    ra.box_offset[0] - infl,
                    ra.box_offset[1] - infl,
                    ra.box_offset[2] - infl,
                ],
                size: [
                    ra.size[0] + infl * 2.0,
                    ra.size[1] + infl * 2.0,
                    ra.size[2] + infl * 2.0,
                ],
                rotation: ra.rotation,
                mirror: ra.mirror,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::RightArm,
            });
        }
        if let Some(la) = &larm_data {
            let uv = uv64x32(48, 16, 4, 12, 4);
            layer1.push(ModelCuboid {
                rotation_point: la.rotation_point,
                box_offset: [
                    la.box_offset[0] - infl,
                    la.box_offset[1] - infl,
                    la.box_offset[2] - infl,
                ],
                size: [
                    la.size[0] + infl * 2.0,
                    la.size[1] + infl * 2.0,
                    la.size[2] + infl * 2.0,
                ],
                rotation: la.rotation,
                mirror: true,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::LeftArm,
            });
        }
    }

    // Boots: legs (layer 1)
    let has_boots = equipment.get(1).and_then(|s| s.as_ref()).is_some();
    if has_boots {
        if let Some(rl) = &rleg_data {
            let uv = uv64x32(0, 16, 4, 12, 4);
            layer1.push(ModelCuboid {
                rotation_point: rl.rotation_point,
                box_offset: [
                    rl.box_offset[0] - infl,
                    rl.box_offset[1] - infl,
                    rl.box_offset[2] - infl,
                ],
                size: [
                    rl.size[0] + infl * 2.0,
                    rl.size[1] + infl * 2.0,
                    rl.size[2] + infl * 2.0,
                ],
                rotation: rl.rotation,
                mirror: rl.mirror,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::RightLeg,
            });
        }
        if let Some(ll) = &lleg_data {
            let uv = uv64x32(0, 16, 4, 12, 4);
            layer1.push(ModelCuboid {
                rotation_point: ll.rotation_point,
                box_offset: [
                    ll.box_offset[0] - infl,
                    ll.box_offset[1] - infl,
                    ll.box_offset[2] - infl,
                ],
                size: [
                    ll.size[0] + infl * 2.0,
                    ll.size[1] + infl * 2.0,
                    ll.size[2] + infl * 2.0,
                ],
                rotation: ll.rotation,
                mirror: true,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::LeftLeg,
            });
        }
    }

    // --- Layer 2: Leggings ---
    let has_leggings = equipment.get(2).and_then(|s| s.as_ref()).is_some();
    if has_leggings {
        if let Some(b) = &body_data {
            let uv = uv64x32(16, 16, 8, 12, 4);
            layer2.push(ModelCuboid {
                rotation_point: b.rotation_point,
                box_offset: [
                    b.box_offset[0] - infl,
                    b.box_offset[1] - infl,
                    b.box_offset[2] - infl,
                ],
                size: [
                    b.size[0] + infl * 2.0,
                    b.size[1] + infl * 2.0,
                    b.size[2] + infl * 2.0,
                ],
                rotation: b.rotation,
                mirror: b.mirror,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::Body,
            });
        }
        if let Some(rl) = &rleg_data {
            let uv = uv64x32(0, 0, 4, 12, 4);
            layer2.push(ModelCuboid {
                rotation_point: rl.rotation_point,
                box_offset: [
                    rl.box_offset[0] - infl,
                    rl.box_offset[1] - infl,
                    rl.box_offset[2] - infl,
                ],
                size: [
                    rl.size[0] + infl * 2.0,
                    rl.size[1] + infl * 2.0,
                    rl.size[2] + infl * 2.0,
                ],
                rotation: rl.rotation,
                mirror: rl.mirror,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::RightLeg,
            });
        }
        if let Some(ll) = &lleg_data {
            let uv = uv64x32(8, 0, 4, 12, 4);
            layer2.push(ModelCuboid {
                rotation_point: ll.rotation_point,
                box_offset: [
                    ll.box_offset[0] - infl,
                    ll.box_offset[1] - infl,
                    ll.box_offset[2] - infl,
                ],
                size: [
                    ll.size[0] + infl * 2.0,
                    ll.size[1] + infl * 2.0,
                    ll.size[2] + infl * 2.0,
                ],
                rotation: ll.rotation,
                mirror: true,
                face_uvs: uv,
                color: [1.0; 4],
                part_type: PartType::LeftLeg,
            });
        }
    }

    (layer1, layer2)
}

fn find_part<'a>(parts: &'a [ModelCuboid], part_type: PartType) -> Option<&'a ModelCuboid> {
    parts.iter().find(|p| p.part_type == part_type)
}

#[cfg(test)]
mod armor_stand_tests {
    use super::*;

    #[test]
    fn model_uses_vanilla_offsets_and_visibility_flags() {
        let plain = armor_stand_model(0);
        assert_eq!(plain.len(), 8);
        assert!(find_part(&plain, PartType::RightArm).is_none());
        assert!(plain.iter().any(|part| part.part_type == PartType::Extra));

        let arms_without_base = armor_stand_model(0x04 | 0x08);
        assert_eq!(arms_without_base.len(), 9);
        let right_arm = find_part(&arms_without_base, PartType::RightArm).unwrap();
        assert_eq!(right_arm.rotation_point, [-5.0 / 16.0, 22.0 / 16.0, 0.0]);
        assert_eq!(
            right_arm.box_offset,
            [-2.0 / 16.0, -10.0 / 16.0, -1.0 / 16.0]
        );
        assert!(!arms_without_base
            .iter()
            .any(|part| part.part_type == PartType::Extra));
    }
}
