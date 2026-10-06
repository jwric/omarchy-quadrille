import QtQuick
import Quickshell

// Debug aid: says, with a millisecond clock, every change of a surface's size,
// visibility and device pixel ratio, and what the kit's unit is at that moment,
// so the first frames of a popup, a tooltip, a menu, an OSD or a toast can be
// read off a log instead of watched. Silent unless the shell was started with
// QUADRILLE_DEBUG_SURFACES=1 (a scratch shell: `plugins/tools/surfaces.sh`).
//
//   SurfaceProbe { window: root; tag: "popup.audio"; extra: function() { return card.width } }
//
// Lines look like `SURF 1791251162123 popup.audio w=204 | dpr=2 unit=2 ...`
// and are read with `grep SURF` on the shell's log.
QtObject {
  id: root

  property var window: null
  property string tag: ""
  // Anything else worth seeing (the card's size, the model's length): a function
  // returning a string.
  property var extra: null
  readonly property bool enabled: Quickshell.env("QUADRILLE_DEBUG_SURFACES") === "1"

  function say(what) {
    if (!enabled || !window) return
    var g = Px.forWindow(window)
    var more = ""
    try { more = extra ? " " + String(extra()) : "" } catch (e) { more = " (extra: " + e + ")" }
    console.log("SURF " + Date.now() + " " + tag + " " + what
      + " | size=" + window.width + "x" + window.height
      + " vis=" + window.visible
      + " screen=" + (window.screen ? window.screen.name : "none")
      + " dpr=" + window.devicePixelRatio + " unit=" + g.unit.toFixed(4) + more)
  }

  property var watch: Connections {
    target: root.enabled ? root.window : null
    ignoreUnknownSignals: true
    function onWidthChanged() { root.say("width") }
    function onHeightChanged() { root.say("height") }
    function onVisibleChanged() { root.say("visible") }
    function onDevicePixelRatioChanged() { root.say("dpr") }
    function onScreenChanged() { root.say("screen") }
  }

  // What the window holds can change with no window event (a card growing when its
  // data arrives): while it is up, look every 4 ms and say when the signature moves.
  property string last: ""
  property var poll: Timer {
    interval: 4
    repeat: true
    running: root.enabled && root.window !== null && root.window.visible
    onTriggered: {
      var now = ""
      try { now = extra ? String(extra()) : "" } catch (e) { now = "?" }
      if (now !== root.last) { root.last = now; root.say("content") }
    }
  }

  Component.onCompleted: say("created")
}
