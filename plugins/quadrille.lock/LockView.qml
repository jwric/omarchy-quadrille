import QtQuick
import qs.Commons
import "Q"

// quadrille.lock's face: omarchy.lock's LockView (MIT) on the pixel grid.
//
// The contract is the stock view's, property for property and signal for signal,
// and Service.qml, which owns the session lock and the PAM conversations, is the
// stock one byte for byte: only what is drawn here differs. A flat void, and on
// it one card in a hairline with the state of the lock on three lines:
//
//   LOCKED                    21:47
//   ──────────────────────────────
//   > ■ ■ ■ ■ ■                      (or the prompt, "CHECKING", the failure)
//   fingerprint  or touch the sensor (when a reader is enrolled)
//
// The password is never drawn as the characters: a 5 x 5 block a character, in
// ink. The card's hairline is the accent while typing and the alarm after a
// wrong password. There is no blurred wallpaper (a blur is the opposite of a
// pixel), no animation, and the typing still goes to a real TextInput, so input
// methods and the keys the stock view handles (Escape and Ctrl+U clear) work.
Item {
  id: root

  // the pixel grid of the window this is in (a lock surface is a window of its own)
  readonly property var g: Px.of(root)

  property string backgroundPath: ""
  property int backgroundVersion: 0
  property bool fingerprintConfigured: false
  property bool authenticatingPassword: false
  property string failureMessage: ""
  property int failedAttempts: 0
  property bool inputEnabled: true
  property bool loadBackground: true
  property string passwordText: ""
  property bool syncingPasswordText: false

  readonly property bool errorState: failureMessage.length > 0
  readonly property bool typing: passwordText.length > 0

  signal submitPassword(string password)
  signal passwordTextEdited(string password)
  signal clearFailureRequested()
  signal wakeRequested()

  function forcePasswordFocus() {
    passwordInput.forceActiveFocus()
  }

  function clearPassword() {
    passwordTextEdited("")
  }

  function syncPasswordText() {
    if (passwordInput.text === passwordText) return
    syncingPasswordText = true
    passwordInput.text = passwordText
    syncingPasswordText = false
  }

  onPasswordTextChanged: syncPasswordText()
  onInputEnabledChanged: {
    if (inputEnabled) Qt.callLater(forcePasswordFocus)
  }
  Component.onCompleted: {
    syncPasswordText()
    if (inputEnabled) Qt.callLater(forcePasswordFocus)
  }

  // the time, to the minute: a lock that does not say what time it is is a wall
  property string clockText: ""
  function tick() {
    var d = new Date()
    function two(n) { return (n < 10 ? "0" : "") + n }
    clockText = two(d.getHours()) + ":" + two(d.getMinutes())
  }
  Timer {
    interval: 10000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.tick()
  }

  Rectangle {
    anchors.fill: parent
    color: Role.void_
    antialiasing: false

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      onClicked: { root.wakeRequested(); root.forcePasswordFocus() }
      onPositionChanged: root.wakeRequested()
    }

    // the card, 176 vpx wide
    Pane {
      id: card
      pad: 6
      rule: root.errorState ? Role.alarm : (root.typing || root.authenticatingPassword ? Role.accent : Role.edge)
      width: root.g.px(176)
      height: root.g.px(2 + 2 * pad + 12 + 3 + 1 + 3 + 14 + (root.fingerprintConfigured ? 3 + 12 : 0))
      x: root.g.centre(parent.width, width)
      y: root.g.centre(parent.height, height)

      // the head: what this is, and the time
      Sprite {
        rows: Sprites.lock
        color: Role.muted
        y: root.g.onCaps(7)
      }
      PixelText {
        x: root.g.px(10)
        text: "LOCKED"
        ink: Role.muted
      }
      PixelText {
        x: card.inner.width - width
        text: root.clockText
        ink: Role.ink
      }
      Hairline {
        y: root.g.px(12 + 3)
        width: card.inner.width
      }

      // the field: the real input sits here, invisible; what is drawn is below
      TextInput {
        id: passwordInput
        x: 0
        y: root.g.px(12 + 3 + 1 + 3)
        width: card.inner.width
        height: root.g.px(14)
        opacity: 0
        activeFocusOnPress: true
        enabled: root.inputEnabled && !root.authenticatingPassword
        readOnly: root.authenticatingPassword
        echoMode: TextInput.Password
        passwordMaskDelay: 0
        font.pixelSize: 8

        onTextChanged: {
          if (!root.syncingPasswordText) root.passwordTextEdited(text)
          if (text.length > 0) root.wakeRequested()
          if (text.length > 0 && root.failureMessage.length > 0) root.clearFailureRequested()
        }

        onAccepted: {
          var submitted = root.passwordText
          root.passwordTextEdited("")
          if (submitted.length > 0) root.submitPassword(submitted)
        }

        Keys.onPressed: function(event) {
          root.wakeRequested()
          if (event.key === Qt.Key_Escape || (event.modifiers & Qt.ControlModifier && event.key === Qt.Key_U)) {
            root.passwordTextEdited("")
            event.accepted = true
          }
        }
      }

      Item {
        id: field
        y: root.g.px(12 + 3 + 1 + 3)
        width: card.inner.width
        height: root.g.px(14)

        // the prompt mark
        PixelText {
          y: root.g.px(1)
          text: ">"
          ink: root.errorState ? Role.alarm : Role.accent
        }

        // the password: a block a character, the tail when it outgrows the line
        Item {
          id: dots
          x: root.g.px(10)
          y: root.g.px(1)
          width: field.width - x
          height: root.g.line
          visible: root.typing && !root.authenticatingPassword
          readonly property int pitch: 7
          readonly property int capacity: Math.max(1, Math.floor(width / root.g.px(pitch)))
          readonly property int count: Math.min(root.passwordText.length, capacity)
          Repeater {
            model: dots.count
            Rectangle {
              required property int index
              x: index * root.g.px(dots.pitch)
              y: root.g.capTop + root.g.px(1)
              width: root.g.px(5)
              height: root.g.px(6)
              color: Role.ink
              antialiasing: false
            }
          }
          // the caret after the last one
          Rectangle {
            x: dots.count * root.g.px(dots.pitch)
            y: root.g.capTop + root.g.px(1)
            width: root.g.px(5)
            height: root.g.px(6)
            color: Role.accent
            visible: root.inputEnabled && x + width <= dots.width
            antialiasing: false
          }
        }

        // the words: the prompt, the check, or what went wrong
        PixelText {
          x: root.g.px(10)
          y: root.g.px(1)
          visible: !(root.typing && !root.authenticatingPassword)
          text: root.authenticatingPassword ? "CHECKING"
              : (root.failureMessage.length > 0 ? root.failureMessage.toUpperCase() : "ENTER PASSWORD")
          columns: Math.floor((field.width - root.g.px(10)) / root.g.cellW)
          ink: root.authenticatingPassword ? Role.live : (root.errorState ? Role.alarm : Role.muted)
        }
      }

      // a reader is enrolled: say so, dropped when it does not fit
      Sprite {
        visible: root.fingerprintConfigured
        rows: Pictograms.fingerprint
        color: Role.faint
        y: root.g.px(12 + 3 + 1 + 3 + 14 + 3) + root.g.onCaps(7)
      }
      PixelText {
        visible: root.fingerprintConfigured
        x: root.g.px(10)
        y: root.g.px(12 + 3 + 1 + 3 + 14 + 3)
        text: "OR TOUCH THE SENSOR"
        room: card.inner.width - root.g.px(10)
        ink: Role.faint
      }
    }
  }
}
