#version 450

layout(std140, set = 0, binding = 0) uniform CameraUbo {
    mat4 mvp;
    vec3 lightPosition;
} camera;

layout(location = 0) in struct V2f {
    vec3 pos;
    vec2 uv;
    vec3 normal;
} v2f;

layout(location = 0) out vec4 outFragColor;

void main() {
    vec3 lightDirection = camera.lightPosition.xyz - v2f.pos.xyz;
    vec3 light = normalize(lightDirection);
    vec3 eye = normalize(-v2f.pos.xyz);
    vec3 reflectedDirection = normalize(-reflect(light, v2f.normal));
    vec3 halfway = normalize(light + eye);

    float diffuseIntensity = dot(v2f.normal, light);

    outFragColor = vec4(0.7, 0.0, 0.0, 1.0) * max(diffuseIntensity, 0.05);
}
