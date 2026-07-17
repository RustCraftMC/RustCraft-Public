use super::keybind::{Action, InputState};
use super::physics::*;
use crate::world::World;
use nalgebra::{Matrix4, Point3, Vector3};

// MC 1.8.9 movement constants
const WALK_SPEED: f32 = 0.1;
const FLY_SPEED: f32 = 0.05;
const JUMP_VELOCITY: f32 = 0.42;
const SPRINT_JUMP_BOOST: f32 = 0.2;
const GRAVITY: f64 = 0.08;
const AIR_FRICTION: f32 = 0.91;
const AIR_ACCELERATION: f32 = 0.02;
const SPRINT_AIR_ACCELERATION: f32 = 0.026;
const GROUND_FRICTION_FACTOR: f32 = 0.16277136;

/// Vanilla Minecraft's sin/cos lookup table. MathHelper.sin and cos use a
/// 65536-entry table indexed by `(int)(value * 10430.378F) & 65535`, which
/// produces slightly different float values than std::sin/cos.  GrimAC
/// replays vanilla's exact arithmetic, so we must match bit-for-bit.
const SIN_TABLE_SIZE: usize = 65536;

fn sin_table() -> &'static [f32; SIN_TABLE_SIZE] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[f32; SIN_TABLE_SIZE]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0.0f32; SIN_TABLE_SIZE];
        let mut i = 0;
        while i < SIN_TABLE_SIZE {
            table[i] = (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32;
            i += 1;
        }
        table
    })
}

fn vanilla_sin(value: f32) -> f32 {
    let index = (value * 10430.378) as i32;
    sin_table()[(index as usize) & 65535]
}

fn vanilla_cos(value: f32) -> f32 {
    let index = (value * 10430.378 + 16384.0) as i32;
    sin_table()[(index as usize) & 65535]
}

fn vanilla_yaw_sin_cos(mc_yaw_degrees: f32) -> (f32, f32) {
    // Entity.moveFlying evaluates rotationYaw * (float)Math.PI / 180.0F in
    // this order. Derive it from the same yaw sent in C05/C06 so movement and
    // server prediction cannot choose adjacent LUT entries after a radian
    // round trip.
    let angle = mc_yaw_degrees * std::f32::consts::PI / 180.0_f32;
    (vanilla_sin(angle), vanilla_cos(angle))
}

/// Vanilla Entity.moveFlying derives horizontal acceleration from yaw alone.
/// Keeping this separate from Camera::front prevents vertical look pitch from
/// changing ground movement.
fn horizontal_movement_basis(mc_yaw_degrees: f32) -> (Vector3<f32>, Vector3<f32>) {
    let (sin_yaw, cos_yaw) = vanilla_yaw_sin_cos(mc_yaw_degrees);
    (
        Vector3::new(-sin_yaw, 0.0, cos_yaw),
        Vector3::new(-cos_yaw, 0.0, -sin_yaw),
    )
}

// EntityLivingBase's synthetic movement-speed modifiers.  S20 supplies the
// server-owned attribute snapshot, while effects and the sprint flag are
// maintained locally by the vanilla client entity.
const SPEED_POTION_MODIFIER: &str = "91AEAA56-376B-4498-935B-2F7F68070635";
const SLOWNESS_POTION_MODIFIER: &str = "7107DE5E-7CE8-4030-940E-514C1F160890";
const SPRINTING_SPEED_MODIFIER: &str = "662A6B8D-DA3E-4C1C-8813-96EA6097278D";

use crate::util::wrap_degrees;

// --- Camera (unchanged) ---

pub struct Frustum {
    planes: [[f32; 4]; 6],
}
impl Frustum {
    pub fn from_view_proj(vp: &Matrix4<f32>) -> Self {
        // nalgebra stores column-major; as_slice() returns in column-major order:
        //   col 0: m[0..4]  = elements (row 0, col 0), (row 1, col 0), (row 2, col 0), (row 3, col 0)
        //   col 1: m[4..8]
        //   col 2: m[8..12]
        //   col 3: m[12..16]
        //
        // clip_i = row_i · p = Σ P[i][j] * p[j]  (i = output component, j = input component)
        // In column-major storage, P[i][j] = m[i + j*4] (i = row, j = column).
        //
        // clip_x = row0 · p → uses m[0], m[4], m[8],  m[12]
        // clip_y = row1 · p → uses m[1], m[5], m[9],  m[13]
        // clip_z = row2 · p → uses m[2], m[6], m[10], m[14]
        // clip_w = row3 · p → uses m[3], m[7], m[11], m[15]
        //
        // Frustum planes (inside = dot(normal, point) + D >= 0):
        //   left:   x_ndc >= -1 → clip_x + clip_w >= 0  → (row0 + row3)·p >= 0
        //   right:  x_ndc <=  1 → clip_w - clip_x >= 0  → (row3 - row0)·p >= 0
        //   bottom: y_ndc >= -1 → clip_y + clip_w >= 0  → (row1 + row3)·p >= 0
        //   top:    y_ndc <=  1 → clip_w - clip_y >= 0  → (row3 - row1)·p >= 0
        // Vulkan's depth range is 0..1, unlike OpenGL's -1..1.
        //   near:   z_ndc >=  0 → clip_z >= 0           → row2·p >= 0
        //   far:    z_ndc <=  1 → clip_w - clip_z >= 0  → (row3 - row2)·p >= 0
        let m = vp.as_slice();
        Frustum {
            planes: [
                // left:   row 3 + row 0
                normalize_plane([m[3] + m[0], m[7] + m[4], m[11] + m[8], m[15] + m[12]]),
                // right:  row 3 - row 0
                normalize_plane([m[3] - m[0], m[7] - m[4], m[11] - m[8], m[15] - m[12]]),
                // bottom: row 3 + row 1
                normalize_plane([m[3] + m[1], m[7] + m[5], m[11] + m[9], m[15] + m[13]]),
                // top:    row 3 - row 1
                normalize_plane([m[3] - m[1], m[7] - m[5], m[11] - m[9], m[15] - m[13]]),
                // near: row 2
                normalize_plane([m[2], m[6], m[10], m[14]]),
                // far:    row 3 - row 2
                normalize_plane([m[3] - m[2], m[7] - m[6], m[11] - m[10], m[15] - m[14]]),
            ],
        }
    }
    pub fn test_aabb(&self, min: [f32; 3], max: [f32; 3]) -> bool {
        for p in &self.planes {
            let px = if p[0] >= 0.0 { max[0] } else { min[0] };
            let py = if p[1] >= 0.0 { max[1] } else { min[1] };
            let pz = if p[2] >= 0.0 { max[2] } else { min[2] };
            if p[0] * px + p[1] * py + p[2] * pz + p[3] < 0.0 {
                return false;
            }
        }
        true
    }
}
fn normalize_plane(p: [f32; 4]) -> [f32; 4] {
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    if len < 1e-10 {
        return p;
    }
    [p[0] / len, p[1] / len, p[2] / len, p[3] / len]
}

