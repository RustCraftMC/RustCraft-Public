use super::helpers::*;
use crate::render::entity::mesh::{ModelCuboid, PartType};

// =========================================================================
// Player model — biped with 64×64 skin texture, arms at sides.
// ModelBiped: head/h $(0,0) 8×8×8, body $(16,16) 8×12×4,
// arms $(40,16) 4×12×4 (slim: 3×12×4), legs $(0,16) 4×12×4.
// origin_y=24 (feet at y=24).

fn player_parts(slim: bool) -> Vec<ModelCuboid> {
    let head = uv64x64(0, 0, 8, 8, 8);
    let hat = uv64x64(32, 0, 8, 8, 8);
    let body = uv64x64(16, 16, 8, 12, 4);
    let jack = uv64x64(16, 32, 8, 12, 4);
    let arm_w = if slim { 3.0 } else { 4.0 };
    let cape = box_uvs(0, 0, 10, 16, 1, 64, 32); // ModelPlayer: 10×16×1 on 64×32

    // Right limbs
    let r_arm = uv64x64(40, 16, if slim { 3 } else { 4 }, 12, 4);
    let r_sleeve = uv64x64(40, 32, if slim { 3 } else { 4 }, 12, 4);
    let r_leg = uv64x64(0, 16, 4, 12, 4);
    let r_pants = uv64x64(0, 32, 4, 12, 4);

    // Left limbs (independent textures in 1.8.9 64x64 skin layout)
    let l_arm = uv64x64(32, 48, if slim { 3 } else { 4 }, 12, 4);
    let l_sleeve = uv64x64(48, 48, if slim { 3 } else { 4 }, 12, 4);
    let l_leg = uv64x64(16, 48, 4, 12, 4);
    let l_pants = uv64x64(0, 48, 4, 12, 4);

    let oy = 24.0;
    let arm_y = if slim { 2.5 } else { 2.0 };
    let arm_box_off_x = if slim { -2.0 } else { -3.0 };

    let mut parts = vec![
        // Head + hat (outer layer)
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, -8.0, -4.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.5, -8.5, -4.5],
            [9.0, 9.0, 9.0],
            oy,
            hat,
            PartType::Head,
        ),
        // Body + jacket
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 0.0, -2.0],
            [8.0, 12.0, 4.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.25, -0.25, -2.25],
            [8.5, 12.5, 4.5],
            oy,
            jack,
            PartType::Body,
        ),
        // Arms (at sides, no forward pose — vanilla player idle)
        mc_cuboid(
            [-5.0, arm_y, 0.0],
            [arm_box_off_x, -2.0, -2.0],
            [arm_w, 12.0, 4.0],
            oy,
            [0.0; 3],
            false,
            r_arm,
            PartType::RightArm,
        ),
        mc_cuboid(
            [5.0, arm_y, 0.0],
            [-1.0, -2.0, -2.0],
            [arm_w, 12.0, 4.0],
            oy,
            [0.0; 3],
            false,
            l_arm,
            PartType::LeftArm,
        ),
        // Sleeves (outer layer)
        mc_cuboid(
            [-5.0, arm_y, 0.0],
            [arm_box_off_x - 0.25, -2.25, -2.25],
            [arm_w + 0.5, 12.5, 4.5],
            oy,
            [0.0; 3],
            false,
            r_sleeve,
            PartType::RightArm,
        ),
        mc_cuboid(
            [5.0, arm_y, 0.0],
            [-1.25, -2.25, -2.25],
            [arm_w + 0.5, 12.5, 4.5],
            oy,
            [0.0; 3],
            false,
            l_sleeve,
            PartType::LeftArm,
        ),
        // Legs + pants
        mc_cuboid(
            [-1.9, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            [0.0; 3],
            false,
            r_leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [1.9, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            [0.0; 3],
            false,
            l_leg,
            PartType::LeftLeg,
        ),
        mc_cuboid(
            [-1.9, 12.0, 0.0],
            [-2.25, -0.25, -2.25],
            [4.5, 12.5, 4.5],
            oy,
            [0.0; 3],
            false,
            r_pants,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [1.9, 12.0, 0.0],
            [-2.25, -0.25, -2.25],
            [4.5, 12.5, 4.5],
            oy,
            [0.0; 3],
            false,
            l_pants,
            PartType::LeftLeg,
        ),
        // ModelPlayer.bipedCape plus LayerCape's +2px Z translation. The
        // dynamic 6° rest lean and 180° Y turn are applied in mesh.rs.
        mc_cuboid(
            [0.0, 0.0, 2.0],
            [-5.0, 0.0, -1.0],
            [10.0, 16.0, 1.0],
            oy,
            [0.0; 3],
            false,
            cape,
            PartType::Cape,
        ),
    ];

    // RenderPlayer.preRenderCallback scales the complete player model by 0.9375.
    for part in &mut parts {
        for value in &mut part.rotation_point {
            *value *= 0.9375;
        }
        for value in &mut part.box_offset {
            *value *= 0.9375;
        }
        for value in &mut part.size {
            *value *= 0.9375;
        }
    }
    parts
}

