//! First-person animation state shared by the client, scripting runtime, and mesh builders.
//!
//! Matrices use column vectors (`projection * view * model * position`). Script matrices are
//! pre-multiplied onto the completed vanilla view-space model, so Lua calls affect the model in
//! the same order they were issued without exposing renderer or Vulkan state.

use nalgebra::{Matrix4, Rotation3, Vector3};

/// Classification of the held item for animation routing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemType {
    #[default]
    Empty,
    Sword,
    Tool,
    Bow,
    Crossbow,
    FishingRod,
    Food,
    Drink,
    Map,
    Shield,
    Block,
    Generic,
}

impl ItemType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Sword => "sword",
            Self::Tool => "tool",
            Self::Bow => "bow",
            Self::Crossbow => "crossbow",
            Self::FishingRod => "fishing_rod",
            Self::Food => "food",
            Self::Drink => "drink",
            Self::Map => "map",
            Self::Shield => "shield",
            Self::Block => "block",
            Self::Generic => "generic",
        }
    }

    pub fn classify(item_id: u16, use_kind: u8) -> Self {
        match use_kind {
            1 => Self::Sword,
            2 => Self::Food,
            3 => Self::Drink,
            4 => Self::Bow,
            _ => match item_id {
                0 => Self::Empty,
                267 | 268 | 272 | 276 | 283 => Self::Sword,
                261 => Self::Bow,
                346 => Self::FishingRod,
                358 => Self::Map,
                id if id >= 256 && id <= 422 => Self::Block,
                id if is_tool(id) => Self::Tool,
                _ => Self::Generic,
            },
        }
    }
}

fn is_tool(item_id: u16) -> bool {
    matches!(
        item_id,
        256..=259   // shovel
            | 269..=271 // shovel
            | 273..=275 // pickaxe
            | 277..=279 // shovel
            | 284..=286 // axe
            | 290..=294 // hoe
            | 298..=302 // chain/iron/diamond/gold axes
            | 309..=313 // hoes
            | 314..=317 // golden/chain axe
    )
}

/// What the player is doing with the held item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UseAction {
    #[default]
    None,
    Block,
    Eat,
    Drink,
    Bow,
    Spear,
    Crossbow,
    Use,
}

impl UseAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Block => "block",
            Self::Eat => "eat",
            Self::Drink => "drink",
            Self::Bow => "bow",
            Self::Spear => "spear",
            Self::Crossbow => "crossbow",
            Self::Use => "use",
        }
    }

    pub fn from_use_kind(kind: u8) -> Self {
        match kind {
            1 => Self::Block,
            2 => Self::Eat,
            3 => Self::Drink,
            4 => Self::Bow,
            _ => Self::None,
        }
    }
}

/// Re-equip animation policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReequipPolicy {
    #[default]
    Vanilla,
    Always,
    SkipSameItem,
    SkipSameSlot,
    Never,
}

impl ReequipPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Always => "always",
            Self::SkipSameItem => "skip_same_item",
            Self::SkipSameSlot => "skip_same_slot",
            Self::Never => "never",
        }
    }
}

/// Vanilla transform stage controls.
#[derive(Clone, Debug)]
pub struct VanillaTransformFlags {
    pub base: bool,
    pub equip: bool,
    pub swing: bool,
    pub use_transform: bool,
    pub block_transform: bool,
    pub bow_transform: bool,
    pub eat_drink_transform: bool,
    pub bob: bool,
}

