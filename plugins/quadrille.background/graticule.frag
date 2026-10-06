#version 440
// A quiet sheet field. Every mark is painted by the integer-grid Canvas.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 cVoid;
};
void main() {
    fragColor = vec4(cVoid.rgb, 1.0) * qt_Opacity;
}
