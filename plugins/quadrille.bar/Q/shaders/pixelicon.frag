#version 440
// An app icon brought down to the pixel grid, one fragment shader.
//
// `src` is the icon rendered into a square texture `cells * 4` pixels a side;
// each output cell (one vpx) averages its 4 x 4 texels, drops to nothing under
// half coverage, and has its colour posterised to `levels` steps a channel, so
// what is left is a flat, restrained palette. Near-white and near-black pixels
// of a low-saturation icon (a symbolic one, or white-on-clear made for a dark
// panel) take `ink`, the theme's, so the icon stays visible on any theme; with
// `silhouette` every cell takes it.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float cells;
    float levels;
    float silhouette;
    float dimmed;
    vec4 ink;
    vec4 faint;
};
layout(binding = 1) uniform sampler2D src;

void main() {
    vec2 cell = floor(qt_TexCoord0 * cells);
    // clamp the last row/column: uv 1.0 would be cell `cells`
    cell = min(cell, vec2(cells - 1.0));
    vec2 origin = cell / cells;
    float step = 1.0 / cells;

    vec4 acc = vec4(0.0);
    for (int j = 0; j < 4; j++) {
        for (int i = 0; i < 4; i++) {
            vec2 p = origin + (vec2(float(i), float(j)) + 0.5) * 0.25 * step;
            acc += texture(src, p);
        }
    }
    acc /= 16.0;                     // premultiplied average

    if (acc.a < 0.5) {
        fragColor = vec4(0.0);
        return;
    }
    vec3 rgb = acc.rgb / acc.a;      // straight colour

    float hi = max(rgb.r, max(rgb.g, rgb.b));
    float lo = min(rgb.r, min(rgb.g, rgb.b));
    float sat = hi > 0.0 ? (hi - lo) / hi : 0.0;
    float lum = dot(rgb, vec3(0.2126, 0.7152, 0.0722));

    bool mono = silhouette > 0.5 || (sat < 0.12 && (lum > 0.88 || lum < 0.12));
    if (mono) {
        rgb = ink.rgb;
    } else {
        float s = max(levels - 1.0, 1.0);
        rgb = floor(rgb * s + 0.5) / s;
    }
    if (dimmed > 0.5) rgb = mix(rgb, faint.rgb, 0.4);
    fragColor = vec4(rgb, 1.0) * qt_Opacity;
}
