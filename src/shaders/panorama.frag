#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform PanoramaUniforms {
    float time;
    float aspect_ratio;
};
layout(set = 0, binding = 1) uniform sampler2D tex_panorama;

vec3 sample_cube(vec3 dir) {
    vec3 a = abs(dir);
    int face;
    vec2 uv;
    if (a.x >= a.y && a.x >= a.z) {
        if (dir.x > 0.0) { face = 1; uv = vec2(-dir.z, -dir.y) / a.x; } // Right
        else             { face = 3; uv = vec2( dir.z, -dir.y) / a.x; } // Left
    } else if (a.y >= a.x && a.y >= a.z) {
        // The title panorama follows vanilla's face order: face 4 is the
        // top/sky texture and face 5 is the bottom/ground texture.
        if (dir.y > 0.0) { face = 4; uv = vec2( dir.x,  dir.z) / a.y; } // Top
        else             { face = 5; uv = vec2( dir.x, -dir.z) / a.y; } // Bottom
    } else {
        if (dir.z > 0.0) { face = 0; uv = vec2( dir.x, -dir.y) / a.z; } // Front
        else             { face = 2; uv = vec2(-dir.x, -dir.y) / a.z; } // Back
    }
    
    // Convert from -1..1 to 0..1 and stay half a texel inside each face.
    uv = clamp(uv * 0.5 + 0.5, vec2(0.5 / 256.0), vec2(255.5 / 256.0));
    
    // Each face is 1/6th of the strip
    float u = (float(face) + uv.x) / 6.0;
    
    return texture(tex_panorama, vec2(u, uv.y)).rgb;
}

vec3 rotate_y(vec3 v, float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return vec3(v.x * c - v.z * s, v.y, v.x * s + v.z * c);
}

vec3 rotate_x(vec3 v, float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return vec3(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
}

void main() {
    // Center at 0,0
    vec2 screen_uv = v_uv * 2.0 - 1.0;
    screen_uv.y = -screen_uv.y;
    // Apply aspect ratio
    screen_uv.x *= aspect_ratio;
    
    // Map to 3D ray. Field of view ~90 degrees.
    vec3 dir = normalize(vec3(screen_uv.x, screen_uv.y, 1.0));
    
    // Match the vanilla title panorama: a slow yaw with a gentle upward pitch.
    float rot_y = time * 0.018;
    float rot_x = radians(18.0) + sin(time * 0.10) * radians(2.0);
    
    dir = rotate_x(dir, rot_x);
    dir = rotate_y(dir, rot_y);
    
    // Vanilla-like softening: linear sampling removes blocky texels, while
    // this compact Gaussian kernel keeps the panorama readable.
    float spread = 0.004;
    vec3 color = sample_cube(dir) * 4.0;
    color += sample_cube(rotate_y(dir, spread)) * 2.0;
    color += sample_cube(rotate_y(dir, -spread)) * 2.0;
    color += sample_cube(rotate_x(dir, spread)) * 2.0;
    color += sample_cube(rotate_x(dir, -spread)) * 2.0;
    color += sample_cube(rotate_x(rotate_y(dir, spread), spread));
    color += sample_cube(rotate_x(rotate_y(dir, spread), -spread));
    color += sample_cube(rotate_x(rotate_y(dir, -spread), spread));
    color += sample_cube(rotate_x(rotate_y(dir, -spread), -spread));
    color /= 16.0;
    
    // Slightly darken like vanilla MC does to make white text pop
    color *= 0.72;
    
    f_color = vec4(color, 1.0);
}
