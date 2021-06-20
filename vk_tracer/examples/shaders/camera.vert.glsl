#version 450

layout(std140, set = 0, binding = 0) uniform CameraUbo {
    layout(row_major) mat4 model;
    layout(row_major) mat4 view;
    layout(row_major) mat4 proj;
} cameraUbo;

layout(location = 0) in vec3 pos;

void main() {
    gl_Position = cameraUbo.proj * cameraUbo.model * vec4(pos, 1.0);
    // gl_Position = vec4(pos, 1.0);
}
