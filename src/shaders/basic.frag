#version 450

layout(location = 1) in vec2 v_uv;
layout(location = 3) in float v_fog_dist;
layout(location = 4) in vec4 v_color;
layout(location = 5) in float v_glint;
layout(location = 10) in vec3 v_light_color;

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

layout(set = 0, binding = 1) uniform sampler2D tex_sampler;

layout(location = 0) out vec4 f_color;

void main() {
    vec4 tex = texture(tex_sampler, v_uv);
    if (tex.a * v_color.a < 0.1) discard;
    vec3 tinted = tex.rgb * v_color.rgb;
    vec3 lit = tinted * v_light_color;

    if (v_glint > 0.5) {
        // RenderItem.renderEffect (1.8.9): two passes of the glint texture,
        // rotated -50 and +10 degrees, scrolling on 3000ms / 4873ms timers,
        // tinted 0xFF8040CC and blended with glBlendFunc(GL_SRC_COLOR, GL_ONE)
        // on top of the normally rendered item. fog_color.w carries the
        // shared wall-clock seconds for the scroll timers.
        float t = fog_color.w;
        vec3 glint_tint = vec3(0.501961, 0.25098, 0.8);
        vec2 p = gl_FragCoord.xy * (1.0 / 128.0);
        float s1 = fract(dot(p, vec2(0.64279, -0.76604)) + fract(t / 3.0));
        float s2 = fract(dot(p, vec2(0.98481, 0.17365)) - fract(t / 4.873));
        float streak = pow(0.5 + 0.5 * cos(s1 * 6.2831853), 4.0)
                     + pow(0.5 + 0.5 * cos(s2 * 6.2831853), 4.0);
        lit += glint_tint * streak * 0.5;
    }

    float fog_start = fog_params.x;
    float fog_end = fog_params.y;
    float fog_factor = clamp(
        (v_fog_dist - fog_start) / max(fog_end - fog_start, 0.001),
        0.0,
        1.0
    );
    vec3 final_color = mix(lit, fog_color.rgb, fog_factor);

    f_color = vec4(final_color, tex.a * v_color.a);
}
