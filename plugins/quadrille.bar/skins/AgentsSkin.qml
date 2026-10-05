import QtQuick
import "../Q"

// omarchy.agents: a robot, in the alarm colour when a limit is nearly used up,
// and a four-cell gauge of how much of the binding limit is gone, when the panel
// knows one.
Skin {
  id: root
  readonly property bool alarming: prop("alarming", false) === true
  readonly property var headline: prop("headline", null)
  readonly property real percent: headline && headline.percent !== undefined ? Math.max(0, Math.min(1, Number(headline.percent))) : -1
  readonly property bool gauged: percent >= 0

  implicitWidth: (7 + (gauged ? 2 + 11 : 0) + 2 * pad) * g.unit

  Sprite {
    id: icon
    x: root.pad * root.g.unit; y: root.g.centre(root.height, height)
    rows: Sprites.robot
    color: root.alarming ? Role.alarm : (root.hot || root.open ? Role.ink : Role.muted)
  }
  BarGauge {
    visible: root.gauged
    x: icon.x + icon.width + root.g.px(2); y: root.g.centre(root.height, height)
    cells: 4; cellWidth: 2; cellHeight: 5
    value: Math.max(root.percent, 0)
    redline: 0.9
    fill: Role.ink
  }
}
