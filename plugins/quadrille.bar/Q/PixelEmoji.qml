import QtQuick
import "."

// An emoji redrawn as pixel art. The glyphs are colour pictures (Noto Color
// Emoji is a bitmap font); here the character is drawn into a texture `cells *
// 4` pixels a side, and pixelpicture.frag reduces it to `cells` x `cells`
// cells, each exactly one virtual pixel on the screen, with its colours cut to
// `levels` steps a channel. So every emoji is the same grid as the icons beside
// it and its edges are whole device pixels at any scale, never smoothed.
//
// If the shader cannot be built the character is drawn as itself, in its own
// size, rather than not at all.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property int cells: 12
  property int levels: 6

  readonly property bool ready: effect.status === ShaderEffect.Compiled

  implicitWidth: cells * g.unit
  implicitHeight: cells * g.unit
  width: implicitWidth
  height: implicitHeight

  Item {
    id: stage
    visible: false
    width: root.cells * 4
    height: root.cells * 4
    Text {
      anchors.centerIn: parent
      text: root.text
      textFormat: Text.PlainText
      font.family: "Noto Color Emoji"
      font.pixelSize: Math.round(stage.height * 0.8)
      renderType: Text.QtRendering
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
    blending: true
    property variant src: texture
    property real cells: root.cells
    property real levels: root.levels
    fragmentShader: Qt.resolvedUrl("shaders/pixelpicture.frag.qsb")
  }

  // the fallback
  Text {
    anchors.centerIn: parent
    visible: effect.status === ShaderEffect.Error
    text: root.text
    textFormat: Text.PlainText
    font.family: "Noto Color Emoji"
    font.pixelSize: Math.round(root.height * 0.8)
  }
}