pub fn player_model(slim: bool, skin_parts_mask: u8, has_cape: bool) -> Vec<ModelCuboid> {
    player_parts(slim)
        .into_iter()
        .enumerate()
        .filter_map(|(index, part)| {
            let visible = match index {
                1 => skin_parts_mask & 0x40 != 0,              // hat
                3 => skin_parts_mask & 0x02 != 0,              // jacket
                6 => skin_parts_mask & 0x08 != 0,              // right sleeve
                7 => skin_parts_mask & 0x04 != 0,              // left sleeve
                10 => skin_parts_mask & 0x20 != 0,             // right pants
                11 => skin_parts_mask & 0x10 != 0,             // left pants
                12 => has_cape && skin_parts_mask & 0x01 != 0, // cape
                _ => true,
            };
            visible.then_some(part)
        })
        .collect()
}

/// Vanilla `LayerBipedArmor` model for one protocol equipment slot.
/// Slots are 1=boots, 2=leggings, 3=chestplate, 4=helmet.
pub fn player_armor_model(equipment_slot: usize) -> Vec<ModelCuboid> {
    let inflation = if equipment_slot == 2 { 0.5 } else { 1.0 };
    let expanded = |rp, off: [f32; 3], size: [f32; 3], uvs, part, mirror| {
        mc_cuboid(
            rp,
            [off[0] - inflation, off[1] - inflation, off[2] - inflation],
            [
                size[0] + inflation * 2.0,
                size[1] + inflation * 2.0,
                size[2] + inflation * 2.0,
            ],
            24.0,
            [0.0; 3],
            mirror,
            uvs,
            part,
        )
    };

    let mut parts = match equipment_slot {
        1 => vec![
            expanded(
                [-1.9, 12.0, 0.0],
                [-2.0, 0.0, -2.0],
                [4.0, 12.0, 4.0],
                uv64x32(0, 16, 4, 12, 4),
                PartType::RightLeg,
                false,
            ),
            expanded(
                [1.9, 12.0, 0.0],
                [-2.0, 0.0, -2.0],
                [4.0, 12.0, 4.0],
                uv64x32(0, 16, 4, 12, 4),
                PartType::LeftLeg,
                true,
            ),
        ],
        2 => vec![
            expanded(
                [0.0, 0.0, 0.0],
                [-4.0, 0.0, -2.0],
                [8.0, 12.0, 4.0],
                uv64x32(16, 16, 8, 12, 4),
                PartType::Body,
                false,
            ),
            expanded(
                [-1.9, 12.0, 0.0],
                [-2.0, 0.0, -2.0],
                [4.0, 12.0, 4.0],
                uv64x32(0, 16, 4, 12, 4),
                PartType::RightLeg,
                false,
            ),
            expanded(
                [1.9, 12.0, 0.0],
                [-2.0, 0.0, -2.0],
                [4.0, 12.0, 4.0],
                uv64x32(0, 16, 4, 12, 4),
                PartType::LeftLeg,
                true,
            ),
        ],
        3 => vec![
            expanded(
                [0.0, 0.0, 0.0],
                [-4.0, 0.0, -2.0],
                [8.0, 12.0, 4.0],
                uv64x32(16, 16, 8, 12, 4),
                PartType::Body,
                false,
            ),
            expanded(
                [-5.0, 2.0, 0.0],
                [-3.0, -2.0, -2.0],
                [4.0, 12.0, 4.0],
                uv64x32(40, 16, 4, 12, 4),
                PartType::RightArm,
                false,
            ),
            expanded(
                [5.0, 2.0, 0.0],
                [-1.0, -2.0, -2.0],
                [4.0, 12.0, 4.0],
                uv64x32(40, 16, 4, 12, 4),
                PartType::LeftArm,
                true,
            ),
        ],
        4 => {
            let mut head = expanded(
                [0.0, 0.0, 0.0],
                [-4.0, -8.0, -4.0],
                [8.0, 8.0, 8.0],
                uv64x32(0, 0, 8, 8, 8),
                PartType::Head,
                false,
            );
            let outer_inflation = inflation + 0.5;
            let headwear = mc_cuboid(
                [0.0, 0.0, 0.0],
                [
                    -4.0 - outer_inflation,
                    -8.0 - outer_inflation,
                    -4.0 - outer_inflation,
                ],
                [
                    8.0 + outer_inflation * 2.0,
                    8.0 + outer_inflation * 2.0,
                    8.0 + outer_inflation * 2.0,
                ],
                24.0,
                [0.0; 3],
                false,
                uv64x32(32, 0, 8, 8, 8),
                PartType::Head,
            );
            head.mirror = false;
            vec![head, headwear]
        }
        _ => Vec::new(),
    };

    for part in &mut parts {
        for value in &mut part.rotation_point {
            *value *= 0.9375;
        }
        for value in &mut part.box_offset {
            *value *= 0.9375;
        }
        for value in &mut part.size {
            *value *= 0.9375;
        }
    }
    parts
}

