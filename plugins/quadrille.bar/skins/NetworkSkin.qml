import QtQuick
import "../Q"

// omarchy.network: a wifi fan whose lit arcs are the signal, a wired socket,
// or a cross when there is no link.
Skin {
  id: root

  readonly property string kind: prop("kind", "")
  readonly property real strength: prop("signalStrength", -1)
  readonly property int level: kind !== "wifi" ? 0 : (strength < 25 ? 1 : (strength < 60 ? 2 : 3))

  implicitWidth: (7 + 2 * pad) * g.unit

  Sprite {
    x: root.pad * g.unit
    y: g.centre(root.height, height)
    rows: root.kind === "wifi" ? Sprites.wifi : (root.kind === "ethernet" ? Sprites.ethernet : Sprites.offline)
    level: root.level
    color: Role.ink
    dim: Role.faint
    accent: Role.alarm
  }
}
