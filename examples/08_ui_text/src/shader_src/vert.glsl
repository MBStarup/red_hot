#version 450

layout(set = 0, binding = 0) uniform DrawUniform {
    float _dummy;
} draw_uniform;

layout(set = 1, binding = 0) uniform StageUniform {
    mat4 view;
    mat4 proj;
} stage_uniform;

layout(set = 2, binding = 0) uniform ObjectUniform {
    mat4 model;
} object_uniform;

in layout(location = 0) vec4 in_position;
in layout(location = 1) vec4 in_normal;
in layout(location = 2) vec4 in_color;

out gl_PerVertex {
    vec4 gl_Position;
};

out layout(location = 0) vec4 color;

void main() {
    gl_Position = in_position * object_uniform.model * stage_uniform.view * stage_uniform.proj;
    color = in_color * 0.5;
}