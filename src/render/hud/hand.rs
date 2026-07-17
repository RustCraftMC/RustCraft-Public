use crate::assets::texture::tile_uv_rect;
use crate::render::item_icons::{item_render_info, ItemCameraTransform};
use crate::world::block_models::BlockModelCache;
use nalgebra::{Matrix4, Rotation3, Vector3, Vector4};

/// Marks a vertex as enchanted for basic.vert. The fragment shader renders the
/// vanilla RenderItem.renderEffect scrolling purple sheen on top of the item's
/// own texture; the scroll timers come from a uniform, so glinted meshes never
/// need a per-frame rebuild.
pub(crate) const GLINT_BLOCK_TYPE: f32 = 11.0;

fn apply_glint(vertices: &mut [crate::world::mesh::Vertex]) {
    for vertex in vertices {
        vertex.block_type = GLINT_BLOCK_TYPE;
    }
}

/// Held models may contain JSON faces whose winding is transformed differently
/// from chunk geometry. Keep their surfaces two-sided so the first-person
/// transform never culls an otherwise valid face or the thin extrusion edge.
fn make_indices_two_sided(indices: &mut Vec<u32>) {
    let original_len = indices.len();
    for start in (0..original_len).step_by(3) {
        let a = indices[start];
        let b = indices[start + 1];
        let c = indices[start + 2];
        indices.extend_from_slice(&[a, c, b]);
    }
}

fn apply_item_camera_transform(
    model: Matrix4<f32>,
    transform: ItemCameraTransform,
    convert_model_space: bool,
) -> Matrix4<f32> {
    let [mut x_rotation, mut y_rotation, z_rotation] = transform.rotation;
    let [mut x_translation, mut y_translation, z_translation] = transform.translation;
    if convert_model_space {
        // RendererLivingEntity applies scale(-1, -1, 1) before model and layer
        // rendering. The entity meshes bake the Y flip into their coordinates;
        // mirror X here as well so the held layer stays on vanilla's right hand.
        x_rotation = -x_rotation;
        y_rotation = -y_rotation;
        x_translation = -x_translation;
        y_translation = -y_translation;
    }

    model
        * Matrix4::new_translation(&Vector3::new(x_translation, y_translation, z_translation))
        * Rotation3::from_axis_angle(&Vector3::y_axis(), y_rotation.to_radians()).to_homogeneous()
        * Rotation3::from_axis_angle(&Vector3::x_axis(), x_rotation.to_radians()).to_homogeneous()
        * Rotation3::from_axis_angle(&Vector3::z_axis(), z_rotation.to_radians()).to_homogeneous()
        * nalgebra::Scale3::new(transform.scale[0], transform.scale[1], transform.scale[2])
            .to_homogeneous()
}

