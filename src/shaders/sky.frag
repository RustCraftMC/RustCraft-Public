#version 450

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform SkyUniforms {
    vec4 zenith;
    vec4 horizon;
    vec4 sun_dir;
    vec4 fog_params;
    vec4 custom_sky;
    mat4 inv_view_proj;
};
layout(set = 0, binding = 1) uniform sampler2D sun_tex;
layout(set = 0, binding = 2) uniform sampler2D moon_tex;
layout(set = 0, binding = 3) uniform sampler2D custom_sky_tex;

float hash13(vec3 p) {
    p = fract(p * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

void cube_face_uv(vec3 direction, out vec2 uv, out float face) {
    vec3 axis = abs(direction);

    if (axis.x >= axis.y && axis.x >= axis.z) {
        if (direction.x >= 0.0) {
            uv = vec2(-direction.z, direction.y) / axis.x;
            face = 0.0;
        } else {
            uv = vec2(direction.z, direction.y) / axis.x;
            face = 1.0;
        }
    } else if (axis.y >= axis.z) {
        if (direction.y >= 0.0) {
            uv = vec2(direction.x, -direction.z) / axis.y;
            face = 2.0;
        } else {
            uv = vec2(direction.x, direction.z) / axis.y;
            face = 3.0;
        }
    } else if (direction.z >= 0.0) {
        uv = vec2(direction.x, direction.y) / axis.z;
        face = 4.0;
    } else {
        uv = vec2(-direction.x, direction.y) / axis.z;
        face = 5.0;
    }

    uv = uv * 0.5 + 0.5;
}

float vanilla_star(vec3 direction) {
    vec2 uv;
    float face;
    cube_face_uv(direction, uv, face);

    const float grid_size = 22.0;
    vec2 grid = uv * grid_size;
    vec2 cell = floor(grid);
    vec2 point = fract(grid);
    vec3 seed = vec3(cell, face * 97.0 + 10842.0);

    if (hash13(seed) > 0.5165) {
        return 0.0;
    }

    vec2 center = vec2(hash13(seed + 17.0), hash13(seed + 43.0));
    center = mix(vec2(0.12), vec2(0.88), center);
    vec2 offset = point - center;

    float rotation = hash13(seed + 71.0) * 6.2831853;
    float sine = sin(rotation);
    float cosine = cos(rotation);
    offset = mat2(cosine, -sine, sine, cosine) * offset;

    float half_size = mix(0.025, 0.045, hash13(seed + 113.0));
    float square_distance = max(abs(offset.x), abs(offset.y));
    float antialias = max(fwidth(square_distance), 0.002);
    return 1.0 - smoothstep(half_size, half_size + antialias, square_distance);
}

bool billboard_uv(vec3 direction, vec3 axis, float half_extent, out vec2 uv) {
    float forward = dot(direction, axis);
    if (forward <= 0.0) {
        return false;
    }

    vec3 right = vec3(0.0, 0.0, 1.0);
    vec3 up = normalize(cross(right, axis));
    vec2 plane = vec2(dot(direction, right), dot(direction, up)) / forward;
    if (max(abs(plane.x), abs(plane.y)) > half_extent) {
        return false;
    }

    uv = plane / (half_extent * 2.0) + 0.5;
    return true;
}

vec4 sample_custom_sky(vec3 direction) {
    // OptiFine CustomSky sources are six square faces packed into a 3x2
    // atlas.  Its tile sequence and orientation come from CustomSkyLayer's
    // renderSide calls, rather than the vanilla title-screen panorama order.
    vec3 axis = abs(direction);
    vec2 face_uv;
    vec2 tile;

    if (axis.x >= axis.y && axis.x >= axis.z) {
        if (direction.x >= 0.0) {
            // +X: side 2, top-right tile
            tile = vec2(2.0, 0.0);
            face_uv = vec2(direction.z, -direction.y) / axis.x;
        } else {
            // -X: side 4, bottom-middle tile
            tile = vec2(1.0, 1.0);
            face_uv = vec2(-direction.z, -direction.y) / axis.x;
        }
    } else if (axis.y >= axis.z) {
        if (direction.y >= 0.0) {
            // +Y: side 1, top-middle tile
            tile = vec2(1.0, 0.0);
            face_uv = vec2(-direction.z, -direction.x) / axis.y;
        } else {
            // -Y: side 0, top-left tile
            tile = vec2(0.0, 0.0);
            face_uv = vec2(-direction.z, direction.x) / axis.y;
        }
    } else if (direction.z >= 0.0) {
        // +Z: side 3, bottom-left tile
        tile = vec2(0.0, 1.0);
        face_uv = vec2(-direction.x, -direction.y) / axis.z;
    } else {
        // -Z: side 5, bottom-right tile
        tile = vec2(2.0, 1.0);
        face_uv = vec2(direction.x, -direction.y) / axis.z;
    }

    face_uv = face_uv * 0.5 + 0.5;
    vec2 atlas_uv = (tile + face_uv) / vec2(3.0, 2.0);
    // Stay within the selected face when linear filtering samples an edge.
    vec2 texel = 0.5 / vec2(textureSize(custom_sky_tex, 0));
    atlas_uv = clamp(
        atlas_uv,
        tile / vec2(3.0, 2.0) + texel,
        (tile + vec2(1.0)) / vec2(3.0, 2.0) - texel
    );
    return texture(custom_sky_tex, atlas_uv);
}

void main() {
    vec2 screen_size = vec2(fog_params.x, fog_params.z);
    vec2 ndc = gl_FragCoord.xy / screen_size * 2.0 - 1.0;
    vec4 unprojected = inv_view_proj * vec4(ndc, 1.0, 1.0);
    vec3 direction = normalize(unprojected.xyz / unprojected.w);
    float elevation = clamp(direction.y * 0.5 + 0.5, 0.0, 1.0);

    vec3 light_direction = normalize(sun_dir.xyz);
    vec3 sun_position = -light_direction;
    vec3 moon_position = light_direction;
    float weather_visibility = clamp(sun_dir.w, 0.0, 1.0);
    float sun_altitude = sun_position.y;

    // Custom sky blending
    float custom_alpha = custom_sky.x;
    if (custom_alpha > 0.001) {
        float sky_rot = custom_sky.y;
        vec3 dir = direction;
        if (abs(sky_rot) > 0.0001) {
            float s = sin(sky_rot);
            float c = cos(sky_rot);
            dir = vec3(
                direction.x * c + direction.z * s,
                direction.y,
                -direction.x * s + direction.z * c
            );
        }
        vec4 custom = sample_custom_sky(dir);
        vec3 procedural = mix(horizon.rgb, zenith.rgb, clamp(pow(clamp(elevation * 2.2, 0.0, 1.0), 0.65), 0.0, 1.0));
        vec3 blended = mix(procedural, custom.rgb, custom.a * custom_alpha);

        // Sun / moon on top of custom sky
        vec2 sun_uv;
        if (billboard_uv(direction, sun_position, 0.3, sun_uv)) {
            vec4 texel = texture(sun_tex, sun_uv);
            blended += texel.rgb * texel.a * weather_visibility;
        }

        vec2 moon_uv;
        if (billboard_uv(direction, moon_position, 0.2, moon_uv)) {
            int phase_index = int(floor(fog_params.y + 0.5)) % 8;
            int column = phase_index % 4;
            int row = phase_index / 4;
            vec2 local_uv = vec2(1.0) - moon_uv;
            vec2 atlas_uv = (vec2(float(column), float(row)) + local_uv) / vec2(4.0, 2.0);
            vec4 texel = texture(moon_tex, atlas_uv);
            blended += texel.rgb * texel.a * weather_visibility;
        }

        float star_factor = clamp(1.0 - (sun_altitude * 2.0 + 0.25), 0.0, 1.0);
        float star_brightness = star_factor * star_factor * 0.5 * weather_visibility;
        if (star_brightness > 0.0 && direction.y > 0.0) {
            blended += vec3(vanilla_star(direction) * star_brightness);
        }

        f_color = vec4(blended, 1.0);
        return;
    }

    // Procedural sky (no custom texture)
    float elev_factor = clamp(elevation * 2.2, 0.0, 1.0);
    elev_factor = pow(elev_factor, 0.65);
    vec3 sky_color = mix(horizon.rgb, zenith.rgb, elev_factor);

    // Sunrise/sunset glow
    if (abs(sun_altitude) <= 0.4) {
        float phase = sun_altitude / 0.4 * 0.5 + 0.5;
        float glow_alpha = 1.0 - (1.0 - sin(phase * 3.14159265)) * 0.99;
        glow_alpha *= glow_alpha;

        vec2 horizon_direction = direction.xz / max(length(direction.xz), 0.0001);
        vec2 sun_horizon = sun_position.xz / max(length(sun_position.xz), 0.0001);
        float facing = max(dot(horizon_direction, sun_horizon), 0.0);
        float horizon_mask = clamp(1.0 - abs(direction.y) * 2.5, 0.0, 1.0);
        float directional_alpha = glow_alpha * facing * facing * horizon_mask;
        vec3 glow_color = vec3(
            phase * 0.3 + 0.7,
            phase * phase * 0.7 + 0.2,
            0.2
        );
        sky_color = mix(sky_color, glow_color, directional_alpha);
    }

    vec3 result = sky_color;

    vec2 sun_uv;
    if (billboard_uv(direction, sun_position, 0.3, sun_uv)) {
        vec4 texel = texture(sun_tex, sun_uv);
        result += texel.rgb * texel.a * weather_visibility;
    }

    vec2 moon_uv;
    if (billboard_uv(direction, moon_position, 0.2, moon_uv)) {
        int phase_index = int(floor(fog_params.y + 0.5)) % 8;
        int column = phase_index % 4;
        int row = phase_index / 4;
        vec2 local_uv = vec2(1.0) - moon_uv;
        vec2 atlas_uv = (vec2(float(column), float(row)) + local_uv) / vec2(4.0, 2.0);
        vec4 texel = texture(moon_tex, atlas_uv);
        result += texel.rgb * texel.a * weather_visibility;
    }

    float star_factor = clamp(1.0 - (sun_altitude * 2.0 + 0.25), 0.0, 1.0);
    float star_brightness = star_factor * star_factor * 0.5 * weather_visibility;
    if (star_brightness > 0.0 && direction.y > 0.0) {
        result += vec3(vanilla_star(direction) * star_brightness);
    }

    if (direction.y < 0.1) {
        float fog_blend = clamp((0.1 - direction.y) * 10.0, 0.0, 1.0);
        result = mix(result, horizon.rgb, fog_blend);
    }

    if (direction.y < 0.0) {
        float below_factor = clamp(-direction.y * 2.0, 0.0, 1.0);
        result = horizon.rgb * mix(1.0, 0.85, below_factor);
    }

    f_color = vec4(result, 1.0);
}