impl Default for VanillaTransformFlags {
    fn default() -> Self {
        Self {
            base: true,
            equip: true,
            swing: true,
            use_transform: true,
            block_transform: true,
            bow_transform: true,
            eat_drink_transform: true,
            bob: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    MainHand,
    OffHand,
}

impl Hand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MainHand => "main_hand",
            Self::OffHand => "off_hand",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FirstPersonAnimationContext {
    pub hand: Hand,
    pub item_id: String,
    pub numeric_item_id: u16,
    pub item_type: ItemType,
    pub use_action: UseAction,
    pub equip_progress: f32,
    pub previous_equip_progress: f32,
    pub swing_progress: f32,
    pub previous_swing_progress: f32,
    pub swinging: bool,
    pub swing_duration_ticks: u16,
    pub use_progress: f32,
    pub use_ticks: u32,
    pub remaining_use_ticks: u32,
    pub max_use_ticks: u32,
    pub attack_cooldown: f32,
    pub using_item: bool,
    pub blocking: bool,
    pub attack_pressed: bool,
    pub attack_held: bool,
    pub use_pressed: bool,
    pub use_held: bool,
    pub sneaking: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub partial_tick: f32,
    pub fov: f32,
    pub aspect_ratio: f32,
}

/// Mutable overrides written by Lua calculate setters.
#[derive(Clone, Debug, Default)]
pub struct AnimationOverrides {
    pub swing_progress: Option<f32>,
    pub equip_progress: Option<f32>,
    pub use_progress: Option<f32>,
    pub swinging: Option<bool>,
    pub swing_duration_ticks: Option<u16>,
    pub blocking: Option<bool>,
    pub using_item: Option<bool>,
    pub reequip_policy: Option<ReequipPolicy>,
    pub equip_speed: Option<f32>,
    pub vanilla: VanillaTransformFlags,
}

impl AnimationOverrides {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug)]
pub struct FirstPersonTransforms {
    pub shared: Matrix4<f32>,
    pub arm: Matrix4<f32>,
    pub item: Matrix4<f32>,
    pub vanilla_flags: VanillaTransformFlags,
}

/// Immutable inputs consumed by first-person arm and held-item mesh builders.
///
/// Keeping the pose together prevents renderer state from leaking into the mesh modules and makes
/// the vanilla transformation chain share one explicit contract.
#[derive(Clone, Debug)]
pub struct FirstPersonPose {
    pub swing_progress: f32,
    pub equip_progress: f32,
    pub render_arm_pitch: f32,
    pub render_arm_yaw: f32,
    /// 0=idle, 1=block, 2=eat, 3=drink, 4=bow draw.
    pub use_kind: u8,
    /// Seconds spent using the active item.
    pub use_progress: f32,
    pub script_transform: Matrix4<f32>,
    pub vanilla_flags: VanillaTransformFlags,
    pub glint: bool,
}

impl Default for FirstPersonTransforms {
    fn default() -> Self {
        Self {
            shared: Matrix4::identity(),
            arm: Matrix4::identity(),
            item: Matrix4::identity(),
            vanilla_flags: VanillaTransformFlags::default(),
        }
    }
}

impl FirstPersonTransforms {
    pub fn combined_arm(&self) -> Matrix4<f32> {
        self.arm * self.shared
    }

    pub fn combined_item(&self) -> Matrix4<f32> {
        self.item * self.shared
    }
}

pub fn apply_script_transform(base: Matrix4<f32>, script: &Matrix4<f32>) -> Matrix4<f32> {
    // Scripted first-person transforms operate in held-model space.  Prepending
    // them rotates the whole camera-space placement vector, which turns a
    // simple sword yaw into screen-space drift and perspective scaling.
    base * script
}

pub fn arm_tracking_transform(
    camera: &crate::client::player::Camera,
    render_arm_pitch: f32,
    render_arm_yaw: f32,
) -> Matrix4<f32> {
    let pitch_delta = crate::util::wrap_degrees(camera.mc_pitch_degrees() - render_arm_pitch) * 0.1;
    let yaw_delta = crate::util::wrap_degrees(camera.mc_yaw_degrees() - render_arm_yaw) * 0.1;
    Rotation3::from_axis_angle(&Vector3::x_axis(), pitch_delta.to_radians()).to_homogeneous()
        * Rotation3::from_axis_angle(&Vector3::y_axis(), yaw_delta.to_radians()).to_homogeneous()
}

pub fn item_resource_id(item_id: u16, damage: u16) -> String {
    if item_id <= 255 {
        if let Some(name) = crate::world::block_models::block_id_to_name(item_id) {
            return format!("minecraft:{name}");
        }
    }
    if let Some(path) = crate::render::item_icons::item_icon_path(item_id, damage) {
        let name = path.rsplit('/').next().unwrap_or(path);
        return format!("minecraft:{name}");
    }
    format!("minecraft:legacy_item_{item_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_transform_is_applied_after_shared_transform() {
        let transforms = FirstPersonTransforms {
            shared: Matrix4::new_translation(&Vector3::new(1.0, 0.0, 0.0)),
            arm: nalgebra::Scale3::new(2.0, 2.0, 2.0).to_homogeneous(),
            item: Matrix4::identity(),
            vanilla_flags: VanillaTransformFlags::default(),
        };
        let point = transforms.combined_arm() * nalgebra::Vector4::new(0.0, 0.0, 0.0, 1.0);
        assert_eq!([point.x, point.y, point.z], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn script_transform_preserves_the_held_item_position() {
        let base = Matrix4::new_translation(&Vector3::new(0.56, -0.52, -0.72));
        let script =
            Rotation3::from_axis_angle(&Vector3::y_axis(), 45.0_f32.to_radians()).to_homogeneous();
        let point =
            apply_script_transform(base, &script) * nalgebra::Vector4::new(0.0, 0.0, 0.0, 1.0);

        assert!((point.x - 0.56).abs() < 1.0e-5);
        assert!((point.y + 0.52).abs() < 1.0e-5);
        assert!((point.z + 0.72).abs() < 1.0e-5);
    }
}