/// Generate a 3D mesh for the held block (or item) in world space.
/// Returns a tuple of (opaque_vertices, opaque_indices, transparent_vertices, transparent_indices).
pub fn generate_held_block_mesh(
    camera: &crate::client::player::Camera,
    item_id: u16,
    item_damage: u16,
    pose: &crate::render::first_person::FirstPersonPose,
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool, // true if uses item atlas
) {
    let op_verts = Vec::new();
    let op_idx = Vec::new();
    let tr_verts = Vec::new();
    let tr_idx = Vec::new();

    if item_id == 0 {
        return (op_verts, op_idx, tr_verts, tr_idx, false);
    }

    // Vanilla suppresses the ordinary swing while an item-use action owns the
    // first-person transform. Script overrides can opt in via vanilla_flags.
    let swing = if pose.use_kind == 0 || pose.vanilla_flags.swing {
        pose.swing_progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sqrt_swing = swing.sqrt();
    let pi = std::f32::consts::PI;

    // MC 1.8.9 doItemUsedTransformations(swingProgress)
    let swing_x = if pose.vanilla_flags.swing {
        -0.4 * (sqrt_swing * pi).sin()
    } else {
        0.0
    };
    let swing_y = if pose.vanilla_flags.swing {
        0.2 * (sqrt_swing * pi * 2.0).sin()
    } else {
        0.0
    };
    let swing_z = if pose.vanilla_flags.swing {
        -0.2 * (swing * pi).sin()
    } else {
        0.0
    };

    // MC 1.8.9 transformFirstPersonItem(equipProgress, swingProgress)
    let f_item = if pose.vanilla_flags.swing {
        (swing * swing * pi).sin()
    } else {
        0.0
    };
    let f1_item = if pose.vanilla_flags.swing {
        (sqrt_swing * pi).sin()
    } else {
        0.0
    };

    let tr = |x: f32, y: f32, z: f32| -> Matrix4<f32> {
        Matrix4::new_translation(&Vector3::new(x, y, z))
    };
    let rx = |a: f32| -> Matrix4<f32> {
        Rotation3::from_axis_angle(&Vector3::x_axis(), a.to_radians()).to_homogeneous()
    };
    let ry = |a: f32| -> Matrix4<f32> {
        Rotation3::from_axis_angle(&Vector3::y_axis(), a.to_radians()).to_homogeneous()
    };
    let rz = |a: f32| -> Matrix4<f32> {
        Rotation3::from_axis_angle(&Vector3::z_axis(), a.to_radians()).to_homogeneous()
    };

    let mut m = crate::render::first_person::arm_tracking_transform(
        camera,
        pose.render_arm_pitch,
        pose.render_arm_yaw,
    );
    if pose.use_kind == 2 || pose.use_kind == 3 {
        // MCP ItemRenderer.performDrinking: food and potions use the same
        // hand-to-mouth curve; the remaining duration runs from 1 → 0.
        let remaining = (1.0 - pose.use_progress / 1.6).clamp(0.0, 1.0);
        let mut bob = ((remaining * 32.0) / 4.0 * pi).cos().abs() * 0.1;
        if remaining >= 0.8 {
            bob = 0.0;
        }
        let mouth = 1.0 - remaining.powi(27);
        m = m * tr(0.0, bob, 0.0);
        m = m * tr(mouth * 0.6, mouth * -0.5, 0.0);
        m = m * ry(mouth * 90.0) * rx(mouth * 10.0) * rz(mouth * 30.0);
    }
    m = m * tr(swing_x, swing_y, swing_z);
    {
        let effective_fov = camera.fov * camera.fov_modifier_at(camera.partial_tick);
        let z_scale = (80.0 / effective_fov).clamp(0.5, 2.0);
        m = m * tr(0.56, -0.52, -0.72 * z_scale);
    }
    if pose.vanilla_flags.equip {
        m = m * tr(0.0, pose.equip_progress * -0.6, 0.0);
    }
    m = m * ry(45.0);
    m = m * ry(f_item * -20.0);
    m = m * rz(f1_item * -20.0);
    m = m * rx(f1_item * -80.0);
    m = m * nalgebra::Scale3::new(0.4, 0.4, 0.4).to_homogeneous();

    match pose.use_kind {
        1 if pose.vanilla_flags.block_transform => {
            // MCP ItemRenderer.doBlockTransformations (GL order):
            //   translate(-0.5,0.2,0) * rotateY(30) * rotateX(-80) * rotateY(60)
            m = m * tr(-0.5, 0.2, 0.0) * ry(30.0) * rx(-80.0) * ry(60.0);
        }
        4 if pose.vanilla_flags.bow_transform => {
            // ItemRenderer.doBowTransformations: the use count has already
            // advanced one tick by the time this transform is evaluated.
            let charge_ticks = (pose.use_progress * 20.0 - 1.0).max(0.0).min(72.0);
            let mut pull = charge_ticks / 20.0;
            pull = (pull * pull + pull * 2.0) / 3.0;
            pull = pull.clamp(0.0, 1.0);
            let tremble = if pull > 0.1 {
                ((charge_ticks - 0.1) * 1.3).sin() * (pull - 0.1) * 0.01
            } else {
                0.0
            };
            m = m
                * rz(-18.0)
                * ry(-12.0)
                * rx(-8.0)
                * tr(-0.9, 0.2, 0.0)
                * tr(0.0, tremble, pull * 0.1)
                * nalgebra::Scale3::new(1.0, 1.0, 1.0 + pull * 0.2).to_homogeneous();
        }
        2 | 3 if pose.vanilla_flags.eat_drink_transform => {
            // eat/drink is handled above for pose.use_kind 2/3
        }
        _ => {}
    }

    let render_info = item_render_info(item_id);
    m = apply_item_camera_transform(m, render_info.first_person, false);
    m = crate::render::first_person::apply_script_transform(m, &pose.script_transform);

    // EntityRenderer.renderHand: apply hurtCameraEffect and setupViewBobbing
    // to the hand's own modelview matrix so the held item sways with walking.
    if camera.view_bobbing && camera.bob_amount > 0.0001 {
        let f1 = camera.bob_phase;
        let f2 = camera.bob_amount;
        let f3 = camera.bob_pitch;
        let sin_f1 = (f1 * pi).sin();
        let cos_f1 = (f1 * pi).cos();
        let tx = sin_f1 * f2 * 0.5;
        let ty = -(cos_f1 * f2).abs();
        let roll = sin_f1 * f2 * 3.0_f32.to_radians();
        let pitch = ((f1 * pi - 0.2).cos().abs() * f2) * 5.0_f32.to_radians() + f3.to_radians();
        m = Matrix4::new_translation(&Vector3::new(tx, ty, 0.0))
            * Matrix4::from_euler_angles(pitch, 0.0, roll)
            * m;
    }
    if camera.hurt_time > 0.0 {
        let f = (camera.hurt_time / 0.45).clamp(0.0, 1.0);
        let roll = -(f * f * f * f * pi).sin() * 14.0_f32.to_radians();
        m = Matrix4::from_euler_angles(0.0, 0.0, roll) * m;
    }

    // Transform from view space to world space.
    // Use view_matrix() (with bob) so the view bob cancels — the direct bob
    // applied above is what the player sees, matching vanilla.
    let inv_view = camera
        .view_matrix()
        .try_inverse()
        .unwrap_or_else(Matrix4::identity);
    let world_m = inv_view * m;
    let normal_matrix = world_m.fixed_view::<3, 3>(0, 0);

    let (mut op_verts, op_idx, mut tr_verts, tr_idx, uses_item_atlas) =
        generate_block_mesh_with_transform(item_id, item_damage, world_m, normal_matrix.into());

    if pose.glint {
        apply_glint(&mut op_verts);
        apply_glint(&mut tr_verts);
    }

    (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas)
}

/// Generate a 3D mesh for the held block (or item) in world space for 3rd person.
pub fn generate_3rd_person_held_item_mesh(
    item_id: u16,
    item_damage: u16,
    player_pos: [f32; 3],
    body_yaw: f32,
    limb_swing: f32,
    limb_swing_amount: f32,
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool,
) {
    if item_id == 0 {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new(), false);
    }

    // MC 1.8.9 Right arm transformation in 3rd person (approximation)
    let phase = limb_swing * 0.6662;
    let arm_swing = phase.cos() * limb_swing_amount;

    let tr = |x: f32, y: f32, z: f32| -> Matrix4<f32> {
        Matrix4::new_translation(&Vector3::new(x, y, z))
    };
    let rx = |a: f32| -> Matrix4<f32> {
        Rotation3::from_axis_angle(&Vector3::x_axis(), a.to_radians()).to_homogeneous()
    };
    let ry = |a: f32| -> Matrix4<f32> {
        Rotation3::from_axis_angle(&Vector3::y_axis(), a.to_radians()).to_homogeneous()
    };
    let mut m = Matrix4::<f32>::identity();
    m = m * tr(player_pos[0], player_pos[1], player_pos[2]);
    m = m * tr(0.0, 1.5, 0.0); // roughly shoulder height
    m = m * ry(180.0 - body_yaw);
    m = m * tr(-0.35, 0.0, 0.0); // arm offset
    m = m * rx(arm_swing * 57.2957); // limb swing in degrees

    // Position item in hand
    m = m * tr(0.0, -0.6, 0.2);

    let render_info = item_render_info(item_id);
    m = apply_item_camera_transform(m, render_info.third_person, true);

    let normal_matrix = m.fixed_view::<3, 3>(0, 0);
    generate_block_mesh_with_transform(item_id, item_damage, m, normal_matrix.into())
}