#[derive(Clone)]
pub struct Camera {
    pub position: Point3<f32>,
    pub front: Vector3<f32>,
    pub right: Vector3<f32>,
    pub up: Vector3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    /// Third-person front view looks opposite the movement/look direction.
    pub reverse_view: bool,
    /// FOV multiplier (1.0 = normal, 60/70 ≈ 0.857 when underwater).
    pub fov_modifier: f32,
    /// Previous tick's fov_modifier, for smooth frame interpolation.
    pub prev_fov_modifier: f32,
    /// Frame interpolation fraction (0.0–1.0, set before each render).
    pub partial_tick: f32,
    /// First-person camera effects, updated by `Player` from vanilla-style limb
    /// animation and hurt timers.  Kept on the camera so every world pass uses
    /// the same transformed view matrix.
    pub view_bobbing: bool,
    pub bob_phase: f32,
    pub bob_amount: f32,
    pub bob_pitch: f32,
    pub hurt_time: f32,
    pub prev_hurt_time: f32,
    pub hurt_cam_enabled: bool,
    pub fov_change_enabled: bool,
}
impl Camera {
    pub fn new(position: Point3<f32>, aspect: f32) -> Self {
        let mut c = Camera {
            position,
            front: Vector3::new(0.0, 0.0, -1.0),
            right: Vector3::new(1.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            yaw: -90.0f32.to_radians(),
            pitch: -30.0f32.to_radians(),
            fov: 80.0,
            aspect,
            near: 0.1,
            far: 500.0,
            reverse_view: false,
            fov_modifier: 1.0,
            prev_fov_modifier: 1.0,
            partial_tick: 0.0,
            view_bobbing: false,
            bob_phase: 0.0,
            bob_amount: 0.0,
            bob_pitch: 0.0,
            hurt_time: 0.0,
            prev_hurt_time: 0.0,
            hurt_cam_enabled: true,
            fov_change_enabled: true,
        };
        c.update_vectors();
        c
    }
    pub fn process_mouse(&mut self, dx: f32, dy: f32, sensitivity: f32, invert_y: bool) {
        // EntityRenderer.updateCameraAndRender in 1.8.9 uses
        // `(sensitivity * 0.6 + 0.2)^3 * 8`.  Preserve the previous 0.5
        // default turn rate and scale it by that vanilla curve.
        let vanilla = (sensitivity.clamp(0.0, 1.0) * 0.6 + 0.2).powi(3) * 8.0;
        let default_vanilla = (0.5_f32 * 0.6 + 0.2).powi(3) * 8.0;
        let scale = 0.003 * vanilla / default_vanilla;
        self.yaw += dx * scale;
        let pitch_delta = if invert_y { dy * scale } else { -dy * scale };
        self.pitch = (self.pitch + pitch_delta).clamp(-89.0f32.to_radians(), 89.0f32.to_radians());
        self.update_vectors();
    }
    fn update_vectors(&mut self) {
        self.front = Vector3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize();
        self.right = self.front.cross(&Vector3::new(0.0, 1.0, 0.0)).normalize();
        self.up = self.right.cross(&self.front).normalize();
    }
    pub fn view_matrix(&self) -> Matrix4<f32> {
        let view_front = if self.reverse_view {
            -self.front
        } else {
            self.front
        };
        let mut view = Matrix4::look_at_rh(&self.position, &(self.position + view_front), &self.up);

        // EntityRenderer.setupViewBobbing — vanilla-compatible bobbing.
        // Transforms are camera-local (happen after the view transform),
        // matching vanilla's GL post-multiplication order conceptually.
        if self.view_bobbing && self.bob_amount > 0.0001 {
            let f1 = self.bob_phase;
            let f2 = self.bob_amount;
            let f3 = self.bob_pitch;

            let sin_f1 = (f1 * std::f32::consts::PI).sin();
            let cos_f1 = (f1 * std::f32::consts::PI).cos();

            let tx = sin_f1 * f2 * 0.5;
            let ty = -(cos_f1 * f2).abs();
            let roll = sin_f1 * f2 * 3.0_f32.to_radians();
            let pitch = ((f1 * std::f32::consts::PI - 0.2).cos().abs() * f2) * 5.0_f32.to_radians()
                + f3.to_radians();

            view = Matrix4::new_translation(&Vector3::new(tx, ty, 0.0))
                * Matrix4::from_euler_angles(pitch, 0.0, roll)
                * view;
        }

        // EntityRenderer.hurtCameraEffect — vanilla-compatible hurt tilt,
        // interpolated between ticks so the roll fades smoothly.
        if self.hurt_cam_enabled && self.hurt_time > 0.0 {
            let h =
                self.prev_hurt_time + (self.hurt_time - self.prev_hurt_time) * self.partial_tick;
            let f = (h / 0.45).clamp(0.0, 1.0);
            let roll = -(f * f * f * f * std::f32::consts::PI).sin() * 14.0_f32.to_radians();
            view = Matrix4::from_euler_angles(0.0, 0.0, roll) * view;
        }
        view
    }

    /// View matrix without walking-bob or hurt-camera effects.
    ///
    /// First-person arm and held-item meshes are built in view space and then
    /// converted to world space with the inverse of this matrix.  The rendering
    /// pipeline applies the full `view_matrix()` (including bob and hurt), so
    /// those effects must NOT be baked into the inverse used during mesh
    /// construction — otherwise the arm stays still while the world shakes.
    pub fn view_look_at_matrix(&self) -> Matrix4<f32> {
        let view_front = if self.reverse_view {
            -self.front
        } else {
            self.front
        };
        Matrix4::look_at_rh(&self.position, &(self.position + view_front), &self.up)
    }

    /// View transform for infinitely distant geometry such as the sky.
    ///
    /// Camera world translation is cancelled so the sky remains infinitely far
    /// away, while camera-local effects stay in the transform. Vanilla 1.8.9
    /// renders the sky after hurt-camera and view-bobbing transforms are applied.
    pub fn sky_view_matrix(&self) -> Matrix4<f32> {
        self.view_matrix() * Matrix4::new_translation(&self.position.coords)
    }
    /// Interpolated FOV modifier for smooth per-frame rendering.
    /// `alpha` is the fraction through the current tick (0.0–1.0).
    pub fn fov_modifier_at(&self, alpha: f32) -> f32 {
        self.prev_fov_modifier + (self.fov_modifier - self.prev_fov_modifier) * alpha
    }

    pub fn projection_matrix(&self) -> Matrix4<f32> {
        self.projection_matrix_at(1.0)
    }

    /// Projection matrix with interpolated FOV modifier.
    pub fn projection_matrix_at(&self, alpha: f32) -> Matrix4<f32> {
        let effective_fov = self.fov * self.fov_modifier_at(alpha);
        let f = 1.0 / (effective_fov.to_radians() / 2.0).tan();
        let n = self.near;
        let fa = self.far;
        // Right-handed Vulkan perspective, with Y negated for Vulkan clip space
        // (Y down). Vulkan clips depth in 0..w; using OpenGL's -w..w projection
        // made geometry near the camera disappear before the configured near plane.
        // Row-major form:
        //   [ f/a   0    0               0             ]
        //   [ 0    -f    0               0             ]
        //   [ 0     0     fa/(n-fa)       fa*n/(n-fa)  ]
        //   [ 0     0    -1              0             ]
        Matrix4::new(
            f / self.aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            -f,
            0.0,
            0.0,
            0.0,
            0.0,
            fa / (n - fa),
            fa * n / (n - fa),
            0.0,
            0.0,
            -1.0,
            0.0,
        )
    }
    pub fn update_aspect(&mut self, w: u32, h: u32) {
        if h > 0 {
            self.aspect = w as f32 / h as f32;
        }
    }
    pub fn frustum(&self) -> Frustum {
        Frustum::from_view_proj(&(self.projection_matrix() * self.view_matrix()))
    }

    /// Convert internal camera yaw to MC 1.8.9 protocol yaw (degrees).
    ///
    /// Camera convention: yaw=0 → +X (east), increases CCW from above.
    /// MC protocol: yaw=0 → +Z (south), increases CW from above. Vanilla keeps
    /// this value continuous, including across full turns.
    /// camera_yaw = 90 + mc_yaw  →  mc_yaw = camera_yaw - 90
    pub fn mc_yaw_degrees(&self) -> f32 {
        self.yaw.to_degrees() - 90.0
    }

    /// Convert internal camera pitch to MC 1.8.9 protocol pitch (degrees).
    ///
    /// Camera convention: pitch>0 → looking UP.
    /// MC protocol: pitch>0 → looking DOWN.
    pub fn mc_pitch_degrees(&self) -> f32 {
        -self.pitch.to_degrees()
    }

    /// Set camera yaw from MC 1.8.9 protocol yaw (degrees).
    /// camera_yaw = 90 + mc_yaw
    pub fn set_from_mc_yaw(&mut self, mc_yaw: f32) {
        self.yaw = (mc_yaw + 90.0).to_radians();
        self.update_vectors();
    }

    /// Set camera pitch from MC 1.8.9 protocol pitch (degrees).
    /// camera_pitch = -mc_pitch
    pub fn set_from_mc_pitch(&mut self, mc_pitch: f32) {
        self.pitch = (-mc_pitch).to_radians();
        self.update_vectors();
    }

    /// Add a delta in MC protocol yaw degrees.
    /// camera_yaw_change = mc_yaw_change (since cam = 90 + mc, d(cam) = d(mc))
    pub fn add_mc_yaw(&mut self, delta_mc_yaw: f32) {
        self.yaw += delta_mc_yaw.to_radians();
        self.update_vectors();
    }

    /// Add a delta in MC protocol pitch degrees.
    pub fn add_mc_pitch(&mut self, delta_mc_pitch: f32) {
        self.pitch += (-delta_mc_pitch).to_radians();
        self.update_vectors();
    }
}

// --- Player ---

/// The current server-owned `generic.movementSpeed` attribute snapshot.
///
/// NetHandlerPlayClient replaces the base and every modifier when it receives
/// S20. Keeping the raw snapshot lets the local player apply the same
/// operation-0/1/2 ordering as `ModifiableAttributeInstance`.
#[derive(Clone, Debug)]
struct MovementSpeedAttribute {
    base: f64,
    modifiers: Vec<crate::net::packet::EntityPropertyModifier>,
}

impl MovementSpeedAttribute {
    fn is_local_dynamic_modifier(modifier: &crate::net::packet::EntityPropertyModifier) -> bool {
        modifier.uuid.eq_ignore_ascii_case(SPEED_POTION_MODIFIER)
            || modifier.uuid.eq_ignore_ascii_case(SLOWNESS_POTION_MODIFIER)
            || modifier.uuid.eq_ignore_ascii_case(SPRINTING_SPEED_MODIFIER)
    }
}

/// MC 1.8.9 player — reads InputState, never raw keys.
pub struct Player {
    pub camera: Camera,
    /// Vanilla Entity.pos* and motion* are doubles. These are authoritative
    /// gameplay state; rendering receives an explicit f32 projection.
    pub position: Point3<f64>,
    pub velocity: Vector3<f64>,
    pub on_ground: bool,
    pub flying: bool,
    pub allow_flying: bool,
    /// Server-synchronized capability values used by EntityPlayer movement.
    pub fly_speed: f32,
    pub food_level: i32,
    pub sneaking: bool,
    pub sprinting: bool,
    /// Sprint state selected by EntityPlayerSP before this tick's movement.
    /// onUpdateWalkingPlayer reports this same current state through C0B.
    movement_sprinting: bool,
    /// Vanilla MovementInput values for C0C while riding. `move_strafe` uses
    /// protocol/vanilla sign convention: positive means left.
    pub move_strafe: f32,
    pub move_forward: f32,
    pub movement_jump: bool,
    /// Whether the player is using an item (eating, drinking, bow, blocking).
    pub using_item: bool,
    // For smooth rendering between ticks
    pub prev_position: Point3<f64>,
    pub render_position: Point3<f32>,
    pub chasing_position: Point3<f64>,
    pub prev_chasing_position: Point3<f64>,
    pub render_chasing_position: Point3<f32>,
    // MC 1.8.9 sprint/fly toggle timers (tick-based, not wall-clock)
    /// Counts down from 7 when the first tap of double-tap-W is detected.
    sprint_toggle_ticks: u32,
    /// EntityPlayerSP.sprintingTicksLeft. Vanilla refreshes this to 600 only
    /// when sprinting is enabled, then clears sprint at expiry.
    sprinting_ticks_left: u32,
    /// EntityPlayerSP horse-jump charge state.
    horse_jump_counter: i32,
    horse_jump_power: f32,
    pending_horse_jump: Option<i32>,
    /// Counts down from 7 when the first tap of double-tap-Space is detected.
    fly_toggle_ticks: u32,
    /// Whether jump was held at the start of the previous tick (for edge detection).
    prev_jump_held: bool,
    /// Whether forward was >= 0.8 at the end of the previous tick.
    prev_forward_fast: bool,
    /// Deferred air movement speed, updated after moveEntityWithHeading to
    /// match EntityPlayer.onLivingUpdate's jumpMovementFactor timing.
    air_acceleration: f32,
    // Fall damage tracking (server calculates actual damage)
    /// Y position when the player started falling (None = not falling)
    fall_start_y: Option<f64>,
    /// Distance fallen since last ground contact (for visual effects)
    pub fall_distance: f32,
    // Water/swimming state
    /// Whether the player is currently submerged in water
    pub in_water: bool,
    /// Entity.isInLava uses a contracted/expanded AABB separate from water.
    in_lava: bool,
    /// Oxygen level (300 = full, 0 = drowning)
    pub oxygen: i32,
    /// Ticks since last oxygen damage
    oxygen_damage_timer: i32,
    /// Whether the player collided horizontally this tick (vanilla: isCollidedHorizontally)
    pub collided_horizontally: bool,
    /// Vehicle attached by S1B; used to exclude the ridden boat from hard
    /// collision queries, matching World.getCollidingBoundingBoxes.
    pub vehicle_id: Option<i32>,
    /// Set by BlockWeb collision callbacks and consumed by the next moveEntity.
    in_web: bool,
    /// EntityLivingBase.jumpTicks — cooldown counter that prevents consecutive jumps.
    /// Vanilla decrements this every tick and only allows a jump when it reaches 0.
    jump_ticks: u32,
    /// Saved motionY before moveEntity for creative fly damping (vanilla d3 in moveEntityWithHeading)
    pre_move_motion_y: f64,
    /// Set by S19 status 9 (server-confirmed food consumption).  The frame
    /// loop polls this to clear `item_use_active` in sync with the server.
    item_use_finished: bool,
    /// Entity.moveEntityWithHeading reads friction twice: once before moveFlying for
    /// speed and once before the final drag multiply. Both reads use the block at the
    /// pre-move position; sampling the post-move block would diverge from vanilla when
    /// crossing a slipperiness boundary within a single tick.
    pre_move_drag_friction: f64,
    /// Camera mode: 0=First Person, 1=Third Person Back, 2=Third Person Front
    pub camera_mode: u8,
    /// Vanilla EntityLivingBase limb animation state.
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
    /// Vanilla EntityPlayerSP first-person arm tracking.
    pub render_arm_yaw: f32,
    pub render_arm_pitch: f32,
    pub prev_render_arm_yaw: f32,
    pub prev_render_arm_pitch: f32,
    /// Vanilla renderYawOffset in protocol/MC degrees.
    pub body_yaw: f32,
    /// Time remaining for the server-authoritative hurt camera effect.
    pub hurt_time: f32,
    pub prev_hurt_time: f32,
    /// Vanilla Entity.distanceWalkedModified: total horizontal distance walked, used by setupViewBobbing.
    pub distance_walked_modified: f32,
    pub prev_distance_walked_modified: f32,
    /// Vanilla EntityPlayer.cameraYaw: horizontal speed factor [0, 1] for view bobbing amplitude.
    pub camera_yaw: f32,
    pub prev_camera_yaw: f32,
    /// Vanilla EntityLivingBase.cameraPitch: vertical motion factor [-1, 1] for view bobbing pitch.
    pub camera_pitch: f32,
    pub prev_camera_pitch: f32,
    /// Server-synchronized potion effects for the local player. The local
    /// player is intentionally not duplicated in EntityManager.
    pub active_effects: Vec<crate::entity::EntityEffectState>,
    movement_speed_attribute: MovementSpeedAttribute,
}

impl Player {
    pub fn new(position: Point3<f64>, aspect: f32) -> Self {
        Player {
            camera: Camera::new(
                Point3::new(
                    position.x as f32,
                    (position.y + PLAYER_EYE_HEIGHT) as f32,
                    position.z as f32,
                ),
                aspect,
            ),
            position,
            velocity: Vector3::zeros(),
            on_ground: false,
            flying: false,
            allow_flying: false,
            fly_speed: FLY_SPEED,
            food_level: 20,
            sneaking: false,
            sprinting: false,
            movement_sprinting: false,
            move_strafe: 0.0,
            move_forward: 0.0,
            movement_jump: false,
            using_item: false,
            prev_position: position,
            render_position: Point3::new(position.x as f32, position.y as f32, position.z as f32),
            chasing_position: position,
            prev_chasing_position: position,
            render_chasing_position: Point3::new(
                position.x as f32,
                position.y as f32,
                position.z as f32,
            ),
            sprint_toggle_ticks: 0,
            sprinting_ticks_left: 0,
            horse_jump_counter: 0,
            horse_jump_power: 0.0,
            pending_horse_jump: None,
            fly_toggle_ticks: 0,
            prev_jump_held: false,
            prev_forward_fast: false,
            air_acceleration: 0.02,
            fall_start_y: None,
            fall_distance: 0.0,
            in_water: false,
            in_lava: false,
            oxygen: 300,
            oxygen_damage_timer: 0,
            collided_horizontally: false,
            vehicle_id: None,
            in_web: false,
            jump_ticks: 0,
            pre_move_motion_y: 0.0,
            item_use_finished: false,
            pre_move_drag_friction: 0.91,
            camera_mode: 0,
            limb_swing: 0.0,
            limb_swing_amount: 0.0,
            render_arm_yaw: -180.0,
            render_arm_pitch: 30.0,
            prev_render_arm_yaw: -180.0,
            prev_render_arm_pitch: 30.0,
            body_yaw: -180.0,
            hurt_time: 0.0,
            prev_hurt_time: 0.0,
            distance_walked_modified: 0.0,
            prev_distance_walked_modified: 0.0,
            camera_yaw: 0.0,
            prev_camera_yaw: 0.0,
            camera_pitch: 0.0,
            prev_camera_pitch: 0.0,
            active_effects: Vec::new(),
            // EntityPlayer.applyEntityAttributes sets this exact Java float
            // value as the base attribute value.
            movement_speed_attribute: MovementSpeedAttribute {
                base: WALK_SPEED as f64,
                modifiers: Vec::new(),
            },
        }
    }

