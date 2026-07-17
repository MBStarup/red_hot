#version 450

in layout(location = 0) vec4 in_color;
in layout(location = 1) vec4 light_space_pos;
flat in layout(location = 2) int is_light;

layout(set = 1, binding = 1) uniform sampler2DShadow shadow_map;

layout(location = 0) out vec4 color;

void main() {
    if (is_light > 0) { color = in_color * 1.0; return; }

    vec3 proj_coords = light_space_pos.xyz / light_space_pos.w;
    proj_coords.xy = proj_coords.xy * 0.5 + 0.5;

    float shadow = texture(
        shadow_map,
        vec3(proj_coords.xy, proj_coords.z)
    );

    float ambient = 0.1;
    float max_light = 0.5;
    float lighting = ambient + (max_light - ambient) * shadow;

    if (proj_coords.x < 0.0 || proj_coords.x > 1.0 ||
        proj_coords.y < 0.0 || proj_coords.y > 1.0 ||
        proj_coords.z < 0.0 || proj_coords.z > 1.0) {
        color = in_color * ambient;
    } else {
        color = in_color * lighting;
    }
}