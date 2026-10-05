import QtQuick
import "."

// An indicator lamp: a square of colour when lit, dark glass in a hairline
// when not (quadrille's `lamp`). Six vpx square, which centres on the capitals
// of a line of body text.
Item {
  id: root

  readonly property var g: Px.of(root)

  property bool on: false
  // The colour it is lit in: accent (engaged), live (running), caution, alarm.
  property color tone: Role.accent
  property int size: 6

  implicitWidth: size * g.unit
  implicitHeight: size * g.unit
  width: implicitWidth
  height: implicitHeight

  Rectangle {
    anchors.fill: parent
    antialiasing: false
    color: root.on ? root.tone : Role.edge
    Rectangle {
      anchors.fill: parent
      anchors.margins: g.hair
      antialiasing: false
      color: root.on ? root.tone : Role.raised
    }
  }
}
