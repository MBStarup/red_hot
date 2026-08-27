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
    mat4 light_view_proj2;
    vec4 light_dir;
    vec4 light_dir2;
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
out layout(location = 2) vec4 light_space_pos2;
out layout(location = 3) vec4 world_normal;
out layout(location = 4) vec4 light_dir;
out layout(location = 5) vec4 light_dir2;
out layout(location = 6) float brightness;
out layout(location = 7) float brightness2;
flat out layout(location = 8) int is_light;


void main() {
    gl_Position = in_position * object_uniform.model * stage_uniform.view * stage_uniform.proj;
    light_space_pos = in_position * object_uniform.model * stage_uniform.light_view_proj;
    light_space_pos2 = in_position * object_uniform.model * stage_uniform.light_view_proj2;
    world_normal = vec4(normalize((in_normal.xyz * mat3(inverse(transpose(object_uniform.model))))), 0.0);
    light_dir = stage_uniform.light_dir;
    light_dir2 = stage_uniform.light_dir2;
    is_light = object_uniform.is_light;
    // https://stackoverflow.com/a/14197892
    brightness = dot(-stage_uniform.light_dir.xyz, normalize(normalize(in_normal.xyz) * inverse(transpose(mat3(object_uniform.model)))));
    brightness2 = dot(-stage_uniform.light_dir2.xyz, normalize(normalize(in_normal.xyz) * inverse(transpose(mat3(object_uniform.model)))));
    color = in_color;
    // color = in_color * ((brightness + brightness2)/2);
}