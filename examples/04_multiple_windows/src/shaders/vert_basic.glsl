#version 450
#define SKELETON_SIZE 32
#define BONES_PER_VERT 3

layout(set = 0, binding = 0) uniform DrawUniform {
    float _dummy;
} draw_uniform;

layout(set = 1, binding = 0) uniform StageUniform {
    mat4 view;
    mat4 proj;
    vec3 light_dir;
    float ambient_light;
} stage_uniform;

layout(set = 2, binding = 0) uniform ObjectUniform {
    mat4 model;
} object_uniform;

layout(location = 0) in vec4 in_position;
layout(location = 1) in vec4 in_normal;
layout(location = 2) in vec4 in_color;

out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
out layout(location = 0) vec4 color;

bool eq_f(float x, float y) {
    return abs(x-y) < 0.0001;
}

void main() {

    vec4 position = in_position;

    gl_Position = position * object_uniform.model * stage_uniform.view * stage_uniform.proj;
    float brightness = max(stage_uniform.ambient_light, dot(-stage_uniform.light_dir, normalize((in_normal * inverse(transpose(object_uniform.model))).xyz)));
    color = in_color * brightness;
    // color = in_color;
    // color = vec4(0.0, 1.0, 0.0, 1.0);
}