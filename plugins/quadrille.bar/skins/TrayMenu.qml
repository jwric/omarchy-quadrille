import QtQuick
import Quickshell
import "../Q"
import "../Q/Glyphs.js" as Glyphs

// A tray item's menu, drawn from the kit: rows of text on whole cells, a pixel
// chevron for a submenu, a tick for a checked entry, the entry under the cursor
// an inverse block, a disabled one faint. Submenus open in place with a way back
// (the platform's own menu is not available: the shell does not run in
// QApplication mode, which is also why the stock tray draws its own).
//
// Give it the tray item's `menu` handle as `handle`; it sizes itself
// (`implicitWidth`/`implicitHeight`, logical px) and calls `done()` after an
// entry has been triggered.
Item {
  id: root

  readonly property var g: Px.of(root)

  property var handle: null
  // The item's own title, to hide the entry some apps repeat as the first row.
  property string title: ""
  property int maxRows: 22
  signal done()

  // ---- submenu drill-down: one live opener per level, deepest destroyed first
  property var stack: []
  readonly property int depth: stack.length
  readonly property string levelTitle: depth > 0 ? stack[depth - 1].title : ""
  readonly property var entries: depth > 0 ? stack[depth - 1].opener.children : rootOpener.children

  Component { id: openerComponent; QsMenuOpener { } }
  QsMenuOpener { id: rootOpener; menu: root.handle }

  // A level change rebuilds the rows under a pointer that has not moved; the
  // click that opened a submenu must not also fire whatever lands beneath it.
  property bool settling: false
  Timer { id: settle; interval: 250; onTriggered: root.settling = false }

  function reset() {
    var open = stack
    stack = []
    for (var i = open.length - 1; i >= 0; i--) open[i].opener.destroy()
    settling = false
    flick.contentY = 0
  }
  function enter(entry, text) {
    var opener = openerComponent.createObject(root, { menu: entry })
    if (!opener) return
    var next = stack.slice()
    next.push({ opener: opener, title: text })
    stack = next
    settling = true; settle.restart()
    flick.contentY = 0
  }
  function leave() {
    if (stack.length === 0) return
    var next = stack.slice()
    var top = next.pop()
    stack = next
    top.opener.destroy()
    settling = true; settle.restart()
    flick.contentY = 0
  }
  onHandleChanged: reset()
  Component.onDestruction: reset()

  // ---- geometry, in vpx: 3 air, 11 for a tick or an icon, 2, text, 3, 7 chevron, 3
  readonly property int rowH: 14
  readonly property int sepH: 5
  readonly property int cells: {
    var widest = 18
    var rows = entries ? entries.values : []
    for (var i = 0; i < rows.length; i++) {
      var e = rows[i]
      if (e && !e.isSeparator) widest = Math.max(widest, Glyphs.length(String(e.text || "")))
    }
    return Math.min(widest, 40)
  }
  readonly property real rowsHeight: column.implicitHeight
  readonly property real headerHeight: depth > 0 ? g.px(rowH + 1) : 0
  readonly property real maxHeight: g.px(rowH * maxRows)

  implicitWidth: g.px(29) + cells * g.cellW
  implicitHeight: headerHeight + Math.min(rowsHeight, maxHeight - headerHeight)
  width: implicitWidth
  height: implicitHeight

  // ---- the way back out of a submenu
  Item {
    id: header
    visible: root.depth > 0
    width: parent.width
    height: root.headerHeight

    Rectangle {
      width: parent.width; height: root.g.px(rowH)
      color: backArea.containsMouse ? Role.hover : "transparent"
      antialiasing: false
      Sprite { rows: Sprites.chevronLeft; color: Role.ink; x: root.g.px(3); y: root.g.centre(parent.height, height) }
      PixelText {
        x: root.g.px(3 + 11 + 2); y: root.g.centre(parent.height, height)
        text: root.levelTitle; ink: Role.ink
        columns: root.cells
      }
      MouseArea {
        id: backArea
        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
        onClicked: if (!root.settling) root.leave()
      }
    }
    Rectangle {
      y: root.g.px(rowH); width: parent.width; height: root.g.hair
      color: Role.edge; antialiasing: false
    }
  }

  Flickable {
    id: flick
    y: root.headerHeight
    width: parent.width
    height: parent.height - root.headerHeight
    contentWidth: width
    contentHeight: column.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height

    Column {
      id: column
      width: flick.width

      Repeater {
        model: root.entries

        delegate: Item {
          id: row
          required property var modelData
          required property int index

          readonly property string label: String(modelData.text || "")
          // Both only ever describe the root menu: an app that repeats its own
          // name as the first row, and a separator right after it.
          readonly property bool atRoot: root.depth === 0
          readonly property bool titleRow: atRoot && index === 0 && modelData.hasChildren && label.toLowerCase() === root.title.toLowerCase()
          readonly property bool leadingRule: atRoot && modelData.isSeparator && index <= 1
          readonly property bool hidden: titleRow || leadingRule
          readonly property bool live: !modelData.isSeparator && modelData.enabled
          readonly property bool lit: live && area.containsMouse

          visible: !hidden
          width: column.width
          height: hidden ? 0 : root.g.px(modelData.isSeparator ? root.sepH : root.rowH)

          Rectangle {
            visible: row.modelData.isSeparator
            x: root.g.px(2); width: parent.width - root.g.px(4)
            y: root.g.centre(parent.height, height); height: root.g.hair
            color: Role.edge; antialiasing: false
          }
          Rectangle {
            visible: !row.modelData.isSeparator
            anchors.fill: parent
            color: row.lit ? Role.accent : "transparent"
            antialiasing: false
          }
          // a tick, or an icon
          Sprite {
            visible: !row.modelData.isSeparator && row.modelData.buttonType !== QsMenuButtonType.None && row.modelData.checkState === Qt.Checked
            rows: Sprites.tick
            color: row.lit ? Role.onAccent : (row.live ? Role.ink : Role.faint)
            x: root.g.px(3 + 2); y: root.g.centre(parent.height, height)
          }
          PixelIcon {
            visible: !row.modelData.isSeparator && String(row.modelData.icon || "") !== "" && row.modelData.buttonType === QsMenuButtonType.None
            source: visible ? String(row.modelData.icon || "") : ""
            cells: 11
            dimmed: !row.live
            x: root.g.px(3); y: root.g.centre(parent.height, height)
          }
          PixelText {
            visible: !row.modelData.isSeparator
            x: root.g.px(3 + 11 + 2); y: root.g.centre(parent.height, height)
            text: row.label
            ink: row.lit ? Role.onAccent : (row.live ? Role.ink : Role.faint)
            columns: root.cells
          }
          Sprite {
            visible: !row.modelData.isSeparator && row.modelData.hasChildren
            rows: Sprites.chevronRight
            color: row.lit ? Role.onAccent : (row.live ? Role.muted : Role.faint)
            x: parent.width - root.g.px(3 + 7); y: root.g.centre(parent.height, height)
          }
          MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            enabled: row.live
            cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: {
              if (root.settling) return
              if (row.modelData.hasChildren) root.enter(row.modelData, row.label)
              else { row.modelData.triggered(); root.done() }
            }
          }
        }
      }
    }
  }

  // Past the first or last row with more behind it: a hairline, as in the menu.
  Rectangle {
    visible: flick.interactive && flick.contentY > 0
    y: root.headerHeight; width: parent.width; height: root.g.hair
    color: Role.muted; antialiasing: false
  }
  Rectangle {
    visible: flick.interactive && flick.contentY + flick.height < flick.contentHeight - 1
    y: parent.height - height; width: parent.width; height: root.g.hair
    color: Role.muted; antialiasing: false
  }
}
