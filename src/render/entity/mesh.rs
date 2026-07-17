//! 3D entity mesh generation for Minecraft entities.
//!
//! Generates vertex data for box-based models (like MC 1.8.9 Java Edition).
//! Each entity is composed of cuboid parts with rotations and animations.

use nalgebra::{Matrix4, Point3, Rotation3, Vector3};

/// Vertex format for entity rendering (position + normal + UV + color tint)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// A single cuboid part of an entity model.
///
/// Matches vanilla `ModelRenderer` + `ModelBox` semantics: a box of `size`
/// whose min corner sits at `box_offset` relative to `rotation_point`, rotated
/// around `rotation_point` by `rotation` (plus per-frame animation for limbs).
/// All spatial fields are in RustCraft block units (+Y up, feet at y=0);
/// `rotation` is in radians (already converted from MC's +Y-down convention —
/// X and Z angles are negated, Y is unchanged).
#[derive(Clone, Copy, Debug)]
pub struct ModelCuboid {
    /// Rotation point (the joint) in block units, +Y up.
    pub rotation_point: [f32; 3],
    /// Box min-corner offset from `rotation_point`, in block units, +Y up.
    pub box_offset: [f32; 3],
    /// Box size (w, h, d) in block units.
    pub size: [f32; 3],
    /// Base rotation (rx, ry, rz) in radians (RC convention), applied around
    /// `rotation_point`. Used as the static pose for Body/Extra parts (e.g.
    /// quadruped body = PI/2, zombie arms = PI/2 forward).
    pub rotation: [f32; 3],
    /// Mirror texture horizontally (vanilla `mirror` flag for left limbs).
    pub mirror: bool,
    /// Per-face UV coordinates in atlas space (0.0–1.0).
    /// Order: [-Z(back), +Z(front), +Y(top), -Y(bottom), -X(left), +X(right)]
    /// (matches `box_uvs` in models.rs). Each face is [u_min, v_min, u_max, v_max].
    pub face_uvs: [[f32; 4]; 6],
    /// Base color tint (multiply with texture)
    pub color: [f32; 4],
    /// Which body part this is (for animation)
    pub part_type: PartType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PartType {
    Head,
    Body,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
    Cape,
    Tail,
    RightWing,
    LeftWing,
    BatRightWing,
    BatLeftWing,
    BatOuterRightWing,
    BatOuterLeftWing,
    BatHangingHead,
    DragonRightWing,
    DragonLeftWing,
    DragonOuterRightWing,
    DragonOuterLeftWing,
    BlazeRod(u8),
    SpiderLeg(u8),
    Tentacle(u8),
    OscillatingBody,
    Extra,
}

/// Entity animation state
#[derive(Clone, Copy, Debug)]
pub struct EntityPose {
    /// Vanilla renderer pre-scale, applied around the entity's feet.
    pub model_scale: f32,
    pub animation_family: AnimationFamily,
    /// Body yaw (radians)
    pub body_yaw: f32,
    /// Head yaw relative to body (radians)
    pub head_yaw: f32,
    /// Head pitch (radians)
    pub pitch: f32,
    /// Cumulative walk phase (radians-ish; advances with distance travelled)
    pub limb_swing: f32,
    /// Walk speed amount (0..1); scales limb swing amplitude
    pub limb_swing_amount: f32,
    /// Attack/hurt swing (0..1)
    pub swing_progress: f32,
    /// Death animation progress (0..1), matching vanilla's 20-tick death window.
    pub death_progress: f32,
    /// Age in game ticks, used by idle/flapping animations.
    pub age_ticks: f32,
    /// ModelBiped heldItemRight != 0.
    pub holding_item: bool,
    /// ArmorStand metadata indices 11..16 in vanilla order, in degrees.
    pub armor_stand_rotations: [[f32; 3]; 6],
    pub blocking: bool,
    pub sneaking: bool,
    pub riding: bool,
    pub cape_rotation: [f32; 3],
}

impl Default for EntityPose {
    fn default() -> Self {
        Self {
            model_scale: 1.0,
            animation_family: AnimationFamily::Generic,
            body_yaw: 0.0,
            head_yaw: 0.0,
            pitch: 0.0,
            limb_swing: 0.0,
            limb_swing_amount: 0.0,
            swing_progress: 0.0,
            death_progress: 0.0,
            age_ticks: 0.0,
            holding_item: false,
            armor_stand_rotations: [[0.0; 3]; 6],
            blocking: false,
            sneaking: false,
            riding: false,
            cape_rotation: [-6.0_f32.to_radians(), std::f32::consts::PI, 0.0],
        }
    }
}

/// Generate mesh for a single cuboid with given transform.
///
/// Replicates vanilla `ModelRenderer.render`: world = rotation_point + R * corner,
/// where corner = box_offset + {0..size} and R = Rz·Ry·Rx (base pose composed
/// with per-frame limb animation for animated part types).
pub fn generate_cuboid(
    cuboid: &ModelCuboid,
    transform: &Matrix4<f32>,
    pose: &EntityPose,
    vertices: &mut Vec<EntityVertex>,
    indices: &mut Vec<u32>,
) {
    let rot = animated_part_rotation(cuboid, pose);
    let rotation = animated_part_rotation_matrix(cuboid, rot);

    let rp = animated_rotation_point(cuboid, pose);
    let rp_mat = Matrix4::new_translation(&Vector3::new(rp[0], rp[1], rp[2]));
    let final_transform = transform * rp_mat * rotation;

    generate_cuboid_geometry(cuboid, &final_transform, vertices, indices);
}

pub fn model_part_transform(
    cuboid: &ModelCuboid,
    transform: &Matrix4<f32>,
    pose: &EntityPose,
) -> Matrix4<f32> {
    let rot = animated_part_rotation(cuboid, pose);
    let rp = animated_rotation_point(cuboid, pose);
    transform
        * Matrix4::new_translation(&Vector3::new(rp[0], rp[1], rp[2]))
        * animated_part_rotation_matrix(cuboid, rot)
}

fn animated_part_rotation_matrix(cuboid: &ModelCuboid, rotation: [f32; 3]) -> Matrix4<f32> {
    if cuboid.part_type == PartType::Cape {
        // LayerCape issues separate OpenGL rotations in X, Z, Y order. They
        // cannot be collapsed into from_euler_angles without changing the
        // result when sideways inertia is non-zero.
        Rotation3::from_axis_angle(&Vector3::x_axis(), rotation[0]).to_homogeneous()
            * Rotation3::from_axis_angle(&Vector3::z_axis(), rotation[2]).to_homogeneous()
            * Rotation3::from_axis_angle(&Vector3::y_axis(), rotation[1]).to_homogeneous()
    } else {
        Matrix4::from_euler_angles(rotation[0], rotation[1], rotation[2])
    }
}

fn animated_part_rotation(cuboid: &ModelCuboid, pose: &EntityPose) -> [f32; 3] {
    if pose.animation_family == AnimationFamily::ArmorStand {
        let index = match cuboid.part_type {
            PartType::Head => Some(0),
            PartType::Body => Some(1),
            PartType::LeftArm => Some(2),
            PartType::RightArm => Some(3),
            PartType::LeftLeg => Some(4),
            PartType::RightLeg => Some(5),
            _ => None,
        };
        if let Some(index) = index {
            let [x, y, z] = pose.armor_stand_rotations[index];
            return [
                cuboid.rotation[0] - x.to_radians(),
                cuboid.rotation[1] + y.to_radians(),
                cuboid.rotation[2] - z.to_radians(),
            ];
        }
    }
    let phase = pose.limb_swing * 0.6662;
    let amt = pose.limb_swing_amount;
    let walk = phase.cos() * amt;
    let leg_swing = walk * 1.4;
    let tail_wag = (phase * 2.0).sin() * 0.3 * amt;
    let age = pose.age_ticks;
    let swing = pose.swing_progress.clamp(0.0, 1.0);
    let body_swing = swing.sqrt().mul_add(std::f32::consts::TAU, 0.0).sin() * 0.2;
    let eased = 1.0 - (1.0 - swing).powi(4);
    let attack_x = (eased * std::f32::consts::PI).sin() * 1.2
        + (swing * std::f32::consts::PI).sin() * -(pose.pitch - 0.7) * 0.75;
    let attack_z_mc = (swing * std::f32::consts::PI).sin() * -0.4;
    let idle_x = (age * 0.067).sin() * 0.05;
    let idle_z = (age * 0.09).cos() * 0.05 + 0.05;
    match cuboid.part_type {
        // Dynamic angles below are first calculated in ModelBiped's coordinate
        // system. Converting to RustCraft's +Y-up model space negates X and Z.
        PartType::Head => [
            cuboid.rotation[0] - pose.pitch,
            cuboid.rotation[1] + pose.head_yaw,
            cuboid.rotation[2],
        ],
        PartType::Body => [
            cuboid.rotation[0]
                - if pose.animation_family == AnimationFamily::Biped && pose.sneaking {
                    0.5
                } else {
                    0.0
                },
            cuboid.rotation[1]
                + if pose.animation_family == AnimationFamily::Biped {
                    body_swing
                } else {
                    0.0
                },
            cuboid.rotation[2],
        ],
        // The vanilla living renderer mirrors X before drawing. RustCraft's
        // model geometry is Y-up, so the on-screen main hand is LeftArm here.
        PartType::LeftArm => {
            let mut x_mc = -walk;
            if pose.riding {
                x_mc -= std::f32::consts::PI / 5.0;
            }
            if pose.blocking {
                x_mc = x_mc * 0.5 - std::f32::consts::PI * 3.0 / 10.0;
            } else if pose.holding_item {
                x_mc = x_mc * 0.5 - std::f32::consts::PI / 10.0;
            }
            x_mc -= attack_x;
            if pose.sneaking {
                x_mc += 0.4;
            }
            x_mc += idle_x;
            [
                cuboid.rotation[0] - x_mc,
                if pose.blocking {
                    0.5235988
                } else {
                    cuboid.rotation[1] - body_swing * 3.0
                },
                cuboid.rotation[2] + attack_z_mc + idle_z,
            ]
        }
        PartType::RightArm => {
            let mut x_mc = walk;
            if pose.riding {
                x_mc -= std::f32::consts::PI / 5.0;
            }
            x_mc += body_swing;
            if pose.sneaking {
                x_mc += 0.4;
            }
            x_mc -= idle_x;
            [
                cuboid.rotation[0] - x_mc,
                cuboid.rotation[1] - body_swing,
                cuboid.rotation[2] - idle_z,
            ]
        }
        PartType::RightLeg => {
            let (x_mc, y_mc) = if pose.riding {
                (
                    -std::f32::consts::PI * 2.0 / 5.0,
                    std::f32::consts::PI / 10.0,
                )
            } else {
                (leg_swing, 0.0)
            };
            [
                cuboid.rotation[0] - x_mc,
                cuboid.rotation[1] + y_mc,
                cuboid.rotation[2],
            ]
        }
        PartType::LeftLeg => {
            let (x_mc, y_mc) = if pose.riding {
                (
                    -std::f32::consts::PI * 2.0 / 5.0,
                    -std::f32::consts::PI / 10.0,
                )
            } else {
                (-leg_swing, 0.0)
            };
            [
                cuboid.rotation[0] - x_mc,
                cuboid.rotation[1] + y_mc,
                cuboid.rotation[2],
            ]
        }
        PartType::Cape => [
            cuboid.rotation[0] + pose.cape_rotation[0],
            cuboid.rotation[1] + pose.cape_rotation[1],
            cuboid.rotation[2] + pose.cape_rotation[2],
        ],
        PartType::Tail => [
            cuboid.rotation[0],
            cuboid.rotation[1] + tail_wag,
            cuboid.rotation[2],
        ],
        PartType::RightWing => [
            cuboid.rotation[0],
            cuboid.rotation[1],
            cuboid.rotation[2] + age,
        ],
        PartType::LeftWing => [
            cuboid.rotation[0],
            cuboid.rotation[1],
            cuboid.rotation[2] - age,
        ],
        PartType::BatHangingHead => [
            pose.pitch + cuboid.rotation[0],
            std::f32::consts::PI - pose.head_yaw + cuboid.rotation[1],
            std::f32::consts::PI + cuboid.rotation[2],
        ],
        PartType::BatRightWing => {
            let flap = (age * 1.3).cos() * std::f32::consts::PI * 0.25;
            [
                cuboid.rotation[0],
                cuboid.rotation[1] + flap,
                cuboid.rotation[2],
            ]
        }
        PartType::BatLeftWing => {
            let flap = (age * 1.3).cos() * std::f32::consts::PI * 0.25;
            [
                cuboid.rotation[0],
                cuboid.rotation[1] - flap,
                cuboid.rotation[2],
            ]
        }
        PartType::BatOuterRightWing => {
            let flap = (age * 1.3).cos() * std::f32::consts::PI * 0.125;
            [
                cuboid.rotation[0],
                cuboid.rotation[1] + flap,
                cuboid.rotation[2],
            ]
        }
        PartType::BatOuterLeftWing => {
            let flap = (age * 1.3).cos() * std::f32::consts::PI * 0.125;
            [
                cuboid.rotation[0],
                cuboid.rotation[1] - flap,
                cuboid.rotation[2],
            ]
        }
        PartType::DragonRightWing => {
            let flap = (age * 0.25).sin();
            [
                cuboid.rotation[0] + 0.125 - flap.abs() * 0.2,
                cuboid.rotation[1] + 0.25,
                cuboid.rotation[2] + (flap + 0.125) * 0.8,
            ]
        }
        PartType::DragonLeftWing => {
            let flap = (age * 0.25).sin();
            [
                cuboid.rotation[0] + 0.125 - flap.abs() * 0.2,
                cuboid.rotation[1] - 0.25,
                cuboid.rotation[2] - (flap + 0.125) * 0.8,
            ]
        }
        PartType::DragonOuterRightWing => {
            let flap = (age * 0.25 + 2.0).sin();
            [
                cuboid.rotation[0],
                cuboid.rotation[1],
                cuboid.rotation[2] - (flap + 0.5) * 0.75,
            ]
        }
        PartType::DragonOuterLeftWing => {
            let flap = (age * 0.25 + 2.0).sin();
            [
                cuboid.rotation[0],
                cuboid.rotation[1],
                cuboid.rotation[2] + (flap + 0.5) * 0.75,
            ]
        }
        PartType::BlazeRod(index) => {
            let ring = (index / 4).min(2) as f32;
            let spin = age * 0.18 + ring * std::f32::consts::FRAC_PI_4;
            [
                cuboid.rotation[0],
                cuboid.rotation[1] + spin,
                cuboid.rotation[2],
            ]
        }
        PartType::SpiderLeg(index) => {
            let i = index.min(7) as usize;
            let walk = phase * 2.0;
            let y_offsets = [
                -(walk + 0.0).cos() * 0.4 * amt,
                (walk + 0.0).cos() * 0.4 * amt,
                -(walk + std::f32::consts::PI).cos() * 0.4 * amt,
                (walk + std::f32::consts::PI).cos() * 0.4 * amt,
                -(walk + std::f32::consts::FRAC_PI_2).cos() * 0.4 * amt,
                (walk + std::f32::consts::FRAC_PI_2).cos() * 0.4 * amt,
                -(walk + std::f32::consts::PI * 1.5).cos() * 0.4 * amt,
                (walk + std::f32::consts::PI * 1.5).cos() * 0.4 * amt,
            ];
            let z_offsets = [
                (phase + 0.0).sin().abs() * 0.4 * amt,
                -(phase + 0.0).sin().abs() * 0.4 * amt,
                (phase + std::f32::consts::PI).sin().abs() * 0.4 * amt,
                -(phase + std::f32::consts::PI).sin().abs() * 0.4 * amt,
                (phase + std::f32::consts::FRAC_PI_2).sin().abs() * 0.4 * amt,
                -(phase + std::f32::consts::FRAC_PI_2).sin().abs() * 0.4 * amt,
                (phase + std::f32::consts::PI * 1.5).sin().abs() * 0.4 * amt,
                -(phase + std::f32::consts::PI * 1.5).sin().abs() * 0.4 * amt,
            ];
            [
                cuboid.rotation[0],
                cuboid.rotation[1] + y_offsets[i],
                cuboid.rotation[2] + z_offsets[i],
            ]
        }
        PartType::Tentacle(index) => {
            let wave = (age * 0.4 + index as f32 * 0.65).sin() * 0.35;
            [
                cuboid.rotation[0] + wave,
                cuboid.rotation[1],
                cuboid.rotation[2],
            ]
        }
        PartType::OscillatingBody => [
            cuboid.rotation[0] + (age * 0.1).cos() * 0.15,
            cuboid.rotation[1],
            cuboid.rotation[2],
        ],
        _ => cuboid.rotation, // Body, Extra — static base pose
    }
}

fn animated_rotation_point(cuboid: &ModelCuboid, pose: &EntityPose) -> [f32; 3] {
    let mut point = cuboid.rotation_point;
    let swing = pose.swing_progress.clamp(0.0, 1.0);
    if swing > 0.0 {
        let body_swing = (swing.sqrt() * std::f32::consts::TAU).sin() * 0.2;
        let radius = point[0].abs();
        match cuboid.part_type {
            PartType::LeftArm => {
                point[0] = body_swing.cos() * radius;
                point[2] = body_swing.sin() * radius;
            }
            PartType::RightArm => {
                point[0] = -body_swing.cos() * radius;
                point[2] = -body_swing.sin() * radius;
            }
            _ => {}
        }
    }

    if pose.animation_family == AnimationFamily::Biped && pose.sneaking {
        match cuboid.part_type {
            PartType::Head => point[1] -= 1.0 / 16.0,
            PartType::Cape => point[1] -= 2.0 / 16.0,
            PartType::RightLeg | PartType::LeftLeg => {
                point[1] += 3.0 / 16.0;
                point[2] = 4.0 / 16.0;
            }
            _ => {}
        }
    }
    point
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationFamily {
    Biped,
    ArmorStand,
    Quadruped,
    #[default]
    Generic,
}

fn generate_cuboid_geometry(
    cuboid: &ModelCuboid,
    final_transform: &Matrix4<f32>,
    vertices: &mut Vec<EntityVertex>,
    indices: &mut Vec<u32>,
) {
    let (w, h, d) = (cuboid.size[0], cuboid.size[1], cuboid.size[2]);
    let (ox, oy, oz) = (
        cuboid.box_offset[0],
        cuboid.box_offset[1],
        cuboid.box_offset[2],
    );

    // 8 corners: min corner at box_offset, extending +size per axis.
    // Index convention must match the `faces` table below (0=--- … 6=+++).
    let corners = [
        [ox, oy, oz],             // 0: ---
        [ox + w, oy, oz],         // 1: +--
        [ox + w, oy + h, oz],     // 2: ++-
        [ox, oy + h, oz],         // 3: -+-
        [ox, oy, oz + d],         // 4: --+
        [ox + w, oy, oz + d],     // 5: +-+
        [ox + w, oy + h, oz + d], // 6: +++
        [ox, oy + h, oz + d],     // 7: -++
    ];

    // Transform corners
    let mut transformed = [[0.0f32; 3]; 8];
    for (i, corner) in corners.iter().enumerate() {
        let p = final_transform.transform_point(&Point3::new(corner[0], corner[1], corner[2]));
        transformed[i] = [p.x, p.y, p.z];
    }

    // 6 faces: front(+Z), back(-Z), top(+Y), bottom(-Y), right(+X), left(-X).
    // Corner order per face matches vanilla ModelBox.TexturedQuad vertex order
    // (transcribed from minecraft/.../ModelBox.java quadList[0..5], mapped into
    // local corner indices 0..7 with the MC(+Y down) → RC(+Y up) flip applied).
    let faces = [
        ([7, 6, 5, 4], [0.0, 0.0, 1.0]),  // +Z front — quadList[5]
        ([2, 3, 0, 1], [0.0, 0.0, -1.0]), // -Z back  — quadList[4]
        ([6, 7, 3, 2], [0.0, 1.0, 0.0]),  // +Y top   — quadList[2]
        ([1, 0, 4, 5], [0.0, -1.0, 0.0]), // -Y bottom— quadList[3]
        ([6, 2, 1, 5], [1.0, 0.0, 0.0]),  // +X right — quadList[0]
        ([3, 7, 4, 0], [-1.0, 0.0, 0.0]), // -X left  — quadList[1]
    ];

    // Fixed per-corner UV pattern (vanilla TexturedQuad assignment), identical
    // for all 6 faces: vtx0=(u_max,v_min), vtx1=(u_min,v_min),
    // vtx2=(u_min,v_max), vtx3=(u_max,v_max).
    let face_corner_uvs = [
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
    ];
    let normal_matrix = final_transform.fixed_view::<3, 3>(0, 0);

    for (face_idx, (corner_indices, normal)) in faces.iter().enumerate() {
        let v_start = vertices.len() as u32;
        // face_uvs array order is [-Z, +Z, +Y, -Y, -X, +X]; face gen order is
        // [+Z, -Z, +Y, -Y, +X, -X]. Remap so each geometric face uses its own
        // vanilla UV region.
        let uv_idx = match face_idx {
            0 => 1, // +Z → +Z
            1 => 0, // -Z → -Z
            2 => 2, // +Y → +Y
            3 => 3, // -Y → -Y
            4 => 5, // +X → +X
            5 => 4, // -X → -X
            _ => face_idx,
        };
        let face_uv = cuboid.face_uvs[uv_idx]; // [u_min, v_min, u_max, v_max]
        let corner_uvs = face_corner_uvs[face_idx];
        // Mirror (vanilla `mirror`): flips the box in X, which mirrors U on the
        // four faces whose U axis maps to X (+Z, -Z, +Y, -Y). +X/-X map U↔Z, so
        // they are unaffected.
        let mirror_u = cuboid.mirror && face_idx < 4;

        for (i, &corner_idx) in corner_indices.iter().enumerate() {
            let pos = transformed[corner_idx];
            let (lu, lv) = corner_uvs[i];
            let lu = if mirror_u { 1.0 - lu } else { lu };
            let u = face_uv[0] + lu * (face_uv[2] - face_uv[0]);
            let v = face_uv[1] + lv * (face_uv[3] - face_uv[1]);
            let transformed_normal =
                (normal_matrix * Vector3::new(normal[0], normal[1], normal[2])).normalize();
            vertices.push(EntityVertex {
                position: pos,
                normal: [
                    transformed_normal.x,
                    transformed_normal.y,
                    transformed_normal.z,
                ],
                uv: [u, v],
                color: cuboid.color,
            });
        }
        indices.extend_from_slice(&[
            v_start,
            v_start + 1,
            v_start + 2,
            v_start,
            v_start + 2,
            v_start + 3,
        ]);
    }
}

/// Generate full entity mesh from cuboid list
pub fn generate_entity_mesh(
    cuboids: &[ModelCuboid],
    world_position: Point3<f32>,
    pose: &EntityPose,
) -> (Vec<EntityVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // World transform: position + body rotation
    let body_rotation = Matrix4::from_euler_angles(0.0, pose.body_yaw, 0.0);
    // RendererLivingEntity.rotateCorpse eases deathTime with sqrt and rolls
    // the model through getDeathMaxRotation() (90 degrees for living mobs).
    let death_rotation = (pose.death_progress * 1.6).sqrt().min(1.0) * std::f32::consts::FRAC_PI_2;
    let translation = Matrix4::new_translation(&Vector3::new(
        world_position.x,
        world_position.y - if pose.sneaking { 0.2 } else { 0.0 },
        world_position.z,
    ));
    let world_transform = translation
        * body_rotation
        * Matrix4::from_euler_angles(0.0, 0.0, death_rotation)
        * Matrix4::new_scaling(pose.model_scale);

    for cuboid in cuboids {
        generate_cuboid(cuboid, &world_transform, pose, &mut vertices, &mut indices);
    }

    (vertices, indices)
}

/// Generate a rotating double-cross mesh for item entities.
/// Two intersecting quads form an X shape, like MC 1.8.9 item entities.
pub fn generate_item_entity_mesh(
    item_id: u16,
    item_damage: u16,
    world_pos: nalgebra::Point3<f32>,
    time: f32,
    atlas: Option<&super::atlas::EntityTextureAtlas>,
) -> (Vec<EntityVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Look up item icon UV from atlas
    let (u_min, v_min, u_max, v_max) = if let Some(atlas) = atlas {
        // For block items (id < 256), try to look up the block icon from the block atlas
        // by converting to the item icon path. Block items don't have item_icon_path entries,
        // so we fall back to __white.
        if let Some(index) = crate::render::item_icons::item_icon_index(item_id, item_damage) {
            let name = format!("item_{}", index);
            if let Some(region) = atlas.region_for(&name) {
                (region.u_min, region.v_min, region.u_max, region.v_max)
            } else if let Some(region) = atlas.region_for("__white") {
                (region.u_min, region.v_min, region.u_max, region.v_max)
            } else {
                return (vertices, indices);
            }
        } else {
            // Item not in icon atlas (block items etc.) — use __white as fallback
            if let Some(region) = atlas.region_for("__white") {
                (region.u_min, region.v_min, region.u_max, region.v_max)
            } else {
                return (vertices, indices);
            }
        }
    } else {
        return (vertices, indices);
    };

    let size = 0.25f32;
    let bob = (time * 2.0).sin() * 0.06;
    let rotation = time * 3.0;

    let px = world_pos.x;
    let py = world_pos.y + 0.25 + bob;
    let pz = world_pos.z;

    let cos_r = rotation.cos();
    let sin_r = rotation.sin();

    // Two perpendicular quads forming an X
    let d1 = [cos_r, 0.0, sin_r];
    let d2 = [sin_r, 0.0, -cos_r];
    let color = [1.0, 1.0, 1.0, 1.0];

    // Quad 1 front
    let base = vertices.len() as u32;
    vertices.push(EntityVertex {
        position: [px - d1[0] * size, py - size, pz - d1[2] * size],
        normal: [d1[0], 0.0, d1[2]],
        uv: [u_min, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px + d1[0] * size, py - size, pz + d1[2] * size],
        normal: [d1[0], 0.0, d1[2]],
        uv: [u_max, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px + d1[0] * size, py + size, pz + d1[2] * size],
        normal: [d1[0], 0.0, d1[2]],
        uv: [u_max, v_min],
        color,
    });
    vertices.push(EntityVertex {
        position: [px - d1[0] * size, py + size, pz - d1[2] * size],
        normal: [d1[0], 0.0, d1[2]],
        uv: [u_min, v_min],
        color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    // Quad 1 back
    let base = vertices.len() as u32;
    vertices.push(EntityVertex {
        position: [px + d1[0] * size, py - size, pz + d1[2] * size],
        normal: [-d1[0], 0.0, -d1[2]],
        uv: [u_min, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px - d1[0] * size, py - size, pz - d1[2] * size],
        normal: [-d1[0], 0.0, -d1[2]],
        uv: [u_max, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px - d1[0] * size, py + size, pz - d1[2] * size],
        normal: [-d1[0], 0.0, -d1[2]],
        uv: [u_max, v_min],
        color,
    });
    vertices.push(EntityVertex {
        position: [px + d1[0] * size, py + size, pz + d1[2] * size],
        normal: [-d1[0], 0.0, -d1[2]],
        uv: [u_min, v_min],
        color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    // Quad 2 front
    let base = vertices.len() as u32;
    vertices.push(EntityVertex {
        position: [px - d2[0] * size, py - size, pz - d2[2] * size],
        normal: [d2[0], 0.0, d2[2]],
        uv: [u_min, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px + d2[0] * size, py - size, pz + d2[2] * size],
        normal: [d2[0], 0.0, d2[2]],
        uv: [u_max, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px + d2[0] * size, py + size, pz + d2[2] * size],
        normal: [d2[0], 0.0, d2[2]],
        uv: [u_max, v_min],
        color,
    });
    vertices.push(EntityVertex {
        position: [px - d2[0] * size, py + size, pz - d2[2] * size],
        normal: [d2[0], 0.0, d2[2]],
        uv: [u_min, v_min],
        color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    // Quad 2 back
    let base = vertices.len() as u32;
    vertices.push(EntityVertex {
        position: [px + d2[0] * size, py - size, pz + d2[2] * size],
        normal: [-d2[0], 0.0, -d2[2]],
        uv: [u_min, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px - d2[0] * size, py - size, pz - d2[2] * size],
        normal: [-d2[0], 0.0, -d2[2]],
        uv: [u_max, v_max],
        color,
    });
    vertices.push(EntityVertex {
        position: [px - d2[0] * size, py + size, pz - d2[2] * size],
        normal: [-d2[0], 0.0, -d2[2]],
        uv: [u_max, v_min],
        color,
    });
    vertices.push(EntityVertex {
        position: [px + d2[0] * size, py + size, pz + d2[2] * size],
        normal: [-d2[0], 0.0, -d2[2]],
        uv: [u_min, v_min],
        color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    (vertices, indices)
}

#[cfg(test)]
mod animation_tests {
    use super::*;

    #[test]
    fn player_limbs_use_vanilla_walk_phase() {
        let model = crate::render::entity::models::player_model(false, 0, false);
        let right_arm = model
            .iter()
            .find(|part| part.part_type == PartType::RightArm)
            .unwrap();
        let left_arm = model
            .iter()
            .find(|part| part.part_type == PartType::LeftArm)
            .unwrap();
        let right_leg = model
            .iter()
            .find(|part| part.part_type == PartType::RightLeg)
            .unwrap();
        let left_leg = model
            .iter()
            .find(|part| part.part_type == PartType::LeftLeg)
            .unwrap();
        let pose = EntityPose {
            limb_swing_amount: 1.0,
            ..EntityPose::default()
        };

        assert!(animated_part_rotation(right_arm, &pose)[0] < 0.0);
        assert!(animated_part_rotation(left_arm, &pose)[0] > 0.0);
        assert!(animated_part_rotation(right_leg, &pose)[0] < 0.0);
        assert!(animated_part_rotation(left_leg, &pose)[0] > 0.0);
    }

    #[test]
    fn attack_swing_moves_the_right_arm_forward() {
        let model = crate::render::entity::models::player_model(false, 0, false);
        let main_hand_arm = model
            .iter()
            .find(|part| part.part_type == PartType::LeftArm)
            .unwrap();
        let idle = animated_part_rotation(main_hand_arm, &EntityPose::default())[0];
        let attacking = animated_part_rotation(
            main_hand_arm,
            &EntityPose {
                swing_progress: 0.5,
                ..EntityPose::default()
            },
        )[0];

        assert!(attacking > idle + 0.5);
    }

    #[test]
    fn blocking_uses_the_vanilla_right_arm_direction() {
        let model = crate::render::entity::models::player_model(false, 0, false);
        let main_hand_arm = model
            .iter()
            .find(|part| part.part_type == PartType::LeftArm)
            .unwrap();
        let rotation = animated_part_rotation(
            main_hand_arm,
            &EntityPose {
                blocking: true,
                ..EntityPose::default()
            },
        );

        assert!((rotation[0] - 0.9424779).abs() < 1.0e-6);
        assert!((rotation[1] - 0.5235988).abs() < 1.0e-6);
    }

    #[test]
    fn positive_minecraft_pitch_rotates_head_down_in_render_space() {
        let model = crate::render::entity::models::player_model(false, 0, false);
        let head = model
            .iter()
            .find(|part| part.part_type == PartType::Head)
            .unwrap();
        let rotation = animated_part_rotation(
            head,
            &EntityPose {
                pitch: 30.0_f32.to_radians(),
                ..EntityPose::default()
            },
        );

        assert!(rotation[0] < 0.0);
    }

    #[test]
    fn cape_uses_layer_cape_rest_rotation_and_sneak_pivot() {
        let model = crate::render::entity::models::player_model(false, 0x01, true);
        let cape = model
            .iter()
            .find(|part| part.part_type == PartType::Cape)
            .unwrap();
        let idle = EntityPose {
            animation_family: AnimationFamily::Biped,
            ..EntityPose::default()
        };
        let rotation = animated_part_rotation(cape, &idle);
        assert!((rotation[0] + 6.0_f32.to_radians()).abs() < 1.0e-6);
        assert!((rotation[1] - std::f32::consts::PI).abs() < 1.0e-6);

        let sneaking = EntityPose {
            animation_family: AnimationFamily::Biped,
            sneaking: true,
            ..EntityPose::default()
        };
        assert!(
            (animated_rotation_point(cape, &sneaking)[1] - (cape.rotation_point[1] - 2.0 / 16.0))
                .abs()
                < 1.0e-6
        );
    }

    #[test]
    fn cape_preserves_layer_cape_x_z_y_rotation_order() {
        let model = crate::render::entity::models::player_model(false, 0x01, true);
        let cape = model
            .iter()
            .find(|part| part.part_type == PartType::Cape)
            .unwrap();
        let rotation = [
            -32.0_f32.to_radians(),
            168.0_f32.to_radians(),
            -12.0_f32.to_radians(),
        ];
        let actual = animated_part_rotation_matrix(cape, rotation);
        let expected = Rotation3::from_axis_angle(&Vector3::x_axis(), rotation[0]).to_homogeneous()
            * Rotation3::from_axis_angle(&Vector3::z_axis(), rotation[2]).to_homogeneous()
            * Rotation3::from_axis_angle(&Vector3::y_axis(), rotation[1]).to_homogeneous();
        let collapsed = Matrix4::from_euler_angles(rotation[0], rotation[1], rotation[2]);

        assert!((actual - expected).abs().max() < 1.0e-6);
        assert!((actual - collapsed).abs().max() > 0.01);
    }

    #[test]
    fn armor_stand_pose_uses_metadata_instead_of_walk_animation() {
        let model = crate::render::entity::models::armor_stand_model(0x04);
        let right_arm = model
            .iter()
            .find(|part| part.part_type == PartType::RightArm)
            .unwrap();
        let mut pose = EntityPose {
            animation_family: AnimationFamily::ArmorStand,
            limb_swing: 4.0,
            limb_swing_amount: 1.0,
            ..EntityPose::default()
        };
        pose.armor_stand_rotations[3] = [-30.0, 20.0, 10.0];
        let rotation = animated_part_rotation(right_arm, &pose);

        assert!((rotation[0] - 30.0_f32.to_radians()).abs() < 1.0e-6);
        assert!((rotation[1] - 20.0_f32.to_radians()).abs() < 1.0e-6);
        assert!((rotation[2] + 10.0_f32.to_radians()).abs() < 1.0e-6);
    }

    #[test]
    fn quadruped_leg_pivots_keep_their_vanilla_front_back_positions() {
        for model in [
            crate::render::entity::models::cow_model(),
            crate::render::entity::models::sheep_model(),
        ] {
            let pose = EntityPose::default();
            let leg_z: Vec<f32> = model
                .iter()
                .filter(|part| matches!(part.part_type, PartType::LeftLeg | PartType::RightLeg))
                .map(|part| animated_rotation_point(part, &pose)[2])
                .collect();

            assert!(leg_z.iter().any(|z| *z > 0.25));
            assert!(leg_z.iter().any(|z| *z < -0.25));
        }
    }
}
