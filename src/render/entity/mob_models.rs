//! Box-based 3D mob models for entity rendering.
//!
//! Each mob is defined as a list of cuboid body parts.
//! The rendering draws 3D box faces (front, top, side) with lighting,
//! and supports limb swing animations for walking mobs.

use crate::entity::EntityType;
use crate::render::gui::GuiVertexBuilder;

#[derive(Clone, Copy, PartialEq)]
pub enum PartType {
    Head,
    Body,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
    Static,
}

#[derive(Clone, Copy)]
pub struct MobPart {
    pub offset: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 4],
    pub part_type: PartType,
}

const fn mp(offset: [f32; 3], size: [f32; 3], color: [f32; 4]) -> MobPart {
    MobPart {
        offset,
        size,
        color,
        part_type: PartType::Static,
    }
}
const fn mp_part(offset: [f32; 3], size: [f32; 3], color: [f32; 4], pt: PartType) -> MobPart {
    MobPart {
        offset,
        size,
        color,
        part_type: pt,
    }
}

// ── Static part arrays ──

static ZOMBIE: [MobPart; 7] = [
    mp_part(
        [0.0, 1.625, 0.0],
        [0.5, 0.5, 0.5],
        [0.29, 0.56, 0.24, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.125, 0.0],
        [0.5, 0.75, 0.25],
        [0.22, 0.37, 0.78, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.29, 0.56, 0.24, 0.92],
        PartType::RightArm,
    ),
    mp_part(
        [0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.29, 0.56, 0.24, 0.92],
        PartType::LeftArm,
    ),
    mp_part(
        [-0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.15, 0.15, 0.55, 0.95],
        PartType::RightLeg,
    ),
    mp_part(
        [0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.15, 0.15, 0.55, 0.95],
        PartType::LeftLeg,
    ),
    // Overlay/hat (slightly larger head)
    mp(
        [0.0, 1.625, 0.0],
        [0.525, 0.525, 0.525],
        [0.26, 0.50, 0.22, 0.35],
    ),
];
static SKELETON: [MobPart; 6] = [
    mp_part(
        [0.0, 1.625, 0.0],
        [0.5, 0.5, 0.5],
        [0.82, 0.80, 0.75, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.125, 0.0],
        [0.5, 0.75, 0.25],
        [0.80, 0.78, 0.72, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.78, 0.76, 0.70, 0.92],
        PartType::RightArm,
    ),
    mp_part(
        [0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.78, 0.76, 0.70, 0.92],
        PartType::LeftArm,
    ),
    mp_part(
        [-0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.75, 0.73, 0.68, 0.95],
        PartType::RightLeg,
    ),
    mp_part(
        [0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.75, 0.73, 0.68, 0.95],
        PartType::LeftLeg,
    ),
];
static PIGZOMBIE: [MobPart; 7] = [
    mp_part(
        [0.0, 1.625, 0.0],
        [0.5, 0.5, 0.5],
        [0.88, 0.78, 0.55, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.125, 0.0],
        [0.5, 0.75, 0.25],
        [0.20, 0.35, 0.65, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.88, 0.78, 0.55, 0.92],
        PartType::RightArm,
    ),
    mp_part(
        [0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.88, 0.78, 0.55, 0.92],
        PartType::LeftArm,
    ),
    mp_part(
        [-0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.12, 0.12, 0.48, 0.95],
        PartType::RightLeg,
    ),
    mp_part(
        [0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.12, 0.12, 0.48, 0.95],
        PartType::LeftLeg,
    ),
    mp(
        [0.0, 1.625, 0.0],
        [0.525, 0.525, 0.525],
        [0.82, 0.72, 0.50, 0.35],
    ),
];
static WITCH_MOB: [MobPart; 7] = [
    mp_part(
        [0.0, 1.625, 0.0],
        [0.5, 0.5, 0.5],
        [0.32, 0.22, 0.38, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.125, 0.0],
        [0.5, 0.75, 0.25],
        [0.25, 0.18, 0.35, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.25, 0.18, 0.35, 0.92],
        PartType::RightArm,
    ),
    mp_part(
        [0.375, 1.125, 0.0],
        [0.25, 0.75, 0.25],
        [0.25, 0.18, 0.35, 0.92],
        PartType::LeftArm,
    ),
    mp_part(
        [-0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.18, 0.12, 0.25, 0.95],
        PartType::RightLeg,
    ),
    mp_part(
        [0.125, 0.375, 0.0],
        [0.25, 0.75, 0.25],
        [0.18, 0.12, 0.25, 0.95],
        PartType::LeftLeg,
    ),
    mp(
        [0.0, 1.625, 0.0],
        [0.52, 0.12, 0.52],
        [0.15, 0.10, 0.20, 0.92],
    ),
];
static ENDERMAN_MOB: [MobPart; 6] = [
    mp_part(
        [0.0, 2.425, 0.0],
        [0.5, 0.5, 0.5],
        [0.10, 0.10, 0.12, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.75, 0.0],
        [0.5, 0.875, 0.25],
        [0.10, 0.10, 0.12, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.375, 1.75, 0.0],
        [0.25, 0.875, 0.25],
        [0.10, 0.10, 0.12, 0.92],
        PartType::RightArm,
    ),
    mp_part(
        [0.375, 1.75, 0.0],
        [0.25, 0.875, 0.25],
        [0.10, 0.10, 0.12, 0.92],
        PartType::LeftArm,
    ),
    mp_part(
        [-0.125, 0.625, 0.0],
        [0.25, 1.25, 0.25],
        [0.10, 0.10, 0.12, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.125, 0.625, 0.0],
        [0.25, 1.25, 0.25],
        [0.10, 0.10, 0.12, 0.92],
        PartType::LeftLeg,
    ),
];
static CREEPER_MOB: [MobPart; 6] = [
    mp_part(
        [0.0, 1.5, 0.0],
        [0.5, 0.5, 0.5],
        [0.28, 0.72, 0.28, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.875, 0.0],
        [0.5, 0.75, 0.5],
        [0.25, 0.68, 0.25, 0.95],
        PartType::Body,
    ),
    mp(
        [-0.125, 0.1875, -0.125],
        [0.25, 0.375, 0.25],
        [0.22, 0.60, 0.22, 0.92],
    ),
    mp(
        [0.125, 0.1875, -0.125],
        [0.25, 0.375, 0.25],
        [0.22, 0.60, 0.22, 0.92],
    ),
    mp(
        [-0.125, 0.1875, 0.125],
        [0.25, 0.375, 0.25],
        [0.22, 0.60, 0.22, 0.92],
    ),
    mp(
        [0.125, 0.1875, 0.125],
        [0.25, 0.375, 0.25],
        [0.22, 0.60, 0.22, 0.92],
    ),
];
static PIG_MOB: [MobPart; 6] = [
    mp_part(
        [0.0, 0.75, 0.35],
        [0.5, 0.5, 0.5],
        [0.92, 0.70, 0.68, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.5, 0.0],
        [0.625, 0.5, 1.0],
        [0.95, 0.75, 0.72, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.22, 0.125, -0.28],
        [0.18, 0.25, 0.18],
        [0.85, 0.62, 0.58, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.22, 0.125, -0.28],
        [0.18, 0.25, 0.18],
        [0.85, 0.62, 0.58, 0.92],
        PartType::LeftLeg,
    ),
    mp_part(
        [-0.22, 0.125, 0.28],
        [0.18, 0.25, 0.18],
        [0.85, 0.62, 0.58, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.22, 0.125, 0.28],
        [0.18, 0.25, 0.18],
        [0.85, 0.62, 0.58, 0.92],
        PartType::LeftLeg,
    ),
];
static COW_MOB: [MobPart; 6] = [
    mp_part(
        [0.0, 0.875, 0.35],
        [0.5, 0.5, 0.5],
        [0.45, 0.30, 0.20, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.5625, 0.0],
        [0.625, 0.625, 1.0],
        [0.42, 0.28, 0.18, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.22, 0.15625, -0.28],
        [0.18, 0.3125, 0.18],
        [0.40, 0.26, 0.16, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.22, 0.15625, -0.28],
        [0.18, 0.3125, 0.18],
        [0.40, 0.26, 0.16, 0.92],
        PartType::LeftLeg,
    ),
    mp_part(
        [-0.22, 0.15625, 0.28],
        [0.18, 0.3125, 0.18],
        [0.40, 0.26, 0.16, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.22, 0.15625, 0.28],
        [0.18, 0.3125, 0.18],
        [0.40, 0.26, 0.16, 0.92],
        PartType::LeftLeg,
    ),
];
static SHEEP_MOB: [MobPart; 7] = [
    mp_part(
        [0.0, 0.875, 0.35],
        [0.4, 0.45, 0.4],
        [0.92, 0.90, 0.88, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.625, 0.0],
        [0.625, 0.625, 0.9],
        [0.90, 0.88, 0.85, 0.95],
        PartType::Body,
    ),
    mp(
        [0.0, 0.875, 0.35],
        [0.38, 0.35, 0.38],
        [0.55, 0.42, 0.38, 0.92],
    ),
    mp_part(
        [-0.22, 0.15625, -0.22],
        [0.16, 0.3125, 0.16],
        [0.50, 0.38, 0.35, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.22, 0.15625, -0.22],
        [0.16, 0.3125, 0.16],
        [0.50, 0.38, 0.35, 0.92],
        PartType::LeftLeg,
    ),
    mp_part(
        [-0.22, 0.15625, 0.22],
        [0.16, 0.3125, 0.16],
        [0.50, 0.38, 0.35, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.22, 0.15625, 0.22],
        [0.16, 0.3125, 0.16],
        [0.50, 0.38, 0.35, 0.92],
        PartType::LeftLeg,
    ),
];
static WOLF_MOB: [MobPart; 7] = [
    mp_part(
        [0.0, 0.55, 0.35],
        [0.35, 0.35, 0.35],
        [0.65, 0.62, 0.58, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.4375, 0.0],
        [0.4, 0.4375, 0.85],
        [0.62, 0.58, 0.55, 0.95],
        PartType::Body,
    ),
    mp(
        [0.0, 0.6, -0.4],
        [0.12, 0.35, 0.35],
        [0.60, 0.55, 0.50, 0.92],
    ),
    mp_part(
        [-0.15, 0.125, -0.2],
        [0.12, 0.25, 0.12],
        [0.55, 0.50, 0.45, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.15, 0.125, -0.2],
        [0.12, 0.25, 0.12],
        [0.55, 0.50, 0.45, 0.92],
        PartType::LeftLeg,
    ),
    mp_part(
        [-0.15, 0.125, 0.2],
        [0.12, 0.25, 0.12],
        [0.55, 0.50, 0.45, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.15, 0.125, 0.2],
        [0.12, 0.25, 0.12],
        [0.55, 0.50, 0.45, 0.92],
        PartType::LeftLeg,
    ),
];
static OZELOT_MOB: [MobPart; 7] = [
    mp_part(
        [0.0, 0.5, 0.3],
        [0.35, 0.35, 0.35],
        [0.92, 0.82, 0.40, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.375, 0.0],
        [0.35, 0.375, 0.75],
        [0.90, 0.80, 0.38, 0.95],
        PartType::Body,
    ),
    mp(
        [0.0, 0.5, -0.35],
        [0.1, 0.15, 0.3],
        [0.88, 0.78, 0.35, 0.90],
    ),
    mp_part(
        [-0.12, 0.09375, -0.15],
        [0.1, 0.1875, 0.1],
        [0.85, 0.75, 0.32, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.12, 0.09375, -0.15],
        [0.1, 0.1875, 0.1],
        [0.85, 0.75, 0.32, 0.92],
        PartType::LeftLeg,
    ),
    mp_part(
        [-0.12, 0.09375, 0.15],
        [0.1, 0.1875, 0.1],
        [0.85, 0.75, 0.32, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.12, 0.09375, 0.15],
        [0.1, 0.1875, 0.1],
        [0.85, 0.75, 0.32, 0.92],
        PartType::LeftLeg,
    ),
];
static HORSE_MOB: [MobPart; 7] = [
    mp_part(
        [0.0, 1.25, 0.4],
        [0.35, 0.45, 0.45],
        [0.55, 0.38, 0.22, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.875, -0.1],
        [0.5, 0.625, 1.1],
        [0.52, 0.35, 0.20, 0.95],
        PartType::Body,
    ),
    mp_part(
        [-0.2, 0.1875, -0.35],
        [0.12, 0.375, 0.12],
        [0.48, 0.32, 0.18, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.2, 0.1875, -0.35],
        [0.12, 0.375, 0.12],
        [0.48, 0.32, 0.18, 0.92],
        PartType::LeftLeg,
    ),
    mp_part(
        [-0.2, 0.1875, 0.2],
        [0.12, 0.375, 0.12],
        [0.48, 0.32, 0.18, 0.92],
        PartType::RightLeg,
    ),
    mp_part(
        [0.2, 0.1875, 0.2],
        [0.12, 0.375, 0.12],
        [0.48, 0.32, 0.18, 0.92],
        PartType::LeftLeg,
    ),
    mp([0.0, 0.7, -0.65], [0.1, 0.5, 0.1], [0.35, 0.22, 0.12, 0.88]),
];
static CHICKEN_MOB: [MobPart; 4] = [
    mp_part(
        [0.0, 0.375, 0.12],
        [0.25, 0.25, 0.25],
        [0.90, 0.88, 0.85, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.25, -0.05],
        [0.3, 0.3, 0.4],
        [0.88, 0.85, 0.82, 0.95],
        PartType::Body,
    ),
    mp(
        [0.0, 0.4, 0.25],
        [0.08, 0.06, 0.12],
        [0.95, 0.75, 0.20, 0.95],
    ),
    mp(
        [0.0, 0.53, 0.1],
        [0.06, 0.08, 0.15],
        [0.85, 0.15, 0.10, 0.92],
    ),
];
static RABBIT_MOB: [MobPart; 3] = [
    mp_part(
        [0.0, 0.375, 0.12],
        [0.25, 0.25, 0.25],
        [0.75, 0.60, 0.40, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.25, -0.05],
        [0.3, 0.25, 0.35],
        [0.72, 0.58, 0.38, 0.95],
        PartType::Body,
    ),
    mp([0.0, 0.3, -0.22], [0.1, 0.1, 0.1], [0.90, 0.88, 0.85, 0.92]),
];
static SQUID_MOB: [MobPart; 2] = [
    mp_part(
        [0.0, 0.625, 0.0],
        [0.625, 0.625, 0.75],
        [0.35, 0.35, 0.55, 0.95],
        PartType::Body,
    ),
    mp_part(
        [0.0, 0.125, 0.0],
        [0.12, 0.25, 0.12],
        [0.30, 0.30, 0.50, 0.90],
        PartType::LeftLeg,
    ),
];
static BAT_MOB: [MobPart; 3] = [
    mp_part(
        [0.0, 0.25, 0.08],
        [0.2, 0.2, 0.2],
        [0.35, 0.28, 0.22, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.1875, -0.05],
        [0.25, 0.125, 0.25],
        [0.32, 0.25, 0.20, 0.95],
        PartType::Body,
    ),
    mp(
        [-0.4, 0.22, 0.0],
        [0.45, 0.03, 0.35],
        [0.30, 0.22, 0.18, 0.90],
    ),
];
static SLIME_MOB: [MobPart; 2] = [
    mp_part(
        [0.0, 0.5, 0.0],
        [0.875, 1.0, 0.875],
        [0.25, 0.75, 0.30, 0.72],
        PartType::Body,
    ),
    mp(
        [0.0, 0.5, 0.0],
        [0.625, 0.75, 0.625],
        [0.18, 0.55, 0.22, 0.60],
    ),
];
static MAGMA_CUBE_MOB: [MobPart; 2] = [
    mp_part(
        [0.0, 0.5, 0.0],
        [0.875, 1.0, 0.875],
        [0.82, 0.35, 0.10, 0.72],
        PartType::Body,
    ),
    mp(
        [0.0, 0.5, 0.0],
        [0.625, 0.75, 0.625],
        [0.72, 0.25, 0.08, 0.60],
    ),
];
static SPIDER_MOB: [MobPart; 3] = [
    mp_part(
        [0.0, 0.375, 0.0],
        [0.625, 0.375, 0.875],
        [0.32, 0.22, 0.18, 0.95],
        PartType::Body,
    ),
    mp_part(
        [0.0, 0.3125, 0.45],
        [0.4, 0.3, 0.35],
        [0.30, 0.20, 0.15, 0.95],
        PartType::Head,
    ),
    mp(
        [-0.35, 0.125, 0.0],
        [0.06, 0.25, 0.06],
        [0.28, 0.18, 0.12, 0.90],
    ),
];
static SNOW_GOLEM_MOB: [MobPart; 3] = [
    mp_part(
        [0.0, 1.5, 0.0],
        [0.5, 0.5, 0.5],
        [0.92, 0.58, 0.12, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.0, 0.0],
        [0.4, 0.5, 0.4],
        [0.92, 0.94, 0.96, 0.95],
        PartType::Body,
    ),
    mp_part(
        [0.0, 0.375, 0.0],
        [0.5, 0.75, 0.5],
        [0.88, 0.90, 0.92, 0.95],
        PartType::Body,
    ),
];
static GHAST_MOB: [MobPart; 2] = [
    mp_part(
        [0.0, 2.0, 0.0],
        [2.0, 2.0, 2.0],
        [0.88, 0.88, 0.92, 0.88],
        PartType::Body,
    ),
    mp(
        [-0.6, 0.5, 0.0],
        [0.25, 0.75, 0.25],
        [0.85, 0.85, 0.90, 0.82],
    ),
];
static BLAZE_MOB: [MobPart; 3] = [
    mp_part(
        [0.0, 1.75, 0.0],
        [0.5, 0.5, 0.5],
        [0.88, 0.72, 0.15, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 1.25, 0.0],
        [0.5, 0.625, 0.5],
        [0.85, 0.68, 0.12, 0.95],
        PartType::Body,
    ),
    mp([0.0, 1.4, 0.0], [0.8, 0.06, 0.06], [0.92, 0.78, 0.20, 0.88]),
];
static SILVERFISH_MOB: [MobPart; 2] = [
    mp_part(
        [0.0, 0.15, 0.1],
        [0.18, 0.18, 0.2],
        [0.52, 0.50, 0.48, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.125, -0.05],
        [0.2, 0.15, 0.35],
        [0.50, 0.48, 0.45, 0.95],
        PartType::Body,
    ),
];
static ENDERMITE_MOB: [MobPart; 2] = [
    mp_part(
        [0.0, 0.1875, 0.05],
        [0.2, 0.2, 0.2],
        [0.18, 0.12, 0.22, 0.95],
        PartType::Head,
    ),
    mp_part(
        [0.0, 0.125, -0.05],
        [0.22, 0.15, 0.3],
        [0.15, 0.10, 0.20, 0.95],
        PartType::Body,
    ),
];
static TNT_MOB: [MobPart; 2] = [
    mp(
        [0.0, 0.49, 0.0],
        [0.98, 0.98, 0.98],
        [0.82, 0.18, 0.12, 0.95],
    ),
    mp(
        [0.0, 0.88, 0.0],
        [0.98, 0.15, 0.98],
        [0.90, 0.90, 0.85, 0.85],
    ),
];

