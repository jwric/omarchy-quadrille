import QtQuick
import "../Q"
import "../Q/Glyphs.js" as Glyphs

// omarchy.active-window: the focused window's title in the body face, cut with an
// ellipsis to `maxWidth` (the widget's setting, 280 by default) or to the room the
// bar has, whichever is less. The stock widget animates its width; this one does not.
Skin {
  id: root
  elastic: true

  readonly property string title: String(prop("title", ""))
  readonly property real limit: Math.min(Number(prop("maxLabelWidth", 280)), room - 2 * pad * g.unit)
  readonly property int cols: Math.max(0, Math.min(Glyphs.length(title), Math.floor(limit / g.cellW)))

  visible: title !== ""
  implicitWidth: title === "" || cols === 0 ? 0 : cols * g.cellW + 2 * pad * g.unit

  PixelText {
    x: root.pad * root.g.unit; y: root.g.centre(root.height, height)
    text: root.title
    columns: root.cols
    ink: Role.muted
  }
}
