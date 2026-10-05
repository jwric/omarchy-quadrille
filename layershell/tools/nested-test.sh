#!/bin/bash
# The panel host against a nested Hyprland with two headless outputs (the
# laptop's 2560x1600 at 1.666667 and the ultrawide's 3440x1440 at 1), with real
# clicks and keys that can only reach that compositor: focus-grab dismissal,
# the control socket, the keyboard, live theme changes, live scale changes, and
# outputs coming and going.
#
#   tools/nested-test.sh [OUTDIR]
#
# The nested compositor is a window of the real session while it runs (about a
# minute). Screenshots and logs are left in OUTDIR.
set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
BIN=$HERE/target/release
N=$HERE/tools/nested.sh
OUT=${1:-${TMPDIR:-/tmp}/quadrille-nested-test}
mkdir -p "$OUT"; rm -f "$OUT"/*.png "$OUT"/*.log
RUNTIME=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
SOCK=quadrille-bar-nested.sock
THEMES=/usr/share/omarchy/themes
FAILS=0

pass() { echo "  ok    $*"; }
fail() { echo "  FAIL  $*"; FAILS=$((FAILS + 1)); }
check() { if eval "$2"; then pass "$1"; else fail "$1"; fi; }

ctl() { QUADRILLE_BAR_SOCKET=$SOCK XDG_RUNTIME_DIR=$RUNTIME "$BIN/quadrille-bar" ctl "$@"; }
# The nested compositor shows its own warnings as a banner over the top of the
# output, which would be in every screenshot: dismiss them first.
shot() { "$N" ctl dismissnotify >/dev/null 2>&1; sleep 0.2; "$N" run grim -o "$1" "$OUT/$2.png"; }
layers() { "$N" ctl layers; }
log_has() { grep -q "$1" "$OUT/bar.log"; }

# Output geometry, as the nested compositor has it: name -> "x y w h scale"
# (global position, physical size, scale).
monitor() {
  "$N" ctl monitors -j | python3 -c '
import json, sys
for m in json.load(sys.stdin):
    if m["name"] == sys.argv[1]:
        print(m["x"], m["y"], m["width"], m["height"], m["scale"])' "$1"
}

# Logical size of an output.
extent() {
  read -r _ _ w h s < <(monitor "$1")
  python3 -c "print(round($w / $s), round($h / $s))"
}

# A surface of a namespace on an output, as "x y w h" in logical pixels local
# to the output.
surface() {
  local output=$1 namespace=$2
  read -r mx my mw mh ms < <(monitor "$output")
  "$N" ctl layers | python3 -c '
import re, sys
text = sys.stdin.read()
out, ns, mx, my = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
block = re.search(r"Monitor " + re.escape(out) + r".*?(?=\nMonitor |\Z)", text, re.S)
if block:
    m = re.search(r"xywh: (-?\d+) (-?\d+) (\d+) (\d+), a: \d+, namespace: " + re.escape(ns), block.group(0))
    if m:
        print(int(m.group(1)) - mx, int(m.group(2)) - my, m.group(3), m.group(4))' "$output" "$namespace" "$mx" "$my"
}

# Pixel clicks, in logical pixels of an output.
click() {
  local output=$1 x=$2 y=$3
  read -r w h < <(extent "$output")
  VPTR_EXTENT="${w}x${h}" "$N" run "$BIN/vptr" "$output" move "$x" "$y" sleep 200 click sleep 300
}

# What `omarchy theme set` does to the state directory: stage, remove, move in,
# write the name.
set_theme() {
  local name=$1 source=$THEMES/$1
  [ -d "$source" ] || source=$HOME/.config/omarchy/themes/$1
  rm -rf "$OUT/current/next-theme"; mkdir -p "$OUT/current/next-theme"
  cp -r "$source/"* "$OUT/current/next-theme/"
  rm -rf "$OUT/current/theme"; mv "$OUT/current/next-theme" "$OUT/current/theme"
  echo "$name" > "$OUT/current/theme.name"
}

# The colour at logical (x, y) of a screenshot, as r,g,b.
pixel() {
  python3 - "$OUT/$1.png" "$2" "$3" "$4" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
x, y, s = float(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])
print(",".join(str(c) for c in im.getpixel((round(x * s), round(y * s)))))
PY
}

hex_rgb() { python3 -c "h='$1'.lstrip('#'); print(','.join(str(int(h[i:i+2],16)) for i in (0,2,4)))"; }
toml() { grep -E "^$2 = " "$OUT/current/theme/colors.toml" | cut -d'"' -f2; }

# Whether a screenshot region is pixel-crisp: few colours, every change on the grid.
crisp() {
  python3 "$HERE/tools/crisp.py" "$OUT/$1.png" "$2" "$3" "$1" --check | tee -a "$OUT/crisp.log" >/dev/null
  local status=${PIPESTATUS[0]}
  if [ "$status" != 0 ]; then
    { echo "== $1 not crisp"; "$N" ctl monitors; "$N" ctl layers; } >> "$OUT/crisp-failures.log" 2>&1
  fi
  return "$status"
}

cleanup() {
  ctl quit >/dev/null 2>&1; sleep 0.5
  "$N" down >/dev/null 2>&1
}
trap cleanup EXIT

echo "== nested compositor"
"$N" up || exit 1
sleep 0.5
for o in QA QB; do echo "   $o: $(monitor $o)"; done

echo "== ctl with no host running"
NOHOST=$(ctl list 2>&1); NOHOST_STATUS=$?
echo "   $(echo "$NOHOST" | head -1)"
check "ctl says the host is not running, and exits 1" "[ $NOHOST_STATUS = 1 ] && echo \"\$NOHOST\" | grep -q 'the host is not running'"

echo "== a bar on every output, at that output's scale"
mkdir -p "$OUT/current/theme"
cp "$HOME/.local/state/omarchy/current/theme.name" "$OUT/current/theme.name"
cp "$HOME/.local/state/omarchy/current/theme/colors.toml" "$OUT/current/theme/"
(
  "$N" run env QUADRILLE_BAR_SOCKET=$SOCK RUST_LOG=iced_layer=debug,quadrille_bar=info \
    ICED_LAYER_STATS=1 "$BIN/quadrille-bar" --output QA --output QB --theme-dir "$OUT/current" \
    --bar-tick-ms 0 > "$OUT/bar.log" 2>&1 &
)
for _ in $(seq 1 50); do ctl list >/dev/null 2>&1 && break; sleep 0.1; done
sleep 1.0
ctl list | sed 's/^/   /'
check "a bar on QA" "[ -n \"\$(surface QA quadrille-bar)\" ]"
check "a bar on QB" "[ -n \"\$(surface QB quadrille-bar)\" ]"
check "QA's bar is 1536x45 logical (25 vpx, exact at 1.6667)" "[ \"\$(surface QA quadrille-bar)\" = '0 0 1536 45' ]"
check "QB's bar is 3440x50 logical (25 vpx at scale 1)" "[ \"\$(surface QB quadrille-bar)\" = '0 0 3440 50' ]"
check "QA reserves 45 at the top" "\"$N\" ctl monitors | awk '/^Monitor QA/{f=1} f&&/reserved/{print; exit}' | grep -q 'reserved: 0 45'"
check "QB reserves 50 at the top" "\"$N\" ctl monitors | awk '/^Monitor QB/{f=1} f&&/reserved/{print; exit}' | grep -q 'reserved: 0 50'"
check "the list has both scales" "ctl list | grep -q 'QA .*scale 1.6667' && ctl list | grep -q 'QB .*scale 1.0000'"
check "the focus grab protocol is bound" "log_has 'hyprland_focus_grab_manager_v1: true'"
shot QA qa_bar; shot QB qb_bar
check "QA's bar is crisp (3 physical pixels to a virtual one)" "crisp qa_bar 0,0,2560,75 3"
check "QB's bar is crisp (2 to 1)" "crisp qb_bar 0,0,3440,50 2"

echo "== summon, and dismissal by a click outside"
ctl summon sysmon '{"output":"QA"}' | sed 's/^/   /'
sleep 1.4
echo "   sysmon on QA: $(surface QA quadrille-sysmon)"
check "the panel is 288x549 logical (305 vpx, exact)" "[ \"\$(surface QA quadrille-sysmon | cut -d' ' -f3-)\" = '288 549' ]"
check "it asked for a focus grab" "log_has 'focus grab on'"
check "there is no panel on QB" "[ -z \"\$(surface QB quadrille-sysmon)\" ]"
shot QA qa_sysmon
read -r PX PY PW PH < <(surface QA quadrille-sysmon)
PB="$(python3 -c "s=5/3; print(round($PX*s), round($PY*s), round(($PX+$PW)*s), round(($PY+$PH)*s))" | tr ' ' ',')"
check "the panel is crisp" "crisp qa_sysmon $PB 3"
check "a click inside the panel keeps it" "click QA $((PX + 100)) $((PY + 200)) && ctl list | grep -q 'sysmon .*visible'"
click QA 300 500
sleep 0.5
check "a click outside clears the grab" "log_has 'focus grab cleared'"
check "and the panel is gone" "[ -z \"\$(surface QA quadrille-sysmon)\" ]"
check "list says hidden" "ctl list | grep -q 'sysmon .*hidden'"

echo "== the bar's SYS button toggles it (the bar is in the grab)"
read -r W H < <(extent QA)
SYS_X=$((W - 31)); SYS_Y=22
click QA $SYS_X $SYS_Y
sleep 1.0
check "SYS shows the panel" "ctl list | grep -q 'sysmon .*visible'"
click QA $SYS_X $SYS_Y
sleep 0.8
check "SYS again hides it" "ctl list | grep -q 'sysmon .*hidden'"
grep -E "pointer button|focus grab|command" "$OUT/bar.log" | tail -12 | cut -c1-140 | sed 's/^/   /'

echo "== the control socket"
ctl hide >/dev/null
check "toggle shows" "ctl toggle sysmon | grep -q shown"
check "toggle hides" "ctl toggle sysmon | grep -q hidden"
check "summon on QB puts it on QB" "ctl summon sysmon '{\"output\":\"QB\"}' >/dev/null; sleep 1; [ -n \"\$(surface QB quadrille-sysmon)\" ] && [ -z \"\$(surface QA quadrille-sysmon)\" ]"
check "summon moves it to QA" "ctl summon sysmon '{\"output\":\"QA\"}' >/dev/null; sleep 1; [ -n \"\$(surface QA quadrille-sysmon)\" ] && [ -z \"\$(surface QB quadrille-sysmon)\" ]"
check "hide" "ctl hide sysmon | grep -q hidden"
check "an unknown panel is an error" "ctl summon nope 2>&1 | grep -q 'error: no panel'"
check "an unknown output is an error" "ctl summon sysmon '{\"output\":\"NOPE-1\"}' | grep -q 'error: no output'"
check "bad JSON is an error" "ctl summon sysmon '{oops' | grep -q 'error: arguments are not JSON'"
check "ctl exits 1 on an error" "! ctl summon nope >/dev/null 2>&1"

echo "== the keyboard, into the demo panel's text field"
ctl summon demo '{"output":"QA"}' >/dev/null
sleep 1.2
echo "   demo on QA: $(surface QA quadrille-demo)"
shot QA qa_demo
grep "keyboard focus" "$OUT/bar.log" | tail -2 | sed 's/^/   /'

# The id the host gave the demo panel's window, and whether it holds the keyboard.
demo_id() { grep 'namespace: "quadrille-demo"' "$OUT/bar.log" | tail -1 | sed -E 's/.*opening surface Id\(([0-9]+)\).*/\1/'; }
focused_on() { grep "keyboard focus" "$OUT/bar.log" | tail -1 | grep -q "keyboard focus: Id($1)\$"; }

