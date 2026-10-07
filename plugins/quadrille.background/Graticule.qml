import QtQuick
import Quickshell
import "Q"
import "Q/Glyphs.js" as Glyphs
import "Q/GlyphsBig.js" as Big
import "Physical.js" as Physical
import "Drafting.js" as Drafting

// The field shader is constant; the exact chart is a static device-pixel texture.
// Both address the same integer virtual grid. Canvas.Image also lets the real
// marks and native bitmap lettering be checked on Qt's offscreen platform.
Item {
  id: root
  property real phys: physical.pixelsPerVpx
  property real dpr: 1
  readonly property bool compactTexture: Math.abs(dpr-Math.round(dpr*120)/120)<0.00001
  property var monitor: ({})
  property var monitors: []
  property var overrides: ({})
  property string composition: "aperture"
  property string starTone: "faint"
  readonly property bool software: Quickshell.env("QT_QPA_PLATFORM") === "offscreen"
  readonly property bool compiled: software || glow.status === ShaderEffect.Compiled
  readonly property bool failed: !software && glow.status === ShaderEffect.Error
  readonly property var physical: Physical.resolve(monitor, overrides)
  readonly property var measured: monitors.map(function(m) { return { input: m, physical: Physical.resolve(m, root.overrides) } })
  readonly property var drawing: Drafting.plan(monitor, physical, measured, Glyphs, Px.barVpx, Big, composition, starTone)
  // on_accent is read through stated(): Role.onAccent did not follow a theme
  // change here (it stayed at its default while every other role updated).
  readonly property var colors: ({ "void": Role.void_, ground: Role.ground,
    raised: Role.raised, hover: Role.hover, edge: Role.edge, line: Role.line,
    faint: Role.faint, accent: Role.accent, on_accent: Role.stated("on_accent", Role.void_), ink: Role.ink, muted: Role.muted,
    live: Role.live, caution: Role.caution, alarm: Role.alarm })

  ShaderEffect {
    id: glow
    anchors.fill: parent
    visible: !root.software
    property color cVoid: Role.void_
    blending: false
    fragmentShader: Qt.resolvedUrl("graticule.frag.qsb")
  }

  Canvas {
    id: marks
    width: root.compactTexture ? Drafting.canvasLength(root.drawing.width,root.dpr) : root.width
    height: root.compactTexture ? Drafting.canvasLength(root.drawing.height,root.dpr) : root.height
    scale: root.compactTexture ? root.physical.pixelsPerVpx : 1
    transformOrigin: Item.TopLeft
    canvasSize: Qt.size(width, height)
    canvasWindow: Qt.rect(0, 0, width, height)
    renderTarget: Canvas.Image
    renderStrategy: Canvas.Immediate
    smooth: false
    antialiasing: false
    onPaint: Drafting.paint(getContext("2d"), root.drawing, root.compactTexture ? 1 : root.physical.pixelsPerVpx, root.colors, root.software, root.dpr)
  }

  onDrawingChanged: marks.requestPaint()
  onColorsChanged: marks.requestPaint()
  onDprChanged: marks.requestPaint()
  Component.onCompleted: marks.requestPaint()
}
