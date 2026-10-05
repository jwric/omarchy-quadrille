import QtQuick
import "."

// A 1-vpx rule in the edge colour. Horizontal by default; it fills the width
// it is given. A vertical one fills the height.
Rectangle {
  id: root
  readonly property var g: Px.of(root)
  property bool vertical: false
  implicitWidth: vertical ? g.hair : 0
  implicitHeight: vertical ? 0 : g.hair
  color: Role.edge
  antialiasing: false
}