DEMO=$(demo_id)
check "the demo panel holds the keyboard (the grab gives it to the popup)" "focused_on $DEMO"
if focused_on "$DEMO"; then
  read -r DX DY DW DH < <(surface QA quadrille-demo)
  FX=$((DX + DW / 2)); FY=$((DY + 94))
  click QA $FX $FY
  shot QA qa_demo_before
  "$N" run wtype -s 30 "pixel 123"
  sleep 0.6
  shot QA qa_demo_typed
  check "typing changed the field" "! cmp -s '$OUT/qa_demo_before.png' '$OUT/qa_demo_typed.png'"
  "$N" run wtype -k Escape
  sleep 0.6
  check "Escape closes the panel" "ctl list | grep -q 'demo .*hidden'"
else
  fail "no keyboard on the demo panel: not typing"
fi

echo "== the theme follows omarchy theme set"
for name in tokyo-night quadrille-paper quadrille-terminal white; do
  set_theme "$name"
  sleep 1.0
  shot QB "qb_$name"
  case "$name" in
    quadrille-paper) want=238,238,238 ;;
    quadrille-terminal) want=34,34,34 ;;
    *) want=$(hex_rgb "$(toml "$name" lighter_background)") ;;
  esac
  check "$name: the bar's ground is $want" "[ \"\$(pixel qb_$name 1500 8 1)\" = '$want' ]"
