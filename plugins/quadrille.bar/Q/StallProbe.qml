import QtQuick
import Quickshell

// Debug aid, silent unless the shell has QUADRILLE_DEBUG_SURFACES=1 (like
// SurfaceProbe): says when the GUI thread was not running. A timer of 4 ms that
// finds itself later than 40 ms logs the gap, so a popup that is slow to appear
// shows as one long stall or many short ones, with the clock of each:
//
//   STALL 1791304465123 popup.network gap=872
//
// `mark("what")` puts a line with the same clock beside them, for the code that
// is suspected (a model being filled, a scan being asked for). Read them with
// `grep STALL` on the shell's log, next to the SURF lines of the surface.
//
//   StallProbe { id: probe; tag: "network"; active: root.open }
QtObject {
  id: root

  property string tag: ""
  property bool active: false
  property real last: 0
  readonly property bool enabled: Quickshell.env("QUADRILLE_DEBUG_SURFACES") === "1"

  function mark(what) {
    if (enabled) console.log("STALL " + Date.now() + " " + tag + " " + what)
  }

  onActiveChanged: { last = 0; mark(active ? "active" : "idle") }

  property var tick: Timer {
    interval: 4
    repeat: true
    running: root.enabled && root.active
    onTriggered: {
      var now = Date.now()
      if (root.last > 0 && now - root.last > 40) console.log("STALL " + now + " " + root.tag + " gap=" + (now - root.last))
      root.last = now
    }
  }
}
