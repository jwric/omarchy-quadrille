import QtQuick
import "Q"

// The Tailscale mark in the bar, a 7 x 7 sprite (Icons.qml): the lit dots in
// `color`, the faded ones faint, the needs-login badge in `badgeColor`; crossed
// (off) is the whole mark faded. Whole virtual pixels in every dimension, so
// whoever places it on the grid keeps it crisp. (The hero draws the 15 x 15
// drawing of the mark: one pixel size to a surface, never this one scaled.)
Sprite {
  id: root

  property color badgeColor: Role.alarm
  property bool crossed: false
  property bool warning: false

  rows: warning ? icons.markLogin : (crossed ? icons.markOff : icons.mark)
  level: 0
  dim: Role.faint
  accent: badgeColor

  Icons { id: icons }
}
