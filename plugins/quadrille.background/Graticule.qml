import QtQuick
import Quickshell
import "Q"
import "Q/Glyphs.js" as Glyphs
import "Physical.js" as Physical
import "Drafting.js" as Drafting

// The glow is a shader; the measuring marks are a static device-pixel raster.
// Both address the same integer virtual grid. Canvas.Image also lets the real
// marks and native bitmap lettering be checked on Qt's offscreen platform.
Item {
  id: root
  property real phys: 2
  property real dpr: 1
  property var monitor: ({})
  property var monitors: []
  property var overrides: ({})
  readonly property bool software: Quickshell.env("QT_QPA_PLATFORM") === "offscreen"
  readonly property bool compiled: software || glow.status === ShaderEffect.Compiled
  readonly property bool failed: !software && glow.status === ShaderEffect.Error
  readonly property var physical: Physical.resolve(monitor, overrides)
  readonly property var measured: monitors.map(function(m) { return { input: m, physical: Physical.resolve(m, root.overrides) } })
  readonly property var drawing: Drafting.plan(monitor, physical, measured, Glyphs, Px.barVpx)
  readonly property size devSize: Qt.size(physical.widthPx, physical.heightPx)
  readonly property var colors: ({ "void": Role.void_, ground: Role.ground,
    edge: Role.edge, line: Role.line, faint: Role.faint, accent: Role.accent, ink: Role.ink })

  ShaderEffect {
    id: glow
    anchors.fill: parent
    visible: !root.software
    property real phys: root.phys
    property size devSize: root.devSize
    property size grid: Qt.size(root.drawing.width, root.drawing.height)
    property color cVoid: Role.void_
    property color cGlow: Role.ground
    blending: false
    fragmentShader: Qt.resolvedUrl("graticule.frag.qsb")
  }

  Canvas {
    id: marks
    anchors.fill: parent
    canvasSize: Qt.size(width, height)
    canvasWindow: Qt.rect(0, 0, width, height)
    renderTarget: Canvas.Image
    renderStrategy: Canvas.Immediate
    smooth: false
    antialiasing: false
    onPaint: Drafting.paint(getContext("2d"), root.drawing, root.physical.pixelsPerVpx, root.colors, root.software, root.dpr)
  }

  onDrawingChanged: marks.requestPaint()
  onColorsChanged: marks.requestPaint()
  Component.onCompleted: marks.requestPaint()
}
