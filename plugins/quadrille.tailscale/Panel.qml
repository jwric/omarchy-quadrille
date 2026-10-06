import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Q"

Panel {
  id: root
  moduleName: "omarchy.tailscale"
  ipcTarget: "omarchy.tailscale"
  manageIpc: false

  property string focusSection: "header"
  property int headerIndex: 0
  property int accountIndex: 0
  property int peerIndex: 0
  property int exitNodeIndex: 0
  property int mullvadRegionIndex: 0
  property bool cursorActive: false
  property bool copyMenuOpen: false
  property bool mullvadPickerOpen: false
  property string mullvadQuery: ""
  property int phraseIndex: 0
  readonly property var activePhrases: [
    "Encrypting connections",
    "Sending secrets",
    "Guarding wires",
    "Braiding packets",
    "Polishing tunnels",
    "Hiding routes",
    "Sealing ports",
    "Sorting tailnets",
    "Shuffling keys",
    "Watching machines"
  ]
  readonly property string heroPhraseText: activePhrases[phraseIndex % activePhrases.length]

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property bool showConnections: tailscale.accounts.length > 1 || tailscale.accountsAccessDenied
  readonly property bool showPeers: tailscale.active && tailscale.peers.length > 0
  readonly property var recentMullvadRegions: settings.recentMullvadRegions instanceof Array ? settings.recentMullvadRegions : (settings.recentMullvadCountries instanceof Array ? settings.recentMullvadCountries : [])
  readonly property var recentMullvadExitNodes: recentMullvadNodes()
  readonly property var exitNodes: displayExitNodes()
  readonly property bool showExitNodes: tailscale.active && (exitNodes.length > 0 || tailscale.mullvadRegions.length > 0)
  readonly property var filteredMullvadRegions: filteredMullvadRegionNodes()
  // Only claim the header cursor when the switch is actually on screen —
  // "header" stays navigable, but an absent CLI leaves nothing to highlight.
  readonly property bool headerHasCursor: cursorActive && focusSection === "header" && tailscale.installed
  readonly property color iconColor: tailscale.active ? foreground : dim
  readonly property string toggleHint: tailscale.active ? "Turn Tailscale off" : (tailscale.needsLogin ? "Authorize this device" : "Turn Tailscale on")
  readonly property color barIconColor: tailscale.active ? barForeground : Qt.darker(barForeground, 1.55)
  readonly property color hoverFill: bar ? Style.hoverFillFor(bar.foreground, Color.accent) : "transparent"
  readonly property color selectedFill: bar ? Style.selectedFillFor(bar.foreground, Color.accent) : "transparent"

  function selectedPeer() {
    if (tailscale.peers.length === 0) return null
    return tailscale.peers[Math.max(0, Math.min(peerIndex, tailscale.peers.length - 1))]
  }

  function selectedExitNode() {
    if (exitNodes.length === 0) return null
    return exitNodes[Math.max(0, Math.min(exitNodeIndex, exitNodes.length - 1))]
  }

  function selectedMullvadRegion() {
    if (filteredMullvadRegions.length === 0) return null
    return filteredMullvadRegions[Math.max(0, Math.min(mullvadRegionIndex, filteredMullvadRegions.length - 1))]
  }

  function displayExitNodes() {
    var nodes = []
    for (var i = 0; i < tailscale.tailnetExitNodes.length; i++) nodes.push(tailscale.tailnetExitNodes[i])
    for (var j = 0; j < recentMullvadExitNodes.length; j++) nodes.push(recentMullvadExitNodes[j])
    if (tailscale.mullvadRegions.length > 0) nodes.push({ id: "mullvad:add", AddMullvad: true, DisplayName: "Choose Mullvad region" })
    return nodes
  }

  function recentMullvadNodes() {
    var nodes = []
    var seen = {}
    for (var a = 0; a < tailscale.mullvadRegions.length && nodes.length < 5; a++) {
      var active = tailscale.mullvadRegions[a]
      var activeKey = mullvadRegionKey(active)
      if (active.ExitNode === true && activeKey !== "" && !seen[activeKey]) {
        nodes.push(active)
        seen[activeKey] = true
      }
    }
    for (var i = 0; i < recentMullvadRegions.length && nodes.length < 5; i++) {
      var region = String(recentMullvadRegions[i] || "")
      if (region === "" || seen[region]) continue
      var node = mullvadRegionNode(region)
      if (node) {
        nodes.push(node)
        seen[region] = true
      }
    }
    return nodes
  }

  function mullvadRegionKey(node) {
    if (!node) return ""
    var country = String(node.Country || "")
    var city = String(node.City || "")
    if (country === "" || city === "") return ""
    return country + "\n" + city
  }

  function mullvadRegionNode(region) {
    for (var i = 0; i < tailscale.mullvadRegions.length; i++) {
      var node = tailscale.mullvadRegions[i]
      if (mullvadRegionKey(node) === String(region || "")) return node
      if (String(node.Country || "") === String(region || "")) return node
    }
    return null
  }

  function filteredMullvadRegionNodes() {
    var query = String(mullvadQuery || "").trim().toLowerCase()
    var result = []
    for (var i = 0; i < tailscale.mullvadRegions.length; i++) {
      var node = tailscale.mullvadRegions[i]
      var label = (String(node.City || "") + " " + String(node.Country || "")).toLowerCase()
      if (query === "" || label.indexOf(query) !== -1) result.push(node)
    }
    return result
  }

  function mullvadRegionTitle(peer) {
    if (!peer) return "Unknown"
    var city = String(peer.City || "").trim()
    var country = String(peer.Country || "").trim()
    if (city === "" || city === "Any") return country || String(peer.DisplayName || "Unknown")
    return city
  }

  function mullvadRegionSubtitle(peer) {
    if (!peer) return ""
    return String(peer.Country || "").trim()
  }

  function persistRecentMullvad(region) {
    var name = String(region || "")
    if (name === "") return
    var next = [name]
    for (var i = 0; i < recentMullvadRegions.length && next.length < 5; i++) {
      var existing = String(recentMullvadRegions[i] || "")
      if (existing !== "" && existing !== name && next.indexOf(existing) === -1) next.push(existing)
    }
    if (!root.bar || !root.bar.shell || typeof root.bar.shell.updateEntryInline !== "function") return
    var entry = { id: root.moduleName }
    for (var key in settings) if (key !== "id") entry[key] = settings[key]
    entry.recentMullvadRegions = next
    root.bar.shell.updateEntryInline(root.moduleName, entry)
  }

  function chooseExitNode(peer) {
    if (!peer) return
    if (peer.AddMullvad === true) {
      mullvadPickerOpen = !mullvadPickerOpen
      mullvadRegionIndex = 0
      if (mullvadPickerOpen) Qt.callLater(function() { if (mullvadSearch) mullvadSearch.forceActiveFocus() })
      return
    }
    if (peer.Mullvad === true) persistRecentMullvad(mullvadRegionKey(peer))
    tailscale.setExitNode(peer)
    mullvadPickerOpen = false
  }

  function selectedAccount() {
    if (tailscale.accounts.length === 0) return null
    return tailscale.accounts[Math.max(0, Math.min(accountIndex, tailscale.accounts.length - 1))]
  }

  function ensureCursor() {
    if (headerIndex < 0) headerIndex = 0
    if (headerIndex > 0) headerIndex = 0
    if (accountIndex >= tailscale.accounts.length) accountIndex = Math.max(0, tailscale.accounts.length - 1)
    if (peerIndex >= tailscale.peers.length) peerIndex = Math.max(0, tailscale.peers.length - 1)
    if (exitNodeIndex >= exitNodes.length) exitNodeIndex = Math.max(0, exitNodes.length - 1)
    if (mullvadRegionIndex >= filteredMullvadRegions.length) mullvadRegionIndex = Math.max(0, filteredMullvadRegions.length - 1)
    if (focusSection === "auth" && !tailscale.accountsAccessDenied) focusSection = tailscale.accounts.length > 1 ? "accounts" : (showExitNodes ? "exitNodes" : (showPeers ? "peers" : "header"))
    if (focusSection === "accounts" && tailscale.accounts.length <= 1) focusSection = tailscale.accountsAccessDenied ? "auth" : (showExitNodes ? "exitNodes" : (showPeers ? "peers" : "header"))
    if (focusSection === "peers" && !showPeers) focusSection = showExitNodes ? "exitNodes" : (tailscale.accountsAccessDenied ? "auth" : (tailscale.accounts.length > 1 ? "accounts" : "header"))
    if (focusSection === "exitNodes" && !showExitNodes) focusSection = showPeers ? "peers" : (tailscale.accountsAccessDenied ? "auth" : (tailscale.accounts.length > 1 ? "accounts" : "header"))
  }

  function moveCursor(dx, dy) {
    cursorActive = true
    ensureCursor()
    if (dy !== 0) {
      if (focusSection === "header") {
        if (dy > 0) {
          if (tailscale.accountsAccessDenied) focusSection = "auth"
          else if (tailscale.accounts.length > 1) focusSection = "accounts"
          else if (showExitNodes) focusSection = "exitNodes"
          else if (showPeers) focusSection = "peers"
        }
      } else if (focusSection === "auth") {
        if (dy < 0) focusSection = "header"
        else if (tailscale.accounts.length > 1) focusSection = "accounts"
        else if (showExitNodes) focusSection = "exitNodes"
        else if (showPeers) focusSection = "peers"
      } else if (focusSection === "accounts") {
        if (dy < 0) {
          if (accountIndex <= 0) focusSection = tailscale.accountsAccessDenied ? "auth" : "header"
          else accountIndex--
        } else {
          if (accountIndex < tailscale.accounts.length - 1) accountIndex++
          else if (showExitNodes) focusSection = "exitNodes"
          else if (showPeers) focusSection = "peers"
        }
      } else if (focusSection === "peers") {
        if (dy < 0) {
          if (peerIndex <= 0) focusSection = showExitNodes ? "exitNodes" : (tailscale.accounts.length > 1 ? "accounts" : (tailscale.accountsAccessDenied ? "auth" : "header"))
          else peerIndex--
        } else if (peerIndex < tailscale.peers.length - 1) {
          peerIndex++
        }
      } else if (focusSection === "exitNodes") {
        if (dy < 0) {
          if (exitNodeIndex <= 0) focusSection = tailscale.accounts.length > 1 ? "accounts" : (tailscale.accountsAccessDenied ? "auth" : "header")
          else exitNodeIndex--
        } else if (exitNodeIndex < exitNodes.length - 1) {
          exitNodeIndex++
        } else if (showPeers) {
          focusSection = "peers"
        }
      }
    }
    ensureCursor()
    scrollCursorIntoView()
  }

  function activateCursor() {
    ensureCursor()
    if (focusSection === "header") {
      tailscale.toggleTailscale()
    } else if (focusSection === "auth") {
      tailscale.authorizeTailscaleOperator()
    } else if (focusSection === "accounts") {
      var account = selectedAccount()
      if (account) tailscale.switchAccount(account.id)
    } else if (focusSection === "peers") {
      openSelectedPeerCopyMenu()
    } else if (focusSection === "exitNodes") {
      chooseExitNode(selectedExitNode())
    }
  }

  function moveMullvadRegionCursor(delta) {
    if (filteredMullvadRegions.length === 0) return
    cursorActive = true
    mullvadRegionIndex = Math.max(0, Math.min(filteredMullvadRegions.length - 1, mullvadRegionIndex + delta))
    scrollMullvadRegionCursorIntoView()
  }

  function activateMullvadRegionCursor() {
    var region = selectedMullvadRegion()
    if (region) chooseExitNode(region)
  }

  // panelFlick is a QScroll: it scrolls in whole virtual pixels and keeps a
  // little air above and below the item it brings into view.
  function scrollItemIntoView(item) {
    if (!panelFlick || !item) return
    Qt.callLater(function() {
      if (!item) return
      panelFlick.ensureItemVisible(item)
    })
  }

  function scrollCursorIntoView() {
    if (focusSection === "peers" && peerColumn && peerIndex >= 0 && peerIndex < peerColumn.children.length) scrollItemIntoView(peerColumn.children[peerIndex])
    else if (focusSection === "exitNodes" && exitNodeColumn && exitNodeIndex >= 0 && exitNodeIndex < exitNodeColumn.children.length) scrollItemIntoView(exitNodeColumn.children[exitNodeIndex])
  }

  function scrollMullvadRegionCursorIntoView() {
    if (mullvadRegionColumn && mullvadRegionIndex >= 0 && mullvadRegionIndex < mullvadRegionColumn.children.length) scrollItemIntoView(mullvadRegionColumn.children[mullvadRegionIndex])
  }

  function setPeerCursor(index) {
    cursorActive = true
    focusSection = "peers"
    peerIndex = index
    scrollCursorIntoView()
  }

  // The file picker takes over from here, so get the panel out of the way.
  function sendPeerFile(peer) {
    if (!tailscale.canSendFiles(peer)) return
    tailscale.sendFile(peer)
    close()
  }

  function openSelectedPeerCopyMenu() {
    if (!peerColumn || peerIndex < 0 || peerIndex >= peerColumn.children.length) return
    var item = peerColumn.children[peerIndex]
    if (item && item.openCopyMenu) item.openCopyMenu()
  }

  function setExitNodeCursor(index) {
    cursorActive = true
    focusSection = "exitNodes"
    exitNodeIndex = index
    scrollCursorIntoView()
  }

  function setAccountCursor(index) {
    cursorActive = true
    focusSection = "accounts"
    accountIndex = index
  }

  function setAuthCursor() {
    cursorActive = true
    focusSection = "auth"
  }

  function setHeaderCursor() {
    cursorActive = true
    focusSection = "header"
    headerIndex = 0
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onOpenedChanged: if (opened) {
    cursorActive = false
    if (panelFlick) panelFlick.reset()
    tailscale.refresh()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }
  onPeerIndexChanged: scrollCursorIntoView()
  onExitNodeIndexChanged: scrollCursorIntoView()
  onMullvadRegionIndexChanged: if (mullvadPickerOpen) scrollMullvadRegionCursorIntoView()
  onShowConnectionsChanged: ensureCursor()
  onShowPeersChanged: ensureCursor()
  onShowExitNodesChanged: ensureCursor()
  onFilteredMullvadRegionsChanged: ensureCursor()

  Service {
    id: tailscale
    settings: root.settings
  }

  Connections {
    target: tailscale
    function onPeersChanged() { root.ensureCursor() }
    function onAccountsChanged() { root.ensureCursor() }
    function onAccountsAccessDeniedChanged() { root.ensureCursor() }
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): string { tailscale.refresh(); return "ok" }
    function up(): string { tailscale.loginOrUp(); return "ok" }
    function down(): string { tailscale.down(); return "ok" }
    function toggleTailscale(): string { tailscale.toggleTailscale(); return "ok" }
    function status(): string { return tailscale.statusText }
  }


  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    // A slot a whole number of virtual pixels wide (3 + 7 + 3, the bar's own
    // marks), so what follows it on the bar stays on the grid.
    readonly property var g: Px.of(button)
    slotSize: g.px(13)
    // The mark is drawn below, from the button's own corner on the bar's grid:
    // the host's optical canvas is centred to a logical pixel, which is not a
    // whole vpx on a fractional output. The icon component only tells the
    // button it has a face.
    iconComponent: Component { Item { } }
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) tailscale.toggleTailscale()
      else if (buttonCode === Qt.MiddleButton) tailscale.refresh()
      else root.toggle()
    }

    TailscaleIcon {
      x: button.g.centre(button.width, width)
      y: button.g.centre(button.height, height)
      color: tailscale.active ? Role.ink : Role.muted
      badgeColor: Role.alarm
      crossed: !tailscale.active && !tailscale.needsLogin
      warning: tailscale.needsLogin
    }
  }

  // ---- the view: sprites for what the stock panel drew in Nerd Font glyphs ----
  Icons { id: icons }

  // Model.osIcon's glyph -> the sprite of that system.
  function spriteForGlyph(glyph) {
    var code = glyph ? glyph.codePointAt(0) : 0
    if (code === 0xf033d) return icons.linux
    if (code === 0xf0035) return icons.apple
    if (code === 0xf0372) return icons.windows
    if (code === 0xf0032) return icons.android
    if (code === 0xf0582) return icons.globe
    return icons.computer
  }
  // The hero's mark, the 15 x 15 drawing (the bar has the 7 x 7 one).
  function heroMark() {
    if (tailscale.needsLogin) return icons.markLargeLogin
    if (!tailscale.active) return icons.markLargeOff
    return icons.markLarge
  }
  // The caps line under the hero's title: the phrase while connected, and
  // otherwise what the matter is (the stock line said "disconnected" for all of
  // them, including a machine without the CLI).
  readonly property string heroStatus: {
    if (tailscale.active) return root.heroPhraseText
    if (!tailscale.installed) return tailscale.statusText
    if (tailscale.needsLogin) return "Needs login"
    return "Tailscale is disconnected"
  }
  // A machine's second line: its IP and DNS name when both fit in `columns`, the
  // IP alone when they do not (a name cut short reads as a different name).
  function addressLine(ip, dns, columns) {
    var parts = []
    if (ip !== "") parts.push(ip)
    if (dns !== "") parts.push(dns)
    var both = parts.join(" · ")
    if (both.length <= columns) return both
    return parts.length > 0 && parts[0].length <= columns ? parts[0] : ""
  }
  // The longest label among `options`, in cells (the copy menu is as wide as it
  // needs, up to what fits in the card; a longer one wraps).
  function longestLabel(options) {
    var n = 0
    for (var i = 0; i < options.length; i++) n = Math.max(n, String(options[i].label || "").length)
    return n
  }

  QPopup {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(panel.cardWidth(54))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, panel.g.px(400))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: root.copyMenuOpen
      onMoveRequested: function(dx, dy) {
        if (!root.cursorActive) { root.cursorActive = true; return }
        root.moveCursor(dx, dy)
      }
      onActivateRequested: if (root.cursorActive) root.activateCursor()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) {
        if (t === "t" || t === "T") tailscale.toggleTailscale()
        else if (t === "c" || t === "C") tailscale.copyPeerIp(root.selectedPeer())
        else if (t === "n" || t === "N") tailscale.copyPeerName(root.selectedPeer())
        else if (t === "d" || t === "D") tailscale.copyPeerDnsName(root.selectedPeer())
        else if (t === "s" || t === "S") root.sendPeerFile(root.selectedPeer())
      }

      // The whole card scrolls when it is taller than the screen allows (the
      // stock Flickable's id is kept: the cursor functions above scroll it).
      QScroll {
        id: panelFlick
        anchors.fill: parent
        contentHeight: column.implicitHeight

        Column {
          id: column
          width: parent.width
          spacing: panel.g.px(6)

          // ---------- Hero: the mark · name/status · on-off switch ----------
          // Status only: the switch owns toggling, mouse and keyboard alike.
          QHero {
            width: parent.width
            icon: root.heroMark()
            iconLevel: 0
            iconInk: tailscale.active ? Role.ink : Role.muted
            iconDim: Role.faint
            iconAccent: Role.alarm
            title: tailscale.installed ? (tailscale.selfName || "Tailscale") : "Tailscale"
            subtitle: root.heroStatus

            QSwitch {
              id: powerSwitch
              visible: tailscale.installed
              y: panel.g.centre(parent.height, height)
              checked: tailscale.active
              busy: tailscale.busy
              hasCursor: root.headerHasCursor
              readonly property string tipText: root.toggleHint
              onHovered: function(on) {
                if (on) root.setHeaderCursor()
                tipLayer.follow(powerSwitch, on)
              }
              onToggled: tailscale.toggleTailscale()
            }
          }

          // ---------- What just happened, or what went wrong ----------
          Item {
            id: statusLine
            readonly property bool isError: tailscale.lastError !== "" && tailscale.actionStatus === ""
            visible: tailscale.actionStatus !== "" || tailscale.lastError !== ""
            width: parent.width
            height: statusText.height

            Sprite {
              visible: statusLine.isError
              x: panel.g.px(3)
              y: panel.g.onCaps(7)
              rows: Sprites.warning
              color: Role.alarm
            }
            PixelParagraph {
              id: statusText
              x: panel.g.px(13)
              // At least a cell: wrapping to none would never end.
              columns: Math.max(1, panel.g.columns(statusLine.width - x))
              maxLines: 4
              text: tailscale.actionStatus !== "" ? tailscale.actionStatus : tailscale.lastError
              ink: statusLine.isError ? Role.alarm : Role.muted
            }
          }

          // ---------- No CLI: say so, and where to get it ----------
          Item {
            id: missing
            visible: !tailscale.installed
            width: parent.width
            height: missingText.height

            Sprite {
              x: panel.g.px(3)
              y: panel.g.onCaps(7)
              rows: Sprites.warning
              color: Role.caution
            }
            Column {
              id: missingText
              x: panel.g.px(13)
              readonly property int columns: Math.max(1, panel.g.columns(missing.width - x))
              PixelParagraph {
                columns: missingText.columns
                text: "Tailscale CLI is not installed or not on PATH."
                ink: Role.ink
              }
              PixelParagraph {
                columns: missingText.columns
                text: "Install it from the Omarchy menu:\nInstall › Service › Tailscale"
                ink: Role.muted
              }
            }
          }

          // ---------- Connections (profiles) ----------
          Group {
            name: "CONNECTIONS"
            visible: root.showConnections
            width: parent.width
            height: implicitHeight

            Column {
              width: parent.width
              spacing: panel.g.px(2)

              AuthRow {
                visible: tailscale.accountsAccessDenied
                width: parent.width
              }

              Repeater {
                model: tailscale.accounts
                AccountRow {
                  required property var modelData
                  required property int index
                  width: parent.width
                  account: modelData
                  rowIndex: index
                }
              }
            }
          }

          // ---------- Exit nodes, and the Mullvad region picker ----------
          Group {
            name: "EXIT NODES"
            visible: root.showExitNodes
            width: parent.width
            height: implicitHeight

            Column {
              id: exitNodeColumn
              width: parent.width
              spacing: panel.g.px(2)

              Repeater {
                model: root.exitNodes
                ExitNodeRow {
                  required property var modelData
                  required property int index
                  width: exitNodeColumn.width
                  peer: modelData
                  rowIndex: index
                }
              }

              Column {
                visible: root.mullvadPickerOpen
                width: parent.width
                spacing: panel.g.px(2)
                topPadding: panel.g.px(2)

                QField {
                  id: mullvadSearch
                  width: parent.width
                  placeholderText: "Search regions"
                  text: root.mullvadQuery
                  onTextChanged: {
                    root.mullvadQuery = text
                    root.mullvadRegionIndex = 0
                  }
                  onAccepted: {
                    root.activateMullvadRegionCursor()
                  }
                  onKeyPressed: function(event) {
                    if (event.key === Qt.Key_Down || event.text === "j") {
                      root.moveMullvadRegionCursor(1)
                      event.accepted = true
                      return
                    }
                    if (event.key === Qt.Key_Up || event.text === "k") {
                      root.moveMullvadRegionCursor(-1)
                      event.accepted = true
                      return
                    }
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                      root.activateMullvadRegionCursor()
                      event.accepted = true
                      return
                    }
                    if (event.key === Qt.Key_Escape) {
                      root.mullvadPickerOpen = false
                      keyCatcher.forceActiveFocus()
                      event.accepted = true
                    }
                  }
                }

                QEmpty {
                  visible: root.filteredMullvadRegions.length === 0
                  width: parent.width
                  icon: icons.globe
                  text: "No Mullvad regions found."
                }

                Column {
                  id: mullvadRegionColumn
                  width: parent.width
                  spacing: panel.g.px(2)

                  Repeater {
                    model: root.filteredMullvadRegions
                    MullvadRegionRow {
                      required property var modelData
                      required property int index
                      width: parent.width
                      peer: modelData
                      rowIndex: index
                    }
                  }
                }
              }
            }
          }

          // ---------- Machines on the tailnet ----------
          Group {
            name: "MACHINES"
            visible: tailscale.installed && tailscale.active
            width: parent.width
            height: implicitHeight

            Column {
              width: parent.width
              spacing: panel.g.px(2)

              QEmpty {
                visible: tailscale.installed && tailscale.active && tailscale.peers.length === 0
                width: parent.width
                icon: icons.computer
                text: "No machines found on this tailnet."
              }

              Column {
                id: peerColumn
                visible: root.showPeers
                width: parent.width
                spacing: panel.g.px(2)

                Repeater {
                  model: tailscale.peers
                  PeerRow {
                    required property var modelData
                    required property int index
                    width: peerColumn.width
                    peer: modelData
                    rowIndex: index
                  }
                }
              }
            }
          }
        }
      }
    }

    // One tooltip for the whole card, drawn over it: a tip that lived inside a
    // row would be cut by the scrolling list. It shows the stock 400 ms after
    // the pointer arrives, hangs beneath what it describes (from its right edge
    // when that is narrower than the tip) and goes above it when the card ends.
    Item {
      id: tipLayer
      anchors.fill: parent
      z: 10

      property Item source: null
      property bool armed: false
      property rect anchorBox: Qt.rect(0, 0, 0, 0)
      readonly property var g: panel.g

      function follow(item, on) {
        if (on) {
          source = item
          armed = false
          tipDelay.restart()
        } else if (source === item) {
          source = null
          armed = false
          tipDelay.stop()
        }
      }

      Timer {
        id: tipDelay
        interval: 400
        onTriggered: {
          if (!tipLayer.source) return
          var p = tipLayer.source.mapToItem(tipLayer, 0, 0)
          tipLayer.anchorBox = Qt.rect(tipLayer.g.snap(p.x), tipLayer.g.snap(p.y), tipLayer.source.width, tipLayer.source.height)
          tipLayer.armed = true
        }
      }

      Connections {
        target: panelFlick
        function onOffsetChanged() { tipLayer.follow(tipLayer.source, false) }
      }

      QTip {
        id: tip
        text: tipLayer.source && tipLayer.source.tipText !== undefined ? tipLayer.source.tipText : ""
        shown: tipLayer.armed && tipLayer.source !== null && root.opened
        x: {
          var f = tipLayer.anchorBox
          var at = f.width < width ? f.x + f.width - width : f.x + tipLayer.g.centre(f.width, width)
          return Math.max(0, Math.min(tipLayer.width - width, at))
        }
        y: {
          var f = tipLayer.anchorBox
          var below = f.y + f.height + tipLayer.g.px(2)
          return below + height <= tipLayer.height ? below : Math.max(0, f.y - height - tipLayer.g.px(2))
        }
      }
    }
  }

  // The phrase changes where it is: nothing fades.
  Timer {
    id: phraseTimer
    interval: 2800
    running: root.opened && tailscale.active
    repeat: true
    onTriggered: root.phraseIndex = (root.phraseIndex + 1) % root.activePhrases.length
  }

  // ---- rows ----

  component AuthRow: QRow {
    id: authRow

    hasCursor: root.cursorActive && root.focusSection === "auth"
    icon: icons.shieldHalf
    text: "Authorize Tailscale operator"
    // Dropped, not cut, if the card is ever too narrow for it.
    readonly property string why: "Allow this user to operate this Tailscale profile"
    sub: why.length <= columns ? why : ""

    // Ignored while a command runs, as the stock row's disabled mouse area was.
    onHovered: if (!tailscale.busy) root.setAuthCursor()
    onClicked: function(b) { if (b === Qt.LeftButton && !tailscale.busy) tailscale.authorizeTailscaleOperator() }
  }

  component AccountRow: QRow {
    id: accountRow
    property var account: null
    property int rowIndex: 0
    readonly property bool selectedAccount: account && account.selected === true
    readonly property bool switchingAccount: account && tailscale.switchingAccountId === String(account.id || "")
    readonly property string accountText: account ? tailscale.accountLabel(account) : "Account"

    hasCursor: root.cursorActive && root.focusSection === "accounts" && root.accountIndex === rowIndex
    current: selectedAccount
    icon: icons.person
    text: accountText
    // A switch in progress is a word, not a pulse.
    detail: switchingAccount ? "SWITCHING" : ""

    onHovered: root.setAccountCursor(accountRow.rowIndex)
    onClicked: function(b) { if (b === Qt.LeftButton && accountRow.account) tailscale.switchAccount(accountRow.account.id) }
  }

  component PeerRow: QRow {
    id: peerRow
    property var peer: null
    property int rowIndex: 0
    readonly property string peerName: peer ? String(peer.DisplayName || peer.HostName || "Unknown") : "Unknown"
    readonly property string peerIp: peer && peer.TailscaleIPs && peer.TailscaleIPs.length > 0 ? String(peer.TailscaleIPs[0]) : ""
    readonly property string peerIpv6: {
      if (!peer || !peer.TailscaleIPv6 || peer.TailscaleIPv6.length === 0) return ""
      return String(peer.TailscaleIPv6[0] || "")
    }
    readonly property string peerDns: peer ? String(peer.DNSName || "") : ""
    readonly property var copyOptions: {
      var options = []
      if (peerName !== "") options.push({ kind: "name", label: peerName })
      if (peerDns !== "") options.push({ kind: "dns", label: peerDns })
      if (peerIpv6 !== "") options.push({ kind: "ipv6", label: peerIpv6 })
      if (peerIp !== "") options.push({ kind: "ip", label: peerIp })
      return options
    }
    property int copyIndex: 0

    hasCursor: root.cursorActive && root.focusSection === "peers" && root.peerIndex === rowIndex
    icon: root.spriteForGlyph(tailscale.osIcon(peer ? peer.OS : ""))
    text: peerName
    sub: root.addressLine(peerIp, peerDns, columns)

    // The pointer moves the panel cursor; a click on the row does nothing (the
    // copy button, Enter and the c/n/d/s keys act on it).
    onHovered: root.setPeerCursor(peerRow.rowIndex)

    function clampCopyIndex() {
      copyIndex = Math.max(0, Math.min(copyIndex, copyOptions.length - 1))
    }

    function openCopyMenu() {
      if (copyOptions.length === 0) return
      clampCopyIndex()
      copyPopup.open()
    }

    function moveCopyCursor(delta) {
      if (copyOptions.length === 0) return
      copyIndex = Math.max(0, Math.min(copyOptions.length - 1, copyIndex + delta))
    }

    function copyOption(kind) {
      if (kind === "name") tailscale.copyPeerName(peer)
      else if (kind === "dns") tailscale.copyPeerDnsName(peer)
      else if (kind === "ipv6") tailscale.copyToClipboard(peerIpv6, peerName + " IPv6")
      else if (kind === "ip") tailscale.copyPeerIp(peer)
      copyPopup.close()
    }

    function copyCurrentOption() {
      clampCopyIndex()
      if (copyOptions.length === 0) return
      copyOption(copyOptions[copyIndex].kind)
    }

    // The actions, parked at the right of the row. As tall as the row, so the
    // row centres nothing: the buttons are placed on the grid here.
    Item {
      width: actions.width
      height: peerRow.height

      Row {
        id: actions
        y: panel.g.centre(parent.height, height)
        spacing: panel.g.px(2)

        QButton {
          id: sendButton
          visible: tailscale.canSendFiles(peerRow.peer)
          icon: icons.send
          air: 2
          readonly property string tipText: "Send files"
          onHovered: function(on) { tipLayer.follow(sendButton, on) }
          onClicked: root.sendPeerFile(peerRow.peer)
        }

        QButton {
          id: copyButton
          icon: icons.copy
          air: 2
          active: copyPopup.opened
          enabledState: peerRow.peerIp !== "" || peerRow.peerName !== "" || peerRow.peerDns !== "" || peerRow.peerIpv6 !== ""
          onClicked: peerRow.openCopyMenu()

          // The copy menu: what can be copied, one row each, the kind of each
          // as a reading. It hangs from the button's right edge, a pixel below
          // the row (not across the row's second line).
          Popup {
            id: copyPopup
            readonly property var g: panel.g
            readonly property int labelCells: Math.min(44, root.longestLabel(peerRow.copyOptions))
            x: copyButton.width - width
            y: peerRow.height - actions.y + g.px(1)
            width: g.cells(labelCells + 5) + g.px(3 + 10 + 3) + 2 * g.hair
            padding: 0
            modal: false
            focus: true
            enter: null
            exit: null
            closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
            function handleKey(event) {
              if (event.key === Qt.Key_Escape) {
                close()
                event.accepted = true
                return
              }
              if (event.key === Qt.Key_Down || event.text === "j") {
                peerRow.moveCopyCursor(1)
                event.accepted = true
                return
              }
              if (event.key === Qt.Key_Up || event.text === "k") {
                peerRow.moveCopyCursor(-1)
                event.accepted = true
                return
              }
              if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                peerRow.copyCurrentOption()
                event.accepted = true
              }
            }
            onOpenedChanged: {
              root.copyMenuOpen = opened
              if (opened) {
                peerRow.clampCopyIndex()
                Qt.callLater(function() { copyPopupContent.forceActiveFocus() })
              } else if (root.opened) {
                Qt.callLater(function() { keyCatcher.forceActiveFocus() })
              }
            }
            background: Rectangle {
              antialiasing: false
              color: Role.edge
              Rectangle {
                anchors.fill: parent
                anchors.margins: copyPopup.g.hair
                antialiasing: false
                color: Role.ground
              }
            }

            contentItem: Column {
              id: copyPopupContent
              width: parent.width
              topPadding: copyPopup.g.hair
              bottomPadding: copyPopup.g.hair
              focus: true
              Keys.priority: Keys.BeforeItem
              Keys.onPressed: function(event) { copyPopup.handleKey(event) }

              Repeater {
                model: peerRow.copyOptions
                CopyChoice {
                  required property var modelData
                  required property int index
                  x: copyPopup.g.hair
                  width: copyPopup.width - 2 * copyPopup.g.hair
                  label: String(modelData.label || "")
                  kind: String(modelData.kind || "")
                  selected: peerRow.copyIndex === index
                  onHovered: peerRow.copyIndex = index
                  onChosen: peerRow.copyOption(String(modelData.kind || ""))
                }
              }
            }
          }
        }
      }
    }
  }

  component CopyChoice: QRow {
    id: copyChoice
    signal chosen()
    property string label: ""
    property string kind: ""
    property bool selected: false

    hasCursor: selected
    icon: icons.copy
    text: label
    detail: kind === "ipv6" ? "IPV6" : kind.toUpperCase()
    onClicked: function(b) { if (b === Qt.LeftButton) copyChoice.chosen() }
  }

  component ExitNodeRow: QRow {
    id: exitNodeRow
    property var peer: null
    property int rowIndex: 0
    readonly property bool addMullvad: peer && peer.AddMullvad === true
    readonly property bool activeExitNode: peer && peer.ExitNode === true
    readonly property bool settingExitNode: peer && tailscale.settingExitNodeId === String(peer.id || "")
    readonly property string peerName: peer ? String(peer.DisplayName || peer.HostName || "Unknown") : "Unknown"
    readonly property string actionTooltip: addMullvad ? "" : (activeExitNode ? "Disconnect" : "Connect")
    readonly property string tipText: actionTooltip

    hasCursor: root.cursorActive && root.focusSection === "exitNodes" && root.exitNodeIndex === rowIndex
    current: activeExitNode || settingExitNode || (addMullvad && root.mullvadPickerOpen)
    icon: addMullvad ? Sprites.plus : (peer && peer.Mullvad === true ? icons.globe : icons.exit)
    text: peerName
    // While the exit node changes, a word instead of the spinning glyph.
    detail: settingExitNode ? (activeExitNode ? "DISCONNECTING" : "CONNECTING") : ""

    onHovered: root.setExitNodeCursor(exitNodeRow.rowIndex)
    onHotChanged: tipLayer.follow(exitNodeRow, hot)
    onClicked: function(b) { if (b === Qt.LeftButton) root.chooseExitNode(exitNodeRow.peer) }
  }

  component MullvadRegionRow: QRow {
    id: regionRow

    property var peer: null
    property int rowIndex: 0
    readonly property string regionName: root.mullvadRegionTitle(peer)
    readonly property string regionDetail: root.mullvadRegionSubtitle(peer)
    readonly property bool activeExitNode: peer && peer.ExitNode === true
    readonly property bool settingExitNode: peer && tailscale.settingExitNodeId === String(peer.id || "")
    readonly property bool selectedRegion: root.mullvadPickerOpen && root.mullvadRegionIndex === rowIndex
    readonly property string actionTooltip: activeExitNode ? "Disconnect" : "Connect"
    readonly property string tipText: actionTooltip

    // The region in use is the inverse block; the picker's own cursor (the one
    // Enter in the search field picks) is the brackets.
    current: activeExitNode || settingExitNode
    hasCursor: selectedRegion
    icon: icons.globe
    // One line a region, the country a reading on the right: the picker is a
    // long list, and a city is short.
    text: regionName
    detail: settingExitNode ? (activeExitNode ? "DISCONNECTING" : "CONNECTING") : regionDetail

    onHovered: root.mullvadRegionIndex = regionRow.rowIndex
    onHotChanged: tipLayer.follow(regionRow, hot)
    onClicked: function(b) { if (b === Qt.LeftButton) root.chooseExitNode(regionRow.peer) }
  }
}
