#version 450
#define SKELETON_SIZE 32
#define BONES_PER_VERT 3

layout(set = 0, binding = 0) uniform DrawUniform {
    float _dummy;
} draw_uniform;

layout(set = 1, binding = 0) uniform StageUniform {
    uint is_typing;
} stage_uniform;

layout(set = 2, binding = 0) uniform ObjectUniform {
    mat4 model;
    vec3 color;
    uint use_color;
    vec2 uv_offset;
    vec2 uv_area;
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
out layout(location = 2) uint is_typing;
out layout(location = 3) uint use_color;

void main() {
    gl_Position = in_position * object_uniform.model;
    is_typing = stage_uniform.is_typing;
    use_color = object_uniform.use_color;
    color = vec4(object_uniform.color, 1.0);
    uv = (in_uv.xz * object_uniform.uv_area) + object_uniform.uv_offset;
    // color = vec4(in_uv.xz, 0.0, 1.0);
}