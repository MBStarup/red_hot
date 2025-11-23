#version 450
#define SKELETON_SIZE 32
#define BONES_PER_VERT 3


layout(set = 0, binding = 0) uniform DrawUniform {
    mat4 view;
    mat4 proj;
    vec3 light_dir;
    float ambient_light;
} draw_uniform;

layout(set = 1, binding = 0) uniform ObjectUniform {
    mat4 model;
    mat4[SKELETON_SIZE] skeleton;
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
    vec4 in_normal = normalize(in_normal);

    vec4 position = vec4(0);
    vec4 normal = vec4(0);

    for (int i = 0; i < BONES_PER_VERT; ++i) {
        position += (in_position * object_uniform.skeleton[bones[i]]) * bone_weights[i];
        normal += (object_uniform.skeleton[bones[i]] * vec4(in_normal.xyz, 0.0));
    }

    gl_Position = position * object_uniform.model * draw_uniform.view * draw_uniform.proj;
    float brightness = max(draw_uniform.ambient_light, dot(-draw_uniform.light_dir, normalize((in_normal * inverse(transpose(object_uniform.model))).xyz)));
    color = in_color * brightness;

    // if (bones[0] == 0 && bones[1] == 1 && bones[2] == 2) {
        // vec3 color1 = mix(BLACK, RED, bone_weights[0]);     //. Bone 0
        // vec3 color2 = mix(color1, GREEN, bone_weights[1]);  //. Bone 1
        // vec3 color3 = mix(color2, BLUE, bone_weights[2]);   //. Bone 2
        // color = vec4(color3, 1.0);
    // } else
    // if (bones[0] == 1 && bones[1] == 2 && bones[2] == 3) {
        // vec3 color1 = mix(BLACK, GREEN, bone_weights[0]);   //. Bone 1
        // vec3 color2 = mix(color1, BLUE, bone_weights[1]);   //. Bone 2
        // vec3 color3 = mix(color2, YELLOW, bone_weights[2]); //. Bone 3
        // color = vec4(color3, 1.0);
    // } else {
        // color = vec4(MAGENTA, 1.0);
    // }

    //. Weight paint for target bone
    // int target_bone = 2;
    // float redness = 0.0;
    // for (int i = 0; i < BONES_PER_VERT; ++i) {
    //     if (bones[i] == target_bone) {
    //         redness += bone_weights[i];
    //     }
    // }
    // color = vec4(redness, 0.0, 0.0, 1.0);


    // color = vec4(1.0, 0.0, 0.0, 1.0);
}