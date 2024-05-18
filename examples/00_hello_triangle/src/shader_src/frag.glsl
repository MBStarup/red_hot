#version 450

in layout(location = 0) vec4 in_color;

//. I'm not sure where in the Vulkan it's specified that the output color for the screen target or whatever is at location 0, but it is
out layout(location = 0) vec4 color;

void main() {
    color = in_color;
}