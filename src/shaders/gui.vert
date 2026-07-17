#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;

layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;

// Manual ortho: pixel(0,0)=top-left → NDC(-1,+1), pixel(w,h)=bottom-right → NDC(+1,-1)
layout(set = 0, binding = 0) uniform GuiUniforms {
    vec2 screen_size;
};

void main() {
    float ndc_x = position.x / (screen_size.x * 0.5) - 1.0;
    float ndc_y = position.y / (screen_size.y * 0.5) - 1.0;
    gl_Position = vec4(ndc_x, ndc_y, 0.0, 1.0);
    v_uv = uv;
    v_color = color;
}
