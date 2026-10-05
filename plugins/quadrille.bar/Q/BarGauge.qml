import QtQuick
import "."

// A stepped bar gauge: a row of cells, lit up to `value`, never past it
// (quadrille's `bar`, segmented). Cells beyond `redline` light in the alarm
// colour; unlit cells are raised glass.
Item {
  id: root

  // 0..1
  property real value: 0
  property real redline: 1
  property int cells: 5
  // Cell width and height in vpx, and the gap between cells (1 vpx).
  property int cellWidth: 2
  property int cellHeight: 5
  property color fill: Role.live
  property color over: Role.alarm
  property color unlit: Role.raised

  readonly property int step: cellWidth + 1
  readonly property int lit: Math.floor(Math.max(0, Math.min(1, value)) * cells + 1e-6)
  readonly property int red: Math.floor(Math.max(0, Math.min(1, redline)) * cells + 1e-6)

  implicitWidth: (cells * step - 1) * Px.unit
  implicitHeight: cellHeight * Px.unit
  width: implicitWidth
  height: implicitHeight

  Repeater {
    model: root.cells
    Rectangle {
      required property int index
      x: index * root.step * Px.unit
      width: root.cellWidth * Px.unit
      height: parent.height
      antialiasing: false
      color: index >= root.lit ? root.unlit : (index >= root.red ? root.over : root.fill)
    }
  }
}
