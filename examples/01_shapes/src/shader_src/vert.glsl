#version 450
#define SKELETON_SIZE 32
#define BONES_PER_VERT 3

layout(set = 0, binding = 0) uniform DrawUniform {
    float _dummy;
} draw_uniform;

layout(set = 1, binding = 0) uniform StageUniform {
    mat4 view;
    mat4 proj;
    mat4 light_view_proj;
} stage_uniform;

layout(set = 2, binding = 0) uniform ObjectUniform {
    mat4 model;
    int is_light;
} object_uniform;

in layout(location = 0) vec4 in_position;
in layout(location = 1) vec4 in_normal;
in layout(location = 2) vec4 in_color;

out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
out layout(location = 0) vec4 color;
out layout(location = 1) vec4 light_space_pos;
flat out layout(location = 2) int is_light;

void main() {
    gl_Position = in_position * object_uniform.model * stage_uniform.view * stage_uniform.proj;
    light_space_pos = in_position * object_uniform.model * stage_uniform.light_view_proj;
    is_light = object_uniform.is_light;
    // https://stackoverflow.com/a/14197892
    // float brightness = max(stage_uniform.ambient_light, dot(-stage_uniform.light_dir, normalize((in_normal * inverse(transpose(object_uniform.model))).xyz)));
    color = in_color;
}