import QtQuick
import Quickshell
import qs.Commons
import "Q"

Item {
  id: root
  property var outputs: Quickshell.env("H_OUTPUTS") ? JSON.parse(Quickshell.env("H_OUTPUTS")) : [
    { name: "eDP-2", make: "AU Optronics", model: "0x07B2", width: 2560, height: 1600,
      physicalWidth: 340, physicalHeight: 220, x: 0, y: 0, scale: 1.666667, refreshRate: 60 },
    { name: "HDMI-A-1", make: "Dell Inc.", model: "DELL U3417W", width: 3440, height: 1440,
      physicalWidth: 800, physicalHeight: 330, x: -952, y: -1440, scale: 1, refreshRate: 59.973 }]
  property int current: parseInt(Quickshell.env("H_CURRENT") || "0", 10)
  property string composition: Quickshell.env("H_COMPOSITION") || "aperture"
  property string starTone: Quickshell.env("H_STAR_TONE") || "faint"
  property var overrides: ({})
  // A window is whole logical pixels, and a grab of one is stretched to the next whole device
  // pixel: 2560 / 1.5 = 1706.67 would be grabbed 2561 wide with a column repeated half way. So
  // the window grows to the first logical size whose device size is whole (1708 -> 2562); the
  // output's own pixels stay at the left, and wallpaper.py checks only those.
  function whole(pixels) {
    var scale = root.outputs[root.current].scale, n = Math.round(pixels / scale)
    for (var k = 0; k < 8 && Math.abs(n * scale - Math.round(n * scale)) > 0.001; k++) n++
    return n
  }
  Stage {
    implicitWidth: root.whole(root.outputs[root.current].transform % 2 ? root.outputs[root.current].height : root.outputs[root.current].width)
    implicitHeight: root.whole(root.outputs[root.current].transform % 2 ? root.outputs[root.current].width : root.outputs[root.current].height)
    visible: true
    Graticule {
      anchors.fill: parent
      monitor: root.outputs[root.current]
      monitors: root.outputs
      overrides: root.overrides
      composition: root.composition
      starTone: root.starTone
      dpr: root.outputs[root.current].scale
      phys: Math.max(1, Math.round(2 * dpr))
    }
  }
}
