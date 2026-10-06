#version 440
// The wallpaper's static Bayer glow, at the output's own virtual-pixel grid.
// The calibrated marks and drafting plates are painted by Graticule.qml.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    // The item in device pixels, device pixels per virtual pixel, and the picture's
    // size in whole virtual pixels.
    vec2 devSize;
    float phys;
    vec2 grid;
    vec4 cVoid;
    vec4 cGlow;
};

// A 4 x 4 Bayer threshold, 0..15, without an array (GLSL ES 100 has no dynamic
// indexing): M[y][x] = 8*a0 + 4*b0 + 2*a1 + b1 with a = x xor y and b = y.
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
    // The device pixel (the +0.5 of its centre keeps floor off the edge) and the
    // virtual pixel it belongs to.
    vec2 p = floor(qt_TexCoord0 * devSize);
    vec2 v = floor(p / phys);
    float x = v.x;
    float y = v.y;
    float w = grid.x;
    float h = grid.y;
    float cx = floor(w / 2.0);
    float cy = floor(h / 2.0);
    float dx = x - cx;
    float dy = y - cy;

    // A radial level, 0 at the rim and 1 at the centre, thresholded by a Bayer
    // matrix: the tone is dither, never a blend.
    float nx = dx / (w * 0.55);
    float ny = dy / (h * 0.62);
    float level = max(0.0, 1.0 - sqrt(nx * nx + ny * ny));
    vec4 col = level * 16.0 > bayer(x, y) + 0.5 ? cGlow : cVoid;

    fragColor = vec4(col.rgb, 1.0) * qt_Opacity;
}
