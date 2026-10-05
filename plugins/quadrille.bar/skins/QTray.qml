import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.SystemTray
import qs.Commons
import qs.Ui
import "../Q"
import "TrayModel.js" as TrayModel

// Replaces omarchy.tray: the status-notifier icons as pixel art.
//
//   * Every icon is a PixelIcon: 11 x 11 virtual pixels, box-filtered, flat
//     palette (a symbolic or white-on-clear one in ink), whole device pixels.
//   * The icons you pinned sit in the bar. The rest, and any pinned ones the bar
//     has no room for (`room`, set by the bar from what the other sections need),
//     are behind a chevron: click it for a list, right-click it to pin or hide.
//   * Left click activates (or opens the menu of an item that is only a menu),
//     middle click is the secondary action, right click opens the item's menu
//     (drawn by TrayMenu), the wheel scrolls it. An item that needs attention
//     carries a lit pixel block.
//
// The stock widget reveals its drawer on hover, over an animation, in a
// reserved width; this one has no reserved width to run out of, so it cannot
// reach the clock or the status icons.
BarWidget {
  id: root
  moduleName: "omarchy.tray"

  readonly property var g: Px.of(root)

  // The bar sets this: the widest the widget may be, in logical px.
  property real room: 1e9
  readonly property bool elastic: true

  readonly property var pinnedIds: settings.pinned instanceof Array ? settings.pinned : []
  readonly property var hiddenIds: settings.hidden instanceof Array ? settings.hidden : []

  function ownedByOmarchy(item) {
    var layout = root.bar && root.bar.layoutConfig ? root.bar.layoutConfig : null
    return TrayModel.ownedByOmarchy(item, layout)
  }
  function bucket(kind) {
    var values = SystemTray.items.values
    var out = []
    for (var i = 0; i < values.length; i++) {
      var item = values[i]
      if (item.status === Status.Passive || ownedByOmarchy(item)) continue
      var id = String(item.id || "")
      var klass = hiddenIds.indexOf(id) !== -1 ? "hidden" : (pinnedIds.indexOf(id) !== -1 ? "pinned" : "drawer")
      if (kind === "all" || klass === kind) out.push(item)
    }
    return out
  }
  readonly property var pinned: bucket("pinned")
  readonly property var drawer: bucket("drawer")
  readonly property var everything: bucket("all")

  // 15 vpx a pinned icon (an 11-vpx icon and two of air a side), 11 for the chevron.
  readonly property real cellW: g.px(15)
  readonly property real chevronW: g.px(11)
  // How many pinned icons the bar shows, and whether the chevron is there: with
  // the room there is, all of them (and the chevron only if something else is
  // behind it), else as many as fit beside the chevron that must then hold the rest.
  readonly property var fit: {
    var total = pinned.length
    var cap = Math.max(0, Math.floor((room - (drawer.length > 0 ? chevronW : 0)) / cellW + 1e-6))
    if (total <= cap) return { shown: total, chevron: drawer.length > 0 }
    return { shown: Math.max(0, Math.floor((room - chevronW) / cellW + 1e-6)), chevron: true }
  }
  readonly property int shown: fit.shown
  readonly property var inlineItems: pinned.slice(0, shown)
  readonly property var behind: drawer.concat(pinned.slice(shown))
  readonly property bool hasChevron: fit.chevron

  function persist(pinnedIds, hiddenIds) {
    if (!root.bar || !root.bar.shell || typeof root.bar.shell.updateEntryInline !== "function") return
    var id = root.moduleName || "omarchy.tray"
    root.bar.shell.updateEntryInline(id, { id: id, pinned: pinnedIds, hidden: hiddenIds })
  }
  function toggle(list, id) {
    var next = list.slice()
    var at = next.indexOf(id)
    if (at !== -1) next.splice(at, 1); else next.push(id)
    return next
  }
  function togglePin(id) {
    var p = toggle(pinnedIds, id), h = hiddenIds.slice()
    if (p.indexOf(id) !== -1 && h.indexOf(id) !== -1) h.splice(h.indexOf(id), 1)
    persist(p, h)
  }
  function toggleHide(id) {
    var h = toggle(hiddenIds, id), p = pinnedIds.slice()
    if (h.indexOf(id) !== -1 && p.indexOf(id) !== -1) p.splice(p.indexOf(id), 1)
    persist(p, h)
  }
  function tip(item) { return item.tooltipTitle || item.title || item.id || "" }
  function nameOf(item) {
    var t = String(item.title || "").trim()
    if (t) return t
    var tt = String(item.tooltipTitle || "").trim()
    if (tt) return tt
    var id = String(item.id || "")
    var slash = id.lastIndexOf("/")
    return slash !== -1 ? id.substring(slash + 1) : (id || "Unknown")
  }
  function iconSilhouette(item) {
    return String(item.icon || "").split("?")[0].slice(-9) === "-symbolic"
  }

  function close() { drawerPopup.open = false; menuPopup.open = false; managePopup.open = false }

  // `omarchy-shell quadrille.tray drawer | manage | menu <i> | close`: the same,
  // from a keybinding or a script, on every bar's tray.
  IpcHandler {
    target: "quadrille.tray"
    function drawer(): void { root.broadcast("showDrawer") }
    function manage(): void { root.broadcast("showManage") }
    function menu(index: int): void {
      var items = root.bar && typeof root.bar.moduleWidgets === "function" ? root.bar.moduleWidgets(root.moduleName) : [root]
      for (var i = 0; i < items.length; i++) if (items[i] && typeof items[i].showMenu === "function") items[i].showMenu(index)
    }
    function close(): void { root.broadcast("close") }
  }

  // For trying it without a pointer: `showDrawer()`, `showManage()`, `showMenu(i)`
  // (the i-th item in the bar, else behind the chevron).
  function showDrawer() { menuPopup.open = false; managePopup.open = false; drawerPopup.open = true }
  function showManage() { menuPopup.open = false; drawerPopup.open = false; managePopup.open = true }
  function showMenu(i) {
    var anchor = i < inlineRow.children.length - 0 && i < inlineItems.length ? inlineRow.children[i] : chevron
    var item = i < inlineItems.length ? inlineItems[i] : behind[i - inlineItems.length]
    openMenu(item, anchor)
  }
  function openMenu(item, anchor) {
    if (!item) return
    if (!item.menu) {
      // no dbusmenu: ask the application to show its own
      var win = anchor.QsWindow.window
      var point = win.contentItem.mapFromItem(anchor, 0, anchor.height)
      item.display(win, point.x, point.y)
      return
    }
    drawerPopup.open = false
    managePopup.open = false
    menuPopup.item = item
    menuPopup.anchorTo = anchor
    menuPopup.open = true
  }
  function activate(item, anchor) {
    if (item.onlyMenu) openMenu(item, anchor)
    else item.activate()
  }

  visible: everything.length > 0
  implicitWidth: vertical ? barSize : inlineRow.width + (hasChevron ? chevronW : 0)
  implicitHeight: vertical ? inlineRow.height : barSize

  // ---- the chevron: what is behind it
  BarButton {
    id: chevron
    visible: root.hasChevron
    bar: root.bar
    x: 0
    y: root.g.px(2)
    width: root.chevronW
    height: root.g.px(12)
    tooltipText: root.behind.length + " more"
    onPressed: function(b) {
      if (b === Qt.RightButton) { drawerPopup.open = false; menuPopup.open = false; managePopup.open = !managePopup.open }
      else { managePopup.open = false; menuPopup.open = false; drawerPopup.open = !drawerPopup.open }
    }
    Rectangle {
      anchors.fill: parent
      antialiasing: false
      color: chevron.down ? Role.hover : (chevron.hovered || drawerPopup.open ? Role.raised : "transparent")
    }
    Sprite {
      x: root.g.centre(parent.width, width); y: root.g.centre(parent.height, height)
      rows: Sprites.chevronLeft
      color: chevron.hovered || drawerPopup.open ? Role.ink : Role.muted
    }
  }

  // ---- the icons that stay in the bar
  Row {
    id: inlineRow
    x: root.hasChevron ? root.chevronW : 0
    y: root.g.centre(root.height, height)
    spacing: 0
    Repeater {
      model: root.inlineItems
      delegate: TrayButton { }
    }
  }

  component TrayButton: BarButton {
    id: button
    required property var modelData
    readonly property var entry: modelData
    bar: root.bar
    width: root.cellW
    height: root.g.px(14)
    tooltipText: root.tip(entry)
    onPressed: function(b) {
      if (b === Qt.RightButton) root.openMenu(entry, button)
      else if (b === Qt.MiddleButton) entry.secondaryActivate()
      else root.activate(entry, button)
    }
    onWheelMoved: function(delta) { entry.scroll(delta, false) }

    Rectangle {
      anchors.fill: parent
      antialiasing: false
      color: button.down ? Role.hover : (button.hovered ? Role.raised : "transparent")
    }
    PixelIcon {
      x: root.g.px(2); y: root.g.px(1)
      source: String(button.entry.icon || "")
      silhouette: root.iconSilhouette(button.entry)
      cells: 11
    }
    Rectangle {          // needs attention: a lit block at the corner
      visible: button.entry.status === Status.NeedsAttention
      x: parent.width - root.g.px(5); y: root.g.px(1)
      width: root.g.px(3); height: root.g.px(3)
      color: Role.alarm; antialiasing: false
    }
  }

  // ---- popups
  PopupFrame {
    id: drawerPopup
    anchorItem: chevron
    bar: root.bar
    owner: drawerPopup
    contentWidth: g.px(3 + 11 + 2) + drawerCells * g.cellW + g.px(3)
    contentHeight: Math.max(1, drawerColumn.implicitHeight)
    readonly property int drawerCells: {
      var widest = 14
      for (var i = 0; i < root.behind.length; i++) widest = Math.max(widest, Math.min(30, root.nameOf(root.behind[i]).length))
      return widest
    }

    Column {
      id: drawerColumn
      width: parent.width
      Repeater {
        model: root.behind
        delegate: BarButton {
          id: drow
          required property var modelData
          bar: root.bar
          width: drawerColumn.width
          height: root.g.px(14)
          tooltipText: ""
          onPressed: function(b) {
            if (b === Qt.RightButton) root.openMenu(modelData, chevron)
            else if (b === Qt.MiddleButton) modelData.secondaryActivate()
            else { root.activate(modelData, drow); if (!modelData.onlyMenu) drawerPopup.open = false }
          }
          Rectangle {
            anchors.fill: parent; antialiasing: false
            color: drow.hovered ? Role.accent : "transparent"
          }
          PixelIcon {
            x: root.g.px(2); y: root.g.px(1)
            source: String(modelData.icon || ""); silhouette: root.iconSilhouette(modelData); cells: 11
            ink: drow.hovered ? Role.onAccent : Role.ink
          }
          PixelText {
            x: root.g.px(2 + 11 + 2); y: root.g.centre(parent.height, height)
            text: root.nameOf(modelData)
            ink: drow.hovered ? Role.onAccent : Role.ink
            columns: drawerPopup.drawerCells
          }
        }
      }
    }
  }

  PopupFrame {
    id: menuPopup
    property var item: null
    property Item anchorTo: null
    anchorItem: anchorTo || root
    bar: root.bar
    owner: menuPopup
    contentWidth: trayMenu.implicitWidth
    contentHeight: trayMenu.implicitHeight
    onDismissed: trayMenu.reset()

    TrayMenu {
      id: trayMenu
      handle: menuPopup.open && menuPopup.item ? menuPopup.item.menu : null
      title: menuPopup.item ? String(menuPopup.item.title || menuPopup.item.id || "") : ""
      onDone: menuPopup.open = false
    }
  }

  PopupFrame {
    id: managePopup
    anchorItem: chevron
    bar: root.bar
    owner: managePopup
    contentWidth: g.px(3 + 11 + 2) + manageCells * g.cellW + g.px(2 + 7 + 2 + 7 + 3)
    contentHeight: manageColumn.implicitHeight
    readonly property int manageCells: {
      var widest = 14
      for (var i = 0; i < root.everything.length; i++) widest = Math.max(widest, Math.min(26, root.nameOf(root.everything[i]).length))
      return widest
    }

    Column {
      id: manageColumn
      width: parent.width
      Item {
        width: parent.width; height: root.g.px(14)
        PixelText { x: root.g.px(3); y: root.g.centre(parent.height, height); text: "TRAY ICONS"; ink: Role.muted }
      }
      Rectangle { width: parent.width; height: root.g.hair; color: Role.edge; antialiasing: false }
      Repeater {
        model: root.everything
        delegate: Item {
          id: mrow
          required property var modelData
          readonly property string iid: String(modelData.id || "")
          readonly property bool isPinned: root.pinnedIds.indexOf(iid) !== -1
          readonly property bool isHidden: root.hiddenIds.indexOf(iid) !== -1
          width: manageColumn.width; height: root.g.px(14)
          PixelIcon { x: root.g.px(2); y: root.g.px(1); source: String(modelData.icon || ""); silhouette: root.iconSilhouette(modelData); cells: 11 }
          PixelText {
            x: root.g.px(2 + 11 + 2); y: root.g.centre(parent.height, height)
            text: root.nameOf(modelData); ink: Role.ink; columns: managePopup.manageCells
          }
          // pin and hide: two 7 x 7 toggles, lit when set
          BarButton {
            id: hideButton
            bar: root.bar
            x: parent.width - root.g.px(3 + 7 + 2 + 7 + 2); y: 0
            width: root.g.px(11); height: parent.height
            tooltipText: mrow.isHidden ? "Show" : "Hide"
            onPressed: function() { root.toggleHide(mrow.iid) }
            Sprite { x: root.g.px(2); y: root.g.centre(parent.height, height); rows: mrow.isHidden ? Sprites.eyeOff : Sprites.eye; color: mrow.isHidden ? Role.alarm : (hideButton.hovered ? Role.ink : Role.muted) }
          }
          BarButton {
            id: pinButton
            bar: root.bar
            x: parent.width - root.g.px(3 + 7 + 2); y: 0
            width: root.g.px(11); height: parent.height
            tooltipText: mrow.isPinned ? "Unpin" : "Pin"
            onPressed: function() { root.togglePin(mrow.iid) }
            Sprite { x: root.g.px(2); y: root.g.centre(parent.height, height); rows: Sprites.pin; color: mrow.isPinned ? Role.accent : (pinButton.hovered ? Role.ink : Role.muted) }
          }
        }
      }
    }
  }
}
