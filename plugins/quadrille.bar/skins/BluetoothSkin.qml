import QtQuick
import "../Q"

// omarchy.bluetooth: the rune, in ink while a device is connected, muted when
// the radio is on and idle, faint when it is off, and gone with no adapter.
Skin {
  id: root

  readonly property var adapter: prop("adapter", null)
  readonly property bool powered: !!adapter && adapter.enabled === true
  readonly property int connected: (prop("connectedDevices", []) || []).length

  visible: !!adapter
  implicitWidth: adapter ? (7 + 2 * pad) * g.unit : 0

  Sprite {
    x: root.pad * g.unit
    y: g.centre(root.height, height)
    rows: Sprites.bluetooth
    color: !root.powered ? Role.faint : (root.connected > 0 ? Role.live : Role.muted)
  }
}
