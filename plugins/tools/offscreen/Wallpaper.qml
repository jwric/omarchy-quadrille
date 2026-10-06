import QtQuick
import Quickshell
import qs.Commons
import "Q"

Item {
  id: root
  property var outputs: [
    { name: "eDP-2", make: "BOE", model: "0x0A7C", width: 2560, height: 1600,
      physicalWidth: 340, physicalHeight: 220, x: 0, y: 0, scale: 1.666667, refreshRate: 60 },
    { name: "HDMI-A-1", make: "Dell Inc.", model: "DELL U3417W", width: 3440, height: 1440,
      physicalWidth: 800, physicalHeight: 330, x: -952, y: -1440, scale: 1, refreshRate: 59.973 }]
  property int current: parseInt(Quickshell.env("H_CURRENT") || "0", 10)
  property var overrides: ({})
  Stage {
    implicitWidth: Math.round(root.outputs[root.current].width / root.outputs[root.current].scale)
    implicitHeight: Math.round(root.outputs[root.current].height / root.outputs[root.current].scale)
    visible: true
    Graticule {
      anchors.fill: parent
      monitor: root.outputs[root.current]
      monitors: root.outputs
      overrides: root.overrides
      dpr: root.outputs[root.current].scale
      phys: Math.max(1, Math.round(2 * dpr))
    }
  }
}
