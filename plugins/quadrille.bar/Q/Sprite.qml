import QtQuick
import "."

// A pixel sprite: rows of '#', digits and '.', drawn as whole-vpx rectangles
// (see Sprites.qml for the alphabet). No smoothing, no scaling: every pixel is
// one virtual pixel of the surface it is on, `g.unit` logical pixels square.
//
// One pixel size to a surface. `unit` is still here for the callers that were
// written before that rule, but a value other than the grid's makes a mixel (a
// 7 x 7 sprite at 2 vpx a pixel, beside text and icons at 1) and is reported on
// the log, once per size. A bigger icon is a sprite of its own, drawn at its own
// size (11, 15, 21: see "Sizes" in Sprites.qml), not the small one scaled.
Item {
  id: root

  readonly property var g: Px.of(root)

  property var rows: []
  property int level: 9
  property color color: Role.ink
  // What an unlit digit shows; transparent hides it.
  property color dim: Role.faint
  property color accent: Role.alarm
  // The mid tone ('o'): the shading step between ink and faint.
  property color mid: Role.muted
  property real unit: g.unit
  // Drawn at some multiple of the surface's pixel: a mixel.
  readonly property bool scaled: Math.abs(unit - g.unit) > 1e-6
  function reportScaled() {
    if (scaled && Sprites.noteScaled(rows, unit / g.unit))
      console.warn("quadrille: a " + Sprites.width(rows) + " x " + Sprites.height(rows)
        + " sprite is drawn at " + (unit / g.unit).toFixed(2) + "x the surface's pixel (a mixel); "
        + "draw it as a native " + Math.round(Sprites.width(rows) * unit / g.unit) + "-pixel sprite instead")
  }
  // Looked at once the bindings have settled, not on the way (a unit and a grid that
  // are resolved a moment apart are not a mixel).
  onScaledChanged: Qt.callLater(reportScaled)
  Component.onCompleted: Qt.callLater(reportScaled)

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
      color: modelData.kind === 0 ? root.color : (modelData.kind === 2 ? root.accent : (modelData.kind === 3 ? root.mid : root.dim))
    }
  }
}
