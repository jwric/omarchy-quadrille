import QtQuick
import "."

// Emphasis: the accent behind text knocked out in on_accent, a line tall.
// Where another interface would reach for a bold weight.
Rectangle {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property color tone: Role.accent
  property color ink: Role.onAccent
  // Air either side of the ink, in vpx (quadrille pads 2, less the face's
  // side bearings: 1 on the left, 0 on the right).
  property int air: 2

  implicitWidth: label.width + (air - 1 + air) * g.unit
  implicitHeight: g.line
  width: implicitWidth
  height: implicitHeight
  color: tone
  antialiasing: false

  PixelText {
    id: label
    x: (root.air - 1) * root.g.unit
    text: root.text
    ink: root.ink
  }
}
