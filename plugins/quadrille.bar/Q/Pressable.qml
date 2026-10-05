import QtQuick
import "."

// What a hand finds: a hit area that reports `hovered` and `pressed`, and
// draws corner brackets while it has the keyboard. Put a visual inside it and
// feed it those two. No tween: whatever it shows changes on the event.
//
//     Pressable { id: p; Tab { text: "1"; hot: p.hovered; down: p.pressed } }
Item {
  id: root

  property bool focused: false
  property bool hit: true
  readonly property alias hovered: area.containsMouse
  readonly property alias pressed: area.pressed
  property int acceptedButtons: Qt.LeftButton
  default property alias content: holder.data

  signal clicked(int button)
  signal wheel(int delta)

  implicitWidth: holder.childrenRect.width
  implicitHeight: holder.childrenRect.height

  Item { id: holder; anchors.fill: parent }
  Loader {
    anchors.fill: parent
    active: root.focused
    sourceComponent: Brackets { }
  }
  MouseArea {
    id: area
    anchors.fill: parent
    enabled: root.hit
    hoverEnabled: true
    acceptedButtons: root.acceptedButtons
    cursorShape: Qt.PointingHandCursor
    onClicked: function(mouse) { root.clicked(mouse.button) }
    onWheel: function(wheel) { root.wheel(wheel.angleDelta.y) }
  }
}
