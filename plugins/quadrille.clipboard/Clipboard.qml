import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import qs.Commons
import qs.Ui
import "ClipboardHistory.js" as ClipboardHistory
import "Q"

Item {
  id: root

  property string omarchyPath: Quickshell.env("OMARCHY_PATH")
  property bool opened: false
  property var screenObj: null
  property string filterText: ""
  property int selectedIndex: 0
  property bool cursorActive: false
  property bool clearConfirmOpen: false
  property var history: []

  property string historyPath: Quickshell.env("HOME") + "/.local/state/omarchy/clipboard-history.json"
  // the stock plugin's own capture script: the watchers below are the same
  // process the stock plugin starts (and kills at startup), so they never run twice
  property string captureScript: root.omarchyPath + "/shell/plugins/clipboard/capture.sh"
  // The pixel grid of the screen the overlay is on, and the card's geometry in
  // vpx: a list of one-line rows, the selected entry in full beside it.
  readonly property var g: Px.forWindow(panel)
  readonly property int pad: 6
  readonly property int listVpx: 144
  readonly property int previewVpx: 147
  readonly property int bodyRows: 13
  readonly property int rowVpx: 14
  readonly property int innerVpx: listVpx + 4 + 1 + 4 + previewVpx
  // theme tokens the logic below still reads
  property color scrim: Color.menu.scrim
  property int historyLimit: 300

  function open(payloadJson) {
    if (!root.opened) root.screenObj = Where.focusedScreen()
    root.opened = true
    root.filterText = ""
    root.selectedIndex = 0
    root.cursorActive = true
    root.disarmPointer()
    root.rebuildDisplay()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.cancelClearHistory()
    root.opened = false
  }

  function toggle() {
    if (root.opened) root.close()
    else root.open("{}")
  }

  function normalizeEntry(value) {
    return ClipboardHistory.normalizeEntry(value)
  }

  function entryKey(entry) {
    return ClipboardHistory.entryKey(entry)
  }

  function loadHistory(raw) {
    root.history = ClipboardHistory.parseHistory(raw)
    if (root.opened) root.rebuildDisplay()
  }

  function saveHistory() {
    historyFile.setText(JSON.stringify(root.history.slice(0, root.historyLimit), null, 2) + "\n")
  }

  function addClipboardEntry(entry) {
    var normalized = ClipboardHistory.normalizeEntry(entry)
    if (!normalized) return

    root.history = ClipboardHistory.addEntry(root.history, normalized, root.historyLimit)
    root.saveHistory()
    if (root.opened) root.rebuildDisplay()
  }

  function addClipboardJson(line) {
    root.addClipboardEntry(ClipboardHistory.parseEntryJson(line))
  }

  function requestClearHistory() {
    if (root.history.length === 0) return
    clearConfirm.selectedIndex = 1
    root.clearConfirmOpen = true
  }

  function cancelClearHistory() {
    root.clearConfirmOpen = false
    root.disarmPointer()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function confirmClearHistory() {
    root.history = ClipboardHistory.clearHistory()
    root.saveHistory()
    root.selectedIndex = 0
    root.cursorActive = false
    root.disarmPointer()
    root.clearConfirmOpen = false
    root.rebuildDisplay()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function removeDisplayIndex(index) {
    if (index < 0 || index >= displayModel.count) return

    var row = displayModel.get(index)
    root.history = ClipboardHistory.removeEntryAt(root.history, row.historyIndex)
    root.saveHistory()

    if (displayModel.count <= 1) {
      root.selectedIndex = 0
      root.cursorActive = false
    } else if (root.selectedIndex >= displayModel.count - 1) {
      root.selectedIndex = displayModel.count - 2
    }

    root.disarmPointer()
    root.rebuildDisplay()
  }

  function rebuildDisplay() {
    var rows = ClipboardHistory.displayRows(root.history, root.filterText, 50)

    displayModel.clear()
    for (var i = 0; i < rows.length; i++) {
      var row = rows[i]
      displayModel.append({
        entryType: row.entryType,
        fullText: row.fullText,
        previewText: row.previewText,
        previewImage: row.previewImage ? Util.fileUrl(row.previewImage) : "",
        path: row.path,
        mime: row.mime,
        historyIndex: row.index
      })
    }

    if (displayModel.count === 0) selectedIndex = 0
    else if (selectedIndex >= displayModel.count) selectedIndex = displayModel.count - 1
    else if (selectedIndex < 0) selectedIndex = 0

    Qt.callLater(function() {
      if (displayModel.count > 0) resultList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
    })
  }

  function select(delta) {
    if (displayModel.count === 0) return
    root.disarmPointer()
    if (!cursorActive) {
      cursorActive = true
      selectedIndex = delta < 0 ? displayModel.count - 1 : 0
    } else {
      selectedIndex = (selectedIndex + delta + displayModel.count) % displayModel.count
    }
    resultList.positionViewAtIndex(selectedIndex, ListView.Contain)
  }

  function selectAbsolute(index) {
    if (displayModel.count === 0) return
    root.disarmPointer()
    root.cursorActive = true
    root.selectedIndex = Math.max(0, Math.min(index, displayModel.count - 1))
    resultList.positionViewAtIndex(root.selectedIndex, ListView.Contain)
  }

  function setFilter(nextFilter) {
    root.filterText = nextFilter
    root.selectedIndex = 0
    root.cursorActive = true
    root.disarmPointer()
    root.rebuildDisplay()
  }

  function disarmPointer() {
    pointerGate.reset()
  }

  function selectFromPointer(index, item, mouse) {
    if (!pointerGate.moved(item, mouse)) return
    root.cursorActive = true
    root.selectedIndex = index
  }

  function activateIndex(index) {
    if (index < 0 || index >= displayModel.count) return
    var row = displayModel.get(index)
    root.applySelected(row)
  }

  function copyIndex(index) {
    if (index < 0 || index >= displayModel.count) return
    var row = displayModel.get(index)
    root.copySelected(row)
  }

  function openIndex(index) {
    if (index < 0 || index >= displayModel.count) return
    var row = displayModel.get(index)
    root.openSelected(row)
  }

  function applySelected(row) {
    if (!row) return
    root.opened = false
    if (row.entryType === "image") {
      Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-clipboard-paste-file", row.mime, row.path])
    } else if (row.fullText) {
      Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-clipboard-paste-text", "--shift-insert", "--history-index", String(row.historyIndex)])
    }
  }

  function copySelected(row) {
    if (!row) return
    root.opened = false
    if (row.entryType === "image") {
      Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-clipboard-paste-file", "--copy-only", row.mime, row.path])
    } else if (row.fullText) {
      Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-clipboard-paste-text", "--copy-only", "--history-index", String(row.historyIndex)])
    }
  }

  function openSelected(row) {
    if (!row) return
    root.opened = false
    Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-clipboard-open", "--history-index", String(row.historyIndex)])
  }

  Component.onCompleted: initProc.running = true

  ListModel { id: displayModel }

  PointerMoveGate {
    id: pointerGate
    referenceItem: card
  }

  FileView {
    id: historyFile
    path: root.historyPath
    watchChanges: true
    atomicWrites: true
    printErrors: false
    onLoaded: root.loadHistory(text())
    onLoadFailed: root.loadHistory("[]")
    onFileChanged: reload()
  }

  // Reap watchers left behind by a previous shell instance, then start our
  // own. The pdeathsig on the watchers makes the kernel kill them whenever
  // the shell exits, however it exits, so no further lifecycle management.
  Process {
    id: initProc
    command: ["pkill", "-f", "wl-paste .*--watch .*/shell/plugins/clipboard/capture\\.sh"]
    onExited: {
      currentProc.running = true
      textWatchProc.running = true
      imageWatchProc.running = true
    }
  }

  Process {
    id: currentProc
    command: [root.captureScript]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.addClipboardJson(text)
    }
  }

  Process {
    id: textWatchProc
    command: ["setpriv", "--pdeathsig", "TERM", "wl-paste", "--type", "text", "--watch", root.captureScript, "text"]
    onExited: watchRestartTimer.restart()
    stdout: SplitParser {
      onRead: function(data) { root.addClipboardJson(data) }
    }
  }

  Process {
    id: imageWatchProc
    command: ["setpriv", "--pdeathsig", "TERM", "wl-paste", "--type", "image/png", "--watch", root.captureScript, "image/png"]
    onExited: watchRestartTimer.restart()
    stdout: SplitParser {
      onRead: function(data) { root.addClipboardJson(data) }
    }
  }

  // A watcher that dies takes clipboard history with it, silently: copying still
  // works, the picker still opens, and the old entries are all still there, so
  // nothing recorded until the next shell reload. Bring it back instead.
  Timer {
    id: watchRestartTimer
    interval: 1000
    repeat: false
    onTriggered: {
      if (!textWatchProc.running) textWatchProc.running = true
      if (!imageWatchProc.running) imageWatchProc.running = true
    }
  }

  // `text` cut to `columns` cells: an ellipsis at the end, no space before it
  function fitCells(text, columns) {
    var chars = Array.from(String(text || ""))
    if (chars.length <= columns) return chars.join("")
    if (columns < 2) return ""
    return chars.slice(0, columns - 1).join("").replace(/\s+$/, "") + "…"
  }

  // One line of an entry, for its row.
  function oneLine(text) {
    return String(text || "").replace(/[\t\r\n]+/g, " ").replace(/ +/g, " ").replace(/^ | $/g, "")
  }

  // A whole-row scroll: the list's top is always a multiple of a row.
  function scrollRows(rows) {
    var top = resultList.originY
    var bottom = Math.max(top, resultList.originY + resultList.contentHeight - resultList.height)
    resultList.contentY = Math.max(top, Math.min(bottom, resultList.contentY + rows * resultList.rowHeight))
  }

  Settle { id: settle; window: panel }
  // the card was hidden for a frame or two: take the keyboard again once it shows
  Connections {
    target: settle
    function onReadyChanged() { if (settle.ready) Qt.callLater(function() { keyCatcher.forceActiveFocus() }) }
  }

  PanelWindow {
    id: panel
    screen: root.screenObj
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "omarchy-clipboard"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    Scrim {
      visible: settle.ready
      tone: Qt.rgba(root.scrim.r, root.scrim.g, root.scrim.b, 1)
      density: root.scrim.a
    }

    MouseArea {
      anchors.fill: parent
      onClicked: root.close()
    }

    Pane {
      id: card
      visible: settle.ready
      pad: root.pad
      width: root.g.px(root.innerVpx + 2 * (root.pad + 1))
      // prompt, rule, body, footer
      height: root.g.px(12 + 3 + 1 + 3 + root.bodyRows * root.rowVpx + 3 + 12 + 2 * (root.pad + 1))
      x: root.g.centre(parent.width, width)
      y: root.g.centre(parent.height, height)

      Item {
        id: keyCatcher
        anchors.fill: parent
        focus: true

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          if (root.clearConfirmOpen) {
            if (clearConfirm.handleKey(event)) event.accepted = true
            return
          }

          if (event.key === Qt.Key_Escape) {
            if (root.filterText) root.setFilter("")
            else root.close()
            event.accepted = true
          } else if (Util.editsFilter(event, root.filterText)) {
            root.setFilter(Util.editedFilter(event, root.filterText))
            event.accepted = true
          } else if (event.key === Qt.Key_Delete) {
            if (event.modifiers & Qt.ShiftModifier) root.requestClearHistory()
            else root.removeDisplayIndex(root.selectedIndex)
            event.accepted = true
          } else if (event.key === Qt.Key_Up) {
            root.select(-1)
            event.accepted = true
          } else if (event.key === Qt.Key_Down) {
            root.select(1)
            event.accepted = true
          } else if (event.key === Qt.Key_PageUp) {
            root.select(-6)
            event.accepted = true
          } else if (event.key === Qt.Key_PageDown) {
            root.select(6)
            event.accepted = true
          } else if (event.key === Qt.Key_Home) {
            root.selectAbsolute(0)
            event.accepted = true
          } else if (event.key === Qt.Key_End) {
            root.selectAbsolute(displayModel.count - 1)
            event.accepted = true
          } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (root.cursorActive && (event.modifiers & Qt.AltModifier)) root.openIndex(root.selectedIndex)
            else if (root.cursorActive && (event.modifiers & Qt.ShiftModifier)) root.copyIndex(root.selectedIndex)
            else if (root.cursorActive) root.activateIndex(root.selectedIndex)
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
        prompt: "Search clipboard…"
      }
      Hairline {
        y: root.g.px(12 + 3)
        width: card.inner.width
      }

      // ---- the list
      Item {
        id: listBox
        y: root.g.px(12 + 3 + 1 + 3)
        width: root.g.px(root.listVpx)
        height: root.g.px(root.bodyRows * root.rowVpx)
        clip: true

        ListView {
          id: resultList
          anchors.fill: parent
          anchors.rightMargin: root.g.px(3)
          readonly property real rowHeight: root.g.px(root.rowVpx)
          model: displayModel
          interactive: false
          spacing: 0
          boundsBehavior: Flickable.StopAtBounds

          delegate: Item {
            id: row
            required property int index
            required property string entryType
            required property string previewText
            required property string fullText
            required property string previewImage

            readonly property bool hasCursor: root.cursorActive && index === root.selectedIndex

            width: ListView.view.width
            height: resultList.rowHeight

            Rectangle {
              anchors.fill: parent
              visible: row.hasCursor
              color: Role.accent
              antialiasing: false
            }
            Sprite {
              x: root.g.px(3)
              y: root.g.onCaps(7) + root.g.px(1)
              rows: row.entryType === "image" ? Pictograms.image : (row.entryType === "file" ? Pictograms.file : Pictograms.text)
              color: row.hasCursor ? Role.onAccent : Role.muted
            }
            PixelText {
              x: root.g.px(3 + 7 + 3)
              y: root.g.px(1)
              text: root.fitCells(root.oneLine(row.previewText), Math.floor((row.width - x) / root.g.cellW) - 1)
              ink: row.hasCursor ? Role.onAccent : (row.entryType === "image" || row.entryType === "file" ? Role.muted : Role.ink)
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onPositionChanged: function(mouse) { root.selectFromPointer(row.index, row, mouse) }
              onClicked: {
                root.cursorActive = true
                root.selectedIndex = row.index
                root.activateIndex(row.index)
              }
            }
          }

          WheelHandler {
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: function(event) {
              var dir = Math.sign(event.angleDelta.y)
              if (dir !== 0) root.scrollRows(-dir * 3)
            }
          }
        }

        // the scroll mark, as the popups have it: a rail and a thumb, one pixel wide
        Rectangle {
          visible: resultList.contentHeight > resultList.height
          x: parent.width - root.g.hair
          width: root.g.hair
          height: parent.height
          color: Role.edge
          antialiasing: false
        }
        Rectangle {
          visible: resultList.contentHeight > resultList.height
          x: parent.width - root.g.hair
          width: root.g.hair
          height: Math.max(root.g.px(6), root.g.snap(parent.height * parent.height / Math.max(1, resultList.contentHeight)))
          y: root.g.snap((parent.height - height) * (resultList.contentHeight > resultList.height
            ? (resultList.contentY - resultList.originY) / (resultList.contentHeight - resultList.height) : 0))
          color: Role.muted
          antialiasing: false
        }

        // empty
        Row {
          visible: displayModel.count === 0
          x: root.g.px(3)
          y: root.g.px(1)
          spacing: root.g.px(3)
          Sprite {
            rows: root.history.length === 0 ? Pictograms.clipboard : Pictograms.search
            color: Role.muted
            anchors.verticalCenter: parent.verticalCenter
          }
          PixelText {
            text: root.history.length === 0 ? "Clipboard is empty" : "No matches"
            ink: Role.muted
          }
        }
      }

      // the rule between list and preview
      Hairline {
        vertical: true
        x: root.g.px(root.listVpx + 4)
        y: listBox.y
        height: listBox.height
      }

      // ---- the selected entry, in full
      Item {
        id: preview
        x: root.g.px(root.listVpx + 4 + 1 + 4)
        y: listBox.y
        width: root.g.px(root.previewVpx)
        height: listBox.height
        clip: true

        readonly property var active: displayModel.count > 0 && root.selectedIndex >= 0 && root.selectedIndex < displayModel.count
          ? displayModel.get(root.selectedIndex) : null

        PixelParagraph {
          visible: preview.active !== null && !preview.active.previewImage
          text: preview.active ? String(preview.active.fullText).replace(/\t/g, "  ") : ""
          columns: Math.floor(preview.width / root.g.cellW)
          maxLines: root.bodyRows * 14 / 12 | 0
          ink: Role.ink
        }

        // an image is a picture, not part of the interface: drawn smooth, fitted
        Image {
          visible: preview.active !== null && preview.active.previewImage.length > 0
          anchors.fill: parent
          source: preview.active ? preview.active.previewImage : ""
          sourceSize.width: Math.round(width * root.g.dpr)
          sourceSize.height: Math.round(height * root.g.dpr)
          fillMode: Image.PreserveAspectFit
          horizontalAlignment: Image.AlignLeft
          verticalAlignment: Image.AlignTop
          asynchronous: true
          smooth: true
        }
      }

      // ---- the keys: the longest line that fits, else none (dropped, not cut)
      PixelText {
        y: root.g.px(12 + 3 + 1 + 3) + listBox.height + root.g.px(3)
        readonly property int cols: Math.floor(card.inner.width / root.g.cellW + 1e-6)
        readonly property var tiers: [
          "ENTER paste  S-ENTER copy  A-ENTER open  DEL drop  S-DEL clear",
          "ENTER paste  S-ENTER copy  A-ENTER open  DEL drop",
          "ENTER paste  S-ENTER copy  DEL drop",
          "ENTER paste  DEL drop"
        ]
        text: {
          for (var i = 0; i < tiers.length; i++) if (tiers[i].length <= cols) return tiers[i]
          return ""
        }
        ink: Role.faint
      }
    }

    // the card eats clicks; the question over it
    MouseArea { x: card.x; y: card.y; width: card.width; height: card.height; onClicked: {} }

    Confirm {
      id: clearConfirm
      x: card.x
      y: card.y
      width: card.width
      height: card.height
      opened: root.clearConfirmOpen
      message: "Delete entire clipboard history?"
      confirmText: "Delete"
      onCanceled: root.cancelClearHistory()
      onConfirmed: root.confirmClearHistory()
    }
  }
}
