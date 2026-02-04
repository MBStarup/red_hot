#version 450
#define SKELETON_SIZE 24
#define BONES_PER_VERT 4


layout(set = 0, binding = 0) uniform DrawUniform {
    mat4 view;
    mat4 proj;
    vec3 light_dir;
    float ambient_light;
} draw_uniform;

layout(set = 1, binding = 0) uniform ObjectUniform {
    mat4 model;
    mat4[SKELETON_SIZE] skeleton;
    uint selected;
} object_uniform;

layout(location = 0) in vec4 in_position;
layout(location = 1) in vec4 in_normal;
layout(location = 2) in vec4 in_color;
layout(location = 3) in uint[BONES_PER_VERT] bones;
layout(location = 3 + BONES_PER_VERT) in float[BONES_PER_VERT] bone_weights;

out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
out layout(location = 0) vec4 color;

vec3 BLACK = vec3(0.0,0.0,0.0);
vec3 RED = vec3(1.0,0.0,0.0);
vec3 YELLOW = vec3(1.0,1.0,0.0);
vec3 GREEN = vec3(0.0,1.0,0.0);
vec3 CYAN = vec3(0.0,1.0,1.0);
vec3 BLUE = vec3(0.0,0.0,1.0);
vec3 MAGENTA = vec3(1.0,0.0,1.0);
vec3 WHITE = vec3(1.0,1.0,1.0);


bool eq_f(float x, float y) {
    return abs(x-y) < 0.0001;
}

void main() {
    color = vec4(1.0, 1.0, 1.0, 1.0);
    if (object_uniform.selected != 0) {
        color = vec4(0.2, 0.5, 1.0, 1.0);
    }

    vec4 in_normal = normalize(in_normal);

    vec4 position = vec4(0);
    vec4 normal = vec4(0);

    for (int i = 0; i < BONES_PER_VERT; ++i) {
        position += (in_position * object_uniform.skeleton[bones[i]]) * bone_weights[i];
        normal += (object_uniform.skeleton[bones[i]] * vec4(in_normal.xyz, 0.0));
    }

    //# Weight paint for target bone
    // int target_bone = 2;
    // float weight = 0.0;
    // for (int i = 0; i < BONES_PER_VERT; ++i) {
    //     if (bones[i] == target_bone) {
    //         weight += bone_weights[i];
    //     }
    // }
    // color = vec4(weight, 0.0, 0.0, 1.0);

    //# weight paint bones 0,1,2 (as red, green blue)
    // float colors[3] = float[3](0.0, 0.0, 0.0);
    // int target_bones[3] = int[3](0, 1, 2);
    // for (int b = 0; b < 3; ++b){
    //     for (int i = 0; i < BONES_PER_VERT; ++i) {
    //         if (bones[i] == target_bones[b]) {
    //             colors[b] += bone_weights[i];
    //         }
    //     }
    // }
    // color = vec4(colors[0], colors[1], colors[2], 1.0);

    //# HAS bones 0,1,2 to colors
    // float colors[3] = float[3](0.0, 0.0, 0.0);
    // int target_bones[3] = int[3](0, 1, 2);
    // for (int b = 0; b < 3; ++b){
    //     for (int i = 0; i < BONES_PER_VERT; ++i) {
    //         if (bones[i] == target_bones[b]) {
    //             colors[b] = 1.0;
    //         }
    //     }
    // }
    // color = vec4(colors[0], colors[1], colors[2], 1.0);

    //# HAS bones 0,1,2 at index 0 to colors
    // float colors[3] = float[3](0.0, 0.0, 0.0);
    // int target_bones[3] = int[3](0, 1, 2);
    // int target_index = 0;
    // for (int b = 0; b < 3; ++b){
        // if (bones[target_index] == target_bones[b]) {
            // colors[b] = 1.0;
        // }
    // }
    // color = vec4(colors[0], colors[1], colors[2], 1.0);

    gl_Position = position * object_uniform.model * draw_uniform.view * draw_uniform.proj;
    float brightness = max(draw_uniform.ambient_light, dot(-draw_uniform.light_dir, normalize((in_normal * inverse(transpose(object_uniform.model))).xyz)));
    color = color * brightness;

}