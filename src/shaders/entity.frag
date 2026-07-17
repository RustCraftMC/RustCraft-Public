#version 450

layout(location = 0) in vec3 v_normal;
layout(location = 1) in vec2 v_uv;
layout(location = 2) in vec3 v_light;
layout(location = 3) in float v_fog_dist;
layout(location = 4) in vec4 v_color;
layout(location = 5) in float v_world_y;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Uniforms {
    mat4 view;
    mat4 proj;
    mat4 view_proj;
    vec4 light_dir;
    vec4 fog_color;
    vec4 fog_params;
    vec4 grass_color;
};

layout(set = 0, binding = 1) uniform sampler2D tex_sampler;

void main() {
    vec4 tex = texture(tex_sampler, v_uv);

    // Alpha test
    if (tex.a < 0.1) discard;

    // Apply entity color tint and lighting
    vec3 tinted = tex.rgb * v_color.rgb;
    vec3 lit = tinted * v_light;

    // Fog
    float fog_start = fog_params.x;
    float fog_end = fog_params.y;
    float camera_y = fog_params.z;
    float height_delta = v_world_y - camera_y;
    float height_density = clamp(-height_delta / 96.0, 0.0, 0.06);

    float dist_factor = clamp((v_fog_dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    float fog_factor = max(dist_factor, height_density);

    // Aerial perspective — only at extreme distance
    float sky_bleed = max(0.0, fog_factor - 0.9) * 10.0;
    vec3 final_color = mix(lit, fog_color.rgb, sky_bleed);

    // Standard distance fog
    final_color = mix(final_color, fog_color.rgb, fog_factor);

    f_color = vec4(final_color, tex.a * v_color.a);
}
