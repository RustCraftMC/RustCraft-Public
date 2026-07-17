use crate::client::inventory::{max_damage, ItemStackView};
use crate::client::physics::{self, BlockFace, BlockPos, RaycastHit};
use crate::client::player::Camera;
use crate::entity::EntityManager;
use crate::world::{block::Block, World};

#[derive(Clone, Copy, Debug)]
pub struct SelectionBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct TargetBlock {
    pub hit: RaycastHit,
    pub boxes: Vec<SelectionBox>,
}

#[derive(Clone, Copy, Debug)]
pub struct TargetEntity {
    pub entity_id: i32,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DigUpdate {
    pub start: Option<RaycastHit>,
    pub cancel: Option<RaycastHit>,
    pub finish: Option<RaycastHit>,
    pub hit_particle: Option<RaycastHit>,
    /// Vanilla plays the block's step sound on the first damage tick and then
    /// every four ticks while the player keeps mining.
    pub hit_sound: Option<BlockPos>,
}

pub struct DigController {
    active: Option<RaycastHit>,
    active_item: Option<ItemStackView>,
    progress: f32,
    ticks: u32,
}

impl DigController {
    pub fn new() -> Self {
        Self {
            active: None,
            active_item: None,
            progress: 0.0,
            ticks: 0,
        }
    }

    pub fn tick(
        &mut self,
        hit: Option<RaycastHit>,
        attacking: bool,
        creative: bool,
        per_tick_progress: f32,
        held_item: &ItemStackView,
    ) -> DigUpdate {
        let mut update = DigUpdate::default();
        let Some(hit) = hit else {
            update.cancel = self.cancel();
            return update;
        };

        if !attacking {
            update.cancel = self.cancel();
            return update;
        }

        let same_position = self.active.is_some_and(|active| active.pos == hit.pos);
        let same_item = self
            .active_item
            .as_ref()
            .is_some_and(|active| same_vanilla_hitting_item(active, held_item));
        if !same_position || !same_item {
            update.cancel = self.cancel();
            self.active = Some(hit);
            self.active_item = Some(held_item.clone());
            self.progress = 0.0;
            self.ticks = 0;
            update.start = Some(hit);
        } else {
            // The face is taken from the current ray for STOP/feedback, but
            // vanilla keeps mining the same block when the cursor crosses a
            // different face of that block.
            self.active = Some(hit);
        }

        // Vanilla getPlayerRelativeBlockHardness returns 0 for unbreakable
        // blocks (hardness < 0), so digging never progresses.
        if per_tick_progress <= 0.0 {
            self.progress = 0.0;
            return update;
        }

        self.ticks = self.ticks.saturating_add(1);
        if creative {
            self.progress = 1.0;
        } else {
            self.progress += per_tick_progress;
        }

        if self.ticks % 4 == 0 {
            update.hit_particle = Some(hit);
        }
        if self.ticks % 4 == 1 {
            update.hit_sound = Some(hit.pos);
        }

        if self.progress >= 1.0 {
            update.finish = Some(hit);
            self.active = None;
            self.active_item = None;
            self.progress = 0.0;
            self.ticks = 0;
        }

        update
    }

    pub fn cancel(&mut self) -> Option<RaycastHit> {
        let old = self.active.take();
        self.active_item = None;
        self.progress = 0.0;
        self.ticks = 0;
        old
    }

    pub fn progress(&self) -> f32 {
        self.progress.clamp(0.0, 1.0)
    }

