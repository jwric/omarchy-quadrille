import QtQuick
import "."
import "Glyphs.js" as Glyphs

// A line of text to type into: a hairline box, a line tall plus a pixel of air,
// the text in the body face and a steady block for the cursor (nothing blinks).
// The box is the accent while the field has the keyboard, muted under the
// pointer or the panel cursor. Text longer than the box shows the stretch around
// the cursor.
//
// The editing is a real TextInput, kept out of sight; this draws what it holds.
// The stock TextField's contract, as far as the panels use it: `text`,
// `placeholderText`, `password`, `forceActiveFocus()`, `accepted()`,
// `editingFinished()`, and the keys as the `keyPressed(event)` signal.
Item {
  id: root

  readonly property var g: Px.of(root)

  property alias text: input.text
  property string placeholderText: ""
  property bool password: false
  property bool hasCursor: false
  property alias validator: input.validator
  property alias maximumLength: input.maximumLength
  property alias inputMethodHints: input.inputMethodHints
  readonly property bool focused: input.activeFocus
  readonly property alias hovered: area.containsMouse
  readonly property alias cursorPosition: input.cursorPosition

  signal accepted()
  signal editingFinished()
  signal keyPressed(var event)

  function forceActiveFocus() { input.forceActiveFocus() }
  function clear() { input.text = "" }
  function selectAll() { input.selectAll() }

  readonly property bool hot: hovered || hasCursor
  readonly property real pad: g.px(3)
  readonly property int columns: Math.max(1, g.columns(width - 2 * pad - g.hair * 2) - 1)
  // The shown stretch: it ends at the cursor when the text is longer than the box.
  readonly property string shownText: password ? Array(input.text.length + 1).join("*") : input.text
  readonly property int cursorAt: input.cursorPosition
  readonly property int start: Math.max(0, cursorAt - columns + 1)
  readonly property string window: shownText.substr(start, columns)

  implicitWidth: g.px(120)
  implicitHeight: g.px(14)
  height: implicitHeight

  Rectangle {
    anchors.fill: parent
    antialiasing: false
    color: root.focused ? Role.accent : (root.hot ? Role.muted : Role.edge)
  }
  Rectangle {
    anchors.fill: parent
    anchors.margins: root.g.hair
    antialiasing: false
    color: Role.ground
  }

  PixelText {
    x: root.pad
    y: root.g.hair
    visible: input.text.length === 0 && !root.focused
    text: root.placeholderText
    ink: Role.faint
    columns: root.columns
  }
  PixelText {
    x: root.pad
    y: root.g.hair
    visible: input.text.length > 0 || root.focused
    text: root.window
    ink: Role.ink
  }
  // The cursor: a block over the cell it stands on, in the accent.
  Rectangle {
    visible: root.focused
    x: root.pad + (root.cursorAt - root.start) * root.g.cellW
    y: root.g.hair + root.g.px(2)
    width: root.g.cellW
    height: root.g.px(8)
    antialiasing: false
    color: Role.accent
    opacity: 0.55
  }

  TextInput {
    id: input
    opacity: 0
    width: 1; height: 1
    echoMode: root.password ? TextInput.Password : TextInput.Normal
    onAccepted: root.accepted()
    onEditingFinished: root.editingFinished()
    Keys.onPressed: function(event) { root.keyPressed(event) }
  }

  MouseArea {
    id: area
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.IBeamCursor
    onPressed: function(mouse) { input.forceActiveFocus(); mouse.accepted = true }
  }
}
