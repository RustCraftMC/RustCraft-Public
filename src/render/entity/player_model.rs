use crate::assets::skin::{PlayerSkin, SkinLayout};
use crate::client::player::Camera;
use crate::client::player_model::{PlayerModel, PlayerModelPart, PlayerPose};
use nalgebra::{Matrix4, Rotation3, Vector3, Vector4};

#[derive(Clone, Copy)]
pub(crate) struct FaceSpec {
    normal: [f32; 3],
    corners: [[f32; 3]; 4],
    uv: SkinUv,
}

#[derive(Clone, Copy)]
pub(crate) struct SkinUv {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    flip_x: bool,
    flip_y: bool,
}

#[derive(Clone)]
pub(crate) struct ProjectedFace {
    depth: f32,
    points: [[f32; 2]; 4],
    pixels: Vec<PixelQuad>,
}

#[derive(Clone)]
struct PixelQuad {
    pub(crate) color: [f32; 4],
    pub(crate) corners: [[f32; 2]; 4],
}

#[derive(Clone, Copy)]
struct PreviewVertex {
    position: Vector3<f32>,
    uv: [f32; 2],
}

#[derive(Clone, Copy)]
struct PreviewQuad {
    vertices: [PreviewVertex; 4],
    normal: Vector3<f32>,
}

#[derive(Clone, Copy)]
struct PreviewTriangle {
    positions: [Vector3<f32>; 3],
    uvs: [[f32; 2]; 3],
    shade: f32,
    overlay: bool,
}

