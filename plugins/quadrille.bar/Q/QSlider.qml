import QtQuick
import "."

// A stepped slider: a row of cells, lit up to the value, never past it, the way
// the bar's gauges are. Click or drag sets the value to the cell under the
// pointer, the wheel steps it, a right click is its own signal. There is no knob
// and nothing glides: the value is where the cells are.
//
// `discrete` makes it a picker: one cell for each of `maximum - minimum + 1`
// choices, the chosen one and every one before it lit.
Item {
  id: root

  readonly property var g: Px.of(root)

  property real value: 0
  property real minimum: 0
  property real maximum: 1
  property real step: 0.05
  property bool integer: false
  property bool discrete: false
  property bool dimmed: false
  property bool dragging: false
  property real liveValue: value
  // Cell width and height in vpx; the gap between cells is one.
  property int cellWidth: 3
  property int cellHeight: 7
  // Value from which cells are lit in the over colour (-1: never).
  property real redline: -1
  property color fill: Role.ink
  property color over: Role.alarm
  property color unlit: Role.raised

  signal moved(real value)
  signal released(real value)
  signal rightClicked()

  onValueChanged: if (!dragging) liveValue = value

  readonly property real range: Math.max(1e-6, maximum - minimum)
  readonly property real progress: Math.max(0, Math.min(1, (liveValue - minimum) / range))
  readonly property int span: g.vpx(width)
  readonly property int choices: Math.max(2, Math.round(maximum - minimum) + 1)
  readonly property int cells: discrete ? choices : Math.max(2, Math.floor((span + 1) / (cellWidth + 1)))
  readonly property int cellPx: discrete ? Math.max(1, Math.floor((span + 1) / cells) - 1) : cellWidth
  readonly property int pitch: cellPx + 1
  readonly property int trackSpan: cells * pitch - 1
  readonly property real trackX: g.floor((width - g.px(trackSpan)) / 2)
  readonly property int lit: discrete ? Math.round(liveValue - minimum) + 1 : Math.round(progress * cells)
  readonly property int red: redline < 0 ? cells : Math.round(Math.max(0, Math.min(1, (redline - minimum) / range)) * cells)

  implicitWidth: g.px(120)
  implicitHeight: g.px(cellHeight)
  height: implicitHeight

  Repeater {
    model: root.cells
    Rectangle {
      required property int index
      x: root.trackX + index * root.pitch * root.g.unit
      width: root.cellPx * root.g.unit
      height: root.height
      antialiasing: false
      color: index >= root.lit ? root.unlit
        : (root.dimmed ? Role.faint : (index >= root.red ? root.over : root.fill))
    }
  }

  MouseArea {
    id: area
    anchors.fill: parent
    enabled: root.enabled
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton

    function valueAt(x) {
      var f = Math.max(0, Math.min(1, (x - root.trackX) / Math.max(1, root.g.px(root.trackSpan))))
      if (root.discrete) {
        var k = Math.min(root.cells - 1, Math.floor((x - root.trackX) / (root.pitch * root.g.unit)))
        return root.minimum + Math.max(0, k)
      }
      var raw = root.minimum + Math.round(f * root.cells) / root.cells * root.range
      if (root.integer) raw = Math.round(raw)
      return Math.max(root.minimum, Math.min(root.maximum, raw))
    }

    onPressed: function(mouse) {
      if (mouse.button !== Qt.LeftButton) return
      root.dragging = true
      var next = valueAt(mouse.x)
      root.liveValue = next
      root.moved(next)
    }
    onClicked: function(mouse) { if (mouse.button === Qt.RightButton) root.rightClicked() }
    onPositionChanged: function(mouse) {
      if (!root.dragging) return
      var next = valueAt(mouse.x)
      if (next === root.liveValue) return
      root.liveValue = next
      root.moved(next)
    }
    onReleased: function(mouse) {
      if (mouse.button !== Qt.LeftButton) return
      root.dragging = false
      root.released(root.liveValue)
      root.liveValue = root.value
    }
    onWheel: function(wheel) {
      var delta = wheel.angleDelta.y > 0 ? root.step : -root.step
      var next = Math.max(root.minimum, Math.min(root.maximum, root.liveValue + delta))
      if (root.integer) next = Math.round(next)
      root.liveValue = next
      root.moved(next)
      root.released(next)
    }
  }
}
