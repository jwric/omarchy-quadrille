import QtQuick
import "../Q"

// omarchy.monitor: brightness, a sun.
Skin {
  id: root
  implicitWidth: (7 + 2 * pad) * g.unit

  Sprite {
    x: root.pad * g.unit
    y: g.centre(root.height, height)
    rows: Sprites.brightness
    color: root.hot || root.open ? Role.ink : Role.muted
  }
}