pub fn mob_parts(entity_type: EntityType) -> &'static [MobPart] {
    match entity_type {
        EntityType::Zombie => &ZOMBIE,
        EntityType::Skeleton => &SKELETON,
        EntityType::PigZombie => &PIGZOMBIE,
        EntityType::Witch => &WITCH_MOB,
        EntityType::Enderman => &ENDERMAN_MOB,
        EntityType::Creeper => &CREEPER_MOB,
        EntityType::Pig => &PIG_MOB,
        EntityType::Cow | EntityType::Mooshroom => &COW_MOB,
        EntityType::Sheep => &SHEEP_MOB,
        EntityType::Wolf => &WOLF_MOB,
        EntityType::Ocelot => &OZELOT_MOB,
        EntityType::Horse => &HORSE_MOB,
        EntityType::Chicken => &CHICKEN_MOB,
        EntityType::Rabbit => &RABBIT_MOB,
        EntityType::Squid => &SQUID_MOB,
        EntityType::Bat => &BAT_MOB,
        EntityType::Slime => &SLIME_MOB,
        EntityType::LavaSlime => &MAGMA_CUBE_MOB,
        EntityType::Spider | EntityType::CaveSpider => &SPIDER_MOB,
        EntityType::SnowMan => &SNOW_GOLEM_MOB,
        EntityType::Ghast => &GHAST_MOB,
        EntityType::Blaze => &BLAZE_MOB,
        EntityType::Silverfish => &SILVERFISH_MOB,
        EntityType::Endermite => &ENDERMITE_MOB,
        EntityType::PrimedTnt | EntityType::FallingBlock => &TNT_MOB,
        _ => &[],
    }
}