#[cfg(test)]
mod player_tests {
    use super::*;

    #[test]
    fn player_skin_mask_only_controls_outer_layers() {
        assert_eq!(player_model(false, 0, false).len(), 6);
        assert_eq!(player_model(false, 0x7e, false).len(), 12);
        assert_eq!(player_model(false, 0x7f, true).len(), 13); // + cape
        assert_eq!(player_model(false, 0x7f, false).len(), 12); // no cape
        assert_eq!(player_model(true, 0x08, false).len(), 7);
    }

    #[test]
    fn player_model_applies_renderplayer_scale_and_layer_inflation() {
        let model = player_model(false, 0x7f, true);
        let head_top = model[0].rotation_point[1] + model[0].box_offset[1] + model[0].size[1];

        assert!((head_top - 1.875).abs() < 1.0e-6);
        assert!(model[3].size[0] > model[2].size[0]);
        assert!(model[6].size[0] > model[4].size[0]);
        assert!(model[10].size[0] > model[8].size[0]);
    }

    #[test]
    fn cape_matches_model_player_box_dimensions() {
        let model = player_model(false, 0x01, true);
        let cape = model
            .iter()
            .find(|part| part.part_type == PartType::Cape)
            .unwrap();
        let scale = 0.9375 / 16.0;
        assert_eq!(cape.size, [10.0 * scale, 16.0 * scale, scale]);
        assert_eq!(cape.rotation_point[0], 0.0);
        assert_eq!(cape.box_offset[2], -scale);
    }

    #[test]
    fn armor_slots_match_layer_biped_armor_visibility() {
        let boots = player_armor_model(1);
        let leggings = player_armor_model(2);
        let chest = player_armor_model(3);
        let helmet = player_armor_model(4);

        assert_eq!(boots.len(), 2);
        assert_eq!(leggings.len(), 3);
        assert_eq!(chest.len(), 3);
        assert_eq!(helmet.len(), 2);
        assert!(leggings[0].size[0] < chest[0].size[0]);
    }
}

// =========================================================================
// Biped models (zombie, skeleton) — re-derived from vanilla ModelBiped /
// ModelZombie / ModelSkeleton (texture 64×64 for zombie, 64×32 for skeleton).
// Legs rp.y=12, height 12 → feet at y=24, so origin_y=24.
// Arms held forward (ModelZombie sets rotateAngleX = -PI/2) with the normal
// biped arm swing layered on top.

