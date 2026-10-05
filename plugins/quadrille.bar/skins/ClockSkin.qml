import QtQuick
import "../Q"

// omarchy.clock: the stock label, upper-cased and set in ink. The stock widget
// still owns the format (left of right-click cycles it) and the calendar.
Skin {
  id: root
  readonly property string label: String(prop("displayText", "")).toUpperCase()

  implicitWidth: label.length * g.cellW + 2 * pad * g.unit

  PixelText {
    x: root.pad * g.unit
    y: g.centre(root.height, height)
    text: root.label
    ink: Role.ink
  }
}
