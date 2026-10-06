#!/bin/bash
# Render foot with candidate font specs in a nested Hyprland at the laptop's scale
# (QA, 1.666667) and the ultrawide's (QB, 1), so a terminal font size can be
# judged on both without touching the real session's workspaces or windows.
#
#   tools/term-test.sh OUTDIR  QA:tag:fontspec  QB:tag:fontspec  ...
#
# Writes OUTDIR/<output>-<tag>.png (the top-left of the output, where the window
# is). Needs layershell/tools/nested.sh. A nested window is visible on the real
# session while this runs; run it under the live-session lock:
#   flock -w 900 /tmp/quadrille-live.lock tools/term-test.sh ...
set -u
here=$(cd "$(dirname "$0")" && pwd)
nested=$here/../layershell/tools/nested.sh
export QUADRILLE_NESTED_DIR=${QUADRILLE_NESTED_DIR:-/tmp/quadrille-nested-term}
out=${1:?usage: term-test.sh OUTDIR OUTPUT:tag:fontspec ...}; shift
mkdir -p "$out"
pattern=$here/term-test-pattern.txt

"$nested" up || exit 1
trap '"$nested" down' EXIT
trap 'exit 130' INT TERM
for spec in "$@"; do
  mon=${spec%%:*}; rest=${spec#*:}; tag=${rest%%:*}; font=${rest#*:}
  "$nested" ctl dispatch "hl.dsp.focus({ monitor = \"$mon\" })" >/dev/null; sleep 0.4
  if [ "$font" = wrapper ]; then   # tools/quadrille-foot picks the size itself
    cmd=("$here/quadrille-foot")
  else
    cmd=(foot -o "main.font=$font")
  fi
  "$nested" run "${cmd[@]}" -T pxtest -o main.pad=0x0 \
    -e sh -c "cat $pattern; sleep 5" >/dev/null 2>&1 &
  fpid=$!
  sleep 2.2
  "$nested" run grim -o "$mon" "$out/$mon-$tag.png"
  wait "$fpid" 2>/dev/null   # foot leaves when its command (the cat and a short sleep) does
  echo "$mon $tag $font"
done
