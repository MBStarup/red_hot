#version 450

in layout(location = 0) vec4 in_color;

out layout(location = 0) vec4 color;

void main() {
    color = in_color;
}