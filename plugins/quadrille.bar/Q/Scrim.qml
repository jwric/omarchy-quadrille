import QtQuick
import "."

// The layer that dims what is behind an overlay: the scrim colour on a Bayer
// dither of `density` (the theme's scrim alpha), one dither cell per virtual
// pixel, so every cell is whole device pixels on any screen and the layer
// makes no new colour. Fills its parent.
//
// Where the shader cannot be built (or in a renderer that has no shaders) it
// is the plain translucent wash the stock overlays use.
Item {
  id: root

  readonly property var g: Px.of(root)

  // The theme's scrim: a colour with its alpha as the density.
  property color tone: Role.void_
  property real density: 0.5

  anchors.fill: parent

  ShaderEffect {
    id: dither
    anchors.fill: parent
    visible: status === ShaderEffect.Compiled
    blending: true
    // reals: an int property reaches a float uniform as 0
    property real phys: root.g.phys
    property real dpr: root.g.dpr
    property size devSize: Qt.size(Math.round(width * dpr), Math.round(height * dpr))
    property real density: root.density
    property color cScrim: root.tone
    fragmentShader: Qt.resolvedUrl("shaders/dither.frag.qsb")
  }

  Rectangle {
    anchors.fill: parent
    visible: dither.status !== ShaderEffect.Compiled
    color: Qt.rgba(root.tone.r, root.tone.g, root.tone.b, root.density)
  }
}
