#version 450

in layout(location = 0) vec4 in_color;
in layout(location = 1) vec4 light_space_pos;
in layout(location = 2) vec4 light_space_pos2;
in layout(location = 3) vec4 world_normal;
in layout(location = 4) vec4 light_dir;
in layout(location = 5) vec4 light_dir2;
in layout(location = 6) float brightness;
in layout(location = 7) float brightness2;
flat in layout(location = 8) int is_light;

layout(set = 1, binding = 1) uniform sampler2DShadow shadow_map;
layout(set = 1, binding = 2) uniform sampler2DShadow shadow_map2;

layout(location = 0) out vec4 color;

void main() {
    color = vec4(0.0, 1.0, 0.0, 1.0);
}