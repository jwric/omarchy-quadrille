import QtQuick
import "."

// A reading as a stepped bar that fills the width it is given: cells two
// virtual pixels wide and one apart (`cellWidth`), lit up to the value, never
// past it, unlit ones raised glass. Past `redline` the lit ones are the alarm
// colour. The cells are counted from the width, so it is whole at any size.
Item {
  id: root

  readonly property var g: Px.of(root)

  property real value: 0
  property real redline: 1
  property int cellWidth: 2
  // Height in vpx.
  property int cellHeight: 3
  property color fill: Role.ink
  property color over: Role.alarm
  property color unlit: Role.raised

  readonly property int pitch: cellWidth + 1
  readonly property int cells: Math.max(1, Math.floor((g.vpx(width) + 1) / pitch))
  readonly property int lit: Math.floor(Math.max(0, Math.min(1, value)) * cells + 1e-6)
  readonly property int red: Math.floor(Math.max(0, Math.min(1, redline)) * cells + 1e-6)
  readonly property real trackX: g.floor((width - g.px(cells * pitch - 1)) / 2)

  implicitHeight: g.px(cellHeight)
  height: implicitHeight

  Repeater {
    model: root.cells
    Rectangle {
      required property int index
      x: root.trackX + index * root.pitch * root.g.unit
      width: root.cellWidth * root.g.unit
      height: root.height
      antialiasing: false
      color: index >= root.lit ? root.unlit : (index >= root.red ? root.over : root.fill)
    }
  }
}
