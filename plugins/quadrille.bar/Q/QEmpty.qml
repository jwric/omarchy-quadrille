import QtQuick
import "."

// What a list says when it is empty: an icon and a muted line, on a row's own
// height, so the panel does not collapse and nothing jumps when the first item
// arrives.
Item {
  id: root

  readonly property var g: Px.of(root)

  property var icon: null
  property string text: ""

  implicitHeight: g.px(14)
  height: implicitHeight

  Sprite {
    visible: root.icon !== null
    x: root.g.px(3)
    y: root.g.hair + root.g.onCaps(7)
    rows: root.icon || []
    color: Role.faint
  }
  PixelText {
    x: root.g.px(root.icon ? 13 : 3)
    y: root.g.hair
    text: root.text
    ink: Role.muted
    columns: root.g.columns(root.width - x)
  }
}
