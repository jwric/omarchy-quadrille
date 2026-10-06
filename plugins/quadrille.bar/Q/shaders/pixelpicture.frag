#version 440
// A colour picture brought down to the pixel grid: `src` is the picture rendered
// into a square texture `cells * 4` pixels a side; each output cell (one vpx)
// averages its 4 x 4 texels, drops to nothing under half coverage and has its
// colour posterised to `levels` steps a channel. The difference from
// pixelicon.frag is that black, white and grey stay what they are: an emoji's
// black pupils must not take the theme's ink. See PixelEmoji.qml.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float cells;
    float levels;
};
layout(binding = 1) uniform sampler2D src;

void main() {
    vec2 cell = min(floor(qt_TexCoord0 * cells), vec2(cells - 1.0));
    vec2 origin = cell / cells;
    float step = 1.0 / cells;
    vec4 acc = vec4(0.0);
    for (int j = 0; j < 4; j++) {
        for (int i = 0; i < 4; i++) {
            vec2 p = origin + (vec2(float(i), float(j)) + 0.5) * 0.25 * step;
            acc += texture(src, p);
        }
    }
    acc /= 16.0;
    if (acc.a < 0.5) {
        fragColor = vec4(0.0);
        return;
    }
    vec3 rgb = acc.rgb / acc.a;
    float s = max(levels - 1.0, 1.0);
    rgb = floor(rgb * s + 0.5) / s;
    fragColor = vec4(rgb, 1.0) * qt_Opacity;
}
