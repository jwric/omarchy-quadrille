import QtQuick
import "."

// A 1-vpx rule in the edge colour. Horizontal by default; it fills the width
// it is given. A vertical one fills the height.
Rectangle {
  property bool vertical: false
  implicitWidth: vertical ? Px.hair : 0
  implicitHeight: vertical ? 0 : Px.hair
  color: Role.edge
  antialiasing: false
}
