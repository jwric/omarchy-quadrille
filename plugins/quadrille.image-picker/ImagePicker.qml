import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import qs.Commons
import "ImagePickerModel.js" as ImagePickerModel
import "Q"

Item {
  id: root

  // Injected by omarchy-shell; defaults to the session OMARCHY_PATH.
  property string omarchyPath: Quickshell.env("OMARCHY_PATH")
  property string stateHome: Quickshell.env("HOME") + "/.local/state"
  property string imageDirs: Quickshell.env("OMARCHY_IMAGE_SELECTOR_DIRS") || Quickshell.env("OMARCHY_IMAGE_SELECTOR_DIR") || Quickshell.env("OMARCHY_STOCK_BACKGROUNDS_DIR") || (stateHome + "/omarchy/current/theme/backgrounds")
  property string imageRows: ""
  property string loadedImageRows: ""
  property string selectionFile: Quickshell.env("OMARCHY_IMAGE_SELECTOR_SELECTION_FILE") || Quickshell.env("OMARCHY_BACKGROUND_SELECTION_FILE")
  property string selectedImage: Quickshell.env("OMARCHY_IMAGE_SELECTOR_SELECTED")
  property int selectedIndex: 0
  property bool imagesLoaded: false
  property bool opened: false
  property var screenObj: null
  property bool showLabels: false
  property bool filterable: false
  property bool layoutSettled: false
  property bool requestActive: false
  property int requestSerial: 0
  property int applySerial: 0
  property string doneFile: ""
  property string filterText: ""
  property var doneFilesToRelease: []
  // Bound to the central [image-picker] section in shell.toml via Color.qml.
  // `dimColor` tints unselected slices and text outlines on top of the scrim;
  // it intentionally tracks the foundational background, not a surface role.
  property color dimColor: Color.background
  property color foreground: Color.imagePicker.text
  property color scrim: Color.imagePicker.scrim
  property color selectedBorder: Color.imagePicker.selectedBorder
  property color unselectedBorder: Color.imagePicker.unselectedBorder
  // The grid, in vpx: thumbnails of 112 x 48 (a wallpaper's 2.4 : 1), three to a
  // row, the name under each when it fits.
  readonly property var g: Px.forWindow(panel)
  readonly property int pad: 6
  readonly property int columns: 3
  readonly property int thumbW: 112
  readonly property int thumbH: 48
  readonly property int cellW: thumbW + 8
  readonly property int cellH: thumbH + 8 + (showLabels ? 14 : 0)
  readonly property int visibleRows: 4
  readonly property int innerVpx: columns * cellW

  onOpenedChanged: if (!opened) layoutSettled = false

  function scriptPath(name) {
    return omarchyPath + "/shell/plugins/image-picker/" + name
  }

  function focusPicker() {
    if (root.opened && root.imagesLoaded && root.layoutSettled)
      keyCatcher.forceActiveFocus()
  }

  function revealWhenSettled(serial) {
    Qt.callLater(function() {
      if (serial === root.requestSerial && root.opened && root.imagesLoaded && root.imageArray.length > 0) {
        root.layoutSettled = true
        root.focusPicker()
      }
    })
  }

  function currentPath() {
    if (imageArray.length === 0 || !itemMatches(selectedIndex)) return ""
    return imageArray[selectedIndex].filePath
  }

  function nameForPath(path) {
    return ImagePickerModel.nameForPath(path)
  }

  function labelForPath(path) {
    return ImagePickerModel.labelForPath(path)
  }

  function currentLabel() {
    var path = currentPath()
    if (!path) return filterText ? "No matches" : ""

    return labelForPath(path)
  }

  function itemMatches(index) {
    return ImagePickerModel.itemMatches(imageArray, index, filterText)
  }

  function firstMatchingIndex() {
    return ImagePickerModel.firstMatchingIndex(imageArray, filterText)
  }

  function filteredPosition(index) {
    return ImagePickerModel.filteredPosition(imageArray, index, filterText)
  }

  function selectedFilteredPosition() {
    return ImagePickerModel.selectedFilteredPosition(imageArray, selectedIndex, filterText)
  }

  function select(index, immediate) {
    if (imageArray.length === 0) return
    if (index < 0) index = 0
    else if (index >= imageArray.length) index = imageArray.length - 1
    if (!itemMatches(index)) return
    if (index === selectedIndex && immediate !== true) return

    selectedIndex = index
  }

  function selectAdjacent(direction) {
    var count = imageArray.length
    if (count === 0) return

    var index = selectedIndex
    for (var i = 0; i < count; i++) {
      index = (index + direction + count) % count
      if (itemMatches(index)) {
        select(index)
        return
      }
    }
  }

  function updateFilter(nextFilterText) {
    filterText = nextFilterText

    if (!itemMatches(selectedIndex)) {
      var first = ImagePickerModel.nextSelectedIndexForFilter(imageArray, selectedIndex, filterText)
      if (first >= 0) selectedIndex = first
    }
  }

  function releaseNextDoneFile() {
    if (releaseProc.running || doneFilesToRelease.length === 0) return

    var path = doneFilesToRelease.shift()
    releaseProc.command = ["bash", "-c", ": > " + Util.shellQuote(path)]
    releaseProc.running = true
  }

  function finishDoneFile(path) {
    if (!path) return
    doneFilesToRelease.push(path)
    releaseNextDoneFile()
  }

  function applySelected() {
    var path = currentPath()
    if (!path || !selectionFile) {
      cancel()
      return
    }

    var activeSelectionFile = selectionFile
    var activeDoneFile = doneFile
    applySerial = requestSerial
    requestActive = false
    selectionFile = ""
    doneFile = ""

    applyProc.command = ["bash", "-c", "printf '%s\\n' " + Util.shellQuote(path) + " > " + Util.shellQuote(activeSelectionFile) + "; : > " + Util.shellQuote(activeDoneFile)]
    applyProc.running = true
  }

  function cancel() {
    if (requestActive)
      finishDoneFile(doneFile)

    requestActive = false
    selectionFile = ""
    doneFile = ""
    root.opened = false
  }

  function closeSelector(nextDoneFile) {
    requestSerial += 1

    if (requestActive)
      finishDoneFile(doneFile)

    if (nextDoneFile && nextDoneFile !== doneFile)
      finishDoneFile(nextDoneFile)

    requestActive = false
    selectionFile = ""
    doneFile = ""
    filterText = ""
    root.opened = false
  }

  function loadRows(rows, reveal) {
    var newImages = ImagePickerModel.loadRows(rows)

    root.loadedImageRows = rows
    root.selectedIndex = root.indexForSelectedImage(newImages)
    root.imageArray = newImages
    root.imagesLoaded = true

    if (reveal !== false) {
      root.opened = true
      root.revealWhenSettled(root.requestSerial)
    }
  }

  function openSelector(nextImageDirs, nextImageRows, nextSelectedImage, nextSelectionFile, nextDoneFile, nextShowLabels, nextFilterable) {
    if (!opened) screenObj = Px.focusedScreen()
    if (requestActive && doneFile && doneFile !== nextDoneFile)
      finishDoneFile(doneFile)

    requestSerial += 1

    imageDirs = nextImageDirs
    imageRows = nextImageRows
    selectedImage = nextSelectedImage
    selectionFile = nextSelectionFile
    doneFile = nextDoneFile
    requestActive = !!doneFile
    showLabels = nextShowLabels === true || nextShowLabels === "true"
    filterable = nextFilterable === true || nextFilterable === "true"
    filterText = ""
    layoutSettled = false

    if (imageRows && imageRows === loadedImageRows && imageArray.length > 0) {
      root.select(root.selectedImageIndex(), true)
      imagesLoaded = true
      opened = true
      root.revealWhenSettled(requestSerial)
      return
    }

    if (imageRows) {
      var rowsToLoad = imageRows
      var rowsSerial = requestSerial
      imageArray = []
      selectedIndex = 0
      imagesLoaded = true
      opened = true
      Qt.callLater(function() {
        if (rowsSerial === root.requestSerial)
          root.loadRows(rowsToLoad, true)
      })
      return
    }

    imageArray = []
    selectedIndex = 0
    imagesLoaded = false
    opened = false
    startImageScan(requestSerial, imageDirs)
  }

  property var imageArray: []

  function startImageScan(serial, dirs) {
    if (loadImagesProc.running) {
      loadImagesProc.queuedSerial = serial
      loadImagesProc.queuedDirs = dirs
      return
    }

    loadImagesProc.activeSerial = serial
    loadImagesProc.queuedSerial = 0
    loadImagesProc.queuedDirs = ""
    loadImagesProc.command = [root.scriptPath("list.sh"), dirs]
    loadImagesProc.running = true
  }

  function indexForSelectedImage(images) {
    return ImagePickerModel.indexForSelectedImage(images, selectedImage)
  }

  function selectedImageIndex() {
    return indexForSelectedImage(imageArray)
  }

  Process {
    id: loadImagesProc
    property int activeSerial: 0
    property int queuedSerial: 0
    property string queuedDirs: ""
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        if (loadImagesProc.activeSerial === root.requestSerial)
          root.loadRows(String(text || ""), true)
      }
    }
    onExited: {
      var serial = queuedSerial
      var dirs = queuedDirs
      activeSerial = 0
      queuedSerial = 0
      queuedDirs = ""
      if (serial > 0 && serial === root.requestSerial)
        root.startImageScan(serial, dirs)
    }
  }

  // Lifecycle hooks invoked by omarchy-shell summon/hide. shell.summon(id,
  // payloadJson) hands the JSON to open() here; shell.hide(id) calls close().
  // The shell host owns the stable `image-selector` IPC target and forwards
  // those lower-level positional calls here.
  function open(payload) {
    var args = {}
    if (payload) {
      try { args = JSON.parse(payload) || {} } catch (e) { args = {} }
    }
    var dirs = String(args.imageDirs || imageDirs)
    var rows = String(args.imageRows || "")
    var sel = String(args.selectedImage || selectedImage)
    var selFile = String(args.selectionFile || "")
    var doneF = String(args.doneFile || "")
    var labels = args.showLabels === true || args.showLabels === "true"
    var filter = args.filterable === true || args.filterable === "true"
    openSelector(dirs, rows, sel, selFile, doneF, labels, filter)
  }

  function close() {
    cancel()
  }

  function preloadRows(nextImageRows, nextSelectedImage, nextShowLabels, nextFilterable) {
    // Theme/background set hooks can warm selector rows after a picker was
    // dismissed. Ignore those preloads while a user-visible request is open;
    // otherwise the preload resets layoutSettled without revealing again,
    // leaving only the fullscreen scrim.
    if (opened || requestActive) return

    requestSerial += 1
    imageRows = nextImageRows
    selectedImage = nextSelectedImage
    showLabels = nextShowLabels === true || nextShowLabels === "true"
    filterable = nextFilterable === true || nextFilterable === "true"
    filterText = ""
    layoutSettled = false

    if (imageRows && imageRows === loadedImageRows && imageArray.length > 0) {
      selectedIndex = selectedImageIndex()
      imagesLoaded = true
    } else if (imageRows) {
      loadRows(imageRows, false)
    }
  }

  Process {
    id: applyProc
    onExited: {
      if (root.applySerial === root.requestSerial)
        root.opened = false
    }
  }

  Process {
    id: releaseProc
    onExited: root.releaseNextDoneFile()
  }

  // ---- the grid's own logic: what matches the filter, in order, and where the
  // selection is among them

  property var shown: []   // indices into imageArray that match the filter

  function rebuildShown() {
    var out = []
    for (var i = 0; i < imageArray.length; i++) if (itemMatches(i)) out.push(i)
    shown = out
  }
  onImageArrayChanged: rebuildShown()
  onFilterTextChanged: { rebuildShown(); Qt.callLater(function() { root.reveal() }) }
  onSelectedIndexChanged: Qt.callLater(function() { root.reveal() })

  function selectedPosition() { return shown.indexOf(selectedIndex) }

  function selectPosition(pos) {
    if (shown.length === 0) return
    pos = Math.max(0, Math.min(shown.length - 1, pos))
    selectedIndex = shown[pos]
  }

  // a step on the grid: dx along a row, dy by whole rows; the ends stop
  function selectGrid(dx, dy) {
    if (shown.length === 0) return
    var pos = selectedPosition()
    if (pos < 0) { selectPosition(0); return }
    selectPosition(pos + dx + dy * columns)
  }

  // Scroll to the row of the selection: whole rows only.
  function reveal() {
    if (shown.length === 0) { grid.contentY = 0; return }
    var row = Math.floor(Math.max(0, selectedPosition()) / columns)
    var top = Math.round(grid.contentY / grid.cellHeight)
    if (row < top) top = row
    else if (row >= top + visibleRows) top = row - visibleRows + 1
    var maxTop = Math.max(0, Math.ceil(shown.length / columns) - visibleRows)
    grid.contentY = Math.max(0, Math.min(maxTop, top)) * grid.cellHeight
  }

  // `text` cut to `columns` cells: an ellipsis at the end, no space before it
  function fitCells(text, cols) {
    var chars = Array.from(String(text || ""))
    if (chars.length <= cols) return chars.join("")
    if (cols < 2) return ""
    return chars.slice(0, cols - 1).join("").replace(/\s+$/, "") + "…"
  }

  Settle { id: settle; window: panel }
  // silent unless the shell runs with QUADRILLE_DEBUG_SURFACES=1: every change of size,
  // visibility and device ratio, with a clock, of the window and of the card
  SurfaceProbe { window: panel; tag: "picker.window" }
  SurfaceProbe { window: card; tag: "picker.card" }
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
    WlrLayershell.namespace: "omarchy-image-selector"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: root.opened && root.imagesLoaded ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore

    Scrim {
      visible: settle.ready && root.opened && root.imagesLoaded
      tone: Qt.rgba(root.scrim.r, root.scrim.g, root.scrim.b, 1)
      density: root.scrim.a
    }

    MouseArea {
      anchors.fill: parent
      enabled: root.opened && root.imagesLoaded
      onClicked: root.cancel()
    }

    Pane {
      id: card
      visible: settle.ready && root.opened && root.imagesLoaded && root.layoutSettled && root.imageArray.length > 0
      pad: root.pad
      width: root.g.px(root.innerVpx + 2 * (root.pad + 1))
      // header, rule, the rows, and the filter line when there is one
      height: root.g.px(12 + 3 + 1 + 3 + root.visibleRows * root.cellH + 2 * (root.pad + 1))
      x: root.g.centre(parent.width, width)
      y: root.g.centre(parent.height, height)

      Item {
        id: keyCatcher
        anchors.fill: parent
        focus: true

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          if (event.key === Qt.Key_Escape) {
            if (root.filterText) root.updateFilter("")
            else root.cancel()
            event.accepted = true
          } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.applySelected()
            event.accepted = true
          } else if (root.filterable && Util.editsFilter(event, root.filterText)) {
            root.updateFilter(Util.editedFilter(event, root.filterText))
            event.accepted = true
          } else if (event.key === Qt.Key_Left) {
            root.selectGrid(-1, 0)
            event.accepted = true
          } else if (event.key === Qt.Key_Right) {
            root.selectGrid(1, 0)
            event.accepted = true
          } else if (event.key === Qt.Key_Up) {
            root.selectGrid(0, -1)
            event.accepted = true
          } else if (event.key === Qt.Key_Down) {
            root.selectGrid(0, 1)
            event.accepted = true
          } else if ((event.key === Qt.Key_Tab && event.modifiers & Qt.ShiftModifier) || event.key === Qt.Key_Backtab) {
            root.selectAdjacent(-1)
            event.accepted = true
          } else if (event.key === Qt.Key_Tab) {
            root.selectAdjacent(1)
            event.accepted = true
          } else if (event.key === Qt.Key_PageUp) {
            root.selectGrid(0, -root.visibleRows)
            event.accepted = true
          } else if (event.key === Qt.Key_PageDown) {
            root.selectGrid(0, root.visibleRows)
            event.accepted = true
          } else if (event.key === Qt.Key_Home) {
            root.selectPosition(0)
            event.accepted = true
          } else if (event.key === Qt.Key_End) {
            root.selectPosition(root.shown.length - 1)
            event.accepted = true
          } else if (root.filterable && event.text && event.text.length === 1 && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127 && (event.modifiers === Qt.NoModifier || event.modifiers === Qt.ShiftModifier)) {
            root.updateFilter(root.filterText + event.text)
            event.accepted = true
          }
        }
        Component.onCompleted: forceActiveFocus()
      }

      // the header: what is selected (or what is being typed), and where
      Item {
        id: header
        width: card.inner.width
        height: root.g.line

        PixelText {
          id: position
          x: header.width - width
          visible: root.shown.length > 0
          text: (root.selectedPosition() + 1) + "/" + root.shown.length
          ink: Role.faint
        }
        Prompt {
          visible: root.filterable && root.filterText.length > 0
          width: header.width - position.width - root.g.px(4)
          text: root.filterText
          prompt: ""
        }
        PixelText {
          visible: !(root.filterable && root.filterText.length > 0)
          text: root.currentLabel() || (root.filterable ? "Type to filter…" : "")
          ink: root.currentPath() ? Role.ink : Role.muted
          columns: Math.floor((header.width - position.width) / root.g.cellW) - 1
        }
      }
      Hairline {
        y: root.g.px(12 + 3)
        width: card.inner.width
      }

      // the thumbnails
      Item {
        id: gridBox
        y: root.g.px(12 + 3 + 1 + 3)
        width: card.inner.width
        height: root.g.px(root.visibleRows * root.cellH)
        clip: true

        GridView {
          id: grid
          anchors.fill: parent
          model: root.shown
          interactive: false
          cellWidth: root.g.px(root.cellW)
          cellHeight: root.g.px(root.cellH)
          boundsBehavior: Flickable.StopAtBounds
          cacheBuffer: 0

          delegate: Item {
            id: cell
            required property int index
            required property int modelData

            readonly property var imageData: root.imageArray[modelData]
            readonly property bool selected: modelData === root.selectedIndex

            width: grid.cellWidth
            height: grid.cellHeight

            // the picture, filling its box
            Rectangle {
              x: root.g.px(4)
              y: root.g.px(4)
              width: root.g.px(root.thumbW)
              height: root.g.px(root.thumbH)
              color: Role.void_
              antialiasing: false
              Image {
                anchors.fill: parent
                source: cell.imageData && cell.imageData.thumbnailPath ? Util.fileUrl(cell.imageData.thumbnailPath) : ""
                sourceSize.width: Math.round(width * root.g.dpr)
                sourceSize.height: Math.round(height * root.g.dpr)
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                smooth: true
              }
            }
            Brackets {
              visible: cell.selected
              x: root.g.px(1)
              y: root.g.px(1)
              width: root.g.px(root.thumbW + 6)
              height: root.g.px(root.thumbH + 6)
              anchors.fill: undefined
              arm: 4
              color: Role.accent
            }
            // the name, when it fits under the thumbnail: dropped, never cut
            PixelText {
              visible: root.showLabels
              x: root.g.px(4)
              y: root.g.px(4 + root.thumbH + 3)
              text: cell.imageData ? root.labelForPath(cell.imageData.filePath) : ""
              room: root.g.px(root.thumbW)
              ink: cell.selected ? Role.ink : Role.muted
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: false
              cursorShape: Qt.PointingHandCursor
              onClicked: cell.selected ? root.applySelected() : root.select(cell.modelData)
            }
          }

          WheelHandler {
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: function(event) {
              var dir = Math.sign(event.angleDelta.y)
              if (dir === 0) return
              var maxTop = Math.max(0, Math.ceil(root.shown.length / root.columns) - root.visibleRows)
              var top = Math.round(grid.contentY / grid.cellHeight) - dir
              grid.contentY = Math.max(0, Math.min(maxTop, top)) * grid.cellHeight
            }
          }
        }

        // nothing matches
        Row {
          visible: root.shown.length === 0
          x: root.g.px(4)
          y: root.g.px(2)
          spacing: root.g.px(3)
          Sprite { rows: Pictograms.search; color: Role.muted; anchors.verticalCenter: parent.verticalCenter }
          PixelText { text: "No matches"; ink: Role.muted }
        }

        // a hairline at an edge with more behind it
        Hairline { visible: grid.contentY > 0; width: parent.width }
        Hairline {
          visible: root.shown.length > 0
            && Math.round(grid.contentY / grid.cellHeight) + root.visibleRows < Math.ceil(root.shown.length / root.columns)
          y: parent.height - height
          width: parent.width
        }
      }
    }

    // the card eats clicks: only the dither outside it dismisses
    MouseArea { x: card.x; y: card.y; width: card.width; height: card.height; visible: card.visible; onClicked: {} }
  }
}