done
check "the host logged the changes" "log_has 'theme: omarchy:tokyo-night' && log_has 'theme: Paper'"
ctl summon sysmon '{"output":"QA"}' >/dev/null; sleep 1.2
set_theme tokyo-night; sleep 1.0
shot QA qa_sysmon_tokyo
set_theme quadrille-terminal; sleep 0.6
ctl hide >/dev/null

echo "== a scale change under live surfaces"
for scale in 1 2 1.25 1.666667; do
  "$N" ctl eval "hl.monitor({ output = 'QA', mode = '2560x1600@60', position = '20000x0', scale = $scale })" >/dev/null
  sleep 1.5
  read -r bx by bw bh < <(surface QA quadrille-bar)
  echo "   QA at $scale: bar $bx,$by ${bw}x${bh} logical"
  shot QA "qa_scale_$scale"
  case $scale in
    1) ps=2; want="0 0 2560 50" ;;
    2) ps=4; want="0 0 1280 50" ;;
    1.25) ps=3; want="0 0 2048 60" ;;
    1.666667) ps=3; want="0 0 1536 45" ;;
  esac
  pw=$(python3 -c "print(round($bw * $scale))"); ph=$(python3 -c "print(round($bh * $scale))")
  check "scale $scale: the bar is $want" "[ \"\$(surface QA quadrille-bar)\" = '$want' ]"
  check "scale $scale: the bar is crisp at $ps physical pixels to a virtual one" "crisp qa_scale_$scale 0,0,$pw,$ph $ps"
