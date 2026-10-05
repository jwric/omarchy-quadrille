import QtQuick
import Quickshell.Io
import qs.Ui
import "Q"

// quadrille.sysmon: CPU and memory as labelled stepped gauges, in the live
// colour (a gauge of live data) turning alarm past 80%. Samples /proc/stat and
// /proc/meminfo with a timer; nothing is executed and nothing is written.
BarWidget {
  id: root
  moduleName: "quadrille.sysmon"

  property real cpu: 0
  property real mem: 0
  property real lastTotal: 0
  property real lastIdle: 0
  readonly property int interval: Math.max(500, Number(setting("interval", 2000)))

  readonly property int cells: 5
  readonly property int pad: 3
  readonly property int groupWidth: 3 * Px.cellW + Px.px(2) + cpuGauge.width

  implicitWidth: groupWidth * 2 + Px.px(4) + 2 * pad * Px.unit
  implicitHeight: barSize

  function sampleCpu(raw) {
    var line = String(raw || "").split("\n")[0].trim().split(/\s+/)
    if (line.length < 5 || line[0] !== "cpu") return
    var total = 0
    for (var i = 1; i < line.length; i++) total += Number(line[i]) || 0
    var idle = (Number(line[4]) || 0) + (Number(line[5]) || 0)
    if (lastTotal > 0 && total > lastTotal) {
      var load = 1 - (idle - lastIdle) / (total - lastTotal)
      cpu = Math.max(0, Math.min(1, load))
    }
    lastTotal = total
    lastIdle = idle
  }

  function sampleMem(raw) {
    var text = String(raw || "")
    var total = text.match(/MemTotal:\s+(\d+)/)
    var avail = text.match(/MemAvailable:\s+(\d+)/)
    if (total && avail && Number(total[1]) > 0)
      mem = Math.max(0, Math.min(1, 1 - Number(avail[1]) / Number(total[1])))
  }

  FileView {
    id: stat
    path: "/proc/stat"
    printErrors: false
    onLoaded: root.sampleCpu(text())
  }
  FileView {
    id: meminfo
    path: "/proc/meminfo"
    printErrors: false
    onLoaded: root.sampleMem(text())
  }
  Timer {
    interval: root.interval
    running: root.visible
    repeat: true
    triggeredOnStart: true
    onTriggered: { stat.reload(); meminfo.reload() }
  }

  BarButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    tooltipText: "CPU " + Math.round(root.cpu * 100) + "%\nMEM " + Math.round(root.mem * 100) + "%"
    onPressed: function(b) { if (root.bar) root.bar.run("omarchy-launch-or-focus-tui btop") }

    Rectangle {
      x: 0; y: Px.px(2); width: parent.width; height: Px.px(12)
      antialiasing: false
      color: button.down ? Role.hover : (button.hovered ? Role.raised : "transparent")
    }
    Row {
      x: root.pad * Px.unit
      y: Px.centre(parent.height, height)
      spacing: Px.px(4)

      Row {
        spacing: Px.px(2)
        PixelText { text: "CPU"; ink: Role.muted }
        BarGauge {
          id: cpuGauge
          y: Px.centre(Px.line, height)
          cells: root.cells; cellHeight: 6; value: root.cpu; redline: 0.8; fill: Role.live
        }
      }
      Row {
        spacing: Px.px(2)
        PixelText { text: "MEM"; ink: Role.muted }
        BarGauge {
          y: Px.centre(Px.line, height)
          cells: root.cells; cellHeight: 6; value: root.mem; redline: 0.8; fill: Role.live
        }
      }
    }
  }
}
