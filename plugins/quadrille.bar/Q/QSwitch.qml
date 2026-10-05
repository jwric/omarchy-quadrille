import QtQuick
import "."

// An on/off switch: a hairline box, 13 x 7 vpx inside it, and a square knob that
// sits left when off (muted on raised glass) and right when on (on_accent on the
// accent). No tween: it is where it is. The box is what a hand finds, so it is
// a box; keyboard focus is corner brackets drawn two virtual pixels outside it.
//
// The caller owns the value: bind `checked` to real state and flip it in
// response to `toggled()`. `busy` swallows clicks without changing how it looks.
Item {
  id: root

  readonly property var g: Px.of(root)

  property bool checked: false
  property bool busy: false
  property bool interactive: true
  property bool hasCursor: false

  signal toggled()
  signal hovered(bool isHovered)

  readonly property alias containsMouse: area.containsMouse
  readonly property bool hot: hasCursor || area.containsMouse

  // The box is 15 x 9; two virtual pixels of air each side for the brackets.
  implicitWidth: g.px(19)
  implicitHeight: g.px(13)
  width: implicitWidth
  height: implicitHeight

  Rectangle {
    x: root.g.px(2); y: root.g.px(2)
    width: root.g.px(15); height: root.g.px(9)
    antialiasing: false
    color: root.hot ? Role.muted : Role.edge

    Rectangle {
      anchors.fill: parent
      anchors.margins: root.g.hair
      antialiasing: false
      color: root.checked ? Role.accent : Role.raised
    }
    Rectangle {
      x: root.g.px(root.checked ? 8 : 2)
      y: root.g.px(2)
      width: root.g.px(5); height: root.g.px(5)
      antialiasing: false
      color: root.checked ? Role.onAccent : Role.muted
    }
  }

  Loader {
    anchors.fill: parent
    active: root.hasCursor
    sourceComponent: Brackets { arm: 3 }
  }

  MouseArea {
    id: area
    anchors.fill: parent
    enabled: root.interactive
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onContainsMouseChanged: root.hovered(containsMouse)
    onClicked: if (!root.busy) root.toggled()
  }
}
