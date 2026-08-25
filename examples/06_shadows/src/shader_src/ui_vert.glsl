#version 450
#define SKELETON_SIZE 32
#define BONES_PER_VERT 3

layout(set = 0, binding = 0) uniform DrawUniform {
    float _dummy;
} draw_uniform;

layout(set = 1, binding = 0) uniform StageUniform {
    float _dummy;
} stage_uniform;

layout(set = 2, binding = 0) uniform ObjectUniform {
    mat4 model;
    vec3 color;
} object_uniform;

layout(location = 0) in vec4 in_position;
layout(location = 1) in vec4 in_normal;
layout(location = 2) in vec4 in_uv;

out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
out layout(location = 0) vec4 color;
out layout(location = 1) vec2 uv;

void main() {
    gl_Position = in_position * object_uniform.model;
    color = vec4(object_uniform.color, 1.0);
    uv = in_uv.xz;
    color = vec4(in_uv.xz, 0.0, 1.0);
}