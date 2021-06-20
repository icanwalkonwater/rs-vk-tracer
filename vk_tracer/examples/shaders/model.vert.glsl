#version 450

layout(std140, set = 0, binding = 0) uniform CameraUbo {
    mat4 mvp;
    vec3 lightPosition;
} camera;

layout(location = 0) in vec3 pos;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec3 normal;

layout(location = 0) out struct V2f {
    vec3 pos;
    vec2 uv;
    vec3 normal;
} v2f;

void main() {
    v2f.pos = pos;
    v2f.uv = uv;
    v2f.normal = normal;

    gl_Position = camera.mvp * vec4(pos, 1.0);
}
