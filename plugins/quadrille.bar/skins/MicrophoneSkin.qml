import QtQuick
import "../Q"

// omarchy.microphone: a microphone, crossed when muted, in the alarm colour while
// something is listening (the stock widget's `inUse`).
Skin {
  id: root
  readonly property bool muted: prop("muted", true) === true
  readonly property bool inUse: prop("inUse", false) === true
  implicitWidth: (7 + 2 * pad) * g.unit

  Sprite {
    x: root.pad * root.g.unit; y: root.g.centre(root.height, height)
    rows: root.muted ? Sprites.microphoneMuted : Sprites.microphone
    color: root.inUse ? Role.alarm : (root.muted ? Role.faint : (root.hot ? Role.ink : Role.muted))
  }
}
