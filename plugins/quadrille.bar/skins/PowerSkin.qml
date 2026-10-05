import QtQuick
import "../Q"

// omarchy.power: a battery of four charge cells and the percentage. It lights
// in the live colour while charging, the alarm colour under 15%, ink between.
Skin {
  id: root

  readonly property bool present: prop("batteryPresent", false) === true
  readonly property real fraction: Math.max(0, Math.min(1, prop("batteryFraction", 0)))
  readonly property bool charging: prop("charging", false) === true
  readonly property int level: Math.max(fraction > 0 ? 1 : 0, Math.floor(fraction * 4 + 0.5))
  readonly property string percent: Math.round(fraction * 100) + "%"
  readonly property color tone: charging ? Role.live : (fraction < 0.15 ? Role.alarm : Role.ink)

  visible: present
  implicitWidth: present ? (7 + 2 + percent.length * 6 + 2 * pad) * Px.unit : 0

  Sprite {
    id: icon
    x: root.pad * Px.unit
    y: Px.centre(root.height, height)
    rows: Sprites.battery
    level: root.level
    color: root.tone
    dim: Role.faint
  }
  PixelText {
    x: icon.x + icon.width + Px.px(2)
    y: Px.centre(root.height, height)
    text: root.percent
    ink: root.tone
  }
}
