import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import qs.Commons
import "EmojiSearch.js" as EmojiSearch
import "Q/Glyphs.js" as Glyphs
import "Q"

// quadrille.emojis: omarchy.emojis (cloned; MIT) on the pixel grid.
//
// The contract is the stock overlay's: open(payloadJson), close(), dismiss(),
// toggle(), `opened`, the emoji list read from the stock plugin's emojis.json,
// the search over its keywords, Enter or a click inserting through
// omarchy-menu-emoji-insert. What changed is the look. Each emoji (a colour
// bitmap in the system font) is redrawn on a 12 x 12 grid of virtual pixels by
// PixelEmoji, so they sit with the pixel icons and are nearest-neighbour at an
// integer scale on every screen; the card is a hairline, the selection is four
// corner brackets, and the name of the selected emoji is a muted line below.
// The list scrolls a whole row at a time.
Item {
  id: root

  property string omarchyPath: Quickshell.env("OMARCHY_PATH")
  property var shell: null
  property var manifest: null

  property bool opened: false
  property string filterText: ""
  property int selectedIndex: 0
  property bool cursorActive: false
  property var emojis: []
  property var filteredEmojis: []

  readonly property var g: Px.forWindow(panel)

  // the grid, in vpx
  readonly property int cellVpx: 18
  readonly property int columns: 10
  readonly property int visibleRows: 8
  readonly property int pad: 6

  function open(payloadJson) {
    root.opened = true
    root.filterText = ""
    root.selectedIndex = 0
    root.cursorActive = true
    root.rebuildDisplay()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "omarchy.emojis")
  }

  function toggle() {
    if (root.opened) root.dismiss()
    else root.open("{}")
  }

  function loadEmojis(raw) {
    root.emojis = EmojiSearch.parseEmojis(raw)
    if (root.opened) root.rebuildDisplay()
  }

  function rebuildDisplay() {
    var out = EmojiSearch.filterEmojis(root.emojis, root.filterText, 1000)
    root.filteredEmojis = out

    displayModel.clear()
    for (var j = 0; j < out.length; j++) {
      displayModel.append({ emoji: out[j].e, keywords: out[j].k || "" })
    }

    if (displayModel.count === 0) selectedIndex = 0
    else if (selectedIndex >= displayModel.count) selectedIndex = displayModel.count - 1
    else if (selectedIndex < 0) selectedIndex = 0
    cursorActive = displayModel.count > 0

    Qt.callLater(function() { root.reveal() })
  }

  // Scroll to the row of the selection, whole rows only.
  function reveal() {
    if (displayModel.count === 0) { resultGrid.contentY = 0; return }
    var row = Math.floor(selectedIndex / columns)
    var top = Math.round(resultGrid.contentY / resultGrid.cellHeight)
    if (row < top) top = row
    else if (row >= top + visibleRows) top = row - visibleRows + 1
    var maxTop = Math.max(0, Math.ceil(displayModel.count / columns) - visibleRows)
    resultGrid.contentY = Math.max(0, Math.min(maxTop, top)) * resultGrid.cellHeight
  }

  function select(delta) {
    if (displayModel.count === 0) return
    if (!cursorActive) {
      cursorActive = true
      selectedIndex = delta < 0 ? displayModel.count - 1 : 0
    } else {
      selectedIndex = (selectedIndex + delta + displayModel.count) % displayModel.count
    }
    reveal()
  }

  function selectRow(delta) {
    if (displayModel.count === 0) return
    if (!cursorActive) {
      cursorActive = true
      selectedIndex = delta < 0 ? displayModel.count - 1 : 0
    } else {
      var newIndex = selectedIndex + delta * columns
      if (newIndex < 0) newIndex = 0
      if (newIndex >= displayModel.count) newIndex = displayModel.count - 1
      selectedIndex = newIndex
    }
    reveal()
  }

  function selectPage(delta) {
    if (displayModel.count === 0) return
    cursorActive = true
    var newIndex = selectedIndex + delta * columns * visibleRows
    if (newIndex < 0) newIndex = 0
    if (newIndex >= displayModel.count) newIndex = displayModel.count - 1
    selectedIndex = newIndex
    reveal()
  }

  function setFilter(nextFilter) {
    root.filterText = nextFilter
    root.selectedIndex = 0
    root.cursorActive = true
    root.rebuildDisplay()
  }

  function activateIndex(index) {
    if (index < 0 || index >= displayModel.count) return
    var row = displayModel.get(index)
    root.applySelected(row.emoji)
  }

  function applySelected(emoji) {
    if (!emoji) return
    root.dismiss()
    Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-menu-emoji-insert", emoji])
  }

  // `text` cut to `columns` cells: an ellipsis at the end, no space before it
  function fitCells(text, cols) {
    var chars = Array.from(String(text || ""))
    if (chars.length <= cols) return chars.join("")
    if (cols < 2) return ""
    return chars.slice(0, cols - 1).join("").replace(/\s+$/, "") + "…"
  }

  ListModel { id: displayModel }

  FileView {
    path: root.omarchyPath + "/shell/plugins/emojis/emojis.json"
    onLoaded: root.loadEmojis(text())
  }

  PanelWindow {
    id: panel
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "omarchy-emojis"
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
      pad: root.pad
      readonly property int gridVpx: root.columns * root.cellVpx
      width: root.g.px(gridVpx + 2 * (root.pad + 1))
      // prompt, rule, grid, name
      height: root.g.px(12 + 3 + 1 + 3 + root.visibleRows * root.cellVpx + 3 + 12 + 2 * (root.pad + 1))
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
          } else if (event.key === Qt.Key_Left) {
            root.select(-1)
            event.accepted = true
          } else if (event.key === Qt.Key_Right) {
            root.select(1)
            event.accepted = true
          } else if (event.key === Qt.Key_Up) {
            root.selectRow(-1)
            event.accepted = true
          } else if (event.key === Qt.Key_Down) {
            root.selectRow(1)
            event.accepted = true
          } else if (event.key === Qt.Key_PageUp) {
            root.selectPage(-1)
            event.accepted = true
          } else if (event.key === Qt.Key_PageDown) {
            root.selectPage(1)
            event.accepted = true
          } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (root.cursorActive) root.activateIndex(root.selectedIndex)
            else if (displayModel.count > 0) root.cursorActive = true
            event.accepted = true
          } else if (event.text && event.text.length === 1 && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127) {
            root.setFilter(root.filterText + event.text)
            event.accepted = true
          }
        }
      }

      Prompt {
        width: card.inner.width
        text: root.filterText
        prompt: "Search emojis…"
      }

      Hairline {
        y: root.g.px(12 + 3)
        width: card.inner.width
      }

      // the grid
      Item {
        id: gridBox
        y: root.g.px(12 + 3 + 1 + 3)
        width: card.inner.width
        height: root.g.px(root.visibleRows * root.cellVpx)
        clip: true

        GridView {
          id: resultGrid
          anchors.fill: parent
          model: displayModel
          interactive: false
          cellWidth: root.g.px(root.cellVpx)
          cellHeight: root.g.px(root.cellVpx)
          boundsBehavior: Flickable.StopAtBounds
          cacheBuffer: 0

          delegate: Item {
            id: cell
            required property int index
            required property string emoji

            readonly property bool hasCursor: root.cursorActive && index === root.selectedIndex

            width: resultGrid.cellWidth
            height: resultGrid.cellHeight

            // the emoji on a 14 x 14 grid of virtual pixels, the same pixel as the rest
            PixelEmoji {
              x: root.g.px(2)
              y: root.g.px(2)
              text: cell.emoji
              cells: 14
            }
            Brackets {
              visible: cell.hasCursor
              arm: 3
              color: Role.accent
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onEntered: { root.cursorActive = true; root.selectedIndex = cell.index }
              onClicked: {
                root.cursorActive = true
                root.selectedIndex = cell.index
                root.activateIndex(cell.index)
              }
            }
          }

          // the wheel moves a whole row (three, as a notch does elsewhere), never a fraction
          WheelHandler {
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: function(event) {
              var notches = Math.sign(event.angleDelta.y)
              if (notches === 0) return
              var maxTop = Math.max(0, Math.ceil(displayModel.count / root.columns) - root.visibleRows)
              var top = Math.round(resultGrid.contentY / resultGrid.cellHeight) - notches * 3
              resultGrid.contentY = Math.max(0, Math.min(maxTop, top)) * resultGrid.cellHeight
            }
          }
        }

        // nothing found
        Row {
          visible: displayModel.count === 0
          x: root.g.px(2)
          y: root.g.px(2)
          spacing: root.g.px(3)
          Sprite { rows: Pictograms.search; color: Role.muted; anchors.verticalCenter: parent.verticalCenter }
          PixelText { text: "No matches"; ink: Role.muted }
        }

        // a hairline at an edge with more behind it
        Hairline {
          visible: resultGrid.contentY > 0
          width: parent.width
        }
        Hairline {
          visible: displayModel.count > 0
            && Math.round(resultGrid.contentY / resultGrid.cellHeight) + root.visibleRows < Math.ceil(displayModel.count / root.columns)
          y: parent.height - height
          width: parent.width
        }
      }

      // the name of the selected emoji
      PixelText {
        y: root.g.px(12 + 3 + 1 + 3) + gridBox.height + root.g.px(3)
        text: displayModel.count > 0 && root.selectedIndex < displayModel.count
          ? root.fitCells(displayModel.get(root.selectedIndex).keywords, Math.floor(card.inner.width / root.g.cellW)) : ""
        ink: Role.muted
      }
    }

    // the card eats clicks: only the dither outside it dismisses
    MouseArea { x: card.x; y: card.y; width: card.width; height: card.height; onClicked: {} }
  }
}
