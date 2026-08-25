#version 450

layout(location = 0) in vec4 in_color;
layout(location = 1) in vec2 in_uv;

layout(set = 1, binding = 1) uniform sampler2D texture_atlas;

layout(location = 0) out vec4 color;

void main() {
    color = texture(texture_atlas, in_uv);
    // color = vec4(in_uv, 0.0, 1.0);
}
