import Quickshell
import Quickshell.Wayland
import QtQuick
import qs.Commons
import "ReminderFlowModel.js" as ReminderFlowModel
import "Q"

// quadrille.reminders: omarchy.reminders (cloned; MIT) on the pixel grid.
//
// The plugin contract is the stock overlay's, untouched: open(payloadJson),
// close(), dismiss(), toggle(), `opened`, the two steps (minutes, then the
// message) and what they run. What changed is the look: a hairline card on the
// ground, the prompt muted, what is typed in ink with a steady block caret, the
// step as "1/2" at the right when it fits, and a Bayer dither behind instead of
// a translucent wash.
Item {
  id: root

  property string omarchyPath: Quickshell.env("OMARCHY_PATH")
  property var shell: null
  property var manifest: null

  property bool opened: false
  property string step: "minutes"
  property string minutes: ""
  property string filterText: ""
  // accepted from the payload (the stock overlay does) and not used: the face is
  // the pixel face
  property string fontFamily: ""

  readonly property var g: Px.forWindow(panel)
  readonly property string promptText: root.step === "message" ? "Reminder message" : "Remind in minutes"

  function open(payloadJson) {
    var payload = ({})
    try { payload = JSON.parse(payloadJson || "{}") } catch (e) { payload = ({}) }
    if (payload.fontFamily) root.fontFamily = payload.fontFamily

    root.opened = true
    root.step = "minutes"
    root.minutes = ""
    root.filterText = ""

    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "omarchy.reminders")
  }

  function toggle() {
    if (root.opened) root.dismiss()
    else root.open("{}")
  }

  function setFilter(nextFilter) {
    root.filterText = nextFilter
  }

  function submit() {
    var selection = root.filterText

    if (root.step === "minutes") {
      var nextMinutes = ReminderFlowModel.validMinutes(selection)

      if (!selection.trim()) {
        root.dismiss()
        return
      }

      if (!nextMinutes) {
        Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-notification-send", "Invalid reminder", "Enter the number of minutes"])
        return
      }

      root.minutes = nextMinutes
      root.step = "message"
      root.filterText = ""
      Qt.callLater(function() { keyCatcher.forceActiveFocus() })
      return
    }

    if (root.step === "message") {
      var args = [root.omarchyPath + "/bin/omarchy-reminder"].concat(ReminderFlowModel.reminderArgs(root.minutes, selection))
      root.dismiss()
      Quickshell.execDetached(args)
    }
  }

  PanelWindow {
    id: panel
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "omarchy-reminders"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    Scrim {
      tone: Qt.rgba(Color.menu.scrim.r, Color.menu.scrim.g, Color.menu.scrim.b, 1)
      density: Color.menu.scrim.a
    }

    MouseArea {
      anchors.fill: parent
      onClicked: root.dismiss()
    }

    Pane {
      id: card
      width: root.g.px(150)
      height: root.g.px(2 + 2 * pad + 12)
      x: root.g.centre(parent.width, width)
      y: root.g.centre(parent.height, height)

      Item {
        id: keyCatcher
        anchors.fill: parent
        focus: true

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          if (event.key === Qt.Key_Escape) {
            if (root.filterText) root.setFilter("")
            else root.dismiss()
            event.accepted = true
          } else if (Util.editsFilter(event, root.filterText)) {
            root.setFilter(Util.editedFilter(event, root.filterText))
            event.accepted = true
          } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.submit()
            event.accepted = true
          } else if (event.text && event.text.length === 1 && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127) {
            root.setFilter(root.filterText + event.text)
            event.accepted = true
          }
        }
      }

      // the step, at the right, when there is room for it beside the line
      PixelText {
        id: stepLabel
        x: card.inner.width - width
        text: root.step === "message" ? "2/2" : "1/2"
        ink: Role.faint
      }
      Prompt {
        width: card.inner.width - stepLabel.width - root.g.px(4)
        text: root.filterText
        prompt: root.promptText + "…"
      }
    }

    // the card eats clicks: only the dither outside it dismisses
    MouseArea { x: card.x; y: card.y; width: card.width; height: card.height; onClicked: {} }
  }
}
