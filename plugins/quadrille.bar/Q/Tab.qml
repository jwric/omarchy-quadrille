import QtQuick
import "."

// A tab: a legend in a hairline until it is the page in use, when it is lit:
// the accent behind on_accent. Under the cursor it steps up to raised,
// pressed to hover. A visual only: put a Pressable (or the bar's BarButton)
// around it and feed `hot` and `down`.
//
// One line tall (12 vpx, the border on the line box's edges, a vpx of air
// inside it above the capitals and below the baseline). The legend's ink is
// centred: padding is `air` less the face's side bearings, as quadrille's
// `Face::padding` does.
Item {
  id: root

  property string text: ""
  property bool active: false
  // Pages with something on them read in ink rather than muted.
  property bool occupied: false
  property bool boxed: true
  property int air: 3
  property bool hot: false
  property bool down: false

  implicitWidth: (text.length * 6 + (air - 1) + air + 2) * Px.unit
  implicitHeight: Px.line
  width: implicitWidth
  height: implicitHeight

  Rectangle {
    anchors.fill: parent
    antialiasing: false
    visible: root.active || root.boxed
    color: root.active ? Role.accent : Role.edge
  }
  Rectangle {
    anchors.fill: parent
    anchors.margins: Px.hair
    antialiasing: false
    color: root.active ? Role.accent
      : (root.down ? Role.hover : (root.hot ? Role.raised : Role.ground))
  }
  PixelText {
    x: root.air * Px.unit
    text: root.text
    ink: root.active ? Role.onAccent
      : (root.hot || root.down || root.occupied ? Role.ink : Role.muted)
  }
}