struct PreviewRaster {
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    colors: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlayerPreviewCacheKey {
    skin_hash: u64,
    x: u32,
    y: u32,
    pixel_scale: u32,
    yaw: u32,
    pitch: u32,
    alpha: u32,
    slim: bool,
    skin_parts_mask: u8,
}

#[derive(Default)]
pub(crate) struct PlayerPreviewCache {
    key: Option<PlayerPreviewCacheKey>,
    raster: Option<PreviewRaster>,
    #[cfg(test)]
    rasterizations: usize,
}

pub fn draw_player_model(
    builder: &mut crate::render::gui::GuiVertexBuilder,
    camera: &Camera,
    skin: &PlayerSkin,
    position: [f32; 3],
    screen_size: [f32; 2],
    pose: PlayerPose,
    slim: bool,
    skin_parts_mask: u8,
    alpha: f32,
) {
    if alpha <= 0.01 {
        return;
    }

    let model = if slim {
        PlayerModel::alex()
    } else {
        PlayerModel::steve()
    };
    let view = camera.view_matrix();
    let proj = camera.projection_matrix();
    let view_proj = proj * view;
    // MC degrees → euler Ry: body_yaw = (180 - mc_yaw)°, head relative = body_mc - head_mc
    let body_rot = Rotation3::from_euler_angles(0.0, (180.0 - pose.body_yaw).to_radians(), 0.0);
    let head_rot = Rotation3::from_euler_angles(
        pose.pitch.to_radians(),
        (pose.body_yaw - pose.head_yaw).to_radians(),
        0.0,
    );
    let shadow_offset = if pose.sneaking { -0.12 } else { 0.0 };
    let mut faces = Vec::new();

    for part in &model.parts {
        if !part_visible(part.part, skin.layout, skin_parts_mask) {
            continue;
        }
        for face in cuboid_faces(part.part, part.origin, part.size, slim, part.mirror) {
            let mut world = [[0.0; 3]; 4];
            let mut normal = Vector3::new(face.normal[0], face.normal[1], face.normal[2]);
            for (idx, corner) in face.corners.iter().enumerate() {
                let mut p = Vector3::new(corner[0], corner[1], corner[2]);
                p = apply_part_pose(part.part, p, &body_rot, &head_rot, pose, slim);
                p /= 16.0;
                p.y += shadow_offset;
                p += Vector3::new(position[0], position[1], position[2]);
                world[idx] = [p.x, p.y, p.z];
            }
            normal = apply_part_normal(part.part, normal, &body_rot, &head_rot, pose, slim);
            let Some(projected) = project_face(
                camera,
                &view_proj,
                skin,
                &face,
                world,
                normal,
                screen_size,
                alpha,
            ) else {
                continue;
            };
            faces.push(projected);
        }
    }

    faces.sort_by(|a, b| {
        b.depth
            .partial_cmp(&a.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for face in faces {
        for pixel in face.pixels {
            fill_pixel_quad(builder, pixel.corners, pixel.color);
        }
        if alpha < 0.99 {
            fill_pixel_quad(builder, face.points, [0.0, 0.0, 0.0, 0.12 * alpha]);
        }
    }
}

pub fn draw_player_preview(
    builder: &mut crate::render::gui::GuiVertexBuilder,
    cache: &mut PlayerPreviewCache,
    skin: &PlayerSkin,
    x: f32,
    y: f32,
    pixel_scale: f32,
    yaw: f32,
    pitch: f32,
    slim: bool,
    skin_parts_mask: u8,
    alpha: f32,
) {
    if alpha <= 0.01 || pixel_scale <= 0.0 {
        return;
    }

    let Some(raster) = cached_player_preview(
        cache,
        skin,
        x,
        y,
        pixel_scale,
        yaw,
        pitch,
        slim,
        skin_parts_mask,
        alpha,
    ) else {
        return;
    };
    for row in 0..raster.height {
        let mut column = 0;
        while column < raster.width {
            let color = raster.colors[row * raster.width + column];
            if color[3] <= 0.001 {
                column += 1;
                continue;
            }
            let mut end = column + 1;
            while end < raster.width && raster.colors[row * raster.width + end] == color {
                end += 1;
            }
            builder.fill_rect(
                (raster.x + column as i32) as f32,
                (raster.y + row as i32) as f32,
                (end - column) as f32,
                1.0,
                color,
            );
            column = end;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cached_player_preview<'a>(
    cache: &'a mut PlayerPreviewCache,
    skin: &PlayerSkin,
    x: f32,
    y: f32,
    pixel_scale: f32,
    yaw: f32,
    pitch: f32,
    slim: bool,
    skin_parts_mask: u8,
    alpha: f32,
) -> Option<&'a PreviewRaster> {
    // The vanilla preview tracks the mouse, but sub-quarter-degree changes are
    // visually indistinguishable and would defeat the last-result cache.
    let yaw = quantize_preview_angle(yaw);
    let pitch = quantize_preview_angle(pitch);
    let key = PlayerPreviewCacheKey {
        skin_hash: player_skin_hash(skin),
        x: x.to_bits(),
        y: y.to_bits(),
        pixel_scale: pixel_scale.to_bits(),
        yaw: yaw.to_bits(),
        pitch: pitch.to_bits(),
        alpha: alpha.to_bits(),
        slim,
        skin_parts_mask,
    };

    if cache.key != Some(key) {
        cache.raster = build_player_preview_raster(
            skin,
            x,
            y,
            pixel_scale,
            yaw,
            pitch,
            slim,
            skin_parts_mask,
            alpha,
        );
        cache.key = Some(key);
        #[cfg(test)]
        {
            cache.rasterizations += 1;
        }
    }

    cache.raster.as_ref()
}

fn quantize_preview_angle(angle: f32) -> f32 {
    (angle * 4.0).round() * 0.25
}

fn player_skin_hash(skin: &PlayerSkin) -> u64 {
    use std::hash::Hasher;

    let mut hasher = fnv::FnvHasher::default();
    let (width, height) = skin.dimensions();
    hasher.write_u32(width);
    hasher.write_u32(height);
    hasher.write_u8(match skin.layout {
        SkinLayout::Classic64x32 => 0,
        SkinLayout::Modern64x64 => 1,
    });
    hasher.write(skin.pixels.as_raw());
    hasher.finish()
}

#[allow(clippy::too_many_arguments)]
fn build_player_preview_raster(
    skin: &PlayerSkin,
    x: f32,
    y: f32,
    pixel_scale: f32,
    yaw: f32,
    pitch: f32,
    slim: bool,
    skin_parts_mask: u8,
    alpha: f32,
) -> Option<PreviewRaster> {
    let model = if slim {
        PlayerModel::alex()
    } else {
        PlayerModel::steve()
    };
    let bottom = y + 32.0 * pixel_scale;
    let transform = vanilla_preview_transform(pixel_scale, yaw, pitch);
    let mut triangles = Vec::with_capacity(model.parts.len() * 12);

    for part in &model.parts {
        if !part_visible(part.part, skin.layout, skin_parts_mask) {
            continue;
        }
        let overlay = is_preview_overlay(part.part);
        for quad in mcp_preview_quads(
            part.origin,
            part.size,
            preview_texture_dimensions(part.part, slim),
            part.uv,
            part.mirror,
        ) {
            let mut positions = [Vector3::zeros(); 4];
            for (index, vertex) in quad.vertices.iter().enumerate() {
                let model_position = preview_part_rotation(part.part, vertex.position, yaw, pitch);
                let projected = transform
                    * Vector4::new(
                        model_position.x / 16.0,
                        model_position.y / 16.0,
                        model_position.z / 16.0,
                        1.0,
                    );
                positions[index] = Vector3::new(x + projected.x, bottom + projected.y, projected.z);
            }

            let normal_tip = transform
                * Vector4::new(
                    quad.normal.x / 16.0,
                    quad.normal.y / 16.0,
                    quad.normal.z / 16.0,
                    0.0,
                );
            let normal = Vector3::new(normal_tip.x, normal_tip.y, normal_tip.z)
                .try_normalize(1.0e-5)
                .unwrap_or_else(Vector3::z);
            let light_a = Vector3::new(-0.35, -0.70, 0.62).normalize();
            let light_b = Vector3::new(0.65, 0.15, 0.45).normalize();
            let shade = (0.58
                + 0.27 * normal.dot(&light_a).max(0.0)
                + 0.15 * normal.dot(&light_b).max(0.0))
            .clamp(0.52, 1.0);

            for indices in [[0, 1, 2], [0, 2, 3]] {
                triangles.push(PreviewTriangle {
                    positions: indices.map(|index| positions[index]),
                    uvs: indices.map(|index| quad.vertices[index].uv),
                    shade,
                    overlay,
                });
            }
        }
    }

    rasterize_player_preview(skin, &triangles, alpha)
}

fn vanilla_preview_transform(pixel_scale: f32, yaw: f32, pitch: f32) -> Matrix4<f32> {
    let gui_scale = Matrix4::new_nonuniform_scaling(&Vector3::new(
        -pixel_scale * 16.0,
        pixel_scale * 16.0,
        pixel_scale * 16.0,
    ));
    let gui_z_flip =
        Rotation3::from_axis_angle(&Vector3::z_axis(), std::f32::consts::PI).to_homogeneous();
    let gui_pitch =
        Rotation3::from_axis_angle(&Vector3::x_axis(), -pitch.to_radians()).to_homogeneous();
    let corpse_yaw =
        Rotation3::from_axis_angle(&Vector3::y_axis(), (180.0 - yaw).to_radians()).to_homogeneous();
    let living_scale = Matrix4::new_nonuniform_scaling(&Vector3::new(-1.0, -1.0, 1.0));
    let model_translation = Matrix4::new_translation(&Vector3::new(0.0, -1.5078125, 0.0));

    gui_scale * gui_z_flip * gui_pitch * corpse_yaw * living_scale * model_translation
}

fn preview_part_rotation(
    part: PlayerModelPart,
    position: Vector3<f32>,
    yaw: f32,
    pitch: f32,
) -> Vector3<f32> {
    if !matches!(part, PlayerModelPart::Head | PlayerModelPart::Hat) {
        return position;
    }

    // ModelRenderer emits Z, Y, X matrix calls, so a vertex is transformed
    // by X first and then Y. The inventory entity uses headYaw-bodyYaw for Y.
    let head_x = Rotation3::from_axis_angle(&Vector3::x_axis(), -pitch.to_radians());
    let head_y = Rotation3::from_axis_angle(&Vector3::y_axis(), yaw.to_radians());
    head_y * (head_x * position)
}

fn is_preview_overlay(part: PlayerModelPart) -> bool {
    matches!(
        part,
        PlayerModelPart::Hat
            | PlayerModelPart::Jacket
            | PlayerModelPart::RightSleeve
            | PlayerModelPart::LeftSleeve
            | PlayerModelPart::RightPants
            | PlayerModelPart::LeftPants
    )
}

fn mcp_preview_quads(
    origin: [f32; 3],
    geometry_size: [f32; 3],
    texture_size: [f32; 3],
    uv: [u32; 2],
    mirror: bool,
) -> [PreviewQuad; 6] {
    let mut x0 = origin[0];
    let mut x1 = origin[0] + geometry_size[0];
    let y0 = 24.0 - (origin[1] + geometry_size[1]);
    let y1 = 24.0 - origin[1];
    let z0 = origin[2];
    let z1 = origin[2] + geometry_size[2];
    if mirror {
        std::mem::swap(&mut x0, &mut x1);
    }

    // Names and order match ModelBox's local variables and quadList exactly.
    let vertex7 = Vector3::new(x0, y0, z0);
    let vertex0 = Vector3::new(x1, y0, z0);
    let vertex1 = Vector3::new(x1, y1, z0);
    let vertex2 = Vector3::new(x0, y1, z0);
    let vertex3 = Vector3::new(x0, y0, z1);
    let vertex4 = Vector3::new(x1, y0, z1);
    let vertex5 = Vector3::new(x1, y1, z1);
    let vertex6 = Vector3::new(x0, y1, z1);
    let u = uv[0] as f32;
    let v = uv[1] as f32;
    // ModelBox expands positions by modelSize, but its texture coordinates
    // always use the original integer width/height/depth arguments.
    let [w, h, d] = texture_size;

    let mut quads = [
        preview_quad(
            [vertex4, vertex0, vertex1, vertex5],
            [1.0, 0.0, 0.0],
            u + d + w,
            v + d,
            u + d + w + d,
            v + d + h,
        ),
        preview_quad(
            [vertex7, vertex3, vertex6, vertex2],
            [-1.0, 0.0, 0.0],
            u,
            v + d,
            u + d,
            v + d + h,
        ),
        preview_quad(
            [vertex4, vertex3, vertex7, vertex0],
            [0.0, -1.0, 0.0],
            u + d,
            v,
            u + d + w,
            v + d,
        ),
        preview_quad(
            [vertex1, vertex2, vertex6, vertex5],
            [0.0, 1.0, 0.0],
            u + d + w,
            v + d,
            u + d + w + w,
            v,
        ),
        preview_quad(
            [vertex0, vertex7, vertex2, vertex1],
            [0.0, 0.0, -1.0],
            u + d,
            v + d,
            u + d + w,
            v + d + h,
        ),
        preview_quad(
            [vertex3, vertex4, vertex5, vertex6],
            [0.0, 0.0, 1.0],
            u + d + w + d,
            v + d,
            u + d + w + d + w,
            v + d + h,
        ),
    ];

    if mirror {
        for quad in &mut quads {
            quad.vertices.reverse();
        }
    }
    quads
}

fn preview_texture_dimensions(part: PlayerModelPart, slim: bool) -> [f32; 3] {
    match part {
        PlayerModelPart::Head | PlayerModelPart::Hat => [8.0, 8.0, 8.0],
        PlayerModelPart::Body | PlayerModelPart::Jacket => [8.0, 12.0, 4.0],
        PlayerModelPart::RightArm
        | PlayerModelPart::LeftArm
        | PlayerModelPart::RightSleeve
        | PlayerModelPart::LeftSleeve => [if slim { 3.0 } else { 4.0 }, 12.0, 4.0],
        PlayerModelPart::RightLeg
        | PlayerModelPart::LeftLeg
        | PlayerModelPart::RightPants
        | PlayerModelPart::LeftPants => [4.0, 12.0, 4.0],
    }
}

fn preview_quad(
    positions: [Vector3<f32>; 4],
    normal: [f32; 3],
    u1: f32,
    v1: f32,
    u2: f32,
    v2: f32,
) -> PreviewQuad {
    let uvs = [[u2, v1], [u1, v1], [u1, v2], [u2, v2]];
    PreviewQuad {
        vertices: std::array::from_fn(|index| PreviewVertex {
            position: positions[index],
            uv: uvs[index],
        }),
        normal: Vector3::new(normal[0], normal[1], normal[2]),
    }
}

fn rasterize_player_preview(
    skin: &PlayerSkin,
    triangles: &[PreviewTriangle],
    alpha: f32,
) -> Option<PreviewRaster> {
    let min_x = triangles
        .iter()
        .flat_map(|triangle| triangle.positions.iter().map(|point| point.x))
        .fold(f32::INFINITY, f32::min)
        .floor() as i32;
    let min_y = triangles
        .iter()
        .flat_map(|triangle| triangle.positions.iter().map(|point| point.y))
        .fold(f32::INFINITY, f32::min)
        .floor() as i32;
    let max_x = triangles
        .iter()
        .flat_map(|triangle| triangle.positions.iter().map(|point| point.x))
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil() as i32;
    let max_y = triangles
        .iter()
        .flat_map(|triangle| triangle.positions.iter().map(|point| point.y))
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil() as i32;
    let width = (max_x - min_x).max(0) as usize;
    let height = (max_y - min_y).max(0) as usize;
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return None;
    }

    let len = width * height;
    let mut base_depth = vec![f32::NEG_INFINITY; len];
    let mut base_color = vec![[0.0; 4]; len];
    let mut overlay_depth = vec![f32::NEG_INFINITY; len];
    let mut overlay_color = vec![[0.0; 4]; len];

    for triangle in triangles.iter().filter(|triangle| !triangle.overlay) {
        rasterize_preview_triangle(
            skin,
            triangle,
            min_x,
            min_y,
            width,
            height,
            &mut base_depth,
            &mut base_color,
        );
    }
    for triangle in triangles.iter().filter(|triangle| triangle.overlay) {
        rasterize_preview_triangle(
            skin,
            triangle,
            min_x,
            min_y,
            width,
            height,
            &mut overlay_depth,
            &mut overlay_color,
        );
    }

    for index in 0..len {
        if overlay_depth[index] + 1.0e-4 < base_depth[index] {
            continue;
        }
        base_color[index] = composite_preview_color(base_color[index], overlay_color[index]);
    }
    for color in &mut base_color {
        color[3] *= alpha;
    }

    Some(PreviewRaster {
        x: min_x,
        y: min_y,
        width,
        height,
        colors: base_color,
    })
}

#[allow(clippy::too_many_arguments)]
fn rasterize_preview_triangle(
    skin: &PlayerSkin,
    triangle: &PreviewTriangle,
    raster_x: i32,
    raster_y: i32,
    width: usize,
    height: usize,
    depths: &mut [f32],
    colors: &mut [[f32; 4]],
) {
    let [a, b, c] = triangle.positions;
    let area = edge(a, b, c.x, c.y);
    if area.abs() <= 1.0e-5 {
        return;
    }
    let x0 = (a.x.min(b.x).min(c.x).floor() as i32 - raster_x).clamp(0, width as i32);
    let y0 = (a.y.min(b.y).min(c.y).floor() as i32 - raster_y).clamp(0, height as i32);
    let x1 = (a.x.max(b.x).max(c.x).ceil() as i32 - raster_x).clamp(0, width as i32);
    let y1 = (a.y.max(b.y).max(c.y).ceil() as i32 - raster_y).clamp(0, height as i32);

    for row in y0..y1 {
        for column in x0..x1 {
            let px = (raster_x + column) as f32 + 0.5;
            let py = (raster_y + row) as f32 + 0.5;
            let w0 = edge(b, c, px, py) / area;
            let w1 = edge(c, a, px, py) / area;
            let w2 = edge(a, b, px, py) / area;
            if w0 < -1.0e-4 || w1 < -1.0e-4 || w2 < -1.0e-4 {
                continue;
            }

            let depth = w0 * a.z + w1 * b.z + w2 * c.z;
            let index = row as usize * width + column as usize;
            if depth <= depths[index] {
                continue;
            }
            let u = w0 * triangle.uvs[0][0] + w1 * triangle.uvs[1][0] + w2 * triangle.uvs[2][0];
            let v = w0 * triangle.uvs[0][1] + w1 * triangle.uvs[1][1] + w2 * triangle.uvs[2][1];
            let sample = skin.sample(
                u.floor().clamp(0.0, 63.0) as u32,
                v.floor().clamp(0.0, 63.0) as u32,
            );
            if sample[3] == 0 {
                continue;
            }
            depths[index] = depth;
            colors[index] = [
                sample[0] as f32 / 255.0 * triangle.shade,
                sample[1] as f32 / 255.0 * triangle.shade,
                sample[2] as f32 / 255.0 * triangle.shade,
                sample[3] as f32 / 255.0,
            ];
        }
    }
}

fn edge(a: Vector3<f32>, b: Vector3<f32>, x: f32, y: f32) -> f32 {
    (x - a.x) * (b.y - a.y) - (y - a.y) * (b.x - a.x)
}

fn composite_preview_color(base: [f32; 4], overlay: [f32; 4]) -> [f32; 4] {
    let a = overlay[3];
    if a <= 0.0 {
        return base;
    }
    let out_a = a + base[3] * (1.0 - a);
    if out_a <= 0.0 {
        return [0.0; 4];
    }
    [
        (overlay[0] * a + base[0] * base[3] * (1.0 - a)) / out_a,
        (overlay[1] * a + base[1] * base[3] * (1.0 - a)) / out_a,
        (overlay[2] * a + base[2] * base[3] * (1.0 - a)) / out_a,
        out_a,
    ]
}

fn fill_pixel_quad(
    builder: &mut crate::render::gui::GuiVertexBuilder,
    points: [[f32; 2]; 4],
    color: [f32; 4],
) {
    builder.fill_quad(
        points[0][0],
        points[0][1],
        points[1][0],
        points[1][1],
        points[2][0],
        points[2][1],
        points[3][0],
        points[3][1],
        color,
    );
}

/// Project a face from view space directly to screen.
/// Used for first-person arm rendering where the arm is already in view space.
pub(crate) fn project_face_view_space(
    skin: &PlayerSkin,
    face: &FaceSpec,
    screen_pts: [[f32; 2]; 4],
    normal_v: Vector3<f32>,
    depth: f32,
    alpha: f32,
) -> Option<ProjectedFace> {
    // In view space, camera is at origin looking along -Z.
    // A face is visible if its normal points toward the camera (+Z in view space since camera is at origin looking along -Z).
    let nv = normal_v.normalize();
    if nv.z < -0.08 {
        return None;
    }

    let light = Vector3::new(-0.35, 0.85, -0.35).normalize();
    let shade = (0.58 + 0.34 * nv.dot(&light).max(0.0)).clamp(0.45, 0.98);
    let pixels = skin_face_pixels(skin, face.uv, screen_pts, shade, alpha);
    if pixels.is_empty() {
        return None;
    }

    Some(ProjectedFace {
        depth,
        points: screen_pts,
        pixels,
    })
}

pub(crate) fn project_face(
    camera: &Camera,
    view_proj: &Matrix4<f32>,
    skin: &PlayerSkin,
    face: &FaceSpec,
    world: [[f32; 3]; 4],
    normal: Vector3<f32>,
    screen_size: [f32; 2],
    alpha: f32,
) -> Option<ProjectedFace> {
    let mut points = [[0.0; 2]; 4];
    let mut depth = 0.0;
    for (idx, p) in world.iter().enumerate() {
        let clip = view_proj * Vector4::new(p[0], p[1], p[2], 1.0);
        if clip.w <= 0.01 || clip.z < 0.0 || clip.z > clip.w {
            return None;
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        points[idx] = [
            (ndc_x * 0.5 + 0.5) * screen_size[0],
            (ndc_y * 0.5 + 0.5) * screen_size[1],
        ];
        depth += clip.w;
    }
    depth /= 4.0;

    let to_camera = (camera.position.coords
        - Vector3::new(
            (world[0][0] + world[1][0] + world[2][0] + world[3][0]) * 0.25,
            (world[0][1] + world[1][1] + world[2][1] + world[3][1]) * 0.25,
            (world[0][2] + world[1][2] + world[2][2] + world[3][2]) * 0.25,
        ))
    .try_normalize(1.0e-4)
    .unwrap_or_else(|| Vector3::new(0.0, 0.0, 1.0));
    if normal.dot(&to_camera) <= -0.08 {
        return None;
    }

    let light = Vector3::new(-0.35, 0.85, -0.35).normalize();
    let shade = (0.58 + 0.34 * normal.normalize().dot(&light).max(0.0)).clamp(0.45, 0.98);
    let pixels = skin_face_pixels(skin, face.uv, points, shade, alpha);
    if pixels.is_empty() {
        return None;
    }

    Some(ProjectedFace {
        depth,
        points,
        pixels,
    })
}

fn skin_face_pixels(
    skin: &PlayerSkin,
    uv: SkinUv,
    points: [[f32; 2]; 4],
    shade: f32,
    alpha: f32,
) -> Vec<PixelQuad> {
    let mut out = Vec::with_capacity((uv.w * uv.h) as usize);
    for py in 0..uv.h {
        for px in 0..uv.w {
            let sx = if uv.flip_x { uv.w - 1 - px } else { px };
            let sy = if uv.flip_y { uv.h - 1 - py } else { py };
            let color = skin.sample(uv.x + sx, uv.y + sy);
            if color[3] == 0 {
                continue;
            }
            let u0 = px as f32 / uv.w as f32;
            let v0 = py as f32 / uv.h as f32;
            let u1 = (px + 1) as f32 / uv.w as f32;
            let v1 = (py + 1) as f32 / uv.h as f32;
            out.push(PixelQuad {
                color: [
                    color[0] as f32 / 255.0 * shade,
                    color[1] as f32 / 255.0 * shade,
                    color[2] as f32 / 255.0 * shade,
                    color[3] as f32 / 255.0 * alpha,
                ],
                corners: [
                    bilerp(points, u0, v0),
                    bilerp(points, u1, v0),
                    bilerp(points, u1, v1),
                    bilerp(points, u0, v1),
                ],
            });
        }
    }
    out
}

fn bilerp(points: [[f32; 2]; 4], u: f32, v: f32) -> [f32; 2] {
    let top = lerp2(points[0], points[1], u);
    let bottom = lerp2(points[3], points[2], u);
    lerp2(top, bottom, v)
}

fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn apply_part_pose(
    part: PlayerModelPart,
    point: Vector3<f32>,
    body_rot: &Rotation3<f32>,
    head_rot: &Rotation3<f32>,
    pose: PlayerPose,
    slim: bool,
) -> Vector3<f32> {
    let mut p = point;
    match part {
        PlayerModelPart::Head | PlayerModelPart::Hat => {
            p = rotate_around(p, Vector3::new(0.0, 24.0, 0.0), head_rot);
        }
        PlayerModelPart::RightArm | PlayerModelPart::RightSleeve => {
            let pivot_x = if slim { -5.5 } else { -6.0 };
            let swing = pose.limb_swing.sin() * 0.55 + pose.swing_progress * 0.9;
            let rot = Rotation3::from_euler_angles(swing, 0.0, pose.swing_progress * 0.18);
            p = rotate_around(p, Vector3::new(pivot_x, 22.0, 0.0), &rot);
        }
        PlayerModelPart::LeftArm | PlayerModelPart::LeftSleeve => {
            let pivot_x = if slim { 5.5 } else { 6.0 };
            let swing = -pose.limb_swing.sin() * 0.55;
            let rot = Rotation3::from_euler_angles(swing, 0.0, 0.0);
            p = rotate_around(p, Vector3::new(pivot_x, 22.0, 0.0), &rot);
        }
        PlayerModelPart::RightLeg | PlayerModelPart::RightPants => {
            let rot = Rotation3::from_euler_angles(-pose.limb_swing.sin() * 0.42, 0.0, 0.0);
            p = rotate_around(p, Vector3::new(-2.0, 12.0, 0.0), &rot);
        }
        PlayerModelPart::LeftLeg | PlayerModelPart::LeftPants => {
            let rot = Rotation3::from_euler_angles(pose.limb_swing.sin() * 0.42, 0.0, 0.0);
            p = rotate_around(p, Vector3::new(2.0, 12.0, 0.0), &rot);
        }
        _ => {}
    }
    body_rot * p
}

fn apply_part_normal(
    part: PlayerModelPart,
    normal: Vector3<f32>,
    body_rot: &Rotation3<f32>,
    head_rot: &Rotation3<f32>,
    pose: PlayerPose,
    slim: bool,
) -> Vector3<f32> {
    let mut n = normal;
    match part {
        PlayerModelPart::Head | PlayerModelPart::Hat => n = head_rot * n,
        PlayerModelPart::RightArm | PlayerModelPart::RightSleeve => {
            let swing = pose.limb_swing.sin() * 0.55 + pose.swing_progress * 0.9;
            n = Rotation3::from_euler_angles(swing, 0.0, pose.swing_progress * 0.18) * n;
        }
        PlayerModelPart::LeftArm | PlayerModelPart::LeftSleeve => {
            n = Rotation3::from_euler_angles(-pose.limb_swing.sin() * 0.55, 0.0, 0.0) * n;
        }
        PlayerModelPart::RightLeg | PlayerModelPart::RightPants => {
            n = Rotation3::from_euler_angles(-pose.limb_swing.sin() * 0.42, 0.0, 0.0) * n;
        }
        PlayerModelPart::LeftLeg | PlayerModelPart::LeftPants => {
            n = Rotation3::from_euler_angles(pose.limb_swing.sin() * 0.42, 0.0, 0.0) * n;
        }
        _ => {}
    }
    let _ = slim;
    body_rot * n
}

fn rotate_around(point: Vector3<f32>, pivot: Vector3<f32>, rot: &Rotation3<f32>) -> Vector3<f32> {
    pivot + rot * (point - pivot)
}

fn part_visible(part: PlayerModelPart, layout: SkinLayout, skin_parts_mask: u8) -> bool {
    match part {
        PlayerModelPart::Hat => skin_parts_mask & 0x40 != 0,
        PlayerModelPart::Jacket => layout == SkinLayout::Modern64x64 && skin_parts_mask & 0x02 != 0,
        PlayerModelPart::RightSleeve => {
            layout == SkinLayout::Modern64x64 && skin_parts_mask & 0x08 != 0
        }
        PlayerModelPart::LeftSleeve => {
            layout == SkinLayout::Modern64x64 && skin_parts_mask & 0x04 != 0
        }
        PlayerModelPart::RightPants => {
            layout == SkinLayout::Modern64x64 && skin_parts_mask & 0x20 != 0
        }
        PlayerModelPart::LeftPants => {
            layout == SkinLayout::Modern64x64 && skin_parts_mask & 0x10 != 0
        }
        _ => true,
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn vanilla_preview_transform_presents_the_north_face() {
        let transform = vanilla_preview_transform(1.0, 0.0, 0.0);
        let front = transform * Vector4::new(2.0 / 16.0, 0.0, -4.0 / 16.0, 1.0);
        let back = transform * Vector4::new(2.0 / 16.0, 0.0, 4.0 / 16.0, 1.0);

        assert!(
            front.z > back.z,
            "the MCP north face must win the depth test"
        );
        assert!(
            front.x > 0.0,
            "the full vanilla matrix cancels its X reflections"
        );
    }

    #[test]
    fn preview_quads_copy_model_box_and_textured_quad_exactly() {
        let quads = mcp_preview_quads(
            [-4.0, 24.0, -4.0],
            [8.0, 8.0, 8.0],
            [8.0, 8.0, 8.0],
            [0, 0],
            false,
        );

        // ModelBox.quadList[4] is the north/front face. TexturedQuad gives
        // U2 to vertex 0 and U1 to vertex 1 instead of applying a global flip.
        let front = quads[4];
        assert_eq!(
            front.vertices.map(|vertex| vertex.position),
            [
                Vector3::new(4.0, -8.0, -4.0),
                Vector3::new(-4.0, -8.0, -4.0),
                Vector3::new(-4.0, 0.0, -4.0),
                Vector3::new(4.0, 0.0, -4.0),
            ]
        );
        assert_eq!(
            front.vertices.map(|vertex| vertex.uv),
            [[16.0, 8.0], [8.0, 8.0], [8.0, 16.0], [16.0, 16.0]]
        );
    }

    #[test]
    fn preview_overlay_expansion_does_not_shift_texture_coordinates() {
        let quads = mcp_preview_quads(
            [-4.25, 11.75, -2.25],
            [8.5, 12.5, 4.5],
            preview_texture_dimensions(PlayerModelPart::Jacket, false),
            [16, 32],
            false,
        );

        let front = quads[4];
        assert_eq!(
            front.vertices.map(|vertex| vertex.uv),
            [[28.0, 36.0], [20.0, 36.0], [20.0, 48.0], [28.0, 48.0]]
        );
        assert_eq!(front.vertices[0].position.x, 4.25);
        assert_eq!(front.vertices[0].position.z, -2.25);
    }

    #[test]
    fn player_preview_reuses_the_last_raster() {
        let skin = PlayerSkin::default_steve();
        let mut cache = PlayerPreviewCache::default();

        for yaw in [10.0, 10.1] {
            assert!(cached_player_preview(
                &mut cache, &skin, 20.0, 30.0, 1.5, yaw, -5.0, false, 0x7f, 1.0,
            )
            .is_some());
        }
        assert_eq!(cache.rasterizations, 1);

        assert!(cached_player_preview(
            &mut cache, &skin, 20.0, 30.0, 1.5, 10.2, -5.0, false, 0x7f, 1.0,
        )
        .is_some());
        assert_eq!(cache.rasterizations, 2);
    }
}

pub(crate) fn cuboid_faces(
    part: PlayerModelPart,
    origin: [f32; 3],
    size: [f32; 3],
    slim: bool,
    mirror: bool,
) -> [FaceSpec; 6] {
    let x0 = origin[0];
    let y0 = origin[1];
    let z0 = origin[2];
    let x1 = origin[0] + size[0];
    let y1 = origin[1] + size[1];
    let z1 = origin[2] + size[2];
    let [up, down, right, front, left, back] = part_uvs(
        part,
        size[0] as u32,
        size[1] as u32,
        size[2] as u32,
        slim,
        mirror,
    );
    [
        FaceSpec {
            normal: [0.0, 1.0, 0.0],
            // ModelBox quadList[3]
            corners: [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]],
            uv: up,
        },
        FaceSpec {
            normal: [0.0, -1.0, 0.0],
            // ModelBox quadList[2]
            corners: [[x1, y0, z1], [x0, y0, z1], [x0, y0, z0], [x1, y0, z0]],
            uv: down,
        },
        FaceSpec {
            normal: [-1.0, 0.0, 0.0],
            // ModelBox quadList[1]
            corners: [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            uv: right,
        },
        FaceSpec {
            normal: [0.0, 0.0, -1.0],
            // ModelBox quadList[4]
            corners: [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
            uv: front,
        },
        FaceSpec {
            normal: [1.0, 0.0, 0.0],
            // ModelBox quadList[0]
            corners: [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
            uv: left,
        },
        FaceSpec {
            normal: [0.0, 0.0, 1.0],
            // ModelBox quadList[5]
            corners: [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            uv: back,
        },
    ]
}

fn part_uvs(
    part: PlayerModelPart,
    w: u32,
    h: u32,
    d: u32,
    slim: bool,
    mirror: bool,
) -> [SkinUv; 6] {
    let base = match part {
        PlayerModelPart::Head => (0, 0),
        PlayerModelPart::Hat => (32, 0),
        PlayerModelPart::Body => (16, 16),
        PlayerModelPart::RightArm => (40, 16),
        PlayerModelPart::LeftArm => {
            if slim {
                (32, 48)
            } else {
                (32, 48)
            }
        }
        PlayerModelPart::RightLeg => (0, 16),
        PlayerModelPart::LeftLeg => (16, 48),
        PlayerModelPart::Jacket => (16, 32),
        PlayerModelPart::RightSleeve => (40, 32),
        PlayerModelPart::LeftSleeve => (48, 48),
        PlayerModelPart::RightPants => (0, 32),
        PlayerModelPart::LeftPants => (0, 48),
    };
    vanilla_box_uvs(base.0, base.1, w, h, d, mirror)
}

fn vanilla_box_uvs(u: u32, v: u32, w: u32, h: u32, d: u32, mirror: bool) -> [SkinUv; 6] {
    [
        SkinUv {
            x: u + d,
            y: v,
            w,
            h: d,
            flip_x: !mirror,
            flip_y: false,
        },
        SkinUv {
            x: u + d + w,
            y: v,
            w,
            h: d,
            flip_x: !mirror,
            flip_y: false,
        },
        SkinUv {
            x: u,
            y: v + d,
            w: d,
            h,
            flip_x: !mirror,
            flip_y: false,
        },
        SkinUv {
            x: u + d,
            y: v + d,
            w,
            h,
            flip_x: !mirror,
            flip_y: false,
        },
        SkinUv {
            x: u + d + w,
            y: v + d,
            w: d,
            h,
            flip_x: !mirror,
            flip_y: false,
        },
        SkinUv {
            x: u + d + w + d,
            y: v + d,
            w,
            h,
            flip_x: !mirror,
            flip_y: false,
        },
    ]
}

/// Generate the first-person right arm mesh in view space.
///
/// Replicates MC 1.8.9's transformation chain:
/// `renderItemInFirstPerson` → `renderPlayerArm` → `renderRightArm` → `ModelRenderer.render(1/16)`
pub fn generate_first_person_arm_mesh(
    camera: &crate::client::player::Camera,
    slim: bool,
    show_sleeve: bool,
    pose: &crate::render::first_person::FirstPersonPose,
) -> (Vec<crate::render::entity::mesh::EntityVertex>, Vec<u32>) {
    let sc = 0.0625f32;

    let swing = pose.swing_progress.clamp(0.0, 1.0);
    let sqrt_swing = swing.sqrt();
    let mc_f = -0.3 * (sqrt_swing * std::f32::consts::PI).sin();
    let mc_f1 = 0.4 * (sqrt_swing * std::f32::consts::PI * 2.0).sin();
    let mc_f2 = -0.4 * (swing * std::f32::consts::PI).sin();
    let mc_f3 = (swing * swing * std::f32::consts::PI).sin();
    let mc_f4 = (sqrt_swing * std::f32::consts::PI).sin();

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

    // Apply vanilla ItemRenderer transformations
    m = m * tr(mc_f, mc_f1, mc_f2);
    {
        let effective_fov = camera.fov * camera.fov_modifier_at(camera.partial_tick);
        let z_scale = (80.0 / effective_fov).clamp(0.5, 2.0);
        m = m * tr(0.64, -0.6, -0.72 * z_scale);
    }
    m = m * tr(0.0, pose.equip_progress * -0.6, 0.0);
    m = m * ry(45.0);
    m = m * ry(mc_f4 * 70.0);
    m = m * rz(mc_f3 * -20.0);
    m = m * tr(-1.0, 3.6, 3.5);
    m = m * rz(120.0);
    m = m * rx(200.0);
    m = m * ry(-135.0);
    m = m * tr(5.6, 0.0, 0.0);

    // ModelBiped.setRotationAngles(0, 0, 0, ...) leaves the right arm with
    // its idle Z rotation of 0.1 radians. ModelRenderer then applies the
    // shoulder pivot before rendering the box at 1/16 scale.
    let pivot_y = if slim { 2.5 } else { 2.0 };
    m = m * tr(-5.0 * sc, pivot_y * sc, 0.0);
    m = m * rz(0.1f32.to_degrees());
    m = m * nalgebra::Scale3::new(sc, sc, sc).to_homogeneous();
    m = crate::render::first_person::apply_script_transform(m, &pose.script_transform);

    // Apply walking bob (matching held-item behaviour in hand.rs).
    // The view_matrix inverse below cancels the camera bob, so only this
    // direct bob transform determines the arm's visual sway.
    if camera.view_bobbing && camera.bob_amount > 0.0001 {
        let f1 = camera.bob_phase;
        let f2 = camera.bob_amount;
        let sin_f1 = (f1 * std::f32::consts::PI).sin();
        let cos_f1 = (f1 * std::f32::consts::PI).cos();
        let tx = sin_f1 * f2 * 0.5;
        let ty = -(cos_f1 * f2).abs();
        let roll = sin_f1 * f2 * 3.0_f32.to_radians();
        let pitch = ((f1 * std::f32::consts::PI - 0.2).cos().abs() * f2) * 5.0_f32.to_radians()
            + camera.bob_pitch.to_radians();
        m = tr(tx, ty, 0.0) * Matrix4::from_euler_angles(pitch, 0.0, roll) * m;
    }

    // Transform from view space to world space.
    // Use view_matrix (with bob) so the camera bob cancels, leaving only
    // the walking bob above and the arm-tracking rotation visible.
    let inv_view = camera
        .view_matrix()
        .try_inverse()
        .unwrap_or_else(Matrix4::identity);
    let world_m = inv_view * m;
    let normal_matrix = world_m.fixed_view::<3, 3>(0, 0);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let arm_w = if slim { 3.0 } else { 4.0 };
    let arm_x = if slim { -2.0 } else { -3.0 };
    let mut layers = vec![(
        PlayerModelPart::RightArm,
        [arm_x, -2.0, -2.0],
        [arm_w, 12.0, 4.0],
    )];
    if show_sleeve {
        layers.push((
            PlayerModelPart::RightSleeve,
            [arm_x - 0.25, -2.25, -2.25],
            [arm_w + 0.5, 12.5, 4.5],
        ));
    }

    for (part, origin, size) in layers {
        let raw_faces = cuboid_faces(part, origin, size, slim, false);
        for face in &raw_faces {
            let v_start = vertices.len() as u32;
            let mut normal_v =
                normal_matrix * Vector3::new(face.normal[0], face.normal[1], face.normal[2]);
            normal_v.normalize_mut();

            // face corner UVs
            let corner_uvs = [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
            ];

            let face_idx = match face.normal {
                [0.0, 0.0, 1.0] => 0,  // front
                [0.0, 0.0, -1.0] => 1, // back
                [0.0, 1.0, 0.0] => 2,  // top
                [0.0, -1.0, 0.0] => 3, // bottom
                [1.0, 0.0, 0.0] => 4,  // right
                [-1.0, 0.0, 0.0] => 5, // left
                _ => 0,
            };
            let c_uvs = corner_uvs[face_idx];
            let f_uv = face.uv;
            let mirror_u = false; // Right arm is not mirrored by default

            for (idx, corner) in face.corners.iter().enumerate() {
                let p = world_m * Vector4::new(corner[0], corner[1], corner[2], 1.0);

                let (lu, lv) = c_uvs[idx];
                let lu = if mirror_u { 1.0 - lu } else { lu };

                let u = (f_uv.x as f32 + lu * f_uv.w as f32) / 64.0;
                let v = (f_uv.y as f32 + lv * f_uv.h as f32) / 64.0;

                vertices.push(crate::render::entity::mesh::EntityVertex {
                    position: [p.x, p.y, p.z],
                    normal: [normal_v.x, normal_v.y, normal_v.z],
                    uv: [u, v],
                    color: [1.0, 1.0, 1.0, 1.0],
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

    (vertices, indices)
}
