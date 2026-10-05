import QtQuick
import "../Q"

// omarchy.clock: the stock label, upper-cased and set in ink. The stock widget
// still owns the format (left of right-click cycles it) and the calendar.
Skin {
  id: root
  readonly property string label: String(prop("displayText", "")).toUpperCase()

  implicitWidth: label.length * Px.cellW + 2 * pad * Px.unit

  PixelText {
    x: root.pad * Px.unit
    y: Px.centre(root.height, height)
    text: root.label
    ink: Role.ink
  }
}
