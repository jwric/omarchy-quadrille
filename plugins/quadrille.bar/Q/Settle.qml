import QtQuick

// A window that has just been shown is drawn, for its first frame or two, at the
// device ratio of the default output (an integer: 2 on a 1.666667 one) until the
// compositor says what its own is; what is drawn then is laid out on the right
// grid (Px resolves it from the output) but rasterised at the wrong ratio, and
// the next frame replaces it. Content that is `visible: settle.ready` skips that
// frame instead of flashing it. If the compositor never settles (or Hyprland is
// not there to say what the scale should be) it shows anyway after 120 ms.
//
//   Settle { id: settle; window: root }
//   Rectangle { visible: settle.ready }
QtObject {
  id: root

  property var window: null
  readonly property bool shown: window ? window.visible : false
  property bool late: false
  readonly property bool ready: late || Px.settled(window)

  property var timer: Timer {
    interval: 120
    running: root.shown && !root.ready
    onTriggered: root.late = true
  }
  onShownChanged: if (!shown) late = false
}