    /// Call each frame with tick interpolation factor (0.0-1.0) for smooth rendering.
    pub fn update_render_position(&mut self, alpha: f32, world: Option<&World>) {
        let alpha64 = alpha as f64;
        self.render_position = Point3::new(
            (self.prev_position.x + (self.position.x - self.prev_position.x) * alpha64) as f32,
            (self.prev_position.y + (self.position.y - self.prev_position.y) * alpha64) as f32,
            (self.prev_position.z + (self.position.z - self.prev_position.z) * alpha64) as f32,
        );
        self.render_chasing_position = Point3::new(
            (self.prev_chasing_position.x
                + (self.chasing_position.x - self.prev_chasing_position.x) * alpha64)
                as f32,
            (self.prev_chasing_position.y
                + (self.chasing_position.y - self.prev_chasing_position.y) * alpha64)
                as f32,
            (self.prev_chasing_position.z
                + (self.chasing_position.z - self.prev_chasing_position.z) * alpha64)
                as f32,
        );
        let eye = if self.sneaking {
            (PLAYER_EYE_HEIGHT - 0.08) as f32
        } else {
            PLAYER_EYE_HEIGHT as f32
        };
        let mut cam_pos = self.render_position + Vector3::new(0.0, eye, 0.0);

        self.camera.reverse_view = self.camera_mode == 2;

        // Third-person camera offset
        if self.camera_mode > 0 {
            let offset = self.camera.front * if self.camera_mode == 2 { 4.0 } else { -4.0 };
            if let Some(w) = world {
                // EntityRenderer.orientCamera checks eight rays around the eye
                // so the camera does not clip through a wall at screen corners.
                let mut allowed_distance = offset.norm();
                for i in 0..8 {
                    let corner = Vector3::new(
                        if i & 1 == 0 { -0.1 } else { 0.1 },
                        if i & 2 == 0 { -0.1 } else { 0.1 },
                        if i & 4 == 0 { -0.1 } else { 0.1 },
                    );
                    let start = cam_pos + corner;
                    let steps = 64;
                    let step = offset / steps as f32;
                    for step_index in 1..=steps {
                        let sample = start + step * step_index as f32;
                        if w.get_block(
                            sample.x.floor() as i32,
                            sample.y.floor() as i32,
                            sample.z.floor() as i32,
                        )
                        .is_solid()
                        {
                            let hit_distance =
                                offset.norm() * (step_index - 1) as f32 / steps as f32;
                            allowed_distance = allowed_distance.min(hit_distance);
                            break;
                        }
                    }
                }
                cam_pos += offset.normalize() * allowed_distance;
            } else {
                cam_pos += offset;
            }
        }

        self.camera.position = cam_pos;
        self.camera.view_bobbing = self.camera_mode == 0 && self.camera.view_bobbing;
        let delta_distance = self.distance_walked_modified - self.prev_distance_walked_modified;
        self.camera.bob_phase = -(self.distance_walked_modified + delta_distance * alpha);
        self.camera.bob_amount =
            self.prev_camera_yaw + (self.camera_yaw - self.prev_camera_yaw) * alpha;
        self.camera.bob_pitch =
            self.prev_camera_pitch + (self.camera_pitch - self.prev_camera_pitch) * alpha;
        self.camera.hurt_time = self.hurt_time;
        self.camera.prev_hurt_time = self.prev_hurt_time;
    }

    /// Starts visual feedback only after an authoritative damage/status packet.
    pub fn trigger_hurt(&mut self) {
        self.prev_hurt_time = 0.45;
        self.hurt_time = 0.45;
    }

    pub fn add_effect(&mut self, effect: crate::entity::EntityEffectState) {
        if let Some(existing) = self
            .active_effects
            .iter_mut()
            .find(|active| active.effect_id == effect.effect_id)
        {
            *existing = effect;
        } else {
            self.active_effects.push(effect);
        }
    }

    pub fn remove_effect(&mut self, effect_id: i8) {
        self.active_effects
            .retain(|effect| effect.effect_id != effect_id);
    }

    fn set_sprinting(&mut self, sprinting: bool) {
        self.sprinting = sprinting;
        self.sprinting_ticks_left = if sprinting { 600 } else { 0 };
    }

    pub fn take_pending_horse_jump(&mut self) -> Option<i32> {
        self.pending_horse_jump.take()
    }

    pub fn movement_sprinting(&self) -> bool {
        self.movement_sprinting
    }

    /// Successful local `attackTargetEntityWithCurrentItem`: sprinting adds one
    /// knockback level, and positive knockback damps horizontal motion by 0.6
    /// and cancels sprinting.
    pub fn on_attack_entity(&mut self, knockback_level: i16) {
        if self.sprinting || knockback_level > 0 {
            self.velocity.x *= 0.6;
            self.velocity.z *= 0.6;
            self.set_sprinting(false);
        }
    }

    /// Vanilla `EntityPlayer.handleStatusUpdate(9)` → `onItemUseFinish()`:
    /// the server sends S19 status 9 when it consumes the food.  The client
    /// must clear its local item-use state in response so movement returns to
    /// full speed on the next tick.
    pub fn on_item_use_finished(&mut self) {
        self.item_use_finished = true;
    }

    /// Returns true once after the server confirms food/potion consumption via
    /// S19 status 9.  The caller (frame loop) clears `item_use_active` and
    /// resets this flag.
    pub fn take_item_use_finished(&mut self) -> bool {
        let finished = self.item_use_finished;
        self.item_use_finished = false;
        finished
    }

    /// Vanilla `Entity.getEyeHeight` position: feet plus 1.62 (minus 0.08
    /// while sneaking). Unlike `camera.position` this is not displaced by
    /// third-person view or view bobbing.
    pub fn eye_position(&self) -> Point3<f64> {
        let eye_height = if self.sneaking {
            PLAYER_EYE_HEIGHT - 0.08
        } else {
            PLAYER_EYE_HEIGHT
        };
        Point3::new(
            self.position.x,
            self.position.y + eye_height,
            self.position.z,
        )
    }

    /// Read-only exposure for owned scripting/debug snapshots. The gameplay
    /// field remains private so callers cannot mutate collision state.
    pub fn in_lava(&self) -> bool {
        self.in_lava
    }

    /// Apply the local player's S20 attribute snapshot. Vanilla's
    /// NetHandlerPlayClient routes this packet to EntityPlayerSP as well as to
    /// every other living entity; the local player is not in our EntityManager.
    pub fn apply_entity_properties(&mut self, properties: Vec<crate::net::packet::EntityProperty>) {
        for property in properties {
            if property.key == "generic.movementSpeed" {
                self.movement_speed_attribute = MovementSpeedAttribute {
                    base: property.value,
                    modifiers: property.modifiers,
                };
            }
        }
    }

