#version 440
// quadrille's dim layer: the scrim colour on a 4 x 4 Bayer dither of `density`,
// each dither cell one virtual pixel (`phys` device pixels), instead of a
// translucent wash. Every pixel of the layer is either the scrim colour or
// nothing, so what is behind shows through at full strength between them and
// no new colour is made. See Scrim.qml.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    // the item in device pixels, device pixels per virtual pixel, the share of
    // pixels that are lit, and the colour they are lit in
    vec2 devSize;
    float phys;
    float density;
    vec4 cScrim;
};

// A 4 x 4 Bayer threshold, 0..15, without an array: M[y][x] = 8*a0 + 4*b0 + 2*a1 + b1
// with a = x xor y and b = y.
float bayer(float x, float y) {
    float xi = mod(x, 4.0);
    float yi = mod(y, 4.0);
    float a0 = abs(mod(xi, 2.0) - mod(yi, 2.0));
    float a1 = abs(floor(xi / 2.0) - floor(yi / 2.0));
    float b0 = mod(yi, 2.0);
    float b1 = floor(yi / 2.0);
    return 8.0 * a0 + 4.0 * b0 + 2.0 * a1 + b1;
}

void main() {
    vec2 dev = floor(qt_TexCoord0 * devSize);
    vec2 v = floor(dev / phys);
    float lit = step((bayer(v.x, v.y) + 0.5) / 16.0, density);
    fragColor = vec4(cScrim.rgb, 1.0) * lit * qt_Opacity;
}