pub fn zombie_model() -> Vec<ModelCuboid> {
    let head = uv64x64(0, 0, 8, 8, 8);
    let hat = uv64x64(32, 0, 8, 8, 8); // bipedHeadwear (outer head layer, +0.5 inflate)
    let body = uv64x64(16, 16, 8, 12, 4);
    let arm = uv64x64(40, 16, 4, 12, 4);
    let leg = uv64x64(0, 16, 4, 12, 4);
    let oy = 24.0;
    let arm_pose = [-std::f32::consts::FRAC_PI_2, 0.0, 0.0]; // forward (MC)

    vec![
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, -8.0, -4.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.5, -8.5, -4.5],
            [9.0, 9.0, 9.0],
            oy,
            hat,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 0.0, -2.0],
            [8.0, 12.0, 4.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-5.0, 2.0, 0.0],
            [-3.0, -2.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            arm_pose,
            false,
            arm,
            PartType::RightArm,
        ),
        mc_cuboid(
            [5.0, 2.0, 0.0],
            [-1.0, -2.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            arm_pose,
            true,
            arm,
            PartType::LeftArm,
        ),
        mc_cuboid(
            [-1.9, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            [0.0; 3],
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [1.9, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            [0.0; 3],
            true,
            leg,
            PartType::LeftLeg,
        ),
    ]
}

pub fn zombie_villager_model() -> Vec<ModelCuboid> {
    let head = uv64x64(0, 0, 8, 10, 8);
    let hat = uv64x64(32, 0, 8, 10, 8);
    let nose = uv64x64(24, 0, 2, 4, 2);
    let body = uv64x64(16, 20, 8, 12, 6);
    let arm = uv64x64(44, 22, 4, 12, 4);
    let leg = uv64x64(0, 22, 4, 12, 4);
    let oy = 24.0;
    let arm_pose = [-std::f32::consts::FRAC_PI_2, 0.0, 0.0];

    vec![
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
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, -10.0, -4.0],
            [8.0, 10.0, 8.0],
            oy,
            hat,
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
        mc_cuboid(
            [-5.0, 2.0, 0.0],
            [-3.0, -2.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            arm_pose,
            false,
            arm,
            PartType::RightArm,
        ),
        mc_cuboid(
            [5.0, 2.0, 0.0],
            [-1.0, -2.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            arm_pose,
            true,
            arm,
            PartType::LeftArm,
        ),
        mc_cuboid(
            [-2.0, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            [0.0; 3],
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [2.0, 12.0, 0.0],
            [-2.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            oy,
            [0.0; 3],
            true,
            leg,
            PartType::LeftLeg,
        ),
    ]
}

pub fn skeleton_model() -> Vec<ModelCuboid> {
    // ModelSkeleton overrides arms/legs to thin 2×2 limbs; arms forward.
    let head = uv64x32(0, 0, 8, 8, 8);
    let hat = uv64x32(32, 0, 8, 8, 8);
    let body = uv64x32(16, 16, 8, 12, 4);
    let arm = uv64x32(40, 16, 2, 12, 2);
    let leg = uv64x32(0, 16, 2, 12, 2);
    let oy = 24.0;
    let arm_pose = [-std::f32::consts::FRAC_PI_2, 0.0, 0.0];

    vec![
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, -8.0, -4.0],
            [8.0, 8.0, 8.0],
            oy,
            head,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.5, -8.5, -4.5],
            [9.0, 9.0, 9.0],
            oy,
            hat,
            PartType::Head,
        ),
        mc_part(
            [0.0, 0.0, 0.0],
            [-4.0, 0.0, -2.0],
            [8.0, 12.0, 4.0],
            oy,
            body,
            PartType::Body,
        ),
        mc_cuboid(
            [-5.0, 2.0, 0.0],
            [-1.0, -2.0, -1.0],
            [2.0, 12.0, 2.0],
            oy,
            arm_pose,
            false,
            arm,
            PartType::RightArm,
        ),
        mc_cuboid(
            [5.0, 2.0, 0.0],
            [-1.0, -2.0, -1.0],
            [2.0, 12.0, 2.0],
            oy,
            arm_pose,
            true,
            arm,
            PartType::LeftArm,
        ),
        mc_cuboid(
            [-2.0, 12.0, 0.0],
            [-1.0, 0.0, -1.0],
            [2.0, 12.0, 2.0],
            oy,
            [0.0; 3],
            false,
            leg,
            PartType::RightLeg,
        ),
        mc_cuboid(
            [2.0, 12.0, 0.0],
            [-1.0, 0.0, -1.0],
            [2.0, 12.0, 2.0],
            oy,
            [0.0; 3],
            true,
            leg,
            PartType::LeftLeg,
        ),
    ]
}
