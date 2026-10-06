import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Q"
import "Q/Glyphs.js" as Glyphs

Panel {
  id: root
  moduleName: "omarchy.agents"
  ipcTarget: "omarchy.agents"
  manageIpc: false

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property color surface: Color.popups.background
  readonly property color track: Style.selectedFillFor(foreground, Color.accent)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property var providers: usage.enabledProviders
  // The selection follows the provider, not the slot it happens to sit in: a
  // provider whose first scan lands while the panel is open would otherwise
  // shift the list underneath you and swap out what you were reading.
  property string selectedProviderId: ""
  readonly property int providerIndex: {
    for (var i = 0; i < providers.length; i++)
      if (providers[i].providerId === selectedProviderId) return i
    return 0
  }
  readonly property var provider: providers.length > 0 ? providers[providerIndex] : null

  property bool cursorActive: false

  // Countdowns and "updated" read this instead of Date.now() so the
  // panel keeps telling the truth while it sits open.
  property double nowMs: Date.now()

  readonly property var limits: limitWindows(provider)
  readonly property var models: modelRows(provider)
  readonly property var headline: bindingWindow(provider)
  readonly property var balance: provider ? (provider.balance || null) : null
  // A prepaid account runs low the way a subscription window fills up: the
  // last 10% of the funded credits lights the same alarm.
  readonly property bool balanceAlarming: !!balance && balance.funded > 0
    && balance.remaining / balance.funded <= 0.1
  readonly property bool alarming: (!!headline && headline.percent >= 0.9) || balanceAlarming

  function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)) }
  function alpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }

  function selectProvider(index) {
    if (providers.length === 0) return
    var wrapped = ((index % providers.length) + providers.length) % providers.length
    selectedProviderId = providers[wrapped].providerId
  }

  function refreshNow() {
    usage.refreshAll(true)
  }

  function launchAgent() {
    if (root.bar) root.bar.run("omarchy-agent --pick")
    root.close()
  }

  // ---------------------------------------------------------------- limits
  //
  // Both providers report the same two shapes: a short rolling session window
  // and a long weekly one. Everything below normalizes them into one record so
  // the meters and the hero speak a single language.

  // Claude spells its windows out ("Session (5-hour)"), Codex abbreviates
  // them ("5h window", "30m window"). Both have to land on the same record.
  function windowIsLong(text) {
    return text.indexOf("week") >= 0 || text.indexOf("7-day") >= 0 || text.indexOf("seven") >= 0
      || text.indexOf("month") >= 0 || text.indexOf("30-day") >= 0
  }

  function windowSpanMs(label) {
    var text = String(label || "").toLowerCase()
    if (text.indexOf("month") >= 0 || text.indexOf("30-day") >= 0) return 30 * 24 * 3600 * 1000
    if (windowIsLong(text)) return 7 * 24 * 3600 * 1000
    var hours = text.match(/(\d+)\s*-?\s*h(?:our)?\b/)
    if (hours) return Number(hours[1]) * 3600 * 1000
    var minutes = text.match(/(\d+)\s*-?\s*m(?:in(?:ute)?s?)?\b/)
    if (minutes) return Number(minutes[1]) * 60 * 1000
    return 0
  }

  function windowTitle(label) {
    var text = String(label || "").toLowerCase()
    if (text.indexOf("month") >= 0) return "Monthly"
    if (windowIsLong(text)) return "Weekly"
    if (text.indexOf("session") >= 0 || windowSpanMs(label) > 0) return "Session"
    var plain = String(label || "").replace(/\s*\(.*\)\s*/, "").trim()
    return plain === "" ? "Limit" : plain
  }

  // A collector that already knows which window a limit belongs to says so,
  // and that beats reading it back out of the label: a model-scoped limit is
  // titled after its model, and a name like "Opus 5 (1M context)" would parse
  // as a one-minute window.
  function limitWindow(label, percent, resetAt, title) {
    return {
      title: String(title || "") !== "" ? String(title) : windowTitle(label),
      percent: Number(percent),
      resetAt: String(resetAt || "")
    }
  }

  function limitWindows(p) {
    if (!p) return []
    var out = []
    var list = p.limits || []
    for (var i = 0; i < list.length; i++) {
      var entry = list[i] || {}
      var percent = Number(entry.percent)
      if (percent >= 0) out.push(limitWindow(entry.label, percent, entry.resetsAt, entry.title))
    }
    return out
  }

  // The window that decides how much room is left — the fullest one, since
  // that is what stops the next prompt.
  function bindingWindow(p) {
    var windows = limitWindows(p)
    var best = null
    for (var i = 0; i < windows.length; i++) {
      if (!best || windows[i].percent > best.percent) best = windows[i]
    }
    return best
  }

  function resetMsFor(w) {
    if (!w || w.resetAt === "") return -1
    var ms = new Date(w.resetAt).getTime()
    return isFinite(ms) ? ms - root.nowMs : -1
  }

  function formatDuration(ms) {
    if (!(ms > 0)) return "now"
    var minutes = Math.floor(ms / 60000)
    var hours = Math.floor(minutes / 60)
    var days = Math.floor(hours / 24)
    if (days > 0) return days + "d " + (hours % 24) + "h"
    if (hours > 0) return hours + "h " + (minutes % 60) + "m"
    return Math.max(1, minutes) + "m"
  }

  // ---------------------------------------------------------------- balance
  //
  // Prepaid agents report a credit ledger instead of rate-limit windows: the
  // record's balance object carries remaining, funded, and spent amounts.

  function currencyPrefix(currency) {
    var code = String(currency || "USD").toUpperCase()
    if (code === "USD") return "$"
    if (code === "EUR") return "€"
    if (code === "GBP") return "£"
    return code + " "
  }

  function formatMoney(value, currency) {
    var amount = Number(value)
    if (!isFinite(amount)) amount = 0
    return currencyPrefix(currency) + amount.toFixed(2)
  }

  function balanceDetailText(b) {
    if (!b || !(b.funded > 0)) return ""
    var text = formatMoney(b.spent, b.currency) + " spent of " + formatMoney(b.funded, b.currency) + " funded"
    if (b.estimated) text += " · estimated"
    return text
  }

  // ---------------------------------------------------------------- content

  // The plan you pay for, under the name of the tool it pays for. Limits live
  // in their own section; the hero just says what this is.
  function heroMeta(p) {
    if (!p) return ""
    if (String(p.usageStatusText || "") !== "") return p.usageStatusText
    var tier = String(p.tierLabel || "")
    if (tier === "") return "Subscription"
    return tier.charAt(0).toUpperCase() + tier.slice(1)
  }

  // Local calendar date, recomputed from nowMs so a panel left open across
  // midnight moves the "Today" row with the clock.
  function todayDate() {
    var now = new Date(root.nowMs)
    return now.getFullYear()
      + "-" + String(now.getMonth() + 1).padStart(2, "0")
      + "-" + String(now.getDate()).padStart(2, "0")
  }

  function dayName(date) {
    var parsed = new Date(String(date || "") + "T00:00:00")
    if (isNaN(parsed.getTime())) return String(date || "")
    return ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][parsed.getDay()]
  }

  function dayLabel(date, today) {
    if (today) return "Today"
    return dayName(date)
  }

  function dayTooltip(day, today) {
    if (!day) return ""
    var parsed = new Date(String(day.date) + "T00:00:00")
    var label = isNaN(parsed.getTime())
      ? String(day.date)
      : dayName(day.date) + " " + (parsed.getMonth() + 1) + "/" + parsed.getDate()
    var text = label + " · " + usage.formatTokenCount(Number(day.messageCount || 0)) + " tokens"
    // Prompt and session counts only exist for today, so they ride along here
    // instead of taking a section of their own. Billing-API agents never
    // count prompts, and "0 prompts" would read as a quiet day, not a gap.
    if (today && provider && provider.hasPromptStats !== false)
      text += " · " + Number(provider.todayPrompts || 0) + " prompts · "
        + Number(provider.todaySessions || 0) + " sessions"
    return text
  }

  function weekPeak(p) {
    var days = p ? (p.recentDays || []) : []
    var peak = 0
    for (var i = 0; i < days.length; i++) peak = Math.max(peak, Number(days[i].messageCount || 0))
    return peak
  }

  function modelRows(p) {
    var usageByModel = p ? (p.modelUsage || {}) : {}
    var rows = []
    for (var id in usageByModel) {
      var bucket = usageByModel[id] || {}
      var input = Number(bucket.inputTokens || 0)
      var output = Number(bucket.outputTokens || 0)
      var cacheRead = Number(bucket.cacheReadInputTokens || 0)
      var cacheWrite = Number(bucket.cacheCreationInputTokens || 0)
      rows.push({
        name: usage.friendlyModelName(id),
        total: input + output + cacheRead + cacheWrite,
        input: input,
        output: output,
        cacheRead: cacheRead,
        cacheWrite: cacheWrite
      })
    }
    rows.sort(function(a, b) { return b.total - a.total })
    return rows.slice(0, 4)
  }

  function modelTooltip(row) {
    if (!row) return ""
    return "In " + usage.formatTokenCount(row.input)
      + " · out " + usage.formatTokenCount(row.output)
      + " · cache read " + usage.formatTokenCount(row.cacheRead)
      + " · cache write " + usage.formatTokenCount(row.cacheWrite)
  }

  // Only speaks up when the numbers cover more than this machine.
  function footerText() {
    if (usage.syncStatusText !== "") return usage.syncStatusText
    if (provider && provider.syncEnabled && provider.syncDeviceCount > 0)
      return "Merged from " + provider.syncDeviceCount + " device" + (provider.syncDeviceCount === 1 ? "" : "s")
    return ""
  }

  // Agents that ship a white mark carry an `assets/<id>-light.svg` twin for
  // light surfaces; marks that work on both (Claude's brand-orange) ship one
  // file. The luminance check decides which candidate to try first.
  function colorChannelLuminance(value) {
    var channel = Number(value)
    if (!isFinite(channel)) return 0
    return channel <= 0.03928 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4)
  }

  function colorLuminance(color) {
    return 0.2126 * colorChannelLuminance(color.r)
      + 0.7152 * colorChannelLuminance(color.g)
      + 0.0722 * colorChannelLuminance(color.b)
  }

  // Marks resolve by convention, so a new agent's data file needs nothing
  // from this panel: assets/<id>.svg if it ships one, the module's bar glyph
  // if it doesn't.
  function iconCandidatesForProvider(p, surfaceColor) {
    if (!p) return []
    var candidates = []
    if (colorLuminance(surfaceColor || Color.background) >= 0.5)
      candidates.push(Qt.resolvedUrl("assets/" + p.providerId + "-light.svg"))
    candidates.push(Qt.resolvedUrl("assets/" + p.providerId + ".svg"))
    return candidates
  }

  // Nothing to report, nothing in the bar: Bar.qml collapses a slot whose item
  // is invisible, so the icon appears the moment the first scan finds usage and
  // stays away entirely on a machine that has never run either CLI.
  visible: providers.length > 0
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onProviderIndexChanged: if (panelFlick) panelFlick.contentY = 0
  onOpenedChanged: if (opened) {
    cursorActive = false
    nowMs = Date.now()
    if (panelFlick) panelFlick.contentY = 0
    usage.refreshLimits()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  Main {
    id: usage
    settings: root.settings
  }

  // Cheap enough to keep running: it only re-evaluates text bindings, and a
  // stale "resets in 2h" on a panel that is open is worse than a timer.
  Timer {
    interval: 30000
    running: root.opened
    repeat: true
    onTriggered: root.nowMs = Date.now()
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): string { root.refreshNow(); return "ok" }
    function next(): string { root.selectProvider(root.providerIndex + 1); return "ok" }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "󱚣"
    active: root.alarming
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) root.launchAgent()
      else if (buttonCode === Qt.MiddleButton) root.selectProvider(root.providerIndex + 1)
      else root.toggle()
    }
  }

  // ---------------------------------------------------------------- view
  //
  // Everything above is the stock panel, line for line. Below is its view on
  // the pixel grid (quadrille.bar/Q): the provider marks are sprites (Icons.qml)
  // instead of the svg pictures in assets/, the meters are stepped cells, the
  // sections are named rules, and nothing tweens.

  Icons { id: icons }

  // The provider's mark. An agent without one wears the bar's robot, as the
  // stock panel falls back to the bar glyph.
  function markFor(p) {
    var id = p ? String(p.providerId || "") : ""
    if (id === "claude") return icons.claude
    if (id === "codex") return icons.codex
    if (id === "fireworks") return icons.fireworks
    return Sprites.robot
  }

  // The same, 15 x 15, for the hero.
  function heroMarkFor(p) {
    var id = p ? String(p.providerId || "") : ""
    if (id === "claude") return icons.claude15
    if (id === "codex") return icons.codex15
    if (id === "fireworks") return icons.fireworks15
    return icons.robot15
  }

  function percentText(w) {
    return w && w.percent >= 0 ? Math.round(w.percent * 100) + "%" : "—"
  }

  function resetText(w) {
    var remainingMs = root.resetMsFor(w)
    return remainingMs > 0 ? "Resets in " + root.formatDuration(remainingMs) : ""
  }

  // The face has no euro sign; the code stands in for it.
  function faceText(text) { return String(text || "").replace(/€/g, "EUR ") }

  function widest(lines) {
    var n = 0
    for (var i = 0; i < lines.length; i++) n = Math.max(n, Glyphs.length(lines[i]))
    return n
  }

  // A tooltip's lines: its " · " parts joined while they fit `columns`, a part
  // longer than a line wrapped on its spaces. Nothing is cut.
  function tipLines(text, columns) {
    var parts = String(text || "").split(" · ")
    var joined = []
    var line = ""
    for (var i = 0; i < parts.length; i++) {
      var next = line === "" ? parts[i] : line + " · " + parts[i]
      if (line === "" || Glyphs.length(next) <= columns) line = next
      else { joined.push(line); line = parts[i] }
    }
    if (line !== "") joined.push(line)
    var out = []
    for (var j = 0; j < joined.length; j++)
      out = out.concat(Glyphs.length(joined[j]) > columns ? Glyphs.wrap(joined[j], columns, 4) : [joined[j]])
    return out
  }

  // The row under the pointer that has something to add, and whether its
  // tooltip is up yet: like the stock ToolTip, it waits 400 ms.
  property Item tipOwner: null
  property bool tipShown: false

  function hoverTip(item, on) {
    if (on) {
      tipOwner = item
      tipShown = false
      tipDelay.restart()
    } else if (tipOwner === item) {
      tipOwner = null
      tipShown = false
      tipDelay.stop()
    }
  }

  Timer {
    id: tipDelay
    interval: 400
    onTriggered: root.tipShown = root.tipOwner !== null
  }

  Connections {
    target: root
    function onOpenedChanged() {
      root.tipOwner = null
      root.tipShown = false
    }
  }

  QPopup {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(panel.cardWidth(46))
    // Taller than the control panels on purpose: this one is a dashboard, and
    // the whole point is reading limits and history without scrolling.
    contentHeight: panel.fittedContentHeight(column.implicitHeight, panel.g.px(640))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent

      onMoveRequested: function(dx, dy) {
        if (dx !== 0) {
          root.cursorActive = true
          root.selectProvider(root.providerIndex + dx)
        }
        if (dy !== 0) panelFlick.scrollBy(dy * panel.g.px(56))
      }
      onActivateRequested: root.refreshNow()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) { if (t === "r" || t === "R") root.refreshNow() }

      // The scroll mark sits in the card's padding, so the content is the full
      // inner width and the margins stay even.
      QScroll {
        id: panelFlick
        // The stock code above resets the view with `panelFlick.contentY = 0`.
        property alias contentY: panelFlick.offset
        width: parent.width + panel.padding
        height: parent.height
        contentHeight: column.implicitHeight

        Column {
          id: column
          width: parent.width
          spacing: panel.g.px(6)

          // ---------- Hero: provider mark · name · plan ----------
          // The mark is drawn at the hero's own size (15 x 15), not the tab's
          // 7 x 7 scaled: one pixel size in the whole card.
          Item {
            id: hero
            visible: !!root.provider
            width: parent.width
            height: 2 * panel.g.line

            readonly property real iconSpace: panel.g.px(15 + 4)
            readonly property int columns: panel.g.columns(width - iconSpace)

            Sprite {
              y: panel.g.centre(hero.height, height)
              rows: root.heroMarkFor(root.provider)
              level: 1
              color: Role.ink
              dim: Role.muted
            }
            Column {
              x: hero.iconSpace
              PixelText {
                text: root.provider ? root.provider.providerName : ""
                ink: Role.ink
                columns: hero.columns
              }
              PixelText {
                text: root.heroMeta(root.provider).toUpperCase()
                ink: Role.muted
                columns: hero.columns
              }
            }
          }

          // ---------- Nothing recorded yet ----------
          Item {
            visible: root.providers.length === 0
            width: parent.width
            height: emptyText.height + panel.g.px(2)

            Sprite {
              x: panel.g.px(3)
              y: panel.g.px(1) + panel.g.onCaps(7)
              rows: Sprites.robot
              color: Role.faint
            }
            PixelParagraph {
              id: emptyText
              x: panel.g.px(13)
              y: panel.g.px(1)
              // Never 0: Glyphs.wrap cannot break a word into lines of no cells.
              columns: Math.max(1, panel.g.columns(parent.width - x))
              maxLines: 4
              text: "No AI coding subscriptions found.\nAgents show up here once you've used them."
              ink: Role.muted
            }
          }

          // ---------- Provider switch ----------
          Row {
            id: providerSwitch
            visible: root.providers.length > 1
            width: parent.width
            spacing: panel.g.px(3)

            readonly property real cellWidth: root.providers.length > 0
              ? panel.g.floor((width - spacing * (root.providers.length - 1)) / root.providers.length)
              : 0
            // Cells left for a name beside the mark; a name that does not fit
            // is left out and the mark stays.
            readonly property int nameColumns: panel.g.columns(cellWidth - panel.g.px(7 + 3 + 2 * 3 + 2))

            Repeater {
              model: root.providers

              QButton {
                required property var modelData
                required property int index

                fixedWidth: providerSwitch.cellWidth
                icon: root.markFor(modelData)
                text: Glyphs.length(modelData.providerName) <= providerSwitch.nameColumns ? modelData.providerName : ""
                active: index === root.providerIndex
                hasCursor: root.cursorActive && index === root.providerIndex
                onClicked: {
                  root.cursorActive = true
                  root.selectProvider(index)
                }
                onHovered: function(isHovered) { if (isHovered) root.cursorActive = true }
              }
            }
          }

          // ---------- Status: sign-in and endpoint problems ----------
          Item {
            id: statusBlock
            visible: !!root.provider && String(root.provider.usageStatusText || "") !== ""
            width: parent.width
            height: Math.max(panel.g.line, statusText.height)

            readonly property string help: root.provider
              ? String(root.provider.authHelpText || root.provider.usageStatusText || "")
              : ""

            Sprite {
              x: panel.g.px(3)
              y: panel.g.onCaps(7)
              rows: Sprites.warning
              color: Role.alarm
            }
            PixelParagraph {
              id: statusText
              x: panel.g.px(13)
              columns: Math.max(1, panel.g.columns(parent.width - x))
              maxLines: 8
              text: statusBlock.help
              ink: Role.muted
            }
          }

          // ---------- Balance ----------
          Group {
            id: balanceSection
            name: "BALANCE"
            visible: !!root.balance
            width: parent.width
            height: implicitHeight

            // The meter shows what is left, not what is used: a prepaid
            // account drains toward empty rather than filling toward a cap.
            readonly property real ratio: root.balance && root.balance.funded > 0
              ? root.clamp(root.balance.remaining / root.balance.funded, 0, 1)
              : -1

            Column {
              width: parent.width
              spacing: panel.g.px(1)

              QReading {
                width: parent.width
                label: "Prepaid credits"
                labelInk: Role.ink
                value: root.balance ? root.faceText(root.formatMoney(root.balance.remaining, root.balance.currency)) : ""
                valueInk: root.balanceAlarming ? Role.alarm : Role.ink
              }
              Item {
                visible: balanceSection.ratio >= 0
                width: parent.width
                height: balanceMeter.height + panel.g.px(2)
                QMeter {
                  id: balanceMeter
                  width: parent.width
                  value: balanceSection.ratio
                  fill: root.balanceAlarming ? Role.alarm : Role.ink
                }
              }
              // Broken at its " · ", like a tooltip, so no dot hangs at a line's end.
              Repeater {
                model: root.tipLines(root.faceText(root.balanceDetailText(root.balance)),
                  Math.max(8, panel.g.columns(parent.width)))
                PixelText {
                  required property string modelData
                  visible: modelData !== ""
                  text: modelData
                  ink: Role.muted
                }
              }
            }
          }

          // ---------- Limits ----------
          Group {
            name: "LIMITS"
            visible: root.limits.length > 0
            width: parent.width
            height: implicitHeight

            Column {
              width: parent.width
              spacing: panel.g.px(4)

              Repeater {
                model: root.limits

                LimitRow {
                  required property var modelData
                  width: parent.width
                  window: modelData
                }
              }
            }
          }

          // ---------- Tokens by day ----------
          Group {
            id: usageSection
            name: "TOKENS BY DAY"
            visible: !!root.provider && root.provider.recentDays && root.provider.recentDays.length > 0
            width: parent.width
            height: implicitHeight

            readonly property var days: root.provider ? (root.provider.recentDays || []) : []
            readonly property real peak: Math.max(1, root.weekPeak(root.provider))
            // The figures' column, as wide as the widest of them (999.9M is six
            // cells), so a figure never reaches the bars.
            readonly property int figureCells: {
              var n = 6
              for (var i = 0; i < days.length; i++)
                n = Math.max(n, Glyphs.length(usage.formatTokenCount(Number(days[i].messageCount || 0))))
              return n
            }

            Column {
              width: parent.width
              spacing: panel.g.px(2)

              Repeater {
                model: usageSection.days

                DayRow {
                  required property var modelData
                  required property int index

                  width: parent.width
                  figureCells: usageSection.figureCells
                  day: modelData
                  ratio: Number(modelData.messageCount || 0) / usageSection.peak
                  // By date, not by position: the Claude stats-cache fallback can
                  // hand us a window that stops short of today.
                  today: String(modelData.date || "") === root.todayDate()
                }
              }
            }
          }

          // ---------- Tokens by model ----------
          Group {
            name: "TOKENS BY MODEL"
            visible: root.models.length > 0
            width: parent.width
            height: implicitHeight

            Column {
              width: parent.width
              spacing: panel.g.px(3)

              Repeater {
                model: root.models

                ModelRow {
                  required property var modelData
                  width: parent.width
                  row: modelData
                  // Scaled to the heaviest model, so the top row is always full:
                  // the same scale-to-peak the weekly chart uses for its busiest day.
                  share: modelData.total / Math.max(1, root.models[0].total)
                }
              }
            }
          }

          // ---------- Footer: only when the numbers cover more than this machine ----------
          Column {
            id: footer
            readonly property var lines: Glyphs.wrap(root.footerText(), Math.max(1, panel.g.columns(width)), 2)
            visible: root.footerText() !== ""
            width: parent.width

            Repeater {
              model: footer.lines
              PixelText {
                required property string modelData
                x: panel.g.centre(footer.width, width)
                text: modelData
                ink: Role.muted
              }
            }
          }
        }
      }
    }

    // Tooltips float over the card, outside the scrolled view, so the last row's
    // is not clipped.
    Item {
      id: tipLayer
      anchors.fill: parent
      z: 10

      AgentTip {
        owner: root.tipOwner
        shown: root.tipShown
        text: root.tipOwner ? root.tipOwner.tipText : ""
      }
    }
  }

  // A limit window: its title and the share used, the meter, and the time
  // until it resets. A title too long for the line (a model-scoped window is
  // named after its model) goes on to a second, never under the percentage.
  component LimitRow: Item {
    id: limitRow
    property var window: null

    readonly property var g: panel.g
    readonly property bool alarming: window && window.percent >= 0.9
    readonly property string percentLabel: root.percentText(window)
    readonly property var titleLines: Glyphs.wrap(window ? window.title : "",
      Math.max(1, g.columns(width) - Glyphs.length(percentLabel) - 1), 2)
    readonly property string reset: root.resetText(window)
    readonly property real meterY: titleLines.length * g.line + g.px(1)

    height: meterY + meter.height + (reset !== "" ? g.px(1) + g.line : 0)

    Column {
      Repeater {
        model: limitRow.titleLines
        PixelText {
          required property string modelData
          text: modelData
          ink: Role.ink
        }
      }
    }
    PixelText {
      x: limitRow.width - width
      text: limitRow.percentLabel
      ink: limitRow.alarming ? Role.alarm : Role.ink
    }
    QMeter {
      id: meter
      y: limitRow.meterY
      width: limitRow.width
      value: limitRow.window ? limitRow.window.percent : 0
      fill: limitRow.alarming ? Role.alarm : Role.ink
    }
    PixelText {
      visible: limitRow.reset !== ""
      y: meter.y + meter.height + limitRow.g.px(1)
      text: limitRow.reset
      ink: Role.muted
      columns: limitRow.g.columns(limitRow.width)
    }
  }

  // One row per day: label, bar, tokens. Today is in ink and the rest muted,
  // so the week reads as a run-up to right now. Hover for the day's detail.
  component DayRow: Item {
    id: dayRow
    property var day: null
    property real ratio: 0
    property bool today: false
    property int figureCells: 6

    readonly property var g: panel.g
    readonly property color ink: today ? Role.ink : Role.muted
    readonly property string tipText: root.dayTooltip(day, today)

    height: g.line

    PixelText {
      text: root.dayLabel(dayRow.day ? dayRow.day.date : "", dayRow.today)
      ink: dayRow.ink
      room: dayRow.g.cells(5)
    }
    QMeter {
      x: dayRow.g.cells(6)
      y: dayRow.g.onCaps(cellHeight)
      width: dayRow.width - dayRow.g.cells(6 + 1 + dayRow.figureCells)
      cellHeight: 6
      value: dayRow.ratio
      fill: dayRow.ink
    }
    PixelText {
      x: dayRow.width - width
      text: usage.formatTokenCount(dayRow.day ? Number(dayRow.day.messageCount || 0) : 0)
      ink: dayRow.ink
    }

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      acceptedButtons: Qt.NoButton
      onContainsMouseChanged: root.hoverTip(dayRow, containsMouse)
    }
  }

  // A model, the tokens it used, and a bar of its share scaled to the heaviest.
  // A long name goes on to a second line. Hover for the input / output / cache split.
  component ModelRow: Item {
    id: modelRow
    property var row: null
    property real share: 0

    readonly property var g: panel.g
    readonly property string figure: modelRow.row ? usage.formatTokenCount(modelRow.row.total) : ""
    readonly property var nameLines: Glyphs.wrap(row ? row.name : "",
      Math.max(1, g.columns(width) - Glyphs.length(figure) - 1), 2)
    readonly property string tipText: root.modelTooltip(row)

    height: nameLines.length * g.line + g.px(1) + bar.height

    Column {
      Repeater {
        model: modelRow.nameLines
        PixelText {
          required property string modelData
          text: modelData
          ink: Role.ink
        }
      }
    }
    PixelText {
      x: modelRow.width - width
      text: modelRow.figure
      ink: Role.muted
    }
    QMeter {
      id: bar
      y: modelRow.nameLines.length * modelRow.g.line + modelRow.g.px(1)
      width: modelRow.width
      cellHeight: 2
      value: modelRow.share
      fill: Role.muted
    }

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      acceptedButtons: Qt.NoButton
      onContainsMouseChanged: root.hoverTip(modelRow, containsMouse)
    }
  }

  // The tooltip: the legend on a raised hairline box (QTip's), wrapped to the
  // card and kept inside it, under the row it describes or over it at the bottom.
  component AgentTip: Item {
    id: tip
    property Item owner: null
    property bool shown: false
    property string text: ""

    readonly property var g: panel.g
    readonly property int maxColumns: Math.max(8, g.columns(parent ? parent.width : 0) - 2)
    readonly property var lines: root.tipLines(text, maxColumns)
    readonly property point at: {
      var scrolled = panelFlick.offset
      return owner && parent ? owner.mapToItem(parent, 0, 0) : Qt.point(0, 0)
    }
    readonly property real below: at.y + (owner ? owner.height : 0) + g.px(2)

    visible: shown && !!owner && text.length > 0
    z: 1000
    width: root.widest(lines) * g.cellW + g.px(2 * 3 + 2)
    height: lines.length * g.line + g.px(2)
    x: Math.max(0, Math.min((parent ? parent.width : 0) - width,
      g.snap(at.x) + g.centre(owner ? owner.width : 0, width)))
    y: below + height <= (parent ? parent.height : 0) ? g.snap(below) : g.snap(at.y - height - g.px(2))

    Rectangle {
      anchors.fill: parent
      antialiasing: false
      color: Role.edge
      Rectangle {
        anchors.fill: parent
        anchors.margins: tip.g.hair
        antialiasing: false
        color: Role.raised
      }
    }
    Column {
      x: tip.g.px(4)
      y: tip.g.px(1)
      Repeater {
        model: tip.lines
        PixelText {
          required property string modelData
          text: modelData
          ink: Role.ink
        }
      }
    }
  }
}
