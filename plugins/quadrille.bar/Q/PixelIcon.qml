import QtQuick
import "."

// An app icon redrawn as pixel art: the picture is rendered into a texture
// `cells * 4` pixels a side, and a fragment shader (shaders/pixelicon.frag)
// reduces it to `cells` x `cells` cells, each exactly one virtual pixel on the
// screen, with transparency cut to on or off and the colours posterised to a few
// flat steps. So an icon looks drawn for a screen of virtual pixels whatever the
// app shipped, and its edges are whole device pixels at any scale. A one-colour
// icon (a symbolic one, or white on clear) is drawn in `ink`, so it stays visible
// on any theme.
//
//     PixelIcon { source: item.icon; cells: 11 }
Item {
  id: root

  readonly property var g: Px.of(root)

  // An image URL: a file, or an image:// URL (the system tray's icons are).
  property string source: ""
  // Side in vpx.
  property int cells: 11
  // Steps per colour channel: 3 is 27 colours at most, 4 is 64.
  property int levels: 4
  // Draw the icon as a silhouette in `ink` whatever its colours.
  property bool silhouette: false
  property color ink: Role.ink
  // Greyed: mixed 40% toward `faint`, for a passive item.
  property bool dimmed: false

  readonly property bool ready: probe.status === Image.Ready
  // The shader could not be built: nothing will ever be drawn (AppIcon falls
  // back to the plain picture).
  readonly property bool shaderFailed: effect.status === ShaderEffect.Error

  implicitWidth: cells * g.unit
  implicitHeight: cells * g.unit
  width: implicitWidth
  height: implicitHeight

  // The icon, decoded by an ordinary Image on the GUI thread (synchronously, as
  // the shell's own icons are) at `cells * 4` pixels, centred in a square so a
  // picture that is not square keeps its shape.
  Item {
    id: stage
    visible: false
    width: root.cells * 4
    height: root.cells * 4
    Image {
      id: probe
      anchors.fill: parent
      asynchronous: false
      cache: false
      smooth: true
      mipmap: true
      fillMode: Image.PreserveAspectFit
      source: root.source
      sourceSize.width: stage.width
      sourceSize.height: stage.height
    }
  }

  ShaderEffectSource {
    id: texture
    sourceItem: stage
    hideSource: false
    live: true
    smooth: false
    textureSize: Qt.size(stage.width, stage.height)
    visible: false
  }

  ShaderEffect {
    id: effect
    anchors.fill: parent
    visible: root.ready
    blending: true
    property variant src: texture
    property real cells: root.cells
    property real levels: root.levels
    property real silhouette: root.silhouette ? 1 : 0
    property real dimmed: root.dimmed ? 1 : 0
    property color ink: root.ink
    property color faint: Role.faint
    fragmentShader: Qt.resolvedUrl("shaders/pixelicon.frag.qsb")
  }
}
