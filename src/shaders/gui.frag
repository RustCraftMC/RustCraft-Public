#version 450

layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;

layout(set = 0, binding = 1) uniform sampler2D gui_tex;

layout(location = 0) out vec4 f_color;

void main() {
    if (v_uv.x < 0.0 || v_uv.y < 0.0) {
        f_color = v_color;
        return;
    }
    vec4 tex = texture(gui_tex, v_uv);
    f_color = tex * v_color;
}
