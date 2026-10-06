import QtQuick
import "."
import "Glyphs.js" as Glyphs

// The head of a popup: an icon drawn at its own native size (a 15 x 15 sprite for
// the two lines of text beside it: never a 7 x 7 scaled up, one pixel size to a
// surface), the title in ink over a muted line of capitals, and, at the right, a
// control or a reading put inside it. Two lines of text tall.
Item {
  id: root

  readonly property var g: Px.of(root)

  property var icon: null
  property int iconLevel: 9
  // Kept for old callers; leave it at 1 and hand over a sprite of the size wanted.
  property int iconScale: 1
  property color iconInk: Role.ink
  // The second tone of a sprite (its unlit digits) and its accent tone.
  property color iconDim: Role.faint
  property color iconAccent: Role.alarm
  property string title: ""
  property string subtitle: ""
  default property alias trailing: trail.data

  readonly property real iconSpace: icon ? g.px(Sprites.width(icon) * iconScale + 4) : 0
  readonly property real trailSpace: trail.children.length > 0 ? trail.childrenRect.width + g.px(3) : 0
  readonly property int columns: g.columns(width - iconSpace - trailSpace)

  implicitHeight: 2 * g.line
  height: implicitHeight

  Sprite {
    id: mark
    visible: root.icon !== null
    // The sprite's own pixel (its window's grid), times 1: asking QHero's grid
    // instead differs for a frame while a window is mapped, which reads as a mixel.
    unit: mark.g.unit * root.iconScale
    rows: root.icon || []
    level: root.iconLevel
    color: root.iconInk
    dim: root.iconDim
    accent: root.iconAccent
    y: root.g.centre(root.height, height)
  }

  Column {
    x: root.iconSpace
    PixelText {
      text: root.title
      ink: Role.ink
      columns: root.columns
    }
    PixelText {
      text: root.subtitle.toUpperCase()
      ink: Role.muted
      columns: root.columns
    }
  }

  Item {
    id: trail
    anchors.right: parent.right
    width: childrenRect.width
    height: parent.height
    // children place themselves; they are vertically centred by the caller
  }
}
