#version 450

layout(location = 0) in vec4 in_color;
layout(location = 1) in vec2 in_uv;
layout(location = 2) flat in uint is_typing;
layout(location = 3) flat in uint use_color;

layout(set = 1, binding = 1) uniform sampler2D texture_atlas;

layout(location = 0) out vec4 color;

void main() {
    color = texture(texture_atlas, in_uv);
    if (color.a < 0.3) {
        if (is_typing > 0) {
            color = vec4(0.8, 0.8, 0.0, 1.0);
        } else {
            color = vec4(0.8, 0.8, 0.0, 0.7);
        }
    }
    else if (use_color > 0) {
        color = in_color;
        // color = vec4(in_uv, 0.0, 1.0);
    }
}
