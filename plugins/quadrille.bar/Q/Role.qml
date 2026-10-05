pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons

// quadrille's palette, as roles (`Role.ink`, `Role.ground`, ...). Nothing in the
// kit names a hex. (Not called Palette: QtQuick has a type of that name.)
//
// A quadrille theme (tools/gen_themes.py) states every role in a `[quadrille]`
// table of its shell.toml, which Color.shellValues carries and which follows
// theme switches. A theme without the table (a stock Omarchy theme, or an
// older generation of ours) still gets a full palette: each role is derived
// from the shell tokens that were generated from it, and live / caution /
// line are read from the theme's colors.toml, where they are green / yellow /
// brown.
QtObject {
  id: root

  readonly property var sv: Color.shellValues

  // Role `key` as stated by the theme, or `fallback`.
  function stated(key, fallback) {
    var v = sv["quadrille." + key]
    if (typeof v !== "string" || v.length === 0) return fallback
    var c = Qt.color(v)
    return c.valid ? c : fallback
  }

  function opaque(c) { return Qt.rgba(c.r, c.g, c.b, 1) }
  // `a` toward `b` by `t`, rounded to 8 bits like quadrille's theme::mix.
  function mix(a, b, t) {
    function ch(x, y) { return Math.round((x + (y - x) * t) * 255) / 255 }
    return Qt.rgba(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b), 1)
  }
  function shellColor(key, fallback) {
    var v = sv[key]
    if (typeof v !== "string" || v.length === 0) return fallback
    return opaque(Color.flatColor(v, fallback))
  }

  // colors.toml, for the hues shell.toml does not carry.
  property var terminal: ({})
  property FileView colorsFile: FileView {
    path: Color.currentThemePath + "/colors.toml"
    watchChanges: false
    printErrors: false
    onLoaded: root.terminal = root.parseColors(text())
    onLoadFailed: root.terminal = ({})
  }
  property Connections themeWatch: Connections {
    target: Color
    function onShellValuesChanged() { root.colorsFile.reload() }
  }
  function parseColors(raw) {
    var out = ({})
    var lines = String(raw || "").split("\n")
    for (var i = 0; i < lines.length; i++) {
      var m = lines[i].match(/^\s*([A-Za-z0-9_-]+)\s*=\s*["']?(#[0-9A-Fa-f]{6})/)
      if (m) out[m[1]] = m[2]
    }
    return out
  }
  function terminalColor(key, fallback) {
    var v = terminal[key]
    return typeof v === "string" ? Qt.color(v) : fallback
  }

  readonly property color void_: stated("void", opaque(Color.background))
  readonly property color ground: stated("ground", opaque(Color.bar.background))
  readonly property color raised: stated("raised", shellColor("tooltip.background", ground))
  // The next step along the surfaces, from the two before it.
  readonly property color hover: stated("hover", Qt.rgba(
    Math.max(0, Math.min(1, 2 * raised.r - ground.r)),
    Math.max(0, Math.min(1, 2 * raised.g - ground.g)),
    Math.max(0, Math.min(1, 2 * raised.b - ground.b)), 1))
  readonly property color edge: stated("edge", shellColor("controls.normal-border", muted))
  readonly property color ink: stated("ink", opaque(Color.foreground))
  readonly property color muted: stated("muted", opaque(Color.muted))
  readonly property color faint: stated("faint", mix(muted, ground, 0.5))
  readonly property color line: stated("line", terminalColor("brown", mix(muted, ground, 0.35)))
  readonly property color accent: stated("accent", opaque(Color.accent))
  readonly property color onAccent: stated("on_accent", shellColor("menu.selected-text", void_))
  readonly property color highlight: stated("highlight", mix(ink, ground, 0.7))
  readonly property color live: stated("live", terminalColor("green", ink))
  readonly property color caution: stated("caution", terminalColor("yellow", accent))
  readonly property color alarm: stated("alarm", opaque(Color.urgent))
}