pub(crate) fn generate_block_mesh_with_transform(
    item_id: u16,
    item_damage: u16,
    world_m: Matrix4<f32>,
    normal_matrix: nalgebra::Matrix3<f32>,
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool,
) {
    let mut op_verts = Vec::new();
    let mut op_idx = Vec::new();
    let mut tr_verts = Vec::new();
    let mut tr_idx = Vec::new();

    let block = crate::world::block::Block::from_id(item_id);
    let is_block = block.to_id() == item_id && !item_render_info(item_id).generated;

    if is_block && BlockModelCache::is_available() {
        let cache = BlockModelCache::global();
        if let Some(model) = cache.get_model(item_id, item_damage as u8) {
            let is_transparent = block.is_transparent();
            let _target_verts = if is_transparent {
                &mut tr_verts
            } else {
                &mut op_verts
            };
            let _target_idx = if is_transparent {
                &mut tr_idx
            } else {
                &mut op_idx
            };

            for face in &model.faces {
                if face.vertices.is_empty() {
                    continue;
                }
                let tile_idx = cache.texture_index(&face.texture);
                let rect = tile_uv_rect(tile_idx);
                let [u_min, v_min, u_max, v_max] = rect;
                let fuvs = face.uvs;

                // Translate slightly based on face index to prevent z-fighting
                let p = Vector3::new(0.0, 0.0, 0.0);
                let push_dir = Vector3::new(face.normal[0], face.normal[1], face.normal[2]);
                let p = p + push_dir * 0.001;

                // Item tint based on face tintindex (MC 1.8.9 ItemColors)
                let bt = match face.tintindex {
                    Some(0) => 6.0, // grass tint
                    Some(1) => 7.0, // foliage tint
                    _ => 0.0,       // no tint
                };

                let mut normal_v =
                    normal_matrix * Vector3::new(face.normal[0], face.normal[1], face.normal[2]);
                normal_v.normalize_mut();

                let mut vs = Vec::new();
                for i in 0..4 {
                    let v = face.vertices[i];
                    let u = u_min + fuvs[i][0] * (u_max - u_min);
                    let v_tex = v_min + fuvs[i][1] * (v_max - v_min);
                    vs.push(crate::world::mesh::Vertex {
                        pos: [
                            (v[0] - 8.0) / 16.0,
                            (v[1] - 8.0) / 16.0,
                            (v[2] - 8.0) / 16.0,
                        ],
                        normal: [normal_v.x, normal_v.y, normal_v.z],
                        uv: [u, v_tex],
                        block_type: bt,
                        sky_light: 15.0,
                        block_light: 0.0,
                        ambient_occlusion: 1.0,
                    });
                }

                if block.is_transparent() {
                    let v_start = tr_verts.len() as u32;
                    for v in &vs {
                        let world_p = world_m
                            * Vector4::new(v.pos[0] + p.x, v.pos[1] + p.y, v.pos[2] + p.z, 1.0);
                        tr_verts.push(crate::world::mesh::Vertex {
                            pos: [world_p.x, world_p.y, world_p.z],
                            ..*v
                        });
                    }
                    tr_idx.extend_from_slice(&[
                        v_start,
                        v_start + 1,
                        v_start + 2,
                        v_start,
                        v_start + 2,
                        v_start + 3,
                    ]);
                } else {
                    let v_start = op_verts.len() as u32;
                    for v in &vs {
                        let world_p = world_m
                            * Vector4::new(v.pos[0] + p.x, v.pos[1] + p.y, v.pos[2] + p.z, 1.0);
                        op_verts.push(crate::world::mesh::Vertex {
                            pos: [world_p.x, world_p.y, world_p.z],
                            ..*v
                        });
                    }
                    op_idx.extend_from_slice(&[
                        v_start,
                        v_start + 1,
                        v_start + 2,
                        v_start,
                        v_start + 2,
                        v_start + 3,
                    ]);
                }
            }
            // Some item/block models intentionally have no baked faces in the
            // partial model cache. Do not turn a cache miss into an invisible
            // held block; fall through to the block-atlas cube below.
            if !op_idx.is_empty() || !tr_idx.is_empty() {
                make_indices_two_sided(&mut op_idx);
                make_indices_two_sided(&mut tr_idx);
                return (op_verts, op_idx, tr_verts, tr_idx, false);
            }
        }
    }

    // Blocks normally use their baked JSON model above. Keep a world-atlas
    // cube fallback for incomplete resource packs / uncached variants so a
    // held block can never silently disappear or be rendered from the item
    // icon atlas with a stretched texture.
    if is_block {
        let (top, bottom, side) = block.tiles();
        let faces = [
            (
                top,
                [0.0, 1.0, 0.0],
                [
                    [-0.5, 0.5, -0.5],
                    [0.5, 0.5, -0.5],
                    [0.5, 0.5, 0.5],
                    [-0.5, 0.5, 0.5],
                ],
            ),
            (
                bottom,
                [0.0, -1.0, 0.0],
                [
                    [-0.5, -0.5, 0.5],
                    [0.5, -0.5, 0.5],
                    [0.5, -0.5, -0.5],
                    [-0.5, -0.5, -0.5],
                ],
            ),
            (
                side,
                [0.0, 0.0, 1.0],
                [
                    [-0.5, -0.5, 0.5],
                    [0.5, -0.5, 0.5],
                    [0.5, 0.5, 0.5],
                    [-0.5, 0.5, 0.5],
                ],
            ),
            (
                side,
                [0.0, 0.0, -1.0],
                [
                    [0.5, -0.5, -0.5],
                    [-0.5, -0.5, -0.5],
                    [-0.5, 0.5, -0.5],
                    [0.5, 0.5, -0.5],
                ],
            ),
            (
                side,
                [1.0, 0.0, 0.0],
                [
                    [0.5, -0.5, 0.5],
                    [0.5, -0.5, -0.5],
                    [0.5, 0.5, -0.5],
                    [0.5, 0.5, 0.5],
                ],
            ),
            (
                side,
                [-1.0, 0.0, 0.0],
                [
                    [-0.5, -0.5, -0.5],
                    [-0.5, -0.5, 0.5],
                    [-0.5, 0.5, 0.5],
                    [-0.5, 0.5, -0.5],
                ],
            ),
        ];
        let transparent = block.is_transparent();
        for (tile, normal, corners) in faces {
            let [u_min, v_min, u_max, v_max] = tile_uv_rect(tile);
            let uvs = [
                [u_min, v_min],
                [u_max, v_min],
                [u_max, v_max],
                [u_min, v_max],
            ];
            let mut transformed_normal =
                normal_matrix * Vector3::new(normal[0], normal[1], normal[2]);
            transformed_normal.normalize_mut();
            let base = if transparent {
                tr_verts.len() as u32
            } else {
                op_verts.len() as u32
            };
            for (corner, uv) in corners.into_iter().zip(uvs) {
                let local = Vector3::new(corner[0], corner[1], corner[2])
                    + Vector3::new(normal[0], normal[1], normal[2]) * 0.001;
                let world_p = world_m * Vector4::new(local.x, local.y, local.z, 1.0);
                let vertex = crate::world::mesh::Vertex {
                    pos: [world_p.x, world_p.y, world_p.z],
                    normal: [
                        transformed_normal.x,
                        transformed_normal.y,
                        transformed_normal.z,
                    ],
                    uv,
                    block_type: 0.0,
                    sky_light: 15.0,
                    block_light: 0.0,
                    ambient_occlusion: 1.0,
                };
                if transparent {
                    tr_verts.push(vertex);
                } else {
                    op_verts.push(vertex);
                }
            }
            let indices = [base, base + 1, base + 2, base, base + 2, base + 3];
            if transparent {
                tr_idx.extend_from_slice(&indices);
            } else {
                op_idx.extend_from_slice(&indices);
            }
        }
        make_indices_two_sided(&mut op_idx);
        make_indices_two_sided(&mut tr_idx);
        return (op_verts, op_idx, tr_verts, tr_idx, false);
    }

    // Render as 3D extruded item or fallback to 2D
    if let Some(tile_idx) = crate::render::item_icons::item_icon_index(item_id, item_damage) {
        let rect = crate::render::item_icons::item_icon_uv_rect(tile_idx);
        let [u_min, v_min, u_max, v_max] = rect;

        if let Some(extruded) =
            crate::render::item_icons::get_cached_extruded_mesh(item_id, item_damage)
        {
            let v_start = op_verts.len() as u32;
            for v in &extruded.vertices {
                let local = Vector4::new(v.pos[0], v.pos[1], v.pos[2], 1.0);
                let world_p = world_m * local;
                let mut n_v = normal_matrix * Vector3::new(v.normal[0], v.normal[1], v.normal[2]);
                n_v.normalize_mut();

                // Map normalized local UV [0.0..1.0] to texture atlas region UV
                let au = u_min + v.uv[0] * (u_max - u_min);
                let av = v_min + v.uv[1] * (v_max - v_min);

                op_verts.push(crate::world::mesh::Vertex {
                    pos: [world_p.x, world_p.y, world_p.z],
                    normal: [n_v.x, n_v.y, n_v.z],
                    uv: [au, av],
                    block_type: 0.0,
                    sky_light: 15.0,
                    block_light: 0.0,
                    ambient_occlusion: 1.0,
                });
            }
            for idx in &extruded.indices {
                op_idx.push(v_start + idx);
            }
        } else {
            // Keep cache misses three-dimensional as well. A thin slab is a
            // better fallback than the old double-sided, zero-thickness quad.
            let v_start = op_verts.len() as u32;
            let p_min = -0.5;
            let p_max = 0.5;
            let half_t = 0.03125;
            let points = [
                [p_min, p_min, -half_t],
                [p_max, p_min, -half_t],
                [p_max, p_max, -half_t],
                [p_min, p_max, -half_t],
                [p_min, p_min, half_t],
                [p_max, p_min, half_t],
                [p_max, p_max, half_t],
                [p_min, p_max, half_t],
            ];
            let faces = [
                ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
                ([1, 0, 3, 2], [0.0, 0.0, -1.0]),
                ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),
                ([5, 1, 2, 6], [1.0, 0.0, 0.0]),
                ([7, 6, 2, 3], [0.0, 1.0, 0.0]),
                ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
            ];
            let uvs = [
                [u_min, v_max],
                [u_max, v_max],
                [u_max, v_min],
                [u_min, v_min],
            ];
            for (face_index, (face, normal)) in faces.into_iter().enumerate() {
                let mut normal_v = normal_matrix * Vector3::new(normal[0], normal[1], normal[2]);
                normal_v.normalize_mut();
                for (corner, uv) in face.into_iter().zip(uvs) {
                    let local = points[corner];
                    let world_p = world_m * Vector4::new(local[0], local[1], local[2], 1.0);
                    op_verts.push(crate::world::mesh::Vertex {
                        pos: [world_p.x, world_p.y, world_p.z],
                        normal: [normal_v.x, normal_v.y, normal_v.z],
                        uv,
                        block_type: 0.0,
                        sky_light: 15.0,
                        block_light: 0.0,
                        ambient_occlusion: 1.0,
                    });
                }
                let base = v_start + face_index as u32 * 4;
                op_idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }

    make_indices_two_sided(&mut op_idx);
    make_indices_two_sided(&mut tr_idx);
    (op_verts, op_idx, tr_verts, tr_idx, true)
}

pub fn generate_entity_held_item_mesh(
    entity_type: crate::entity::EntityType,
    item_id: u16,
    item_damage: u16,
    world_position: [f32; 3],
    pose: &crate::render::entity::mesh::EntityPose,
    cuboids: &[crate::render::entity::mesh::ModelCuboid],
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool,
) {
    use crate::render::entity::mesh::PartType;

    let tr = |x: f32, y: f32, z: f32| Matrix4::new_translation(&Vector3::new(x, y, z));
    let rx =
        |a: f32| Rotation3::from_axis_angle(&Vector3::x_axis(), a.to_radians()).to_homogeneous();
    let ry =
        |a: f32| Rotation3::from_axis_angle(&Vector3::y_axis(), a.to_radians()).to_homogeneous();
    let rz =
        |a: f32| Rotation3::from_axis_angle(&Vector3::z_axis(), a.to_radians()).to_homogeneous();
    let root = tr(world_position[0], world_position[1], world_position[2])
        * ry(pose.body_yaw.to_degrees());

    let fallback_armor_stand_arm = (entity_type == crate::entity::EntityType::ArmorStand)
        .then_some(crate::render::entity::mesh::ModelCuboid {
            rotation_point: [-5.0 / 16.0, 22.0 / 16.0, 0.0],
            box_offset: [-2.0 / 16.0, -10.0 / 16.0, -1.0 / 16.0],
            size: [2.0 / 16.0, 12.0 / 16.0, 2.0 / 16.0],
            rotation: [0.0; 3],
            mirror: false,
            face_uvs: [[0.0; 4]; 6],
            color: [1.0; 4],
            part_type: PartType::RightArm,
        });
    let held_part = if entity_type == crate::entity::EntityType::ArmorStand {
        PartType::RightArm
    } else {
        PartType::LeftArm
    };
    let held_arm = cuboids
        .iter()
        .find(|part| part.part_type == held_part)
        .or(fallback_armor_stand_arm.as_ref());

    let mut model = if entity_type == crate::entity::EntityType::Witch {
        // Vanilla: villagerNose.postRender + translate + final common rotate
        root * tr(-0.3125, 0.125, 0.0) // nose pivot ≈ arm pivot
            * tr(-0.0625, 0.53125, 0.21875) // item offset from nose
            * rx(-15.0) * rz(40.0) // common witch final
    } else if let Some(arm) = held_arm {
        // `player_model` bakes RenderPlayer's 0.9375 pre-render scale into
        // its cuboids. LayerHeldItem runs under that same scale, including its
        // grip translation. ModelPlayer also shifts the slim right-arm pivot by
        // one model pixel during postRenderArm.
        let player_scale = (entity_type == crate::entity::EntityType::Player).then_some(0.9375);
        let mut held_arm = *arm;
        if player_scale.is_some() && held_arm.size[0] < 0.22 {
            held_arm.rotation_point[0] -= 0.0625 * 0.9375;
        }
        let arm_m = crate::render::entity::mesh::model_part_transform(&held_arm, &root, pose);
        let grip_scale = player_scale.unwrap_or(1.0);
        arm_m
            * tr(
                0.0625 * grip_scale,
                -0.4375 * grip_scale,
                0.0625 * grip_scale,
            ) // item grip offset from arm tip
    } else {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new(), false);
    };

    // Sneaking offset (vanilla: translate(0, 0.203125, 0))
    if pose.sneaking {
        model = model * tr(0.0, -0.203125, 0.0);
    }

    // Blocking handled by arm animation (setRotationAngles), not extra item transform.

    // `LayerHeldItem` applies this transform before `renderItem` for ordinary
    // block items.  A block model without an item JSON `thirdperson` section
    // must therefore not be rendered at its full one-block world size.
    //
    // Vanilla GL: translate(0, .1875, -.3125), rotateX(20), rotateY(45),
    // scale(-.375, -.375, .375).  RustCraft's model space has +Y upward, so
    // the Y translation and X rotation are converted here.
    if matches!(item_id, 54 | 130 | 146) {
        model = model
            * tr(0.0, -0.1875, -0.3125)
            * rx(-20.0)
            * ry(45.0)
            * nalgebra::Scale3::new(-0.375, -0.375, 0.375).to_homogeneous();
    }

    let render_info = item_render_info(item_id);
    model = apply_item_camera_transform(model, render_info.third_person, true);
    if render_info.generated {
        // ItemModelGenerator's sprite origin is opposite the entity layer's
        // grip origin. Rotate sprite-derived meshes so the texture's handle
        // end, rather than its blade end, meets the hand.
        model = model * rz(180.0);
    }
    let normal_matrix = model.fixed_view::<3, 3>(0, 0).into_owned();
    generate_block_mesh_with_transform(item_id, item_damage, model, normal_matrix)
}

