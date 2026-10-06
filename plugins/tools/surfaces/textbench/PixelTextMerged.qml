import QtQuick
import "quadrille.bar/Q"
import "Runs.js" as Big

// Variant 1: the same rectangles as PixelText, with runs that repeat down consecutive rows merged into one.
Item {
  id: root
  readonly property var g: Px.of(root)
  property string text: ""
  property color ink: Role.ink
  width: Array.from(text).length * g.cellW
  height: g.line
  Repeater {
    model: Big.rects(root.text)
    Rectangle {
      required property var modelData
      x: modelData.x * root.g.unit
      y: modelData.y * root.g.unit
      width: modelData.w * root.g.unit
      height: modelData.h * root.g.unit
      color: root.ink
      antialiasing: false
    }
  }
}