/// Draw a 3D mob model at the given screen position.
/// Renders front/top/side faces of each cuboid for true 3D appearance.
pub fn draw_mob_model(
    builder: &mut GuiVertexBuilder,
    parts: &[MobPart],
    sx: f32,
    sy: f32,
    _body_w: f32,
    body_h: f32,
    gs: f32,
    yaw_deg: f32,
    hurt_alpha: f32,
    swing_alpha: f32,
) {
    if parts.is_empty() {
        return;
    }

    let total_height = parts
        .iter()
        .map(|p| (p.offset[1] + p.size[1]).abs())
        .fold(0.0f32, f32::max)
        .max(0.5);
    let scale = body_h / total_height;

    // Yaw rotation factor: how much of the Z-depth is visible as width offset
    let yaw_rad = yaw_deg.to_radians();
    let cos_yaw = yaw_rad.cos();
    let sin_yaw = yaw_rad.sin();

    // Limb swing animation (radians)
    let swing = swing_alpha * 0.9;

    for part in parts {
        let (offset_x, offset_y, size_x, size_y, size_z) = (
            part.offset[0],
            part.offset[1],
            part.size[0],
            part.size[1],
            part.size[2],
        );

        // Apply limb animation offsets
        let (anim_offset_x, anim_size_z) = match part.part_type {
            PartType::RightArm | PartType::LeftLeg => {
                let s = if part.part_type == PartType::RightArm {
                    swing
                } else {
                    -swing
                };
                let dz = s.sin() * size_z * 0.5;
                (dz * 0.3, size_z * 0.5 + dz.abs() * 0.5)
            }
            PartType::LeftArm | PartType::RightLeg => {
                let s = if part.part_type == PartType::LeftArm {
                    -swing
                } else {
                    swing
                };
                let dz = s.sin() * size_z * 0.5;
                (-dz * 0.3, size_z * 0.5 + dz.abs() * 0.5)
            }
            _ => (0.0, size_z),
        };

        let px_base = sx + (offset_x + anim_offset_x) * scale;
        let py_base = sy - offset_y * scale;
        let pw = size_x * scale;
        let ph = size_y * scale;
        let pz = anim_size_z * scale;

        // Yaw-based depth offset: parts further in Z appear offset in X
        let depth_offset_x = size_z * 0.5 * sin_yaw * scale * 0.5;

        let mut color = part.color;
        let fade = 1.0f32.max(0.35);
        color[0] *= fade;
        color[1] *= fade;
        color[2] *= fade;

        // === FRONT FACE (main visible face) ===
        let front_x = px_base - pw * 0.5 + depth_offset_x;
        let front_y = py_base - ph;
        let front_color = color;
        builder.fill_rect(front_x, front_y, pw, ph, front_color);
        // Dark outline
        builder.fill_rect(
            front_x - 0.5 * gs,
            front_y - 0.5 * gs,
            pw + 1.0 * gs,
            ph + 1.0 * gs,
            [0.0, 0.0, 0.0, color[3] * 0.3],
        );
        builder.fill_rect(front_x, front_y, pw, ph, front_color);

        // === TOP FACE (visible when looking down at entity) ===
        let top_vis = cos_yaw * 0.6 + 0.4; // more visible when facing camera
        if top_vis > 0.1 {
            let top_color = [
                color[0] * (0.85 + top_vis * 0.2),
                color[1] * (0.85 + top_vis * 0.2),
                color[2] * (0.85 + top_vis * 0.2),
                color[3] * top_vis,
            ];
            // Parallelogram for perspective top
            let top_h = pz * 0.35 * top_vis;
            let top_x_off = -pz * 0.2 * sin_yaw;
            builder.fill_rect(
                front_x + top_x_off,
                front_y - top_h,
                pw,
                top_h.max(1.0 * gs),
                top_color,
            );
        }

        // === SIDE FACE (visible from yaw rotation) ===
        let side_vis = sin_yaw.abs();
        if side_vis > 0.15 {
            let side_w = pz * 0.4 * side_vis;
            let side_x = if sin_yaw > 0.0 {
                front_x + pw
            } else {
                front_x - side_w
            };
            let side_color = [
                color[0] * 0.72,
                color[1] * 0.72,
                color[2] * 0.72,
                color[3] * side_vis,
            ];
            builder.fill_rect(side_x, front_y, side_w.max(1.0 * gs), ph, side_color);
        }

        // Highlight line on top edge
        builder.fill_rect(
            front_x,
            front_y,
            pw,
            1.0 * gs.max(0.5),
            [
                color[0] * 1.2,
                color[1] * 1.2,
                color[2] * 1.2,
                color[3] * 0.4,
            ],
        );

        // Hurt flash
        if hurt_alpha > 0.0 {
            builder.fill_rect(
                front_x - 1.0 * gs,
                front_y - 1.0 * gs,
                pw + 2.0 * gs,
                ph + 2.0 * gs,
                [1.0, 0.08, 0.04, 0.22 * hurt_alpha],
            );
        }
    }
}
