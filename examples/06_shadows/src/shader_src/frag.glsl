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
flat in layout(location = 9) float control;

layout(set = 1, binding = 1) uniform sampler2DShadow shadow_map;
layout(set = 1, binding = 2) uniform sampler2DShadow shadow_map2;

layout(location = 0) out vec4 color;

const int PCF_RADIUS = 1; // tweak this: 1 = 3x3, 2 = 5x5, 3 = 7x7, ...
// Generic PCF sampler: averages a (2*radius+1)^2 kernel of texel-sized
// offsets around `coords.xy`, comparing against `coords.z` each time.
float sampleShadowPCF(sampler2DShadow shadow_sampler, vec3 coords, int radius) {
    vec2 texel_size = 1.0 / vec2(textureSize(shadow_sampler, 0));

    float sum = 0.0;
    int samples = 0;
    for (int x = -radius; x <= radius; x++) {
        for (int y = -radius; y <= radius; y++) {
            vec2 offset = vec2(x, y) * texel_size;
            sum += texture(shadow_sampler, vec3(coords.xy + offset, coords.z));
            samples++;
        }
    }

    return sum / float(samples);
}

void main() {
    if (is_light > 0) { color = in_color * 1.0; return; }
    float shadow_offset = -0.00002;

    vec3 normal  = normalize(world_normal.xyz);

    // # Shadow 1
    vec3 proj_coords = light_space_pos.xyz / light_space_pos.w;
    proj_coords.xy = proj_coords.xy * 0.5 + 0.5;

    float lights = 2.0;
    float in_light = 0.0;
    float shadow = 0.0;
    if(    proj_coords.x >= 0.0 && proj_coords.x <= 1.0
        && proj_coords.y >= 0.0 && proj_coords.y <= 1.0 
        && proj_coords.z >= 0.0 && proj_coords.z <= 1.0
    ) {
        float bias1 = 0.00001 * (dot(normal, normalize(light_dir.xyz)));
        shadow = sampleShadowPCF(
            shadow_map,
            vec3(proj_coords.xy, proj_coords.z + bias1),
            PCF_RADIUS
        );
        in_light += shadow;
    }

    // # Shadow 2
    vec3 proj_coords2 = light_space_pos2.xyz / light_space_pos2.w;
    proj_coords2.xy = proj_coords2.xy * 0.5 + 0.5;

    float shadow2 = 0.0;
    if(    proj_coords2.x >= 0.0 && proj_coords2.x <= 1.0
        && proj_coords2.y >= 0.0 && proj_coords2.y <= 1.0
        && proj_coords2.z >= 0.0 && proj_coords2.z <= 1.0
    ) {
        float bias = 0.00001 * (dot(normal, normalize(light_dir2.xyz)));
        shadow2 = sampleShadowPCF(
            shadow_map2,
            vec3(proj_coords2.xy, proj_coords2.z + bias),
            PCF_RADIUS
        );
        in_light += shadow2;
    }



    const float FOG_START  = 50.0;
    const float FOG_END    = 300.0;
    float depth = 1.0 / gl_FragCoord.w; //. gl_FragCoord.w scales with depth (z) of view-space position
    float fog_factor = clamp((FOG_END - depth) / (FOG_END - FOG_START), 0.0, 1.0);

    float ambient = clamp((100.0 - depth) / (100.0 - 1.0), 0.4, 0.1);
    float b = ambient + ((1.0 - ambient)/lights) * in_light;

    vec3 shaded = (in_color * b).rgb;
    color = vec4(mix(vec3(0.7, 0.8, 1.0) * ambient, shaded, fog_factor), in_color.a);
}