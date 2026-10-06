import Quickshell
import Quickshell.Io
import QtQuick
import "quadrille.bar/Q"
import "quadrille.bar/Q/Glyphs.js" as Glyphs

// A bench for the ways of drawing a line of pixel text: how long a thousand of them take to make and to put on
// screen, and whether the pixels they draw are the same. Run by plugins/tools/surfaces/textbench.sh in the nested
// compositor; the shell logs `BENCH ...` lines.
ShellRoot {
  PanelWindow {
    id: win
    screen: Quickshell.screens.find(function(s) { return s.name === (Quickshell.env("BENCH_SCREEN") || "QA") }) || Quickshell.screens[0]
    anchors { top: true; left: true }
    implicitWidth: 760
    implicitHeight: 560
    color: Role.ground
    exclusionMode: ExclusionMode.Ignore

    Item {
      id: stage
      anchors.fill: parent
    }
  }

  Component { id: v0; PixelText { } }
  Component { id: v1; PixelTextMerged { } }
  Component { id: v2; PixelTextShape { } }

  property double t0: 0
  property double t1: 0
  property string pending: ""

  Timer {
    id: afterFrame
    interval: 1
    onTriggered: {
      var t2 = Date.now()
      console.log("BENCH " + root.pending + " create=" + (root.t1 - root.t0) + "ms then=" + (t2 - root.t1) + "ms")
    }
  }
  id: root

  function clearStage() {
    for (var i = stage.children.length - 1; i >= 0; i--) stage.children[i].destroy()
  }

  IpcHandler {
    target: "bench"
    function clear(): string { root.clearStage(); return "ok" }
    // run <variant 0|1|2> <count> <text>: `count` lines of text, laid out 12 vpx apart, wrapping into columns
    function run(variant: string, count: string, text: string): string {
      root.clearStage()
      var comp = variant === "1" ? v1 : (variant === "2" ? v2 : v0)
      var g = Px.forWindow(win)
      var n = parseInt(count, 10)
      var perCol = 36
      root.t0 = Date.now()
      for (var i = 0; i < n; i++) {
        var col = Math.floor(i / perCol), row = i % perCol
        comp.createObject(stage, { text: text + " " + i, x: col * g.px(120), y: row * g.px(14) })
      }
      root.t1 = Date.now()
      root.pending = "v" + variant + " n=" + n + " chars=" + (text.length + 3)
      afterFrame.restart()
      return "ok"
    }
  }
}
