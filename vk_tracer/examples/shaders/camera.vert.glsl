#version 450

layout(std140, set = 0, binding = 0) uniform CameraUbo {
    mat4 model;
    mat4 view;
    mat4 proj;
} cameraUbo;

layout(location = 0) in vec3 pos;

void main() {
    gl_Position = cameraUbo.proj * cameraUbo.view * cameraUbo.model * vec4(pos, 1.0);
}
