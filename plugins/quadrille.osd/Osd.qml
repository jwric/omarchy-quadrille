import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "Q"
import "Q/Glyphs.js" as Glyphs
import "OsdModel.js" as OsdModel

// quadrille.osd: omarchy.osd (cloned; MIT) with its card replaced.
//
// The host contract is the stock plugin's, unchanged: the `osd` IPC target
// (show / close / state / ping), open(payloadJson) and close(), and the
// payload's icon / message / value / max / progressText / duration. What
// changed is the card: a flat ground zone in a hairline, a 7 x 7 pixel icon at
// twice its size (so every pixel is 2 vpx), a stepped gauge, and the readout in
// the body face. The gauge steps to the new value at once; nothing eases.
Item {
  id: root

  property bool opened: false
  property string icon: ""
  property string message: ""
  property string iconKey: ""
  property int value: 0
  property int maxValue: 100
  property bool hasProgress: true
  property int duration: 1200

  // Where the card sits and how it is built, in vpx.
  readonly property int pad: 8
  readonly property int gap: 8
  readonly property int iconSize: 14
  readonly property int gaugeCells: 20
  // A message wraps on this many cells, over at most two lines.
  readonly property int maxMessage: 36
  readonly property int maxLines: 2

  readonly property real fraction: maxValue > 0 ? value / maxValue : 0
  readonly property string readout: message
  readonly property int messageColumns: Math.min(Glyphs.length(root.readout), maxMessage)
  readonly property int messageLines: hasProgress ? 1 : Math.max(1, Math.min(maxLines, Glyphs.wrap(root.readout, Math.max(1, messageColumns), maxLines).length))
  readonly property var look: spriteFor(iconKey, hasProgress ? Math.round(fraction * 100) : 100)

  // The icon for a key from the payload, and the level its waves show.
  function spriteFor(key, percent) {
    var k = String(key || "")
    if (k.indexOf("volume") === 0 || k === "mute" || k === "muted") {
      var muted = k.indexOf("mute") !== -1 || percent <= 0
      return { rows: muted ? Sprites.muted : Sprites.volume, level: percent < 50 ? 1 : 2 }
    }
    if (k.indexOf("microphone") === 0 || k.indexOf("mic") === 0)
      return { rows: k.indexOf("muted") !== -1 || k.indexOf("off") !== -1 ? Sprites.microphoneMuted : Sprites.microphone, level: 9 }
    if (k === "brightness" || k === "display") return { rows: Sprites.brightness, level: 9 }
    if (k === "keyboard") return { rows: Sprites.keyboard, level: 9 }
    if (k === "reboot" || k === "restart" || k === "shutdown" || k === "power" || k === "poweroff" || k === "logout" || k === "sign-out" || k === "leave")
      return { rows: Sprites.power, level: 9 }
    if (k === "media-play" || k === "player-play") return { rows: Sprites.play, level: 9 }
    if (k === "media-pause" || k === "player-pause") return { rows: Sprites.pause, level: 9 }
    if (k === "media-next" || k === "player-next") return { rows: Sprites.next, level: 9 }
    if (k === "media-previous" || k === "player-previous") return { rows: Sprites.previous, level: 9 }
    if (k === "lock") return { rows: Sprites.lock, level: 9 }
    if (k === "touchpad") return { rows: Pictograms.touchpad, level: 9 }
    if (k === "touch" || k === "touchscreen") return { rows: Pictograms.touchscreen, level: 9 }
    if (k === "media" || k === "player" || k === "media-source" || k === "player-source") return { rows: Sprites.play, level: 9 }
    // a caller may pass the glyph itself, as the stock icon table does: the
    // few of them in use are mapped, the rest are a bell
    if (k === "\udb80\udfda") return { rows: Pictograms.download, level: 9 }
    return { rows: Sprites.bell, level: 9 }
  }

  function show(iconName, rawMessage, rawValue, rawMax, rawProgressText, rawDuration) {
    var next = OsdModel.stateForShow(iconName, rawMessage, rawValue, rawMax, rawProgressText, rawDuration)
    iconKey = next.iconKey
    maxValue = next.maxValue
    hasProgress = next.hasProgress
    value = next.value
    message = next.message
    icon = next.icon
    duration = next.duration
    opened = true
    if (duration > 0) hideTimer.restart()
    else hideTimer.stop()
  }

  function open(payloadJson) {
    try {
      var p = JSON.parse(payloadJson || "{}")
      show(p.icon || "", p.message || "", p.value === undefined ? "" : String(p.value), p.max === undefined ? "100" : String(p.max), p.progressText || "", p.duration === undefined ? "1200" : String(p.duration))
    } catch (e) {}
  }

  function close() { opened = false }

  Timer {
    id: hideTimer
    interval: root.duration
    onTriggered: root.opened = false
  }

  IpcHandler {
    target: "osd"
    function show(payloadJson: string): string {
      root.open(payloadJson)
      return "ok"
    }
    function close(): string { root.close(); return "ok" }
    function state(): string { return root.opened ? "open" : "closed" }
    function ping(): string { return "ok" }
  }

  PanelWindow {
    id: panel

    // The grid of the screen this card is on.
    readonly property var g: Px.forWindow(panel)
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "omarchy-osd"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    // Visual-only surface: the input region stays empty so the card never
    // blocks clicks to the desktop below it.
    mask: Region {}

    Rectangle {
      id: card
      readonly property real content: panel.g.px(Math.max(root.iconSize, root.hasProgress ? root.iconSize : root.messageLines * 12))
      width: panel.g.px(root.pad * 2 + root.iconSize + root.gap)
        + (root.hasProgress ? panel.g.px(root.gaugeCells * 4 - 1 + root.gap) + readoutBox.width
                            : root.messageColumns * panel.g.cellW)
        + panel.g.px(2)
      height: panel.g.px(root.pad * 2 + 2) + content
      x: panel.g.centre(parent.width, width)
      y: parent.height - height - panel.g.px(32)
      color: Role.edge
      antialiasing: false

      Rectangle {
        anchors.fill: parent
        anchors.margins: panel.g.hair
        color: Role.ground
        antialiasing: false
      }

      Sprite {
        id: iconSprite
        x: panel.g.px(root.pad + 1)
        y: panel.g.px(root.pad + 1) + panel.g.centre(card.content, height)
        unit: 2 * panel.g.unit
        rows: root.look.rows
        level: root.look.level
        color: Role.ink
        dim: Role.faint
        accent: Role.alarm
      }

      BarGauge {
        id: gauge
        visible: root.hasProgress
        x: iconSprite.x + iconSprite.width + panel.g.px(root.gap)
        y: panel.g.px(root.pad + 1) + panel.g.centre(iconSprite.height, height)
        cells: root.gaugeCells
        cellWidth: 3
        cellHeight: 6
        value: root.fraction
        fill: Role.accent
        redline: 1
      }

      // The readout takes the width of the widest value, so it does not
      // jitter as the digits change.
      Item {
        id: readoutBox
        width: root.hasProgress ? 4 * panel.g.cellW : 0
        height: panel.g.line
        x: root.hasProgress ? gauge.x + gauge.width + panel.g.px(root.gap) : 0
        y: panel.g.px(root.pad + 1) + panel.g.centre(iconSprite.height, height)
        visible: root.hasProgress
        PixelText {
          id: readout
          x: parent.width - width
          text: root.readout
          ink: Role.ink
        }
      }

      PixelParagraph {
        visible: !root.hasProgress
        x: iconSprite.x + iconSprite.width + panel.g.px(root.gap)
        y: panel.g.px(root.pad + 1) + panel.g.centre(card.content, height)
        text: root.readout
        columns: Math.max(1, root.messageColumns)
        maxLines: root.maxLines
        ink: Role.ink
      }
    }
  }
}
