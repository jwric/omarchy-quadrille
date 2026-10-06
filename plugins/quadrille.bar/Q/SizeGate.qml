import QtQuick

// A popup's card is sized from content that can arrive after the popup is open (a
// list read from a process, a menu fetched over D-Bus): drawn at once it opens at
// one size and then jumps to another a frame or two later. The gate holds the card
// back until what decides its size and place (`signature`) has stayed the same for
// `stable` polls of 16 ms, or `limit` ms have passed since it opened, whichever is
// first. A card whose content is static is ready as soon as its window is (the
// polls run while the window is still being made); one that is waiting for data is
// held for that long, never longer.
//
//   SizeGate { id: gate; active: root.open; signature: card.width + "," + card.height }
//   Rectangle { id: card; visible: gate.ready }
QtObject {
  id: root

  property bool active: false
  property var signature: ""
  property int stable: 3
  property int limit: 260
  readonly property bool ready: done

  property bool done: false
  property int held: 0
  property string last: ""
  property real since: 0

  onActiveChanged: {
    done = false
    held = 0
    since = Date.now()
    last = String(signature)
  }

  property var poll: Timer {
    interval: 16
    repeat: true
    running: root.active && !root.done
    onTriggered: {
      var now = String(root.signature)
      if (now === root.last) root.held++
      else { root.held = 0; root.last = now }
      if (root.held >= root.stable || Date.now() - root.since >= root.limit) root.done = true
    }
  }
}