    /// Called once per tick (20 Hz). Reads from InputState.
    /// Follows MC 1.8.9 EntityPlayerSP.onLivingUpdate → EntityLivingBase.onLivingUpdate flow.
    pub fn tick(
        &mut self,
        input: &InputState,
        world: &World,
        entities: &crate::entity::EntityManager,
        local_entity_id: Option<i32>,
    ) {
        self.prev_position = self.position;

        // If the world has no chunk at the player position the physics tick
        // would let the player fall through the (not-yet-loaded) ground.  Wait
        // until the chunk arrives before applying gravity/collision.
        let player_chunk_x = (self.position.x / 16.0).floor() as i32;
        let player_chunk_z = (self.position.z / 16.0).floor() as i32;
        let has_chunk = world.chunks.contains_key(&(player_chunk_x, player_chunk_z));

        // Entity.updateRidden clears motion before its nested onUpdate call.
        // This matters on the mounting tick, when on-foot momentum would
        // otherwise feed one speculative collision/movement update.
        if self.vehicle_id.is_some() {
            self.velocity = Vector3::zeros();
        }
        self.prev_hurt_time = self.hurt_time;
        self.hurt_time = (self.hurt_time - 1.0 / 20.0).max(0.0);
        for effect in &mut self.active_effects {
            effect.duration -= 1;
        }
        self.active_effects.retain(|effect| effect.duration > 0);

        // --- Decrement timers (vanilla: top of onLivingUpdate) ---
        if self.jump_ticks > 0 {
            self.jump_ticks -= 1;
        }
        if self.fly_toggle_ticks > 0 {
            self.fly_toggle_ticks -= 1;
        }
        if self.sprint_toggle_ticks > 0 {
            self.sprint_toggle_ticks -= 1;
        }
        if self.sprinting_ticks_left > 0 {
            self.sprinting_ticks_left -= 1;
            if self.sprinting_ticks_left == 0 {
                self.set_sprinting(false);
            }
        }

        // Entity.onEntityUpdate calls handleWaterMovement before living
        // movement. Its AABB is expanded 0.4 down, contracted by 0.001, and
        // water flow contributes a normalized 0.014 impulse.
        self.in_water = self.handle_water_movement(world);
        self.in_lava = self.is_in_lava(world);

        // EntityLivingBase.onLivingUpdate clears tiny motion components before
        // jump/input acceleration. This is an exact 0.005D deadband.
        if self.velocity.x.abs() < 0.005 {
            self.velocity.x = 0.0;
        }
        if self.velocity.y.abs() < 0.005 {
            self.velocity.y = 0.0;
        }
        if self.velocity.z.abs() < 0.005 {
            self.velocity.z = 0.0;
        }

        // --- Read input (vanilla: movementInput.updatePlayerMoveState) ---
        if input.is_just_pressed(Action::TogglePerspective) {
            self.camera_mode = (self.camera_mode + 1) % 3;
            // When switching to Third Person Front (2), we invert the camera view direction during render
            // but we'll handle the camera look vector properly if needed.
        }

        let prev_jump_held = self.prev_jump_held;
        let jump_held = input.is_held(Action::Jump);
        self.prev_jump_held = jump_held;

        // EntityLivingBase.onLivingUpdate jumpTicks else branch:
        // when the jump key is released, reset the cooldown immediately
        // so an immediate re-press can trigger a new jump while still on ground.
        if !jump_held {
            self.jump_ticks = 0;
        }

        self.sneaking = input.is_held(Action::Sneak);

        // Vanilla Entity.moveFlying rotates movement using rotationYaw only;
        // rotationPitch is deliberately ignored.  The old code projected the
        // 3D camera look vector onto XZ, which collapses to zero at ±90° pitch
        // and made forward/backward movement impossible when looking straight
        // up or down.
        let (fwd, rgt) = horizontal_movement_basis(self.camera.mc_yaw_degrees());

        let mut mf = 0.0f32;
        let mut ms = 0.0f32;
        if input.is_held(Action::Forward) {
            mf += 1.0;
        }
        if input.is_held(Action::Backward) {
            mf -= 1.0;
        }
        if input.is_held(Action::StrafeLeft) {
            ms -= 1.0;
        }
        if input.is_held(Action::StrafeRight) {
            ms += 1.0;
        }

        // Sneak reduces movement speed (vanilla: MovementInputFromOptions)
        if self.sneaking {
            mf *= 0.3;
            ms *= 0.3;
        }

        // Item use slows movement to 20% but vanilla does NOT cancel sprinting —
        // EntityPlayerSP.onLivingUpdate and EntityLivingBase.moveEntityWithHeading
        // only scale input, they leave isSprinting() alone. Cancelling it locally
        // sends a spurious C0B STOP_SPRINTING that desyncs the server simulation.
        if self.using_item && self.vehicle_id.is_none() {
            mf *= 0.2;
            ms *= 0.2;
            self.sprint_toggle_ticks = 0;
        }

        // EntityPlayerSP.onLivingUpdate probes each lower-body corner before
        // sprint and movement dispatch. Its override uses a deterministic
        // horizontal 0.1 impulse, unlike Entity.pushOutOfBlocks' random push.
        let push_x = PLAYER_WIDTH * 0.35;
        let push_y = self.position.y + 0.5;
        self.push_out_of_blocks(
            world,
            self.position.x - push_x,
            push_y,
            self.position.z + push_x,
        );
        self.push_out_of_blocks(
            world,
            self.position.x - push_x,
            push_y,
            self.position.z - push_x,
        );
        self.push_out_of_blocks(
            world,
            self.position.x + push_x,
            push_y,
            self.position.z - push_x,
        );
        self.push_out_of_blocks(
            world,
            self.position.x + push_x,
            push_y,
            self.position.z + push_x,
        );

        // EntityPlayerSP passes these MovementInput values to C0C while
        // riding. RustCraft's horizontal strafe convention is positive-right,
        // whereas MovementInput/C0C is positive-left.
        self.move_strafe = -ms;
        self.move_forward = mf;
        self.movement_jump = jump_held;

        let forward_fast = mf >= 0.8;
        let can_sprint = self.food_level > 6 || self.allow_flying;
        let blinded = self
            .active_effects
            .iter()
            .any(|effect| effect.effect_id == 15);

        // --- Sprint logic (vanilla: EntityPlayerSP.onLivingUpdate lines 799-821) ---

        // Auto-sprint from double-tap W: fires when forward transitions slow → fast
        if self.on_ground
            && !self.sneaking
            && !self.prev_forward_fast
            && forward_fast
            && !self.sprinting
            && can_sprint
            && !self.using_item
            && !blinded
        {
            if self.sprint_toggle_ticks == 0 && !input.is_held(Action::Sprint) {
                self.sprint_toggle_ticks = 7;
            } else if self.sprint_toggle_ticks > 0 {
                self.set_sprinting(true);
            }
        }

        // Sprint from holding sprint key + forward
        if !self.sprinting
            && forward_fast
            && can_sprint
            && !self.using_item
            && !blinded
            && input.is_held(Action::Sprint)
        {
            self.set_sprinting(true);
        }

        // Cancel sprint when forward becomes slow, or collided horizontally
        if self.sprinting && (!forward_fast || self.collided_horizontally || !can_sprint) {
            self.set_sprinting(false);
        }

        // Cancel sprint when sneaking
        if self.sneaking {
            self.set_sprinting(false);
        }

        self.prev_forward_fast = forward_fast;

        // --- Creative fly toggle (vanilla: timer-based double-tap space) ---
        if self.allow_flying && !prev_jump_held && jump_held {
            if self.fly_toggle_ticks == 0 {
                self.fly_toggle_ticks = 7;
            } else {
                self.flying = !self.flying;
                self.velocity.y = 0.0;
                self.fly_toggle_ticks = 0;
            }
        }

        // --- Flying vertical movement (vanilla: before super.onLivingUpdate) ---
        if self.flying {
            let fly_spd = self.fly_speed * if self.sprinting { 2.0 } else { 1.0 };
            if jump_held {
                self.velocity.y += (fly_spd * 3.0) as f64;
            }
            if self.sneaking {
                self.velocity.y -= (fly_spd * 3.0) as f64;
            }
            // Save pre-move motionY for damping after collision (vanilla: d3 in moveEntityWithHeading)
            self.pre_move_motion_y = self.velocity.y;
        }

        // EntityPlayerSP horse-jump charge/release state machine. It runs
        // before super.onLivingUpdate, and sends RIDING_JUMP on key release.
        let riding_saddled_horse = self
            .vehicle_id
            .and_then(|vehicle_id| entities.get(vehicle_id))
            .is_some_and(|vehicle| {
                vehicle.entity_type == crate::entity::EntityType::Horse
                    && vehicle.visual.horse_saddled
            });
        if riding_saddled_horse {
            if self.horse_jump_counter < 0 {
                self.horse_jump_counter += 1;
                if self.horse_jump_counter == 0 {
                    self.horse_jump_power = 0.0;
                }
            }
            if prev_jump_held && !jump_held {
                self.horse_jump_counter = -10;
                self.pending_horse_jump = Some((self.horse_jump_power * 100.0) as i32);
            } else if !prev_jump_held && jump_held {
                self.horse_jump_counter = 0;
                self.horse_jump_power = 0.0;
            } else if prev_jump_held {
                self.horse_jump_counter += 1;
                if self.horse_jump_counter < 10 {
                    self.horse_jump_power = self.horse_jump_counter as f32 * 0.1;
                } else {
                    self.horse_jump_power = 0.8 + 2.0 / (self.horse_jump_counter - 9) as f32 * 0.1;
                }
            }
        } else {
            self.horse_jump_power = 0.0;
        }

        // EntityLivingBase.onLivingUpdate damps movement inputs immediately
        // before moveEntityWithHeading. Sprint eligibility above intentionally
        // uses the undamped EntityPlayerSP input, matching vanilla ordering.
        mf *= 0.98;
        ms *= 0.98;

        if !has_chunk {
            return;
        }

        // EntityPlayerSP selects sprint before movement and
        // onUpdateWalkingPlayer reports that same current state afterward.
        self.movement_sprinting = self.sprinting;

        // --- Main movement dispatch ---
        if self.flying {
            self.tick_flying(&fwd, &rgt, mf, ms, input);
        } else if self.in_water {
            self.tick_swimming(&fwd, &rgt, mf, ms, input, world);
        } else if self.in_lava {
            self.tick_lava(&fwd, &rgt, mf, ms, input);
        } else {
            self.tick_walking(&fwd, &rgt, mf, ms, input, world);
        }

        let on_ladder = self.is_on_ladder(world);
        if on_ladder {
            self.velocity.x = self.velocity.x.clamp(-0.15, 0.15);
            self.velocity.z = self.velocity.z.clamp(-0.15, 0.15);
            self.fall_distance = 0.0;
            self.velocity.y = self.velocity.y.max(-0.15);
            if self.sneaking && self.velocity.y < 0.0 {
                self.velocity.y = 0.0;
            }
        }

        // --- Collision (vanilla: Entity.moveEntity) ---
        // Web slowdown applies to this tick's requested displacement, while
        // motion is cleared before collision and rebuilt by post-move gravity.
        let was_on_ground = self.on_ground;
        let water_start_y = self.position.y;
        let mut delta = self.velocity;
        if self.in_web {
            self.in_web = false;
            delta.x *= 0.25;
            delta.y *= 0.05000000074505806;
            delta.z *= 0.25;
            self.velocity = Vector3::zeros();
        }
        let entity_collision_boxes = self.boat_collision_boxes(entities, local_entity_id);
        self.collided_horizontally = move_with_collision(
            &mut self.position,
            &mut self.velocity,
            &delta,
            PLAYER_WIDTH,
            PLAYER_HEIGHT,
            world,
            &entity_collision_boxes,
            &mut self.on_ground,
            self.sneaking,
        );
        if self.on_ground != was_on_ground {
            let below_x = self.position.x.floor() as i32;
            let below_y = (self.position.y - 0.20000000298023224).floor() as i32;
            let below_z = self.position.z.floor() as i32;
            log::debug!(
                target: "rustcraft::movement",
                "ground transition: {} -> {}, pos=({:.6},{:.6},{:.6}), requested=({:.6},{:.6},{:.6}), below=({below_x},{below_y},{below_z}), block_state={}",
                was_on_ground,
                self.on_ground,
                self.position.x,
                self.position.y,
                self.position.z,
                delta.x,
                delta.y,
                delta.z,
                world.get_block_state(below_x, below_y, below_z)
            );
        }

        // Vanilla checks isOnLadder again after moveEntity, at the resulting
        // position, before applying the 0.2 climb impulse.
        if self.collided_horizontally && self.is_on_ladder(world) {
            self.velocity.y = 0.2;
        }

        let block_below = self.block_below(world);
        if delta.y < 0.0
            && self.on_ground
            && block_below == crate::world::block::Block::SlimeBlock
            && !self.sneaking
        {
            self.velocity.y = -delta.y;
        }
        // BlockSoulSand.onEntityCollidedWithBlock is invoked by
        // Entity.doBlockCollisions for every Soul Sand voxel overlapped by the
        // entity AABB; it is not limited to the grounded block below.
        if self.intersects_block(world, crate::world::block::Block::SoulSand) {
            self.velocity.x *= 0.4;
            self.velocity.z *= 0.4;
        }
        if self.on_ground
            && block_below == crate::world::block::Block::SlimeBlock
            && !self.sneaking
            && self.velocity.y.abs() < 0.1
        {
            let factor = 0.4 + self.velocity.y.abs() * 0.2;
            self.velocity.x *= factor;
            self.velocity.z *= factor;
        }
        self.in_web = self.intersects_block(world, crate::world::block::Block::Cobweb);

        // EntityPlayer.onUpdate: smooth the cape anchor independently of the body.
        self.prev_chasing_position = self.chasing_position;
        let chasing_delta = self.position - self.chasing_position;
        for axis in 0..3 {
            if chasing_delta[axis].abs() > 10.0 {
                self.chasing_position[axis] = self.position[axis];
                self.prev_chasing_position[axis] = self.position[axis];
            }
        }
        self.chasing_position += (self.position - self.chasing_position) * 0.25;

        // EntityLivingBase: update limb swing from actual distance travelled.
        let dx = self.position.x - self.prev_position.x;
        let dz = self.position.z - self.prev_position.z;
        let distance_sq = dx * dx + dz * dz;
        // EntityLivingBase.onUpdate: f2 = sqrt(dx² + dz²) * 3.0F.
        // Keeping this exact is important because limbSwingAmount then feeds
        // both the stride amplitude and phase in ModelBiped.
        let target_limb_amount = (distance_sq.sqrt() * 3.0).min(1.0) as f32;
        self.limb_swing_amount += (target_limb_amount - self.limb_swing_amount) * 0.4;
        self.limb_swing += self.limb_swing_amount;

        // Entity.onUpdate: accumulate horizontal distance walked.
        // Used by EntityRenderer.setupViewBobbing to determine the bobbing phase.
        self.prev_distance_walked_modified = self.distance_walked_modified;
        self.distance_walked_modified += (distance_sq.sqrt() * 0.6) as f32;

        // EntityPlayer.onLivingUpdate: cameraYaw from horizontal speed.
        // cameraYaw [0, 1] controls the amplitude (f2) in setupViewBobbing.
        self.prev_camera_yaw = self.camera_yaw;
        let horiz_speed =
            (self.velocity.x * self.velocity.x + self.velocity.z * self.velocity.z).sqrt() as f32;
        let mut camera_yaw_target = horiz_speed;
        if camera_yaw_target > 0.1 {
            camera_yaw_target = 0.1;
        }
        if !self.on_ground {
            camera_yaw_target = 0.0;
        }
        self.camera_yaw += (camera_yaw_target - self.camera_yaw) * 0.4;

        // EntityLivingBase.onLivingUpdate: cameraPitch from vertical motion.
        // cameraPitch [-1, 1] directly feeds the last rotation in setupViewBobbing.
        self.prev_camera_pitch = self.camera_pitch;
        let camera_pitch_target = (-self.velocity.y as f32 * 0.20000000298023224).atan() * 15.0;
        let camera_pitch_target = if self.on_ground {
            0.0
        } else {
            camera_pitch_target
        };
        self.camera_pitch += (camera_pitch_target - self.camera_pitch) * 0.8;

        // EntityLivingBase.updateDistance: body follows movement while the head
        // may turn independently, clamped to the vanilla +/-75 degree range.
        let head_yaw = self.camera.mc_yaw_degrees();
        let mut desired_body_yaw = self.body_yaw;
        if distance_sq > 0.0025000002 {
            desired_body_yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
        }
        self.body_yaw += wrap_degrees(desired_body_yaw - self.body_yaw) * 0.3;
        let mut head_delta = wrap_degrees(head_yaw - self.body_yaw).clamp(-75.0, 75.0);
        self.body_yaw = head_yaw - head_delta;
        if head_delta * head_delta > 2500.0 {
            self.body_yaw += head_delta * 0.2;
            head_delta = wrap_degrees(head_yaw - self.body_yaw);
            self.body_yaw = head_yaw - head_delta;
        }

        self.prev_render_arm_yaw = self.render_arm_yaw;
        self.prev_render_arm_pitch = self.render_arm_pitch;
        self.render_arm_yaw += wrap_degrees(head_yaw - self.render_arm_yaw) * 0.5;
        self.render_arm_pitch +=
            wrap_degrees(self.camera.mc_pitch_degrees() - self.render_arm_pitch) * 0.5;

        // --- Post-move physics ---
        if self.flying {
            self.velocity.y = self.pre_move_motion_y * 0.6;
            self.velocity.x *= AIR_FRICTION as f64;
            self.velocity.z *= AIR_FRICTION as f64;
        } else {
            self.apply_post_move_physics(world);
        }

        // EntityLivingBase.moveEntityWithHeading performs the water/lava
        // escape check after drag and gravity. The resulting 0.3 impulse is
        // therefore not damped again before the next movement tick.
        if !self.flying
            && (self.in_water || self.in_lava)
            && self.collided_horizontally
            && self.is_offset_position_in_liquid(
                world,
                self.velocity.x,
                self.velocity.y + 0.6000000238418579 - self.position.y + water_start_y,
                self.velocity.z,
            )
        {
            self.velocity.y = 0.30000001192092896;
        }

        // EntityPlayer.onLivingUpdate updates jumpMovementFactor after
        // super.onLivingUpdate(), so sprint changes affect air acceleration on
        // the next tick. EntityPlayer.getAIMoveSpeed is different: its override
        // reads the current movement-speed attribute directly.
        let speed_in_air = 0.02f32;
        self.air_acceleration = speed_in_air;
        if self.sprinting {
            self.air_acceleration = (speed_in_air as f64 + speed_in_air as f64 * 0.3) as f32;
        }
        // EntityPlayerSP disables ordinary creative flight after the superclass
        // has completed this tick's flying movement and damping.
        if self.on_ground && self.flying {
            self.flying = false;
        }

        // --- Fall tracking ---
        if self.in_water {
            // Entity.handleWaterMovement clears fallDistance before the living
            // movement branch runs.
            self.fall_start_y = None;
            self.fall_distance = 0.0;
        } else if self.on_ground {
            if let Some(start_y) = self.fall_start_y {
                self.fall_distance = (start_y - self.position.y).max(0.0) as f32;
                self.fall_start_y = None;
            } else {
                self.fall_distance = 0.0;
            }
        } else if !self.flying && self.velocity.y < 0.0 {
            if self.fall_start_y.is_none() {
                self.fall_start_y = Some(self.position.y);
            }
        } else {
            self.fall_start_y = None;
            self.fall_distance = 0.0;
        }

        // Entity.updateRidden runs the rider's normal update with zeroed
        // motion, then lets the vehicle place the rider. The remote vehicle is
        // authoritative in this client, so mirror its final placement after
        // the local update instead of leaving the player at the speculative
        // on-foot collision position.
        self.update_riding_position(entities);

        // --- Sync camera with eye height ---
        // Done dynamically in update_render_position now to include third person offsets,
        // but we sync it here as a baseline for physics.
        let eye = if self.sneaking {
            (PLAYER_EYE_HEIGHT - 0.08) as f32
        } else {
            PLAYER_EYE_HEIGHT as f32
        };
        self.camera.position = Point3::new(
            self.position.x as f32,
            self.position.y as f32 + eye,
            self.position.z as f32,
        );
    }

