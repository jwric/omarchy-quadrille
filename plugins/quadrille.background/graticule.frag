#version 440
// quadrille's graticule wallpaper, drawn per output at that output's own pixel
// grid. The same picture tools/gen_themes.py writes as a PNG (graticule() there),
// computed per fragment from the virtual pixel it falls in, so a virtual pixel is
// exactly `phys` device pixels on every screen. See Graticule.qml.

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
    vec4 cGrid;
    vec4 cTick;
    vec4 cAccent;
    vec4 cEdge;
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

    // The graticule: minor lines every 40 virtual pixels dotted every other pixel,
    // ticks every 200, the accent crosshair at the centre.
    if ((mod(dx, 40.0) == 0.0 && mod(dy, 2.0) == 0.0) || (mod(dy, 40.0) == 0.0 && mod(dx, 2.0) == 0.0))
        col = cGrid;
    if ((mod(dx, 200.0) == 0.0 && mod(dy, 40.0) <= 2.0) || (mod(dy, 200.0) == 0.0 && mod(dx, 40.0) <= 2.0))
        col = cTick;
    if ((dx == 0.0 && abs(dy) < 14.0 && abs(dy) > 3.0) || (dy == 0.0 && abs(dx) < 14.0 && abs(dx) > 3.0))
        col = cAccent;

    // Corner brackets, the way quadrille marks a focus: 16-pixel arms from a
    // corner 24 pixels in.
    for (int i = 0; i < 2; i++) {
        float sx = i == 0 ? 1.0 : -1.0;
        float x0 = i == 0 ? 24.0 : w - 25.0;
        for (int j = 0; j < 2; j++) {
            float sy = j == 0 ? 1.0 : -1.0;
            float y0 = j == 0 ? 24.0 : h - 25.0;
            float ax = (x - x0) * sx;
            float ay = (y - y0) * sy;
            if ((y == y0 && ax >= 0.0 && ax < 16.0) || (x == x0 && ay >= 0.0 && ay < 16.0))
                col = cEdge;
        }
    }

    fragColor = vec4(col.rgb, 1.0) * qt_Opacity;
}
