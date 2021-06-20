#version 450

layout(std140, set = 0, binding = 0) uniform CameraUbo {
    mat4 mvp;
} camera;

layout(location = 0) in vec3 pos;

void main() {
    gl_Position = camera.mvp * vec4(pos, 1.0);
}