    pub fn active_pos(&self) -> Option<BlockPos> {
        self.active.map(|hit| hit.pos)
    }
}

pub fn target_block(world: &World, camera: &Camera, max_dist: f32) -> Option<TargetBlock> {
    let hit = physics::raycast(&camera.position, &camera.front, max_dist, world)?;
    let block = world.get_block(hit.pos.0, hit.pos.1, hit.pos.2);
    if block == Block::Air || block.is_liquid() {
        return None;
    }

    let boxes = selection_boxes(world, hit.pos, block);
    if boxes.is_empty() {
        return None;
    }

    Some(TargetBlock { hit, boxes })
}

pub fn target_hit(world: &World, camera: &Camera, max_dist: f32) -> Option<RaycastHit> {
    target_hit_from(world, camera.position, camera.front, max_dist)
}

/// Gameplay interaction must originate at the entity eye position rather than
/// the rendered camera, which may be interpolated or third-person offset.
pub fn target_hit_from(
    world: &World,
    origin: nalgebra::Point3<f32>,
    direction: nalgebra::Vector3<f32>,
    max_dist: f32,
) -> Option<RaycastHit> {
    physics::raycast(&origin, &direction, max_dist, world)
}

pub fn target_entity(
    entities: &EntityManager,
    camera: &Camera,
    max_dist: f32,
    skip_entity_id: Option<i32>,
) -> Option<TargetEntity> {
    target_entity_from(
        entities,
        camera.position,
        camera.front,
        max_dist,
        skip_entity_id,
    )
}

pub fn target_entity_from(
    entities: &EntityManager,
    origin: nalgebra::Point3<f32>,
    direction: nalgebra::Vector3<f32>,
    max_dist: f32,
    skip_entity_id: Option<i32>,
) -> Option<TargetEntity> {
    let direction = direction.normalize();
    entities
        .entities
        .values()
        .filter(|entity| Some(entity.entity_id) != skip_entity_id)
        .filter(|entity| entity.entity_type.can_be_collided_with())
        .filter_map(|entity| {
            let (width, height) = entity.entity_type.bounding_box();
            let half = width * 0.5 + 0.12;
            let min = [
                entity.position.x - half,
                entity.position.y,
                entity.position.z - half,
            ];
            let max = [
                entity.position.x + half,
                entity.position.y + height + 0.12,
                entity.position.z + half,
            ];
            ray_aabb_distance(origin.coords, direction, min, max)
                .filter(|distance| *distance <= max_dist)
                .map(|distance| TargetEntity {
                    entity_id: entity.entity_id,
                    distance,
                })
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}

fn selection_boxes(world: &World, pos: BlockPos, block: Block) -> Vec<SelectionBox> {
    if matches!(
        block,
        Block::Chest | Block::TrappedChest | Block::EnderChest
    ) {
        let (from, to) = crate::client::physics::chest_bounds(world, block, pos.0, pos.1, pos.2);
        return vec![SelectionBox {
            min: [
                pos.0 as f32 + from[0] / 16.0,
                pos.1 as f32 + from[1] / 16.0,
                pos.2 as f32 + from[2] / 16.0,
            ],
            max: [
                pos.0 as f32 + to[0] / 16.0,
                pos.1 as f32 + to[1] / 16.0,
                pos.2 as f32 + to[2] / 16.0,
            ],
        }];
    }
    let state = world.get_block_state(pos.0, pos.1, pos.2);
    let elements = crate::world::shape::block_elements(
        block,
        state,
        0,
        0,
        0,
        |dx, dy, dz| world.get_block(pos.0 + dx, pos.1 + dy, pos.2 + dz),
        |dx, dy, dz| world.get_block_state(pos.0 + dx, pos.1 + dy, pos.2 + dz),
    );

    if let Some(elements) = elements {
        elements
            .into_iter()
            .map(|mut element| {
                // Fence post extends to 1.5 blocks tall for selection outline
                if matches!(block, Block::OakFence | Block::NetherBrickFence) {
                    if element.from[0] >= 5.0
                        && element.from[0] <= 7.0
                        && element.to[0] >= 9.0
                        && element.to[0] <= 11.0
                        && element.from[2] >= 5.0
                        && element.from[2] <= 7.0
                        && element.to[2] >= 9.0
                        && element.to[2] <= 11.0
                        && element.to[1] >= 14.0
                    {
                        element.to[1] = 24.0;
                    }
                }
                SelectionBox {
                    min: [
                        pos.0 as f32 + element.from[0] / 16.0,
                        pos.1 as f32 + element.from[1] / 16.0,
                        pos.2 as f32 + element.from[2] / 16.0,
                    ],
                    max: [
                        pos.0 as f32 + element.to[0] / 16.0,
                        pos.1 as f32 + element.to[1] / 16.0,
                        pos.2 as f32 + element.to[2] / 16.0,
                    ],
                }
            })
            .collect()
    } else {
        vec![SelectionBox {
            min: [pos.0 as f32, pos.1 as f32, pos.2 as f32],
            max: [pos.0 as f32 + 1.0, pos.1 as f32 + 1.0, pos.2 as f32 + 1.0],
        }]
    }
}

fn ray_aabb_distance(
    origin: nalgebra::Vector3<f32>,
    direction: nalgebra::Vector3<f32>,
    min: [f32; 3],
    max: [f32; 3],
) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = f32::MAX;

    for axis in 0..3 {
        let origin_axis = origin[axis];
        let dir_axis = direction[axis];
        let min_axis = min[axis];
        let max_axis = max[axis];

        if dir_axis.abs() < 1.0e-6 {
            if origin_axis < min_axis || origin_axis > max_axis {
                return None;
            }
            continue;
        }

        let inv = 1.0 / dir_axis;
        let mut t1 = (min_axis - origin_axis) * inv;
        let mut t2 = (max_axis - origin_axis) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }

    (t_max >= 0.0).then_some(t_min.max(0.0))
}

fn same_vanilla_hitting_item(current: &ItemStackView, held: &ItemStackView) -> bool {
    if current.is_empty() || held.is_empty() {
        return current.is_empty() && held.is_empty();
    }
    if current.item_id != held.item_id || current.nbt != held.nbt {
        return false;
    }

    // ItemStack.areItemStacksEqual in PlayerControllerMP ignores damage for
    // damageable items, while non-damageable items must keep the same metadata.
    max_damage(current.item_id) > 0 || current.damage == held.damage
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(face: BlockFace) -> RaycastHit {
        RaycastHit {
            pos: (1, 2, 3),
            face,
            distance: 2.0,
        }
    }

    fn item(item_id: u16, damage: u16) -> ItemStackView {
        ItemStackView {
            item_id,
            count: 1,
            damage,
            nbt: None,
        }
    }

    #[test]
    fn changing_face_on_the_same_block_keeps_dig_progress() {
        let held = item(278, 0);
        let mut dig = DigController::new();

        let first = dig.tick(Some(hit(BlockFace::North)), true, false, 0.6, &held);
        assert!(first.start.is_some());

        let second = dig.tick(Some(hit(BlockFace::South)), true, false, 0.6, &held);
        assert!(second.cancel.is_none());
        assert!(second.start.is_none());
        assert_eq!(second.finish.unwrap().face, BlockFace::South);
    }

    #[test]
    fn changing_the_held_item_restarts_digging() {
        let mut dig = DigController::new();
        let diamond_pickaxe = item(278, 0);
        let iron_pickaxe = item(257, 0);

        dig.tick(
            Some(hit(BlockFace::North)),
            true,
            false,
            0.6,
            &diamond_pickaxe,
        );
        let update = dig.tick(Some(hit(BlockFace::North)), true, false, 0.6, &iron_pickaxe);

        assert!(update.cancel.is_some());
        assert!(update.start.is_some());
        assert!(update.finish.is_none());
    }

    #[test]
    fn damageable_item_wear_does_not_restart_digging() {
        assert!(same_vanilla_hitting_item(&item(278, 10), &item(278, 11)));
        assert!(!same_vanilla_hitting_item(&item(1, 0), &item(1, 1)));
    }
}
