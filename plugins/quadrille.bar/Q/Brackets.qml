import QtQuick
import "."

// Four L-shaped marks at the corners: quadrille's selection and focus frame,
// which does not box its contents in. Fills its parent; `arm` and `weight` are
// in vpx.
Item {
  id: root

  readonly property var g: Px.of(root)

  property int arm: 3
  property int weight: 1
  property color color: Role.accent

  anchors.fill: parent

  readonly property real a: arm * g.unit
  readonly property real w: weight * g.unit

  Repeater {
    model: [
      { l: true, t: true }, { l: false, t: true },
      { l: true, t: false }, { l: false, t: false }
    ]
    Item {
      required property var modelData
      anchors.fill: parent
      Rectangle {   // horizontal arm
        x: modelData.l ? 0 : parent.width - root.a
        y: modelData.t ? 0 : parent.height - root.w
        width: root.a; height: root.w
        color: root.color; antialiasing: false
      }
      Rectangle {   // vertical arm
        x: modelData.l ? 0 : parent.width - root.w
        y: modelData.t ? 0 : parent.height - root.a
        width: root.w; height: root.a
        color: root.color; antialiasing: false
      }
    }
  }
}
