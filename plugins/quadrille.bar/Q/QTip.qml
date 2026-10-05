import QtQuick
import "."
import "Glyphs.js" as Glyphs

// A tooltip: the legend in the body face on a raised hairline box, a virtual
// pixel or two of air about it. It is where it is the moment `shown` is true.
// Put it inside the item it describes; by default it hangs centred beneath.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property bool shown: false

  visible: shown && text.length > 0
  z: 1000
  width: Glyphs.length(text) * g.cellW + g.px(2 * 3 + 2)
  height: g.line + g.px(2)
  x: g.centre(parent ? parent.width : 0, width)
  y: (parent ? parent.height : 0) + g.px(2)

  Rectangle {
    anchors.fill: parent
    antialiasing: false
    color: Role.edge
    Rectangle {
      anchors.fill: parent
      anchors.margins: root.g.hair
      antialiasing: false
      color: Role.raised
    }
  }
  PixelText {
    x: root.g.px(4)
    y: root.g.px(1)
    text: root.text
    ink: Role.ink
  }
}
