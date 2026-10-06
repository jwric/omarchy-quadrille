import QtQuick
import "."
import "Glyphs.js" as Glyphs

// One line of input on the pixel grid: what has been typed in ink with a steady
// block caret after it, or the prompt in muted when nothing has. Text longer than
// the line shows its tail, so the caret stays in view. A line (12 vpx) tall.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property string prompt: ""
  // The caret is steady, never blinking: nothing here animates.
  property bool caret: true
  property color ink: Role.ink
  property color promptInk: Role.muted

  readonly property int columns: Math.max(1, Math.floor(width / g.cellW + 1e-6))
  readonly property var chars: Array.from(text)
  readonly property string shown: chars.length > columns - 1 ? chars.slice(chars.length - (columns - 1)).join("") : text

  height: g.line
  clip: true

  PixelText {
    visible: root.text.length > 0
    text: root.shown
    ink: root.ink
  }
  PixelText {
    visible: root.text.length === 0
    text: root.prompt
    columns: root.columns
    ink: root.promptInk
  }
  // the caret: a cell wide and as tall as the capitals, on the accent
  Rectangle {
    visible: root.caret && root.text.length > 0
    x: Math.min(Array.from(root.shown).length, root.columns - 1) * root.g.cellW
    y: root.g.capTop
    width: root.g.cellW - root.g.unit
    height: root.g.cap
    color: Role.accent
    antialiasing: false
  }
}
