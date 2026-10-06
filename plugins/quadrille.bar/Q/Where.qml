pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Where an overlay should open. A window with no `screen` of its own is made on
// the compositor's choice of output (the focused one) and only learns which it
// is after it has been shown; until then its `screen` is the first output, and a
// grid resolved from that is the wrong one for a frame (a menu opening on the
// ultrawide with the laptop's 1.8-pixel unit, then jumping to 2). Giving the
// window the focused output as its `screen` before it is shown makes the grid
// right from the first frame.
QtObject {
  // The Quickshell screen of the monitor Hyprland has focused, or null (the
  // window then takes the default, as it does without this).
  function focusedScreen() {
    var monitor = Hyprland.focusedMonitor
    var name = monitor ? monitor.name : ""
    if (name === "") return null
    var all = Quickshell.screens
    for (var i = 0; i < all.length; i++) if (all[i].name === name) return all[i]
    return null
  }
}