pub fn generate_dropped_item_mesh(
    item_id: u16,
    item_damage: u16,
    world_position: [f32; 3],
    age_ticks: f32,
    hover_start: f32,
    wall_clock: f32,
    glint: bool,
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool,
) {
    let partial_ticks = (wall_clock * 20.0) - (wall_clock * 20.0).floor();
    let effective_age = age_ticks + partial_ticks;
    let bob = ((effective_age) / 10.0 + hover_start).sin() * 0.1 + 0.1;
    let rotation = ((effective_age) / 20.0 + hover_start) * std::f32::consts::PI;
    let model = Matrix4::new_translation(&Vector3::new(
        world_position[0],
        world_position[1] + bob + 0.06,
        world_position[2],
    )) * Rotation3::from_axis_angle(&Vector3::y_axis(), rotation).to_homogeneous()
        * nalgebra::Scale3::new(0.35, 0.35, 0.35).to_homogeneous();
    let normal_matrix = model.fixed_view::<3, 3>(0, 0).into_owned();
    let (mut op_verts, op_idx, mut tr_verts, tr_idx, uses_item_atlas) =
        generate_block_mesh_with_transform(item_id, item_damage, model, normal_matrix);

    if glint {
        apply_glint(&mut op_verts);
        apply_glint(&mut tr_verts);
    }

    (op_verts, op_idx, tr_verts, tr_idx, uses_item_atlas)
}

