import QtQuick
import "."
import "Glyphs.js" as Glyphs

// The head of a popup: an icon at twice its size (a 7 x 7 sprite at 2 vpx a
// pixel), the title in ink over a muted line of capitals, and, at the right, a
// control or a reading put inside it. Two lines of text tall.
Item {
  id: root

  readonly property var g: Px.of(root)

  property var icon: null
  property int iconLevel: 9
  property int iconScale: 2
  property color iconInk: Role.ink
  property string title: ""
  property string subtitle: ""
  default property alias trailing: trail.data

  readonly property real iconSpace: icon ? g.px(7 * iconScale + 4) : 0
  readonly property real trailSpace: trail.children.length > 0 ? trail.childrenRect.width + g.px(3) : 0
  readonly property int columns: g.columns(width - iconSpace - trailSpace)

  implicitHeight: 2 * g.line
  height: implicitHeight

  Sprite {
    visible: root.icon !== null
    unit: root.g.unit * root.iconScale
    rows: root.icon || []
    level: root.iconLevel
    color: root.iconInk
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
