#version 450

layout(location = 0) in vec4 in_position;
layout(location = 1) in vec4 in_color;

out gl_PerVertex {
    vec4 gl_Position;
    float gl_PointSize;
    float gl_ClipDistance[];
};
out layout(location = 0) vec4 color;

void main() {
    gl_Position = in_position;
    color = in_color;
}