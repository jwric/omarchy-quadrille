import QtQuick
import "."

// A pixel sprite: rows of '#', digits and '.', drawn as whole-vpx rectangles
// (see Sprites.qml for the alphabet). No smoothing, no scaling: every pixel is
// `unit` logical pixels square.
Item {
  id: root

  property var rows: []
  property int level: 9
  property color color: Role.ink
  // What an unlit digit shows; transparent hides it.
  property color dim: Role.faint
  property color accent: Role.alarm
  property int unit: Px.unit

  implicitWidth: Sprites.width(rows) * unit
  implicitHeight: Sprites.height(rows) * unit
  width: implicitWidth
  height: implicitHeight

  Repeater {
    model: Sprites.runs(root.rows, root.level)
    Rectangle {
      required property var modelData
      x: modelData.x * root.unit
      y: modelData.y * root.unit
      width: modelData.w * root.unit
      height: root.unit
      antialiasing: false
      color: modelData.kind === 0 ? root.color : (modelData.kind === 2 ? root.accent : root.dim)
    }
  }
}
