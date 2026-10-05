import QtQuick
import "."

// A yes / no question over whatever it is parented to, on the pixel grid: the
// stock Ui.ConfirmDialog's contract (opened, message, cancelText, confirmText,
// selectedIndex, handleKey(event), canceled(), confirmed()) with the look
// replaced. A card in a hairline (the alarm colour: what it asks for cannot be
// undone), the message wrapped on whole cells, two buttons; the one that Enter
// would press is an inverse block. Nothing fades.
//
// The colour properties of the stock dialog are accepted and ignored: colour
// here is a role.
Item {
  id: root

  readonly property var g: Px.of(root)

  property bool opened: false
  property string message: ""
  property string cancelText: "Cancel"
  property string confirmText: "Confirm"
  property int selectedIndex: 1
  property bool destructive: true

  // accepted for compatibility with Ui.ConfirmDialog, and not used
  property color background
  property color foreground
  property color scrim
  property color selectedBackground
  property color selectedText
  property string fontFamily
  property int cornerRadius

  signal canceled()
  signal confirmed()

  function handleKey(event) {
    if (!root.opened) return false
    if (event.key === Qt.Key_Escape) {
      root.canceled()
      return true
    } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
      root.selectedIndex = root.selectedIndex === 0 ? 1 : 0
      return true
    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
      if (root.selectedIndex === 0) root.canceled()
      else root.confirmed()
      return true
    }
    return false
  }

  visible: opened

  // geometry, in vpx
  readonly property int pad: 6
  readonly property int buttonHeight: 14
  readonly property int cardCols: Math.max(12, Math.min(28, Math.floor((width / g.unit - 2 * 8 - 2 * pad - 2) / 6)))
  readonly property int cardVpx: cardCols * 6 + 2 * pad + 2

  // a wash of the ground over what is behind (a dither is the toolkit's
  // answer; the card alone is enough here)
  Rectangle {
    anchors.fill: parent
    color: Role.void_
    opacity: 0.7
    MouseArea { anchors.fill: parent; onClicked: root.canceled() }
  }

  Rectangle {
    id: card
    width: g.px(root.cardVpx)
    height: g.px(2 + root.pad + paragraph.lines.length * 12 + 6 + root.buttonHeight + root.pad)
    x: g.centre(root.width, width)
    y: g.centre(root.height, height)
    color: root.destructive ? Role.alarm : Role.edge
    antialiasing: false

    Rectangle {
      anchors.fill: parent
      anchors.margins: g.hair
      color: Role.ground
      antialiasing: false
    }

    MouseArea { anchors.fill: parent; onClicked: {} }

    PixelParagraph {
      id: paragraph
      x: g.px(1 + root.pad)
      y: g.px(1 + root.pad)
      text: root.message
      columns: root.cardCols
      maxLines: 4
      ink: Role.ink
    }

    Row {
      anchors.right: parent.right
      anchors.rightMargin: g.px(1 + root.pad)
      anchors.bottom: parent.bottom
      anchors.bottomMargin: g.px(1 + root.pad)
      spacing: g.px(4)

      Repeater {
        model: [root.cancelText, root.confirmText]

        Item {
          id: button
          required property int index
          required property string modelData
          readonly property bool selected: root.selectedIndex === index
          readonly property bool danger: index === 1 && root.destructive
          width: g.px(modelData.length * 6 + 2 * 4 + 2)
          height: g.px(root.buttonHeight)

          // the hairline box (the edge, or the alarm of a destructive answer)
          Rectangle {
            anchors.fill: parent
            color: button.selected ? (button.danger ? Role.alarm : Role.accent) : Role.edge
            antialiasing: false
            Rectangle {
              anchors.fill: parent
              anchors.margins: g.hair
              visible: !button.selected
              color: Role.ground
              antialiasing: false
            }
          }
          PixelText {
            x: g.px(1 + 4)
            y: g.centre(parent.height, height)
            text: modelData
            ink: button.selected ? Role.onAccent : (button.danger ? Role.alarm : Role.ink)
          }
          MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: root.selectedIndex = button.index
            onClicked: {
              if (button.index === 0) root.canceled()
              else root.confirmed()
            }
          }
        }
      }
    }
  }
}
