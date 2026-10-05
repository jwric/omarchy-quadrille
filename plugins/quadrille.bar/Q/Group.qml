import QtQuick
import "."

// A group under a named rule: `┌── NAME ──┐`. The rule says where the group
// starts, and the ends of it drop a few pixels; there are no sides and no
// bottom (quadrille's `group`). The name is centred and set in muted; if it
// will not fit between the ends the rule runs unbroken.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string name: ""
  property color rule: Role.line
  property color nameInk: Role.muted
  // How far the ends of the rule drop, in vpx.
  property int drop: 3
  // Space between the name's line and the content, in vpx.
  property int spacing: 4
  default property alias content: holder.data

  readonly property real header: g.line + spacing * g.unit
  readonly property real nameWidth: name.length * g.cellW
  readonly property real labelLeft: g.floor((width - nameWidth) / 2)
  readonly property bool broken: nameWidth > 0
    && labelLeft - g.cellW > 0 && labelLeft + nameWidth + g.cellW <= width

  implicitWidth: Math.max(holder.childrenRect.width, nameWidth + 4 * g.cellW)
  implicitHeight: header + holder.childrenRect.height

  // y of the rule: the middle of the capitals.
  readonly property real ruleY: g.capTop + g.floor(g.cap / 2)

  // left and right pieces of the rule
  Rectangle {
    visible: root.broken
    x: 0; y: root.ruleY; width: root.labelLeft - g.cellW + g.unit; height: g.hair
    color: root.rule; antialiasing: false
  }
  Rectangle {
    visible: root.broken
    x: root.labelLeft + root.nameWidth + g.cellW - g.unit; y: root.ruleY
    width: root.width - x; height: g.hair
    color: root.rule; antialiasing: false
  }
  Rectangle {
    visible: !root.broken
    x: 0; y: root.ruleY; width: root.width; height: g.hair
    color: root.rule; antialiasing: false
  }
  // the dropped ends
  Rectangle {
    x: 0; y: root.ruleY + g.hair; width: g.hair; height: root.drop * g.unit
    color: root.rule; antialiasing: false
  }
  Rectangle {
    x: root.width - g.hair; y: root.ruleY + g.hair; width: g.hair; height: root.drop * g.unit
    color: root.rule; antialiasing: false
  }
  PixelText {
    visible: root.broken
    x: root.labelLeft
    text: root.name
    ink: root.nameInk
  }
  Item {
    id: holder
    y: root.header
    width: root.width
    height: parent.height - root.header
  }
}
