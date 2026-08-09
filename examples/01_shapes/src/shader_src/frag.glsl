#version 450

in layout(location = 0) vec4 in_color;
in layout(location = 1) vec4 light_space_pos;
in layout(location = 2) vec4 light_space_pos2;
flat in layout(location = 3) int is_light;

layout(set = 1, binding = 1) uniform sampler2DShadow shadow_map;
layout(set = 1, binding = 2) uniform sampler2DShadow shadow_map2;

layout(location = 0) out vec4 color;

void main() {
    if (is_light > 0) { color = in_color * 1.0; return; }

    // # Shadow 1
    vec3 proj_coords = light_space_pos.xyz / light_space_pos.w;
    proj_coords.xy = proj_coords.xy * 0.5 + 0.5;

    float shadow = 0.0;
    if(    proj_coords.x >= 0.0 && proj_coords.x <= 1.0
        && proj_coords.y >= 0.0 && proj_coords.y <= 1.0 
        && proj_coords.z >= 0.0 && proj_coords.z <= 1.0
    ) {
        shadow = texture(
            shadow_map,
            vec3(proj_coords.xy, proj_coords.z)
        );
    }

    // # Shadow 2
    vec3 proj_coords2 = light_space_pos2.xyz / light_space_pos2.w;
    proj_coords2.xy = proj_coords2.xy * 0.5 + 0.5;

    float shadow2 = 0.0;
    if(    proj_coords2.x >= 0.0 && proj_coords2.x <= 1.0
        && proj_coords2.y >= 0.0 && proj_coords2.y <= 1.0
        && proj_coords2.z >= 0.0 && proj_coords2.z <= 1.0
    ) {
        shadow2 = texture(
            shadow_map2,
            vec3(proj_coords2.xy, proj_coords2.z)
        );
    }

    float ambient = 0.1;

    color = vec4(shadow, shadow2, 0.0, 1.0);
    color = in_color * max(ambient, ((shadow + shadow2) / 2.0));
}