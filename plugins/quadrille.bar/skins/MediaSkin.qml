import QtQuick
import "../Q"
import "../Q/Glyphs.js" as Glyphs

// omarchy.media: a play or pause mark and "title · artist" in the body face, cut
// with an ellipsis where the bar has no more room. (The stock widget scrolls it
// in a clip; nothing here moves. The full text is the tooltip.)
Skin {
  id: root
  elastic: true

  readonly property var player: prop("activePlayer", null)
  readonly property bool hasMedia: prop("hasMedia", false) === true
  readonly property bool playing: player ? player.isPlaying === true : false
  readonly property string title: String(prop("title", ""))
  readonly property string artist: String(prop("artist", ""))
  readonly property string line: title + (artist !== "" ? " · " + artist : "")
  readonly property int cols: Math.max(0, Math.min(Glyphs.length(line), 28, Math.floor((room - (7 + 2 + 2 * pad) * g.unit) / g.cellW)))

  visible: hasMedia
  implicitWidth: hasMedia ? (7 + (cols > 0 ? 2 + cols * 6 : 0) + 2 * pad) * g.unit : 0

  Sprite {
    id: mark
    x: root.pad * root.g.unit; y: root.g.centre(root.height, height)
    rows: root.playing ? Sprites.pause : Sprites.play
    color: root.playing ? Role.ink : Role.muted
  }
  PixelText {
    visible: root.cols > 0
    x: mark.x + mark.width + root.g.px(2); y: root.g.centre(root.height, height)
    text: root.line
    columns: root.cols
    ink: root.playing ? Role.ink : Role.muted
  }
}