done

echo "== outputs come and go"
ctl summon sysmon '{"output":"QB"}' >/dev/null; sleep 1.2
check "the panel is on QB" "[ -n \"\$(surface QB quadrille-sysmon)\" ]"
"$N" ctl output remove QB >/dev/null
sleep 1.5
ctl list | sed 's/^/   /'
check "QB is gone from the list" "! ctl list | grep -q '^output QB'"
check "the panel moved to QA" "[ -n \"\$(surface QA quadrille-sysmon)\" ]"
ctl hide >/dev/null
"$N" ctl output create headless QB >/dev/null
sleep 2.0
check "QB is back, with a bar" "[ \"\$(surface QB quadrille-bar)\" = '0 0 3440 50' ]"

echo "== quit"
check "quit answers" "ctl quit | grep -q bye"
sleep 1
check "the process exited" "! pgrep -f 'quadrille-bar --output QA' >/dev/null"
check "the socket is gone" "[ ! -e \"$RUNTIME/$SOCK\" ]"


# ------------------------------------------------------------ panels only

# Physical box, "x0,y0,x1,y1", of a surface on an output, and its pixel scale.
box_of() {
  local output=$1 namespace=$2
  read -r _ _ _ _ scale < <(monitor "$output")
  read -r x y w h < <(surface "$output" "$namespace")
  python3 -c "s=$scale; print(round($x*s), round($y*s), round(($x+$w)*s), round(($y+$h)*s))" | tr ' ' ','
}
pixscale() { read -r _ _ _ _ scale < <(monitor "$1"); python3 -c "print(round(2 * $scale))"; }
rss_kb() { awk '/VmRSS/ {print $2}' "/proc/$1/status"; }
cpu_ns() { cat /proc/$1/task/*/schedstat 2>/dev/null | awk '{s+=$1} END {print s+0}'; }
ours_on_nested() { "$N" ctl layers | grep -c 'namespace: quadrille'; }

echo "== --no-bar: a service for the panels, beside another bar"
(
  "$N" run env QUADRILLE_BAR_SOCKET=$SOCK RUST_LOG=iced_layer=debug,quadrille_bar=info \
    "$BIN/quadrille-bar" --no-bar --theme-dir "$OUT/current" > "$OUT/nobar.log" 2>&1 &
)
for _ in $(seq 1 50); do ctl list >/dev/null 2>&1 && break; sleep 0.1; done
sleep 1.5
PID=$(pgrep -fn -- '--no-bar --theme-dir')
check "the host is up" "[ -n \"$PID\" ] && ctl list | grep -q 'output QA'"
check "no surface of ours on any output" "[ \"\$(ours_on_nested)\" = 0 ]"
check "no space is claimed on QA or QB" "! \"$N\" ctl monitors | awk '/^Monitor (QA|QB)/{f=1} /^Monitor WAYLAND/{f=0} f&&/reserved/' | grep -qv 'reserved: 0 0 0 0'"
C0=$(cpu_ns "$PID"); sleep 3; C1=$(cpu_ns "$PID")
IDLE_RSS=$(rss_kb "$PID")
echo "   idle, nothing ever shown: RSS $((IDLE_RSS / 1024)) MB, $(( (C1 - C0) / 1000 )) us of CPU in 3 s, $(ls /proc/$PID/task | wc -l) threads"
check "idle: no CPU to speak of" "[ $(( (C1 - C0) / 1000 )) -lt 2000 ]"
check "idle: under 10 MB resident" "[ $IDLE_RSS -lt 10240 ]"
check "ctl list names the outputs and the theme" "ctl list | grep -q 'QB .*scale 1.0000' && ctl list | grep -q 'theme'"

echo "== --no-bar: a panel on a named output, at that output's scale"
ctl summon sysmon '{"output":"QA"}' >/dev/null
sleep 1.4
check "the panel is 288x549 logical on QA" "[ \"\$(surface QA quadrille-sysmon | cut -d' ' -f3-)\" = '288 549' ]"
check "it is the only surface of ours" "[ \"\$(ours_on_nested)\" = 1 ]"
shot QA nb_qa_sysmon
check "the panel is crisp on QA" "crisp nb_qa_sysmon \$(box_of QA quadrille-sysmon) $(pixscale QA)"
check "list says where it is" "ctl list | grep -q 'sysmon .*visible on QA'"
check "it asked for a focus grab" "grep -q 'focus grab on' '$OUT/nobar.log'"
read -r PX PY PW PH < <(surface QA quadrille-sysmon)
check "a click inside keeps it" "click QA $((PX + 100)) $((PY + 200)) && ctl list | grep -q 'sysmon .*visible'"
click QA 300 700
sleep 0.6
check "a click outside dismisses it" "grep -q 'focus grab cleared' '$OUT/nobar.log' && ctl list | grep -q 'sysmon .*hidden'"
check "and nothing of ours is left on screen" "[ \"\$(ours_on_nested)\" = 0 ]"

echo "== --no-bar: with no output named, on the output that has the focus"
for target in QB QA QB; do
  read -r w h < <(extent "$target")
  VPTR_EXTENT="${w}x${h}" "$N" run "$BIN/vptr" "$target" move $((w / 2)) $((h / 2)) sleep 300
  ctl summon sysmon >/dev/null
  sleep 1.4
  where=$(ctl list | sed -n 's/^panel  sysmon .*visible on \([A-Za-z0-9-]*\).*/\1/p')
  echo "   pointer on $target: the panel is on ${where:-?}: $(surface "$target" quadrille-sysmon)"
  check "pointer on $target: the panel is on $target" "[ \"$where\" = $target ] && [ -n \"\$(surface $target quadrille-sysmon)\" ]"
  shot "$target" "nb_${target}_focus"
  want=$([ "$target" = QA ] && echo '288 549' || echo '320 610')
  check "pointer on $target: it is $want logical, the exact size at that scale" "[ \"\$(surface $target quadrille-sysmon | cut -d' ' -f3-)\" = '$want' ]"
  check "pointer on $target: it is crisp" "crisp nb_${target}_focus \$(box_of $target quadrille-sysmon) $(pixscale $target)"
  ctl hide >/dev/null
  sleep 0.5
done

echo "== --no-bar: the theme, and what is held afterwards"
ctl summon sysmon '{"output":"QA"}' >/dev/null
sleep 1.2
set_theme tokyo-night
sleep 1.0
shot QA nb_qa_tokyo
read -r PX PY PW PH < <(surface QA quadrille-sysmon)
check "a panel restyles live (the ground is tokyo-night's)" "[ \"\$(pixel nb_qa_tokyo $((PX + 5)) $((PY + 5)) 1.6666666)\" = 36,40,59 ]"
set_theme quadrille-terminal
sleep 0.5
ctl hide >/dev/null
sleep 1.0
C0=$(cpu_ns "$PID"); sleep 3; C1=$(cpu_ns "$PID")
WARM_RSS=$(rss_kb "$PID")
echo "   idle after showing panels: RSS $((WARM_RSS / 1024)) MB, $(( (C1 - C0) / 1000 )) us of CPU in 3 s, $(ls /proc/$PID/task | wc -l) threads"
check "idle again: no CPU to speak of" "[ $(( (C1 - C0) / 1000 )) -lt 2000 ]"
check "nothing of ours is on screen" "[ \"\$(ours_on_nested)\" = 0 ]"
check "no space is claimed" "! \"$N\" ctl monitors | awk '/^Monitor (QA|QB)/{f=1} /^Monitor WAYLAND/{f=0} f&&/reserved/' | grep -qv 'reserved: 0 0 0 0'"
check "quit answers" "ctl quit | grep -q bye"
sleep 1
check "the process exited" "! kill -0 $PID 2>/dev/null"

echo
echo "screenshots and logs in $OUT"
[ "$FAILS" = 0 ] && echo "all passed" || echo "$FAILS FAILED"
exit "$FAILS"
