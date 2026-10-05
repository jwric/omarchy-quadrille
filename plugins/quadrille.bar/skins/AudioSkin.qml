import QtQuick
import "../Q"

// omarchy.audio: a speaker whose waves are the level, and a five-cell gauge.
// Muted, the waves are a cross in the alarm colour and the gauge is dark.
Skin {
  id: root

  readonly property real volume: Math.max(0, Math.min(1, prop("outputVolume", 0)))
  readonly property bool muted: prop("outputMuted", false) === true
  readonly property bool present: prop("hasOutput", false) === true
  readonly property int level: muted ? 0 : (volume <= 0.005 ? 0 : (volume < 0.5 ? 1 : 2))

  implicitWidth: (7 + 3 + gauge.width / g.unit + 2 * pad) * g.unit

  Sprite {
    id: icon
    x: root.pad * g.unit
    y: g.centre(root.height, height)
    rows: root.muted ? Sprites.muted : Sprites.volume
    level: root.level
    color: root.present ? Role.ink : Role.faint
    dim: Role.faint
  }
  BarGauge {
    id: gauge
    x: icon.x + icon.width + g.px(3)
    y: g.centre(root.height, height)
    cells: 5
    value: root.muted || !root.present ? 0 : root.volume
    fill: Role.ink
    cellHeight: 5
  }
}
