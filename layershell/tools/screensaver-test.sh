#!/bin/bash
# The screensaver in the nested Hyprland (tools/nested.sh up first): a
# surface on each output in its namespace, a second instance leaving at
# once, input in the first second ignored, then the pointer moving and a key
# each ending it; screenshots of both outputs, and what it costs while it
# runs. Nothing here reaches the real session: input goes through vptr and
# wtype on the nested display only.
#
#   tools/screensaver-test.sh [OUT]     screenshots and the log go to OUT
set -u

here=$(cd "$(dirname "$0")" && pwd)
N=$here/nested.sh
BIN=$here/../target/release
OUT=${1:-${TMPDIR:-/tmp}/quadrille-screensaver-test}
NS=quadrille-screensaver-test
failures=0

mkdir -p "$OUT"
"$N" env >/dev/null || exit 1

fail() { echo "FAIL: $*"; failures=$((failures + 1)); }
pass() { echo "ok: $*"; }

saver() { pgrep -f "^[^ ]*/quadrille-screensaver --namespace $NS" | head -1; }
layers() { "$N" ctl -j layers | jq --arg ns "$NS" '[.. | objects | select(.namespace? == $ns)] | length'; }
start() {
  "$N" run "$BIN/quadrille-screensaver" --namespace "$NS" "$@" >>"$OUT/saver.log" 2>&1 &
  for _ in $(seq 1 50); do [ -n "$(saver)" ] && [ "$(layers)" -ge 2 ] && return; sleep 0.1; done
}
gone_within() { # SECONDS
  for _ in $(seq 1 $(($1 * 10))); do [ -z "$(saver)" ] && return 0; sleep 0.1; done
  return 1
}
cpu() { # PID SECONDS: percent of one core over the interval
  local a b
  a=$(awk '{print $14 + $15}' "/proc/$1/stat"); sleep "$2"
  b=$(awk '{print $14 + $15}' "/proc/$1/stat")
  awk -v a="$a" -v b="$b" -v hz="$(getconf CLK_TCK)" -v s="$2" 'BEGIN { printf "%.1f", (b - a) * 100 / hz / s }'
}
size() { # OUTPUT: its width and height in logical pixels
  "$N" ctl -j monitors | jq -r --arg o "$1" '.[] | select(.name == $o) | "\(.width / .scale | floor) \(.height / .scale | floor)"'
}
move() { # OUTPUT X Y [X Y]...
  local output=$1; shift
  read -r w h <<<"$(size "$output")"
  local steps=()
  while [ $# -gt 0 ]; do steps+=(move "$1" "$2" sleep 120); shift 2; done
  VPTR_EXTENT="${w}x${h}" "$N" run "$BIN/vptr" "$output" "${steps[@]}" >/dev/null
}

: >"$OUT/saver.log"
[ -z "$(saver)" ] || { echo "a test screensaver is already running"; exit 1; }

# 1. Surfaces on every output, in the namespace asked for.
start --subject gears
pid=$(saver)
[ -n "$pid" ] || { fail "it did not start: $(tail -3 "$OUT/saver.log")"; exit 1; }
count=$(layers)
outputs=$("$N" ctl -j monitors | jq length)
[ "$count" -eq "$outputs" ] && pass "$count surfaces for $outputs outputs" || fail "$count surfaces for $outputs outputs"

# 2. A second one leaves and the first stays.
"$N" run "$BIN/quadrille-screensaver" --namespace "$NS" >>"$OUT/saver.log" 2>&1
[ "$(saver)" = "$pid" ] && pass "a second instance left" || fail "a second instance changed things"

# 3. What it costs while it plots and while it runs; and what it looks like.
echo "cpu while plotting: $(cpu "$pid" 3)% of a core"
sleep 9
echo "cpu while running: $(cpu "$pid" 5)% of a core, rss $(awk '/VmRSS/ {print $2 / 1024 " MiB"}' "/proc/$pid/status")"
for o in QA QB; do "$N" run grim -o "$o" "$OUT/$o.png" && pass "screenshot $OUT/$o.png"; done

# 4. The pointer moving ends it.
move QA 400 400 460 420
gone_within 2 && pass "the pointer moving ended it" || { fail "still running after the pointer moved"; kill "$(saver)" 2>/dev/null; }

# 5. Input in the first second is the one that started it: ignored.
start --subject gears
move QA 300 300 380 360
sleep 0.3
[ -n "$(saver)" ] && pass "movement in the first second was ignored" || fail "movement in the first second ended it"

# 6. A key ends it.
sleep 1.2
"$N" run wtype -k space
gone_within 2 && pass "a key ended it" || { fail "still running after a key"; kill "$(saver)" 2>/dev/null; }

echo "$failures failure(s)"
exit "$failures"
