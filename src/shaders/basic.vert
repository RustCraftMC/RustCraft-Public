#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in float block_type;
layout(location = 4) in float sky_light;
layout(location = 5) in float block_light;
layout(location = 6) in float ambient_occlusion;

layout(set = 0, binding = 0) uniform Uniforms {
    mat4 view;
    mat4 proj;
    mat4 view_proj;
    vec4 light_dir;
    vec4 fog_color;
    vec4 fog_params;
    vec4 grass_color;
    vec4 lightmap_params;
};

layout(push_constant) uniform PushConstants {
    vec3 chunk_offset;
};

layout(location = 1) out vec2 v_uv;
layout(location = 3) out float v_fog_dist;
layout(location = 4) out vec4 v_color;
layout(location = 5) out float v_glint;
layout(location = 10) out vec3 v_light_color;

float light_table(float level_01, float minimum) {
    float f1 = 1.0 - level_01;
    return (1.0 - f1) / (f1 * 3.0 + 1.0) * (1.0 - minimum) + minimum;
}

// EntityRenderer.updateLightmap from Minecraft 1.8.9. The fixed-function
// lightmap is vertex-driven and interpolated across each face, so calculate it
// once per vertex instead of repeating the same table math for every pixel.
vec3 vanilla_lightmap(float sky_level, float block_level) {
    float sun = clamp(fog_params.w, 0.0, 1.0);
    float dimension = grass_color.w;
    float table_minimum = dimension < -0.5 ? 0.1 : 0.0;
    float sky = light_table(clamp(sky_level, 0.0, 15.0) / 15.0, table_minimum)
        * (sun * 0.95 + 0.05);
    float block = light_table(clamp(block_level, 0.0, 15.0) / 15.0, table_minimum) * 1.5;

    float sky_red = sky * (sun * 0.65 + 0.35);
    float block_green = block * ((block * 0.6 + 0.4) * 0.6 + 0.4);
    float block_blue = block * (block * block * 0.6 + 0.4);
    vec3 color = vec3(sky_red + block, sky_red + block_green, sky + block_blue);
    color = color * 0.96 + 0.03;

    if (dimension > 0.5) {
        color = vec3(
            0.22 + block * 0.75,
            0.28 + block_green * 0.75,
            0.25 + block_blue * 0.75
        );
    }

    float night_vision = clamp(lightmap_params.y, 0.0, 1.0);
    if (night_vision > 0.0) {
        float inv_scale = 1.0 / max(color.r, 1.0e-5);
        inv_scale = min(inv_scale, 1.0 / max(color.g, 1.0e-5));
        inv_scale = min(inv_scale, 1.0 / max(color.b, 1.0e-5));
        color = color * (1.0 - night_vision) + color * inv_scale * night_vision;
    }

    color = clamp(color, 0.0, 1.0);

    // EntityRenderer.updateLightmap gamma (1.8.9):
    // c = c*(1-g) + (1-(1-c)^4)*g; then c = c*0.96+0.03
    float gamma = clamp(lightmap_params.x, 0.0, 1.0);
    vec3 inv = vec3(1.0) - color;
    vec3 curved = vec3(1.0) - inv * inv * inv * inv;
    color = color * (1.0 - gamma) + curved * gamma;
    color = color * 0.96 + 0.03;

    return clamp(color, 0.0, 1.0);
}

void main() {
    vec3 world_pos = position + chunk_offset;
    gl_Position = view_proj * vec4(world_pos, 1.0);
    v_uv = uv;
    v_glint = 0.0;

    if (block_type > 0.5 && block_type < 1.5) {
        v_color = vec4(grass_color.rgb, 1.0);
    } else if (block_type > 1.5 && block_type < 2.5) {
        v_color = vec4(grass_color.rgb * 0.8, 1.0);
    } else if (block_type > 2.5 && block_type < 3.5) {
        v_color = vec4(vec3(0.4667, 0.6706, 0.1843), 1.0);
    } else if (block_type > 3.5 && block_type < 4.5) {
        v_color = vec4(0.2, 0.4, 0.85, 0.72);
    } else if (block_type > 4.5 && block_type < 5.5) {
        v_color = vec4(1.0, 1.0, 1.0, 1.0);
    } else if (block_type > 5.5 && block_type < 6.5) {
        v_color = vec4(0.49, 0.74, 0.49, 1.0); // item grass tint (default ~0x7CBD7C)
    } else if (block_type > 6.5 && block_type < 7.5) {
        v_color = vec4(0.28, 0.51, 0.18, 1.0); // item foliage tint (default ~0x478247)
    } else if (block_type > 7.5 && block_type < 9.5) {
        // BlockRedstoneWire#colorMultiplier (1.8.9), encoded as 8 + power/15.
        float power = clamp(block_type - 8.0, 0.0, 1.0);
        float red = power * 0.6 + 0.4;
        if (power == 0.0) red = 0.3;
        float green = max(0.0, power * power * 0.7 - 0.5);
        float blue = max(0.0, power * power * 0.6 - 0.7);
        v_color = vec4(red, green, blue, 1.0);
    } else if (block_type > 10.5 && block_type < 11.5) {
        // Enchanted item marker. Vanilla RenderItem.renderEffect draws the
        // item with its own colors first, then adds the scrolling purple
        // glint as an extra pass; the fragment shader reproduces that pass.
        v_color = vec4(1.0);
        v_glint = 1.0;
    } else {
        v_color = vec4(1.0);
    }

    // FaceBakery.getFaceBrightness (1.8.9): axis-fixed shade, not sun-dot.
    // UP=1.0, DOWN=0.5, N/S=0.8, E/W=0.6.
    vec3 n = normalize(normal);
    float an = abs(n.y) >= abs(n.x) && abs(n.y) >= abs(n.z)
        ? (n.y >= 0.0 ? 1.0 : 0.5)
        : (abs(n.z) >= abs(n.x) ? 0.8 : 0.6);
    float ao = clamp(ambient_occlusion, 0.2, 1.0);
    v_light_color = vanilla_lightmap(sky_light, block_light) * an * ao;

    vec4 view_pos = view * vec4(world_pos, 1.0);
    v_fog_dist = length(view_pos.xyz);
}
