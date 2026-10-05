import QtQuick
import "."

// A group under a named rule: `┌── NAME ──┐`. The rule says where the group
// starts, and the ends of it drop a few pixels; there are no sides and no
// bottom (quadrille's `group`). The name is centred and set in muted; if it
// will not fit between the ends the rule runs unbroken.
Item {
  id: root

  property string name: ""
  property color rule: Role.line
  property color nameInk: Role.muted
  // How far the ends of the rule drop, in vpx.
  property int drop: 3
  // Space between the name's line and the content, in vpx.
  property int spacing: 4
  default property alias content: holder.data

  readonly property int header: Px.line + spacing * Px.unit
  readonly property int nameWidth: name.length * Px.cellW
  readonly property int labelLeft: Px.floor((width - nameWidth) / 2)
  readonly property bool broken: nameWidth > 0
    && labelLeft - Px.cellW > 0 && labelLeft + nameWidth + Px.cellW <= width

  implicitWidth: Math.max(holder.childrenRect.width, nameWidth + 4 * Px.cellW)
  implicitHeight: header + holder.childrenRect.height

  // y of the rule: the middle of the capitals.
  readonly property int ruleY: Px.capTop + Px.floor(Px.cap / 2)

  // left and right pieces of the rule
  Rectangle {
    visible: root.broken
    x: 0; y: root.ruleY; width: root.labelLeft - Px.cellW + Px.unit; height: Px.hair
    color: root.rule; antialiasing: false
  }
  Rectangle {
    visible: root.broken
    x: root.labelLeft + root.nameWidth + Px.cellW - Px.unit; y: root.ruleY
    width: root.width - x; height: Px.hair
    color: root.rule; antialiasing: false
  }
  Rectangle {
    visible: !root.broken
    x: 0; y: root.ruleY; width: root.width; height: Px.hair
    color: root.rule; antialiasing: false
  }
  // the dropped ends
  Rectangle {
    x: 0; y: root.ruleY + Px.hair; width: Px.hair; height: root.drop * Px.unit
    color: root.rule; antialiasing: false
  }
  Rectangle {
    x: root.width - Px.hair; y: root.ruleY + Px.hair; width: Px.hair; height: root.drop * Px.unit
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