    /// Creative mode flying — vanilla MC 1.8.9 EntityPlayer.moveEntityWithHeading when flying.
    /// XZ movement uses moveFlying with flySpeed as the acceleration factor.
    /// Y velocity is set in tick() before this call (vanilla: onLivingUpdate before super).
    fn tick_flying(
        &mut self,
        fwd: &Vector3<f32>,
        rgt: &Vector3<f32>,
        mf: f32,
        ms: f32,
        _input: &InputState,
    ) {
        // jumpMovementFactor = flySpeed * (sprinting ? 2 : 1)  (vanilla: EntityPlayer.moveEntityWithHeading)
        let fly_spd = self.fly_speed * if self.sprinting { 2.0 } else { 1.0 };

        // moveFlying adds acceleration to motionX/motionZ based on input direction
        self.move_flying(fwd, rgt, mf, ms, fly_spd);
        self.pre_move_drag_friction = AIR_FRICTION as f64;
        // Note: motionY was already set in tick() (vanilla: onLivingUpdate before super).
        // The damping (d3 * 0.6) and XZ friction (0.91) are applied in tick() after collision.
    }

    /// Walking/running physics — vanilla MC 1.8.9 EntityLivingBase.moveEntityWithHeading (ground branch).
    fn tick_walking(
        &mut self,
        fwd: &Vector3<f32>,
        rgt: &Vector3<f32>,
        mf: f32,
        ms: f32,
        input: &InputState,
        world: &World,
    ) {
        // MC 1.8.9 jump logic: uses held state (movementInput.jump) with a
        // 10-tick cooldown (jumpTicks). The else branch resets the cooldown
        // when the key is released so an immediate re-press can jump again.
        if input.is_held(Action::Jump) {
            if self.on_ground && self.jump_ticks == 0 {
                self.velocity.y = self.jump_upwards_motion();
                if self.sprinting {
                    // EntityLivingBase.jump uses yaw * 0.017453292F here,
                    // unlike moveFlying's yaw * PI / 180.0F expression.
                    let angle = self.camera.mc_yaw_degrees() * 0.017453292_f32;
                    self.velocity.x -= (vanilla_sin(angle) * SPRINT_JUMP_BOOST) as f64;
                    self.velocity.z += (vanilla_cos(angle) * SPRINT_JUMP_BOOST) as f64;
                }
                self.jump_ticks = 10;
            }
        } else {
            self.jump_ticks = 0;
        }

        let friction = if self.on_ground {
            self.block_slipperiness_below(world) * AIR_FRICTION
        } else {
            AIR_FRICTION
        };

        // Vanilla reads friction twice (pre- and post-moveFlying), both from the
        // pre-move position. Save it so apply_post_move_physics uses the same value.
        self.pre_move_drag_friction = friction as f64;

        // EntityPlayer overrides getAIMoveSpeed to read the current attribute,
        // so ground sprint changes apply this tick. Only jumpMovementFactor
        // (the air branch) is deferred by EntityPlayer.onLivingUpdate.
        let acceleration = if self.on_ground {
            self.ground_movement_speed()
                * (GROUND_FRICTION_FACTOR / (friction * friction * friction))
        } else {
            self.air_acceleration
        };

        self.move_flying(fwd, rgt, mf, ms, acceleration);
    }

    /// Swimming physics (when in water) — vanilla MC 1.8.9 EntityLivingBase.moveEntityWithHeading (water branch).
    fn tick_swimming(
        &mut self,
        fwd: &Vector3<f32>,
        rgt: &Vector3<f32>,
        mf: f32,
        ms: f32,
        input: &InputState,
        world: &World,
    ) {
        // EntityLivingBase.updateAITick is called when the jump key is held in
        // water. Sneak does not apply an artificial downward impulse in 1.8.9.
        if input.is_held(Action::Jump) {
            self.velocity.y += 0.03999999910593033;
        }

        // Water movement acceleration (vanilla: moveFlying(strafe, forward, 0.02F))
        // Drag is applied in apply_post_move_physics, NOT here (to avoid double-drag).
        let acceleration = 0.02;
        self.move_flying(fwd, rgt, mf, ms, acceleration);

        // Update oxygen
        if self.is_head_in_liquid(world) {
            self.oxygen -= 1;
            if self.oxygen <= 0 {
                self.oxygen_damage_timer += 1;
                if self.oxygen_damage_timer >= 20 {
                    self.oxygen_damage_timer = 0;
                }
            } else {
                self.oxygen_damage_timer = 0;
            }
        } else {
            self.oxygen = (self.oxygen + 5).min(300);
            self.oxygen_damage_timer = 0;
        }
    }

    /// EntityLivingBase.moveEntityWithHeading lava branch. It has the same
    /// moveFlying factor and jump impulse as water, but uses 0.5 drag.
    fn tick_lava(
        &mut self,
        fwd: &Vector3<f32>,
        rgt: &Vector3<f32>,
        mf: f32,
        ms: f32,
        input: &InputState,
    ) {
        if input.is_held(Action::Jump) {
            self.velocity.y += 0.03999999910593033;
        }
        self.move_flying(fwd, rgt, mf, ms, 0.02);
    }

    /// Post-move gravity and friction — applied after move_with_collision.
    /// Matches vanilla MC 1.8.9 EntityLivingBase.moveEntityWithHeading post-moveEntity section.
    pub fn apply_post_move_physics(&mut self, world: &World) {
        if self.in_water {
            // Water physics (vanilla: EntityLivingBase.moveEntityWithHeading water branch)
            self.velocity.x *= 0.8_f32 as f64;
            self.velocity.y *= 0.8_f32 as f64;
            self.velocity.z *= 0.8_f32 as f64;
            self.velocity.y -= 0.02;
        } else if self.in_lava {
            self.velocity.x *= 0.5;
            self.velocity.y *= 0.5;
            self.velocity.z *= 0.5;
            self.velocity.y -= 0.02;
        } else {
            // Land/air physics (vanilla: EntityLivingBase.moveEntityWithHeading ground branch).
            // The drag friction must use the pre-move block slipperiness, matching the second
            // f4 read in vanilla's moveEntityWithHeading (which happens before moveEntity).
            self.velocity.y -= GRAVITY;
            self.velocity.y *= 0.9800000190734863;
            self.velocity.x *= self.pre_move_drag_friction;
            self.velocity.z *= self.pre_move_drag_friction;
        }
    }

    fn move_flying(
        &mut self,
        _fwd: &Vector3<f32>,
        _rgt: &Vector3<f32>,
        forward: f32,
        strafe_right: f32,
        friction: f32,
    ) {
        // Vanilla Entity.moveFlying: strafe is moveStrafe (positive = left),
        // forward is moveForward (positive = forward).  RustCraft's strafe_right
        // is positive-right, so we negate to get vanilla's strafe.
        let strafe = -strafe_right;
        let mut f = strafe * strafe + forward * forward;

        if f < 1.0e-4 {
            return;
        }

        f = f.sqrt();
        if f < 1.0 {
            f = 1.0;
        }

        f = friction / f;
        let strafe = strafe * f;
        let forward = forward * f;
        let (f1, f2) = vanilla_yaw_sin_cos(self.camera.mc_yaw_degrees());
        // motionX += strafe * f2 - forward * f1
        // motionZ += forward * f2 + strafe * f1
        self.velocity.x += (strafe * f2 - forward * f1) as f64;
        self.velocity.z += (forward * f2 + strafe * f1) as f64;
    }

    fn block_slipperiness_below(&self, world: &World) -> f32 {
        let x = self.position.x.floor() as i32;
        let y = (self.position.y - 1.0).floor() as i32;
        let z = self.position.z.floor() as i32;
        world.get_block(x, y, z).properties().slipperiness
    }