pub fn generate_arrow_entity_mesh(
    world_position: [f32; 3],
    yaw_deg: f32,
    pitch_deg: f32,
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool,
) {
    let arrow_id = crate::world::item::Item::Arrow.to_id();
    let model = Matrix4::new_translation(&Vector3::new(
        world_position[0],
        world_position[1],
        world_position[2],
    )) * Rotation3::from_axis_angle(&Vector3::y_axis(), -yaw_deg.to_radians())
        .to_homogeneous()
        * Rotation3::from_axis_angle(&Vector3::x_axis(), pitch_deg.to_radians()).to_homogeneous()
        * nalgebra::Scale3::new(0.5, 0.5, 0.5).to_homogeneous();
    let normal_matrix = model.fixed_view::<3, 3>(0, 0).into_owned();
    generate_block_mesh_with_transform(arrow_id, 0, model, normal_matrix)
}

pub fn generate_projectile_item_mesh(
    item_id: u16,
    item_damage: u16,
    world_position: [f32; 3],
    scale: f32,
) -> (
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    Vec<crate::world::mesh::Vertex>,
    Vec<u32>,
    bool,
) {
    let model = Matrix4::new_translation(&Vector3::new(
        world_position[0],
        world_position[1],
        world_position[2],
    )) * nalgebra::Scale3::new(scale, scale, scale).to_homogeneous();
    let normal_matrix = model.fixed_view::<3, 3>(0, 0).into_owned();
    generate_block_mesh_with_transform(item_id, item_damage, model, normal_matrix)
}

