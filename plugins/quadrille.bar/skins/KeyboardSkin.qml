import QtQuick
import "../Q"

// omarchy.keyboard-layout: the layout's short code (EN), upper case, in ink,
// drawn from the bitmap face. The stock widget still owns the switching.
Skin {
  id: root
  readonly property string label: String(prop("layoutLabel", "")).toUpperCase()

  implicitWidth: label.length * g.cellW + 2 * pad * g.unit

  PixelText {
    x: root.pad * root.g.unit
    y: root.g.centre(root.height, height)
    text: root.label
    ink: Role.ink
  }
}
