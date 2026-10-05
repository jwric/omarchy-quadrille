import QtQuick
import "."

// Wrapped text in the body face: `columns` cells wide, whole 12-vpx lines, at
// most `maxLines` of them, the last ending in an ellipsis when cut. As
// PixelText, it is exact: the width is columns * Px.cellW and the height
// lineCount * Px.line, so what it sits beside can be laid out by arithmetic.
Item {
  id: root

  property string text: ""
  property color ink: Role.ink
  property int columns: 30
  property int maxLines: 3
  readonly property int lines: text.length > 0 ? label.lineCount : 0

  width: columns * Px.cellW
  height: lines * Px.line
  visible: text.length > 0

  Text {
    id: label
    y: -Px.unit
    width: parent.width
    text: root.text
    color: root.ink
    font.family: Px.face
    font.pixelSize: Px.size
    font.hintingPreference: Font.PreferFullHinting
    font.kerning: false
    font.preferShaping: false
    renderType: Text.NativeRendering
    textFormat: Text.PlainText
    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
    elide: Text.ElideRight
    maximumLineCount: root.maxLines
    lineHeightMode: Text.FixedHeight
    lineHeight: Px.line
  }
}
