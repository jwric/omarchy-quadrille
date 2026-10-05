import QtQuick
import "."

// A button: a legend in a hairline, a line tall (the same box as a Tab). The one
// that is chosen (a profile, a scale, a mode) is the accent behind on_accent;
// under the pointer it steps up to raised; keyboard focus is corner brackets.
// An icon, when there is one, is a 7 x 7 sprite before the legend.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property var icon: null
  property int iconLevel: 9
  property bool active: false
  property bool hasCursor: false
  property bool danger: false
  // Air either side of the contents, in vpx.
  property int air: 3
  // Stretch to this width (logical px) instead of fitting the legend.
  property real fixedWidth: -1
  // A button that cannot be pressed reads faint.
  property bool enabledState: true

  signal clicked()
  signal hovered(bool isHovered)

  readonly property bool hot: area.containsMouse && enabledState
  readonly property int cells: text.length
  readonly property real natural: ((icon ? 7 + (cells > 0 ? 3 : 0) : 0) + cells * 6 + 2 * air + 2) * g.unit

  implicitWidth: fixedWidth > 0 ? fixedWidth : natural
  implicitHeight: g.line
  width: implicitWidth
  height: implicitHeight

  readonly property color ink: !enabledState ? Role.faint
    : (active ? Role.onAccent : (danger && hot ? Role.alarm : (hot || hasCursor ? Role.ink : Role.muted)))

  Rectangle {
    anchors.fill: parent
    antialiasing: false
    color: root.active ? Role.accent : (root.danger && root.hot ? Role.alarm : Role.edge)
  }
  Rectangle {
    anchors.fill: parent
    anchors.margins: root.g.hair
    antialiasing: false
    color: root.active ? Role.accent : (area.pressed && root.enabledState ? Role.hover : (root.hot ? Role.raised : Role.ground))
  }

  Row {
    id: body
    x: root.g.floor((root.width - width) / 2)
    y: 0
    spacing: root.g.px(3)
    Sprite {
      visible: root.icon !== null
      rows: root.icon || []
      level: root.iconLevel
      color: root.ink
      dim: Role.faint
      y: root.g.onCaps(7)
    }
    PixelText {
      visible: root.text.length > 0
      text: root.text
      ink: root.ink
    }
  }

  Loader {
    anchors.fill: parent
    active: root.hasCursor
    sourceComponent: Brackets { arm: 3; color: root.active ? Role.ink : Role.accent }
  }

  MouseArea {
    id: area
    anchors.fill: parent
    enabled: root.enabledState
    hoverEnabled: true
    cursorShape: root.enabledState ? Qt.PointingHandCursor : Qt.ArrowCursor
    onContainsMouseChanged: root.hovered(containsMouse)
    onClicked: root.clicked()
  }
}