    /// EntityPlayerSP.pushOutOfBlocks. A position is open only when its block
    /// and the block directly above are both non-normal cubes; the nearest
    /// horizontal open neighbor replaces one motion component with +/-0.1.
    fn push_out_of_blocks(&mut self, world: &World, x: f64, y: f64, z: f64) {
        let block_x = x.floor() as i32;
        let block_y = y.floor() as i32;
        let block_z = z.floor() as i32;
        let is_open =
            |x, y, z| !world.is_normal_cube(x, y, z) && !world.is_normal_cube(x, y + 1, z);
        if is_open(block_x, block_y, block_z) {
            return;
        }

        let local_x = x - block_x as f64;
        let local_z = z - block_z as f64;
        let mut distance = 9999.0;
        let mut direction = None;
        if is_open(block_x - 1, block_y, block_z) && local_x < distance {
            distance = local_x;
            direction = Some(0);
        }
        if is_open(block_x + 1, block_y, block_z) && 1.0 - local_x < distance {
            distance = 1.0 - local_x;
            direction = Some(1);
        }
        if is_open(block_x, block_y, block_z - 1) && local_z < distance {
            distance = local_z;
            direction = Some(4);
        }
        if is_open(block_x, block_y, block_z + 1) && 1.0 - local_z < distance {
            direction = Some(5);
        }

        match direction {
            Some(0) => self.velocity.x = -0.1,
            Some(1) => self.velocity.x = 0.1,
            Some(4) => self.velocity.z = -0.1,
            Some(5) => self.velocity.z = 0.1,
            _ => {}
        }
    }

    fn ground_movement_speed(&self) -> f32 {
        let attribute = &self.movement_speed_attribute;
        let mut base = attribute.base;
        for modifier in attribute.modifiers.iter().filter(|modifier| {
            modifier.operation == 0 && !MovementSpeedAttribute::is_local_dynamic_modifier(modifier)
        }) {
            base += modifier.amount;
        }
        let mut value = base;
        for modifier in attribute.modifiers.iter().filter(|modifier| {
            modifier.operation == 1 && !MovementSpeedAttribute::is_local_dynamic_modifier(modifier)
        }) {
            value += base * modifier.amount;
        }
        for modifier in attribute.modifiers.iter().filter(|modifier| {
            modifier.operation == 2 && !MovementSpeedAttribute::is_local_dynamic_modifier(modifier)
        }) {
            value *= 1.0 + modifier.amount;
        }
        for effect in &self.active_effects {
            let amplifier = (effect.amplifier as i32 + 1) as f64;
            match effect.effect_id {
                1 => value *= 1.0 + 0.20000000298023224 * amplifier,
                2 => value *= 1.0 - 0.15000000596046448 * amplifier,
                _ => {}
            }
        }
        // Vanilla applies the sprint modifier through the attribute system
        // (operation 2, amount 0.30000001192092896D) in double precision,
        // then casts the final result to float.  Computing it separately in
        // f32 produces a slightly different bit pattern that accumulates over
        // ticks and causes GrimAC Simulation violations.
        if self.sprinting {
            value *= 1.0 + 0.30000001192092896;
        }
        value.clamp(0.0, 1024.0) as f32
    }

    fn jump_upwards_motion(&self) -> f64 {
        let mut motion = JUMP_VELOCITY as f64;
        if let Some(effect) = self
            .active_effects
            .iter()
            .find(|effect| effect.effect_id == 8)
        {
            let boost = (effect.amplifier as i32 + 1) as f32 * 0.1_f32;
            motion += boost as f64;
        }
        motion
    }

    fn block_below(&self, world: &World) -> crate::world::block::Block {
        let x = self.position.x.floor() as i32;
        let y = (self.position.y - 0.20000000298023224).floor() as i32;
        let z = self.position.z.floor() as i32;
        let block = world.get_block(x, y, z);
        if block == crate::world::block::Block::Air {
            let below = world.get_block(x, y - 1, z);
            if matches!(
                below,
                crate::world::block::Block::OakFence
                    | crate::world::block::Block::SpruceFence
                    | crate::world::block::Block::BirchFence
                    | crate::world::block::Block::JungleFence
                    | crate::world::block::Block::DarkOakFence
                    | crate::world::block::Block::AcaciaFence
                    | crate::world::block::Block::NetherBrickFence
                    | crate::world::block::Block::CobblestoneWall
                    | crate::world::block::Block::OakFenceGate
                    | crate::world::block::Block::SpruceFenceGate
                    | crate::world::block::Block::BirchFenceGate
                    | crate::world::block::Block::JungleFenceGate
                    | crate::world::block::Block::DarkOakFenceGate
                    | crate::world::block::Block::AcaciaFenceGate
            ) {
                return below;
            }
        }
        block
    }

    fn is_on_ladder(&self, world: &World) -> bool {
        matches!(
            world.get_block(
                self.position.x.floor() as i32,
                self.position.y.floor() as i32,
                self.position.z.floor() as i32,
            ),
            crate::world::block::Block::Ladder | crate::world::block::Block::Vine
        )
    }

    fn intersects_block(&self, world: &World, target: crate::world::block::Block) -> bool {
        let bb = Aabb::new(&self.position, PLAYER_WIDTH, PLAYER_HEIGHT);
        let min_x = (bb.min_x + 0.001).floor() as i32;
        let min_y = (bb.min_y + 0.001).floor() as i32;
        let min_z = (bb.min_z + 0.001).floor() as i32;
        let max_x = (bb.max_x - 0.001).floor() as i32;
        let max_y = (bb.max_y - 0.001).floor() as i32;
        let max_z = (bb.max_z - 0.001).floor() as i32;
        (min_x..=max_x).any(|x| {
            (min_y..=max_y).any(|y| (min_z..=max_z).any(|z| world.get_block(x, y, z) == target))
        })
    }

    fn boat_collision_boxes(
        &self,
        entities: &crate::entity::EntityManager,
        local_entity_id: Option<i32>,
    ) -> Vec<Aabb> {
        entities
            .entities
            .values()
            .filter(|entity| entity.entity_type == crate::entity::EntityType::Boat)
            .filter(|entity| Some(entity.entity_id) != self.vehicle_id)
            .filter(|entity| entity.vehicle_id != local_entity_id)
            .map(|entity| {
                let position = Point3::new(
                    entity.position.x as f64,
                    entity.position.y as f64,
                    entity.position.z as f64,
                );
                Aabb::new(&position, 1.5_f32 as f64, 0.6_f32 as f64)
            })
            .collect()
    }

    fn update_riding_position(&mut self, entities: &crate::entity::EntityManager) {
        let Some(vehicle_id) = self.vehicle_id else {
            return;
        };
        let Some(vehicle) = entities.get(vehicle_id) else {
            return;
        };

        let vehicle_position = vehicle.position;
        let (x, y, z) = match vehicle.entity_type {
            // EntityBoat.updateRiderPosition overrides the generic entity
            // placement with a 0.4 block yaw-relative seat offset.
            crate::entity::EntityType::Boat => {
                let yaw = (vehicle.yaw as f64).to_radians();
                (
                    vehicle_position.x as f64 + yaw.cos() * 0.4,
                    vehicle_position.y as f64 - 0.3 - 0.35,
                    vehicle_position.z as f64 + yaw.sin() * 0.4,
                )
            }
            // Entity.updateRiderPosition uses getMountedYOffset() + the
            // player's -0.35 yOffset. Minecarts override mounted offset to 0;
            // other currently represented vehicles use Entity's height * .75.
            crate::entity::EntityType::MinecartEmpty
            | crate::entity::EntityType::MinecartChest
            | crate::entity::EntityType::MinecartFurnace
            | crate::entity::EntityType::MinecartTNT
            | crate::entity::EntityType::MinecartHopper
            | crate::entity::EntityType::MinecartSpawner
            | crate::entity::EntityType::MinecartCommand => (
                vehicle_position.x as f64,
                vehicle_position.y as f64 - 0.35,
                vehicle_position.z as f64,
            ),
            _ => {
                let (_, height) = vehicle.entity_type.bounding_box();
                (
                    vehicle_position.x as f64,
                    vehicle_position.y as f64 + height as f64 * 0.75 - 0.35,
                    vehicle_position.z as f64,
                )
            }
        };
        self.position = Point3::new(x, y, z);
        self.velocity = Vector3::zeros();
        self.fall_start_y = None;
        self.fall_distance = 0.0;
    }

    fn handle_water_movement(&mut self, world: &World) -> bool {
        // Vanilla: expand(0, -0.4, 0).contract(0.001, 0.001, 0.001)
        // expand(-y) raises minY and lowers maxY (shrinks vertically inward);
        // contract shrinks all six faces inward.
        // Result: detects water touching the player's lower body (0.4–1.4 above feet).
        let hw = PLAYER_WIDTH / 2.0;
        let bb = Aabb {
            min_x: self.position.x - hw + 0.001,
            min_y: self.position.y + 0.4000000059604645 + 0.001,
            min_z: self.position.z - hw + 0.001,
            max_x: self.position.x + hw - 0.001,
            max_y: self.position.y + PLAYER_HEIGHT - 0.4000000059604645 - 0.001,
            max_z: self.position.z + hw - 0.001,
        };
        let min_x = (bb.min_x + 0.001).floor() as i32;
        let min_y = (bb.min_y + 0.001).floor() as i32;
        let min_z = (bb.min_z + 0.001).floor() as i32;
        let max_x = (bb.max_x - 0.001 + 1.0).floor() as i32;
        let max_y = (bb.max_y + 1.0).floor() as i32;
        let max_z = (bb.max_z - 0.001 + 1.0).floor() as i32;
        let mut flow = Vector3::zeros();
        let mut in_water = false;

        for x in min_x..max_x {
            for y in min_y..max_y {
                for z in min_z..max_z {
                    let block = world.get_block(x, y, z);
                    if !matches!(
                        block,
                        crate::world::block::Block::FlowingWater
                            | crate::world::block::Block::StillWater
                    ) {
                        continue;
                    }
                    // World.handleMaterialAcceleration only considers a
                    // liquid voxel when the scanned AABB's integer upper Y
                    // bound reaches that voxel's real fluid surface.  Treating
                    // every flowing-water voxel as full height made the client
                    // enter swimming physics too early at shallow edges.
                    let metadata = world.get_block_metadata(x, y, z) as i32;
                    let effective_level = if metadata >= 8 { 0 } else { metadata };
                    let liquid_height_percent = (effective_level + 1) as f64 / 9.0;
                    let liquid_surface = y as f64 + 1.0 - liquid_height_percent;
                    if (max_y as f64) < liquid_surface {
                        continue;
                    }
                    in_water = true;
                    flow += self.water_flow_vector(world, x, y, z, block);
                }
            }
        }

        if in_water && flow.magnitude_squared() > 0.0 {
            self.velocity += flow.normalize() * 0.014;
        }
        in_water
    }

    /// Applies the client-side entity movement from an extending piston.
    /// Vanilla performs this in TileEntityPiston before the authoritative block
    /// updates arrive, so it cannot wait for normal static-block collision.
    pub fn push_by_extending_piston(
        &mut self,
        world: &World,
        piston_x: i32,
        piston_y: i32,
        piston_z: i32,
        facing: u8,
    ) {
        let (dx, dy, dz) = match facing {
            0 => (0, -1, 0),
            1 => (0, 1, 0),
            2 => (0, 0, -1),
            3 => (0, 0, 1),
            4 => (-1, 0, 0),
            5 => (1, 0, 0),
            _ => return,
        };

        let player_bb = Aabb::new(&self.position, PLAYER_WIDTH, PLAYER_HEIGHT);
        let mut block_x = piston_x + dx;
        let mut block_y = piston_y + dy;
        let mut block_z = piston_z + dz;
        let mut pushed = false;
        let mut slime = false;

        // The head always sweeps through the first cell ahead of the base.
        // Consecutive occupied cells are the blocks carried by this extension.
        for index in 0..12 {
            let block = world.get_block(block_x, block_y, block_z);
            let overlaps = player_bb.overlaps_block(block_x, block_y, block_z)
                || player_bb.overlaps_block(block_x + dx, block_y + dy, block_z + dz);
            pushed |= overlaps;
            slime |= overlaps && block == crate::world::block::Block::SlimeBlock;

            if index == 11 || block == crate::world::block::Block::Air {
                break;
            }
            block_x += dx;
            block_y += dy;
            block_z += dz;
        }

        if !pushed {
            return;
        }

        if slime {
            self.velocity.x = dx as f64;
            self.velocity.y = dy as f64;
            self.velocity.z = dz as f64;
        } else {
            self.position.x += dx as f64;
            self.position.y += dy as f64;
            self.position.z += dz as f64;
        }
    }

