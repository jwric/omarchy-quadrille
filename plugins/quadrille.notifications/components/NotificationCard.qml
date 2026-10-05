// Notification card, drawn on the pixel grid.
//
// This replaces the stock card (plugins/notifications/components/
// NotificationCard.qml) and keeps its contract: Service.qml sets `app`,
// `appIcon`, `summary`, `body`, `image`, `glyph`, `urgency`, `timestamp`,
// `cornerRadius` and `fontFamily`, reads `hovered` and the implicit size, and
// listens for `closeRequested` and `cardClicked`.
//
// What it draws: a hairline zone on the ground, the app in muted capitals, the
// summary in ink and the body below it, set on whole 6 x 12 cells and wrapped
// there; an app icon, when there is one, beside them. A critical notification
// takes the alarm hairline and a lit lamp. The face steps to raised under the
// cursor; nothing fades or slides. Left click activates, right click dismisses.

import QtQuick
import Quickshell
import qs.Commons
import "../Q"
import "../NotificationLogic.js" as NotificationLogic

Item {
  id: root

  readonly property var g: Px.of(root)

  property string app: ""
  property string appIcon: ""
  property string summary: ""
  property string body: ""
  property string image: ""
  // Nerd Font glyph for a toast with no icon (omarchy-notification-send -g).
  property string glyph: ""
  // NotificationUrgency: Low=0, Normal=1, Critical=2.
  property int urgency: 1
  property double timestamp: 0
  // Square, always: kept so the service can keep assigning it.
  property int cornerRadius: 0
  property string fontFamily: ""

  readonly property bool hovered: hoverTracker.hovered

  signal closeRequested()
  signal cardClicked()

  // ---- content
  readonly property string smallIconSource: image.length > 0 ? image : iconSource(appIcon)
  readonly property bool hasGlyph: glyph.length > 0
  readonly property string plainBody: plain(NotificationLogic.sanitizeBody(body, app, appIcon))
  readonly property bool singleLine: plainBody.length === 0
  readonly property bool critical: urgency === 2
  readonly property bool hasIcon: smallIconSource.length > 0 && iconImage.status !== Image.Error

  function iconSource(icon) {
    var value = String(icon || "")
    if (value.length === 0) return ""
    if (value.indexOf("file://") === 0 || value.indexOf("image://") === 0) return value
    if (value.charAt(0) === "/") return Util.fileUrl(value)
    return Quickshell.iconPath(value, true)
  }

  // The body arrives as the markup the spec allows; the card shows its text.
  function plain(s) {
    return String(s || "")
      .replace(/<br\s*\/?>/gi, "\n")
      .replace(/<[^>]*>/g, "")
      .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"")
      .replace(/&#39;|&apos;/g, "'").replace(/&amp;/g, "&")
      .replace(/^\s+|\s+$/g, "")
  }

  // ---- geometry, in vpx
  readonly property int cardWidth: 194
  readonly property int padX: 5
  readonly property int padY: 4
  readonly property int iconBox: 16
  readonly property int textColumns: Math.floor((cardWidth - 2 - 2 * padX - (hasIcon ? iconBox + 4 : 0)) / 6)
  readonly property bool showApp: app.length > 0 && !singleLine

  implicitWidth: g.px(cardWidth)
  implicitHeight: g.px(2 + 2 * padY) + column.height

  Rectangle {
    anchors.fill: parent
    color: root.critical ? Role.alarm : Role.edge
    antialiasing: false
    Rectangle {
      anchors.fill: parent
      anchors.margins: g.hair
      color: hoverTracker.hovered ? Role.raised : Role.ground
      antialiasing: false
    }
  }

  HoverHandler { id: hoverTracker }

  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onClicked: function(mouse) {
      if (mouse.button === Qt.RightButton) root.closeRequested()
      else root.cardClicked()
    }
  }

  // The app icon, nearest-neighbour: an icon is a picture, not a drawing, and
  // is not smoothed into one.
  Image {
    id: iconImage
    visible: root.hasIcon
    x: g.px(1 + root.padX)
    y: g.px(1 + root.padY)
    width: g.px(root.iconBox)
    height: g.px(root.iconBox)
    source: root.smallIconSource
    sourceSize.width: width * Screen.devicePixelRatio
    sourceSize.height: height * Screen.devicePixelRatio
    fillMode: Image.PreserveAspectFit
    asynchronous: true
    smooth: false
    mipmap: false
  }

  Column {
    id: column
    x: g.px(1 + root.padX + (root.hasIcon ? root.iconBox + 4 : 0))
    y: g.px(1 + root.padY)
    spacing: 0

    // app, and the lamp of a critical one
    Item {
      visible: root.showApp
      width: root.textColumns * g.cellW
      height: root.showApp ? g.line : 0
      PixelText {
        text: root.app.toUpperCase().slice(0, root.textColumns - (root.critical ? 3 : 0))
        ink: Role.muted
      }
      Lamp {
        visible: root.critical
        on: true
        tone: Role.alarm
        x: parent.width - width
        y: g.onCaps(6)
      }
    }

    // the summary: a glyph toast leads with its glyph
    Row {
      spacing: g.px(2)
      visible: root.summary.length > 0 || root.hasGlyph
      Item {
        visible: root.hasGlyph && !root.hasIcon
        width: 2 * g.cellW
        height: g.line
        Text {
          anchors.centerIn: parent
          text: root.glyph
          color: Role.ink
          font.family: Px.face
          font.pixelSize: Math.round(11 * root.g.unit)
          renderType: Text.NativeRendering
        }
      }
      PixelParagraph {
        text: root.summary
        ink: Role.ink
        maxLines: 2
        columns: root.textColumns - (root.hasGlyph && !root.hasIcon ? 3 : 0)
      }
    }

    PixelParagraph {
      text: root.plainBody
      ink: Role.muted
      maxLines: 3
      columns: root.textColumns
    }
  }
}
