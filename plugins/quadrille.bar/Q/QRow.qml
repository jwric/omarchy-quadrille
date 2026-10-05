import QtQuick
import "."
import "Glyphs.js" as Glyphs

// A row of a list: a 7 x 7 icon, a name, a reading on the right, and a second
// line when there is something to say. The row in use (the output device, the
// network joined) is an inverse block, the accent behind on_accent; under the
// pointer it steps up to raised; keyboard focus is corner brackets. A name that
// does not fit in a line goes on to a second (`maxLines`), and only on the last
// does it end in an ellipsis.
//
// Whatever is put inside the row (small buttons: forget, share) is parked at its
// right edge, ahead of the reading, and gets the clicks before the row does.
//
// A row is a whole number of virtual pixels tall: a line of text and one vpx of
// air above and below.
Item {
  id: root

  readonly property var g: Px.of(root)

  property var icon: null
  property int iconLevel: 9
  property string text: ""
  property string sub: ""
  property string detail: ""
  property bool current: false
  property bool hasCursor: false
  // Unavailable or inactive: everything faint.
  property bool dimmed: false
  property int maxLines: 2
  // Ink of the reading when the row is not the current one.
  property color detailInk: Role.muted
  default property alias trailing: trail.data

  signal clicked(int button)
  signal hovered()

  readonly property bool hot: area.containsMouse
  readonly property color ink: current ? Role.onAccent : (dimmed ? Role.faint : Role.ink)
  readonly property color quiet: current ? Role.onAccent : (dimmed ? Role.faint : Role.muted)

  readonly property real pad: g.px(3)
  readonly property real iconSpace: icon ? g.px(7 + 3) : 0
  readonly property real detailSpace: detail.length > 0 ? (Glyphs.length(detail) + 1) * g.cellW : 0
  readonly property real trailSpace: trail.children.length > 0 ? trail.width + g.px(2) : 0
  readonly property real textWidth: Math.max(0, width - 2 * pad - iconSpace - detailSpace - trailSpace)
  readonly property int columns: g.columns(textWidth)
  readonly property var lines: Glyphs.wrap(text, Math.max(1, columns), maxLines)
  readonly property int subLines: sub.length > 0 ? 1 : 0

  implicitWidth: g.px(120)
  implicitHeight: (lines.length + subLines) * g.line + 2 * g.hair
  width: implicitWidth
  height: implicitHeight

  Rectangle {
    anchors.fill: parent
    antialiasing: false
    color: root.current ? Role.accent : (root.hot ? Role.raised : "transparent")
  }

  MouseArea {
    id: area
    anchors.fill: parent
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    cursorShape: Qt.PointingHandCursor
    onEntered: root.hovered()
    onClicked: function(mouse) { root.clicked(mouse.button) }
  }

  Sprite {
    visible: root.icon !== null
    x: root.pad
    y: root.g.hair + root.g.onCaps(7)
    rows: root.icon || []
    level: root.iconLevel
    color: root.ink
    dim: root.current ? Role.onAccent : Role.faint
    accent: root.current ? Role.onAccent : Role.alarm
  }

  Column {
    x: root.pad + root.iconSpace
    y: root.g.hair
    width: root.textWidth
    Repeater {
      model: root.lines
      PixelText {
        required property string modelData
        text: modelData
        ink: root.ink
      }
    }
    PixelText {
      visible: root.subLines > 0
      text: root.sub
      ink: root.quiet
      columns: root.columns
    }
  }

  PixelText {
    visible: root.detail.length > 0
    x: root.width - root.pad - width - root.trailSpace
    y: root.g.hair
    text: root.detail
    ink: root.current ? Role.onAccent : (root.dimmed ? Role.faint : root.detailInk)
  }

  Row {
    id: trail
    anchors.right: parent.right
    anchors.rightMargin: root.g.px(2)
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.g.px(2)
  }

  Loader {
    anchors.fill: parent
    active: root.hasCursor
    sourceComponent: Brackets { arm: 3; color: root.current ? Role.ink : Role.accent }
  }
}
