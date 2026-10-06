import QtQuick
import "."
import "Glyphs.js" as Glyphs
import "GlyphsBig.js" as Big

// The body face at twice or three times its size (quadrille's DISPLAY and HERO
// readouts: a hero number), drawn at the surface's own pixel. Not the small
// face with every pixel repeated: that is a mixel, a pixel of another size
// beside everything else. GlyphsBig.js expands the glyph with Scale2x / Scale3x,
// which keeps the stems and gives every diagonal its own pixels, and this draws it
// one pixel to one pixel: cells are `6 * scale` wide and the line `12 * scale`
// tall, so layout is still arithmetic, and every edge is a whole device pixel
// for the same reason PixelText's are.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property color ink: Role.ink
  // 2 or 3 (other values repeat pixels, which is the mixel this avoids).
  property int scale: 2

  readonly property int cells: Glyphs.length(text)
  readonly property real cellWidth: g.cellW * scale

  width: cells * cellWidth
  height: g.line * scale

  Repeater {
    model: Big.rects(root.text, root.scale)
    Rectangle {
      required property var modelData
      x: modelData.x * root.g.unit
      y: modelData.y * root.g.unit
      width: modelData.w * root.g.unit
      height: modelData.h * root.g.unit
      color: root.ink
      antialiasing: false
    }
  }
}
