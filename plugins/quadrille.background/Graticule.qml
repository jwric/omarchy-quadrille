import QtQuick
import "Q"

// The graticule wallpaper, drawn at the pixel grid of the screen it is on.
//
// A virtual pixel is `phys` device pixels (Px: 2 at scale 1, 3 at 1.666667), so
// every block of the dither and every line of the graticule is a whole number of
// device pixels on any output, which a bitmap scaled by a non-integer factor
// cannot be. The picture is tools/gen_themes.py's graticule(), computed per
// fragment (graticule.frag), in the live theme's roles: it re-themes with the
// shell, and `tools/gen_wallpapers.py` writes the same picture as a PNG.
//
// `compiled` is false until the shader has built, and stays false if it cannot
// (the caller then shows the theme's PNG, as the stock background does).
ShaderEffect {
  id: root

  // Device pixels per virtual pixel on this screen (Px.forWindow(win).phys) and the
  // window's device pixel ratio.
  // (a real: ShaderEffect hands an int property to a float uniform as 0)
  property real phys: 2
  property real dpr: 1

  readonly property bool compiled: status === ShaderEffect.Compiled
  readonly property bool failed: status === ShaderEffect.Error

  // What the shader reads. The item is as big as its window, whose buffer is a
  // whole number of device pixels.
  property size devSize: Qt.size(Math.round(width * dpr), Math.round(height * dpr))
  // Whole virtual pixels, the last one cropped by the screen's edge when the size
  // is not a multiple of `phys` (854 x 534 on 2560 x 1600 at 3).
  property size grid: Qt.size(Math.ceil(devSize.width / phys), Math.ceil(devSize.height / phys))
  // gen_themes.py: grid = mix(void, line, 0.35), tick = mix(void, line, 0.8).
  property color cVoid: Role.void_
  property color cGlow: Role.ground
  property color cGrid: Role.mix(Role.void_, Role.line, 0.35)
  property color cTick: Role.mix(Role.void_, Role.line, 0.8)
  property color cAccent: Role.accent
  property color cEdge: Role.edge

  blending: false
  fragmentShader: Qt.resolvedUrl("graticule.frag.qsb")
}