    fn is_in_lava(&self, world: &World) -> bool {
        // Vanilla: expand(-0.1, -0.4, -0.1)
        // Shrinks laterally by 0.1 on each side and vertically inward
        // (raises minY by 0.4, lowers maxY by 0.4) — only the core body.
        let hw = PLAYER_WIDTH / 2.0;
        let bb = Aabb {
            min_x: self.position.x - hw + 0.10000000149011612,
            min_y: self.position.y + 0.4000000059604645,
            min_z: self.position.z - hw + 0.10000000149011612,
            max_x: self.position.x + hw - 0.10000000149011612,
            max_y: self.position.y + PLAYER_HEIGHT - 0.4000000059604645,
            max_z: self.position.z + hw - 0.10000000149011612,
        };
        self.aabb_contains_material(world, &bb, false)
    }

    fn water_flow_vector(
        &self,
        world: &World,
        x: i32,
        y: i32,
        z: i32,
        water: crate::world::block::Block,
    ) -> Vector3<f64> {
        let decay = |x, y, z| {
            if world.get_block(x, y, z) != water {
                -1
            } else {
                let level = world.get_block_metadata(x, y, z) as i32;
                if level >= 8 {
                    0
                } else {
                    level
                }
            }
        };
        let level = decay(x, y, z);
        let mut flow = Vector3::zeros();
        for (dx, dz) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let neighbor = decay(x + dx, y, z + dz);
            if neighbor < 0 {
                let neighbor_block = world.get_block(x + dx, y, z + dz);
                if !neighbor_block.is_solid() {
                    let below = decay(x + dx, y - 1, z + dz);
                    if below >= 0 {
                        let delta = below - (level - 8);
                        flow.x += dx as f64 * delta as f64;
                        flow.z += dz as f64 * delta as f64;
                    }
                }
            } else {
                let delta = neighbor - level;
                flow.x += dx as f64 * delta as f64;
                flow.z += dz as f64 * delta as f64;
            }
        }
        // Falling liquid adds a strong down component next to a solid face.
        if world.get_block_metadata(x, y, z) >= 8
            && [(0, -1), (0, 1), (-1, 0), (1, 0)]
                .into_iter()
                .any(|(dx, dz)| {
                    world.get_block(x + dx, y, z + dz).is_solid()
                        || world.get_block(x + dx, y + 1, z + dz).is_solid()
                })
        {
            if flow.magnitude_squared() > 0.0 {
                flow = flow.normalize();
            }
            flow.y -= 6.0;
        }
        flow
    }

    fn is_offset_position_in_liquid(&self, world: &World, x: f64, y: f64, z: f64) -> bool {
        let bb = Aabb::new(&self.position, PLAYER_WIDTH, PLAYER_HEIGHT).offset(x, y, z);
        // Despite the vanilla method name, this is the escape check used
        // after a horizontal water/lava collision: the offset AABB must be
        // clear of blocks and outside all liquid.
        get_colliding_boxes(&bb, world).is_empty() && !self.aabb_contains_liquid(world, &bb)
    }

    fn aabb_contains_liquid(&self, world: &World, bb: &Aabb) -> bool {
        self.aabb_contains_material(world, bb, true)
    }

    fn aabb_contains_material(&self, world: &World, bb: &Aabb, include_water: bool) -> bool {
        let min_x = bb.min_x.floor() as i32;
        let min_y = bb.min_y.floor() as i32;
        let min_z = bb.min_z.floor() as i32;
        let max_x = (bb.max_x + 1.0).floor() as i32;
        let max_y = (bb.max_y + 1.0).floor() as i32;
        let max_z = (bb.max_z + 1.0).floor() as i32;
        (min_x..max_x).any(|x| {
            (min_y..max_y).any(|y| {
                (min_z..max_z).any(|z| {
                    if include_water {
                        matches!(
                            world.get_block(x, y, z),
                            crate::world::block::Block::FlowingWater
                                | crate::world::block::Block::StillWater
                                | crate::world::block::Block::FlowingLava
                                | crate::world::block::Block::StillLava
                        )
                    } else {
                        matches!(
                            world.get_block(x, y, z),
                            crate::world::block::Block::FlowingLava
                                | crate::world::block::Block::StillLava
                        )
                    }
                })
            })
        })
    }

    /// Check if the player's eyes are submerged in liquid.
    fn is_head_in_liquid(&self, world: &World) -> bool {
        let x = self.position.x.floor() as i32;
        let y = (self.position.y + PLAYER_EYE_HEIGHT * 0.5).floor() as i32;
        let z = self.position.z.floor() as i32;
        world.get_block(x, y, z).is_liquid()
    }

    /// Check if the player's feet are in liquid.
    fn is_feet_in_liquid(&self, world: &World) -> bool {
        let x = self.position.x.floor() as i32;
        let y = self.position.y.floor() as i32;
        let z = self.position.z.floor() as i32;
        world.get_block(x, y, z).is_liquid()
    }

