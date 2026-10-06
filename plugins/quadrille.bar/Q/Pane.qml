import QtQuick
import "."

// A card in a hairline: the edge colour, the ground inset by one vpx. Content
// goes inside `inner`, `pad` vpx in from the hairline. (Quadrille prefers zones
// to cards; an overlay is the one place a card is right, because it floats.)
Item {
  id: root

  readonly property var g: Px.of(root)

  property int pad: 6
  property color fill: Role.ground
  property color rule: Role.edge
  // The rule's colour is the alarm when a card asks for something that cannot be
  // undone.
  default property alias content: inner.data
  property alias inner: inner
  readonly property real inset: g.px(pad) + g.hair

  Rectangle {
    anchors.fill: parent
    color: root.rule
    antialiasing: false
  }
  Rectangle {
    anchors.fill: parent
    anchors.margins: root.g.hair
    color: root.fill
    antialiasing: false
  }
  Item {
    id: inner
    x: root.inset
    y: root.inset
    width: root.width - 2 * root.inset
    height: root.height - 2 * root.inset
  }
}
