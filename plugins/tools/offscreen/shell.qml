import QtQuick
import Quickshell
import qs.Commons

// Driver for plugins/tools/offscreen/run.sh: loads the component named by H_SRC,
// then plays the steps in H_STEPS (JSON): {"wait": ms}, {"call": "fn", "args": []},
// {"set": {"prop": value}}, {"eval": "js"} (evaluated here, `ld.item` is the
// component) and {"grab": "name"} (every visible window's stage to H_OUT/name.png).
ShellRoot {
  id: top
  property var steps: JSON.parse(Quickshell.env("H_STEPS") || "[]")
  property int at: 0
  property string out: Quickshell.env("H_OUT") || "/tmp"

  Loader {
    id: ld
    source: Quickshell.env("H_SRC")
    onLoaded: { console.log("loaded", item); stepTimer.start() }
    onStatusChanged: if (status === Loader.Error) { console.log("LOAD ERROR"); Qt.quit() }
  }

  function windows() {
    var list = []
    var it = ld.item
    if (!it) return list
    var d = it.data
    for (var i = 0; i < d.length; i++) if (d[i] && d[i].stage !== undefined && d[i].visible) list.push(d[i])
    return list
  }

  Timer { id: stepTimer; interval: 50; repeat: false; onTriggered: top.next() }

  function next() {
    if (at >= steps.length) { Qt.quit(); return }
    var s = steps[at++]
    var wait = 50
    if (s.call) {
      try { var r = ld.item[s.call].apply(ld.item, s.args || []); console.log("call", s.call, "->", r) } catch (e) { console.log("call error", s.call, e) }
    } else if (s.set) {
      for (var k in s.set) ld.item[k] = s.set[k]
    } else if (s.wait) {
      wait = s.wait
    } else if (s.grab) {
      var ws = windows()
      for (var i = 0; i < ws.length; i++) {
        ;(function(w, i) {
          w.stage.grabToImage(function(r) { r.saveToFile(top.out + "/" + s.grab + (ws.length > 1 ? "-" + i : "") + ".png") })
        })(ws[i], i)
      }
      wait = 400
    } else if (s.eval) {
      try { console.log("eval", JSON.stringify(eval(s.eval))) } catch (e) { console.log("eval error", e) }
    }
    stepTimer.interval = wait
    stepTimer.start()
  }
}