    pub fn update_aspect(&mut self, w: u32, h: u32) {
        self.camera.update_aspect(w, h);
    }
    pub fn process_mouse(&mut self, dx: f32, dy: f32, sensitivity: f32, invert_y: bool) {
        self.camera.process_mouse(dx, dy, sensitivity, invert_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{block::Block, chunk::Chunk};

    fn test_world(block: Block, metadata: u8) -> World {
        let mut world = World::new();
        world.chunks.insert((0, 0), Chunk::new(0, 0).into());
        world.set_block_state(0, 0, 0, (block.to_id() << 4) | metadata as u16);
        world
    }

    #[test]
    fn horizontal_movement_basis_ignores_look_pitch() {
        // Looking straight up/down changes Camera::front's XZ length, but
        // vanilla moveFlying still produces the same yaw-based movement.
        // Uses MathHelper's lookup table, so tolerance is the table's ULP.
        let (forward, right) = horizontal_movement_basis(-180.0);
        assert!((forward.z + 1.0).abs() < 0.001, "forward.z = {}", forward.z);
        assert!(forward.x.abs() < 0.001, "forward.x = {}", forward.x);
        assert!((right.x - 1.0).abs() < 0.001, "right.x = {}", right.x);
        assert!(right.z.abs() < 0.001, "right.z = {}", right.z);
    }

    #[test]
    fn water_and_lava_use_vanilla_aabb_material_checks() {
        let mut player = Player::new(Point3::new(0.5, 0.0, 0.5), 1.0);
        let water = test_world(Block::StillWater, 0);
        assert!(player.handle_water_movement(&water));
        assert!(!player.is_in_lava(&water));

        let lava = test_world(Block::StillLava, 0);
        assert!(!player.handle_water_movement(&lava));
        assert!(player.is_in_lava(&lava));
    }

    #[test]
    fn water_escape_check_requires_an_open_non_liquid_offset() {
        let mut world = test_world(Block::Air, 0);
        let player = Player::new(Point3::new(0.5, 0.0, 0.5), 1.0);

        // The offset AABB is clear and outside liquid, so vanilla applies the
        // upward escape impulse after a horizontal water collision.
        assert!(player.is_offset_position_in_liquid(&world, 1.0, 0.0, 0.0));

        world.set_block(1, 0, 0, Block::StillWater);
        assert!(!player.is_offset_position_in_liquid(&world, 1.0, 0.0, 0.0));

        world.set_block(1, 0, 0, Block::Stone);
        assert!(!player.is_offset_position_in_liquid(&world, 1.0, 0.0, 0.0));
    }

    #[test]
    fn swimming_keeps_vertical_motion_without_a_non_vanilla_clamp() {
        let world = test_world(Block::StillWater, 0);
        let mut player = Player::new(Point3::new(0.5, 0.0, 0.5), 1.0);
        player.velocity.y = 0.42;
        let (forward_basis, right_basis) =
            horizontal_movement_basis(player.camera.mc_yaw_degrees());
        let input = InputState::new();

        player.tick_swimming(&forward_basis, &right_basis, 0.0, 0.0, &input, &world);

        assert_eq!(player.velocity.y, 0.42);
    }

    #[test]
    fn extending_piston_moves_players_and_slime_sets_piston_velocity() {
        let world = test_world(Block::Air, 0);
        let mut player = Player::new(Point3::new(0.5, 1.0, 0.5), 1.0);
        player.push_by_extending_piston(&world, 0, 0, 0, 1);
        assert_eq!(player.position.y, 2.0);

        let slime = test_world(Block::SlimeBlock, 0);
        let mut player = Player::new(Point3::new(0.5, 1.0, 0.5), 1.0);
        player.push_by_extending_piston(&slime, 0, -1, 0, 1);
        assert_eq!(player.velocity.y, 1.0);
    }

    #[test]
    fn movement_potions_apply_vanilla_attribute_operations() {
        let mut player = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        player.active_effects = vec![
            crate::entity::EntityEffectState {
                effect_id: 1,
                amplifier: 1,
                duration: 100,
                hide_particles: false,
            },
            crate::entity::EntityEffectState {
                effect_id: 2,
                amplifier: 0,
                duration: 100,
                hide_particles: false,
            },
        ];
        let expected = ((WALK_SPEED as f64 * (1.0 + 0.20000000298023224 * 2.0))
            * (1.0 - 0.15000000596046448)) as f32;
        assert_eq!(player.ground_movement_speed(), expected);

        player.sprinting = true;
        assert_eq!(
            player.ground_movement_speed(),
            ((WALK_SPEED as f64 * (1.0 + 0.20000000298023224 * 2.0))
                * (1.0 - 0.15000000596046448)
                * (1.0 + 0.30000001192092896)) as f32
        );
    }

    #[test]
    fn ground_sprint_attribute_applies_on_the_transition_tick() {
        let world = test_world(Block::Stone, 0);
        let mut player = Player::new(Point3::new(0.5, 1.0, 0.5), 1.0);
        player.on_ground = true;
        let (forward_basis, right_basis) =
            horizontal_movement_basis(player.camera.mc_yaw_degrees());
        let input = InputState::new();

        let horizontal_acceleration = |player: &mut Player| {
            player.velocity = Vector3::zeros();
            player.tick_walking(&forward_basis, &right_basis, 0.98, 0.0, &input, &world);
            (player.velocity.x * player.velocity.x + player.velocity.z * player.velocity.z).sqrt()
        };

        player.set_sprinting(false);
        let walking = horizontal_acceleration(&mut player);
        player.set_sprinting(true);
        let sprinting = horizontal_acceleration(&mut player);
        player.set_sprinting(false);
        let walking_after_stop = horizontal_acceleration(&mut player);

        let expected_ratio = (1.0 + 0.30000001192092896) as f32 as f64;
        assert!((sprinting / walking - expected_ratio).abs() < 1.0e-6);
        assert_eq!(walking_after_stop, walking);
    }

    #[test]
    fn sprinting_uses_vanilla_six_hundred_tick_lifetime() {
        let mut player = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        player.set_sprinting(true);
        assert!(player.sprinting);
        assert_eq!(player.sprinting_ticks_left, 600);
        player.set_sprinting(false);
        assert!(!player.sprinting);
        assert_eq!(player.sprinting_ticks_left, 0);
    }

    #[test]
    fn successful_attack_applies_vanilla_knockback_slowdown() {
        let mut sprinting = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        sprinting.velocity = Vector3::new(0.5, 0.25, -0.25);
        sprinting.set_sprinting(true);
        sprinting.on_attack_entity(0);
        assert_eq!(sprinting.velocity, Vector3::new(0.3, 0.25, -0.15));
        assert!(!sprinting.sprinting);

        let mut enchanted = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        enchanted.velocity = Vector3::new(0.5, 0.25, -0.25);
        enchanted.on_attack_entity(1);
        assert_eq!(enchanted.velocity, Vector3::new(0.3, 0.25, -0.15));

        let mut ordinary = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        ordinary.velocity = Vector3::new(0.5, 0.25, -0.25);
        ordinary.on_attack_entity(0);
        assert_eq!(ordinary.velocity, Vector3::new(0.5, 0.25, -0.25));
    }

    #[test]
    fn saddled_horse_jump_sends_vanilla_release_charge() {
        let world = test_world(Block::Air, 0);
        let mut entities = crate::entity::EntityManager::new();
        let mut horse = crate::entity::Entity::new(
            7,
            crate::entity::EntityType::Horse,
            Point3::new(0.0, 0.0, 0.0),
        );
        horse.visual.horse_saddled = true;
        entities.spawn(horse);
        let mut player = Player::new(Point3::new(0.0, 2.0, 0.0), 1.0);
        player.vehicle_id = Some(7);
        let mut input = InputState::new();
        input.on_key_down(Action::Jump);
        player.tick(&input, &world, &entities, Some(1));
        input.tick_reset();
        player.tick(&input, &world, &entities, Some(1));
        input.on_key_up(Action::Jump);
        player.tick(&input, &world, &entities, Some(1));
        assert_eq!(player.take_pending_horse_jump(), Some(10));
    }

    #[test]
    fn entity_properties_replace_local_movement_speed_snapshot() {
        let mut player = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        player.sprinting = true;
        player.add_effect(crate::entity::EntityEffectState {
            effect_id: 1,
            amplifier: 0,
            duration: 100,
            hide_particles: false,
        });
        player.apply_entity_properties(vec![crate::net::packet::EntityProperty {
            key: "generic.movementSpeed".to_string(),
            value: 0.2,
            modifiers: vec![
                crate::net::packet::EntityPropertyModifier {
                    uuid: "custom-add".to_string(),
                    amount: 0.1,
                    operation: 0,
                },
                crate::net::packet::EntityPropertyModifier {
                    uuid: "custom-base-scale".to_string(),
                    amount: 0.5,
                    operation: 1,
                },
                // A snapshot may already contain a local potion/sprint UUID.
                // Vanilla replaces that modifier rather than applying it twice.
                crate::net::packet::EntityPropertyModifier {
                    uuid: SPEED_POTION_MODIFIER.to_string(),
                    amount: 0.2,
                    operation: 2,
                },
                crate::net::packet::EntityPropertyModifier {
                    uuid: SPRINTING_SPEED_MODIFIER.to_ascii_lowercase(),
                    amount: 0.3,
                    operation: 2,
                },
            ],
        }]);

        // ModifiableAttributeInstance: ((0.2 + 0.1) + 0.3 * 0.5) * 1.2 * 1.3.
        assert_eq!(
            player.ground_movement_speed(),
            (0.45_f64 * 1.2 * 1.3) as f32
        );
    }

    #[test]
    fn riding_position_uses_vanilla_boat_and_minecart_offsets() {
        let mut player = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        let mut entities = crate::entity::EntityManager::new();
        let mut boat = crate::entity::Entity::new(
            7,
            crate::entity::EntityType::Boat,
            Point3::new(10.0, 20.0, 30.0),
        );
        boat.yaw = 90.0;
        entities.spawn(boat);
        player.vehicle_id = Some(7);
        player.update_riding_position(&entities);
        assert_eq!(
            player.position,
            Point3::new(10.0, 20.0_f32 as f64 - 0.3 - 0.35, 30.4)
        );

        let minecart = crate::entity::Entity::new(
            8,
            crate::entity::EntityType::MinecartHopper,
            Point3::new(-4.0, 12.0, 3.0),
        );
        entities.spawn(minecart);
        player.vehicle_id = Some(8);
        player.update_riding_position(&entities);
        assert_eq!(player.position, Point3::new(-4.0, 11.65, 3.0));
    }

    #[test]
    fn player_push_out_uses_nearest_horizontal_open_block() {
        let world = test_world(Block::Stone, 0);
        let mut player = Player::new(Point3::new(0.0, 0.0, 0.0), 1.0);
        player.push_out_of_blocks(&world, 0.21, 0.5, 0.21);
        // West wins the equal-distance tie because vanilla checks it first.
        assert_eq!(player.velocity.x, -0.1);
        assert_eq!(player.velocity.z, 0.0);
    }

    #[test]
    fn player_push_out_does_not_treat_slab_as_normal_cube() {
        let world = test_world(Block::StoneSlab, 0);
        let mut player = Player::new(Point3::new(0.0, 0.0, 0.0), 1.0);
        player.push_out_of_blocks(&world, 0.21, 0.5, 0.21);
        assert_eq!(player.velocity, Vector3::zeros());
    }

    #[test]
    fn player_push_out_does_not_treat_redstone_block_as_normal_cube() {
        let world = test_world(Block::RedstoneBlock, 0);
        let mut player = Player::new(Point3::new(0.0, 0.0, 0.0), 1.0);
        player.push_out_of_blocks(&world, 0.21, 0.5, 0.21);
        assert_eq!(player.velocity, Vector3::zeros());
    }

    #[test]
    fn soul_sand_collision_detects_airborne_aabb_overlap() {
        let world = test_world(Block::SoulSand, 0);
        let player = Player::new(Point3::new(0.5, 0.5, 0.5), 1.0);
        assert!(player.intersects_block(&world, Block::SoulSand));
    }

    #[test]
    fn jump_boost_uses_float_increment_promoted_to_double() {
        let mut player = Player::new(Point3::new(0.0, 64.0, 0.0), 1.0);
        player
            .active_effects
            .push(crate::entity::EntityEffectState {
                effect_id: 8,
                amplifier: 2,
                duration: 100,
                hide_particles: false,
            });
        assert_eq!(
            player.jump_upwards_motion(),
            JUMP_VELOCITY as f64 + (3.0_f32 * 0.1_f32) as f64
        );
    }

    #[test]
    fn test_frustum_signs() {
        let f = 1.0 / (80.0f32.to_radians() / 2.0).tan();
        let n = 0.1;
        let fa = 100.0;
        let aspect = 1.77;

        let proj = Matrix4::new(
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            -f,
            0.0,
            0.0,
            0.0,
            0.0,
            fa / (n - fa),
            fa * n / (n - fa),
            0.0,
            0.0,
            -1.0,
            0.0,
        );

        let pos = Point3::new(10.0, 20.0, 30.0);

        for yaw_deg in (-180..=180).step_by(30) {
            for pitch_deg in (-60..=60).step_by(30) {
                let yaw = (yaw_deg as f32).to_radians();
                let pitch = (pitch_deg as f32).to_radians();

                let front = Vector3::new(
                    yaw.cos() * pitch.cos(),
                    pitch.sin(),
                    yaw.sin() * pitch.cos(),
                )
                .normalize();
                let right = front.cross(&Vector3::new(0.0, 1.0, 0.0)).normalize();
                let up = right.cross(&front).normalize();

                let view = Matrix4::look_at_rh(&pos, &(pos + front), &up);
                let vp = proj * view;
                let frustum = Frustum::from_view_proj(&vp);

                // 1. Point in center front: should be inside all planes
                let pt_center = pos + front * 10.0;
                let pt_center_arr = [pt_center.x, pt_center.y, pt_center.z];
                for (idx, plane) in frustum.planes.iter().enumerate() {
                    let val = plane[0] * pt_center_arr[0]
                        + plane[1] * pt_center_arr[1]
                        + plane[2] * pt_center_arr[2]
                        + plane[3];
                    assert!(
                        val >= -0.01,
                        "Center point culled! yaw: {}, pitch: {}, plane: {}, val: {}",
                        yaw_deg,
                        pitch_deg,
                        idx,
                        val
                    );
                }

                // 2. Point far left (off-screen left): should be culled by the left-right plane
                let pt_left = pos + front * 10.0 - right * 15.0;
                let pt_left_arr = [pt_left.x, pt_left.y, pt_left.z];
                let mut left_inside = true;
                for plane in &frustum.planes {
                    let val = plane[0] * pt_left_arr[0]
                        + plane[1] * pt_left_arr[1]
                        + plane[2] * pt_left_arr[2]
                        + plane[3];
                    if val < 0.0 {
                        left_inside = false;
                        break;
                    }
                }
                assert!(
                    !left_inside,
                    "Off-screen left point not culled! yaw: {}, pitch: {}",
                    yaw_deg, pitch_deg
                );

                // 3. Point far right (off-screen right): should be culled
                let pt_right = pos + front * 10.0 + right * 15.0;
                let pt_right_arr = [pt_right.x, pt_right.y, pt_right.z];
                let mut right_inside = true;
                for plane in &frustum.planes {
                    let val = plane[0] * pt_right_arr[0]
                        + plane[1] * pt_right_arr[1]
                        + plane[2] * pt_right_arr[2]
                        + plane[3];
                    if val < 0.0 {
                        right_inside = false;
                        break;
                    }
                }
                assert!(
                    !right_inside,
                    "Off-screen right point not culled! yaw: {}, pitch: {}",
                    yaw_deg, pitch_deg
                );

                // 4. Point far up (off-screen top): should be culled
                let pt_up = pos + front * 10.0 + up * 15.0;
                let pt_up_arr = [pt_up.x, pt_up.y, pt_up.z];
                let mut up_inside = true;
                for plane in &frustum.planes {
                    let val = plane[0] * pt_up_arr[0]
                        + plane[1] * pt_up_arr[1]
                        + plane[2] * pt_up_arr[2]
                        + plane[3];
                    if val < 0.0 {
                        up_inside = false;
                        break;
                    }
                }
                assert!(
                    !up_inside,
                    "Off-screen top point not culled! yaw: {}, pitch: {}",
                    yaw_deg, pitch_deg
                );

                // 5. Point far down (off-screen bottom): should be culled
                let pt_down = pos + front * 10.0 - up * 15.0;
                let pt_down_arr = [pt_down.x, pt_down.y, pt_down.z];
                let mut down_inside = true;
                for plane in &frustum.planes {
                    let val = plane[0] * pt_down_arr[0]
                        + plane[1] * pt_down_arr[1]
                        + plane[2] * pt_down_arr[2]
                        + plane[3];
                    if val < 0.0 {
                        down_inside = false;
                        break;
                    }
                }
                assert!(
                    !down_inside,
                    "Off-screen bottom point not culled! yaw: {}, pitch: {}",
                    yaw_deg, pitch_deg
                );
            }
        }
    }

    #[test]
    fn projection_uses_vulkan_zero_to_one_depth_range() {
        let camera = Camera::new(Point3::origin(), 16.0 / 9.0);
        let projection = camera.projection_matrix();
        let near = projection * nalgebra::Vector4::new(0.0, 0.0, -camera.near, 1.0);
        let far = projection * nalgebra::Vector4::new(0.0, 0.0, -camera.far, 1.0);
        let before_near = projection * nalgebra::Vector4::new(0.0, 0.0, -camera.near * 0.5, 1.0);

        assert!(near.z.abs() < 1e-5);
        assert!((far.z / far.w - 1.0).abs() < 1e-5);
        assert!(before_near.z < 0.0);
    }

    #[test]
    fn sky_view_ignores_world_position_but_preserves_camera_effects() {
        let mut camera = Camera::new(Point3::new(12.0, 80.0, -4.0), 16.0 / 9.0);
        let stable = camera.sky_view_matrix();

        camera.position = Point3::new(-35.0, 7.0, 128.0);
        let moved = camera.sky_view_matrix();
        assert!((stable - moved).abs().max() < 1e-5);

        camera.view_bobbing = true;
        camera.bob_amount = 0.8;
        camera.bob_phase = 0.35;
        camera.hurt_time = 0.2;
        camera.prev_hurt_time = 0.2;

        assert!((camera.sky_view_matrix() - moved).abs().max() > 1e-3);
    }

    #[test]
    fn protocol_yaw_remains_continuous_across_a_full_turn() {
        let mut camera = Camera::new(Point3::origin(), 1.0);
        camera.set_from_mc_yaw(179.0);
        camera.add_mc_yaw(2.0);

        assert_eq!(camera.mc_yaw_degrees(), 181.0);
    }
}
