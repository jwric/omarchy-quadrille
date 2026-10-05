import QtQuick
import "."
import "Glyphs.js" as Glyphs

// A reading: its name in muted on the left and its value in ink on the right,
// one line (quadrille's `reading`). When both will not fit the name is left out
// and the value stays: a cut name reads as a different name.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string label: ""
  property string value: ""
  property color valueInk: Role.ink
  property color labelInk: Role.muted

  readonly property real valueWidth: Glyphs.length(value) * g.cellW
  readonly property real labelWidth: Glyphs.length(label) * g.cellW

  implicitWidth: g.px(120)
  implicitHeight: g.line
  width: implicitWidth
  height: implicitHeight

  PixelText {
    visible: root.labelWidth + root.valueWidth + root.g.cellW <= root.width
    text: root.label
    ink: root.labelInk
  }
  PixelText {
    x: Math.max(0, root.width - width)
    text: root.value
    ink: root.valueInk
    columns: root.g.columns(root.width)
  }
}