#[cfg(test)]
mod third_person_tests {
    use super::*;
    use crate::render::entity::mesh::{AnimationFamily, EntityPose};

    fn item_center(blocking: bool) -> [f32; 3] {
        let model = crate::render::entity::models::player_model(false, 0, false);
        let pose = EntityPose {
            animation_family: AnimationFamily::Biped,
            holding_item: true,
            blocking,
            ..EntityPose::default()
        };
        let (opaque, _, transparent, _, _) = generate_entity_held_item_mesh(
            crate::entity::EntityType::Player,
            276,
            0,
            [0.0; 3],
            &pose,
            &model,
        );
        let vertices: Vec<_> = opaque.iter().chain(&transparent).collect();
        assert!(!vertices.is_empty());
        let sum = vertices.iter().fold([0.0; 3], |mut sum, vertex| {
            for (total, value) in sum.iter_mut().zip(vertex.pos) {
                *total += value;
            }
            sum
        });
        let count = vertices.len() as f32;
        [sum[0] / count, sum[1] / count, sum[2] / count]
    }

    #[test]
    fn held_item_uses_the_on_screen_right_hand() {
        assert!(item_center(false)[0] > 0.0);
    }

    #[test]
    fn sword_blocking_raises_the_held_item_with_the_main_arm() {
        assert!(item_center(true)[1] > item_center(false)[1]);
    }
}
