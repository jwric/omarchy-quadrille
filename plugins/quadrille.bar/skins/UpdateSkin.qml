import QtQuick
import "../Q"

// omarchy.system-update: an arrow into a tray when an update is waiting. (The
// stock widget hides itself otherwise, and so does this.)
Skin {
  id: root
  readonly property bool available: prop("updateAvailable", false) === true
  visible: available
  implicitWidth: available ? (7 + 2 * pad) * g.unit : 0

  Sprite {
    x: root.pad * root.g.unit; y: root.g.centre(root.height, height)
    rows: Sprites.update
    color: root.hot ? Role.ink : Role.caution
  }
}
