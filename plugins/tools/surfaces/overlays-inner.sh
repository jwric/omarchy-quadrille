#!/bin/bash
# overlays-inner.sh SCREEN [CASES]: inside the nested compositor and a private bus (see overlays.sh).
# Opens each overlay in the scratch shell and marks the moment; the shell logs SURF lines
# (QUADRILLE_DEBUG_SURFACES=1) for the window and the card of each, which overlays-report.py reads.
# CASES: menu apps osd emojis reminders picker toast popup-menu (a popup, then the menu 300 ms later)
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done   # never hand the live lock to a child
cleanup() {
  kill $(jobs -p) 2>/dev/null
  for p in $(pgrep -f "[i]notifywait.*$S/rep"); do kill $p 2>/dev/null; done
  for p in $(pgrep -f "[q]uickshell -p .*$S/"); do kill $p 2>/dev/null; done
}
trap cleanup EXIT INT TERM
S=${SURF_WORK:-${TMPDIR:-/tmp}/quadrille-surfaces}
R=$S/rep
screen=$1; cases=${2:-"menu apps osd emojis reminders picker toast popup-menu"}
export HOME=$R/home OMARCHY_PATH=$R/omarchy QUADRILLE_PIXEL_TRAY=1 QUADRILLE_DEBUG_SURFACES=1 QUADRILLE_ONLY_SCREEN=$screen
q() { timeout 5 quickshell -p $R/omarchy/shell "$@"; }
env QT_QPA_PLATFORM=wayland quickshell -p $R/omarchy/shell > $S/overlays-$screen.log 2>&1 &
qpid=$!
for i in $(seq 1 20); do sleep 0.5; [ "$(q ipc call shell ping 2>&1)" = ok ] && break; done
echo "shell up after $i"
sleep 2.5
hyprctl dispatch "hl.dsp.focus({ monitor = \"$screen\" })" >/dev/null
sleep 0.5
mark() { echo "MARK $(date +%s%3N) $1"; }
rows=$(ls /usr/share/omarchy/themes/*/backgrounds/* | head -8 | awk '{printf "%s\t%s\n",$0,$0}')
payload=$(jq -cn --arg rows "$rows" '{imageRows:$rows,selectedImage:"x",selectionFile:"",doneFile:"",showLabels:true,filterable:true}')
for t in $cases; do
  case $t in
    menu)      mark "open menu"; q ipc call shell summon omarchy.menu '{"menu":"root"}' >/dev/null; sleep 1; q ipc call shell hide omarchy.menu >/dev/null;;
    apps)      mark "open apps"; q ipc call shell summon omarchy.menu '{"menu":"apps"}' >/dev/null; sleep 1.2; q ipc call shell hide omarchy.menu >/dev/null;;
    osd)       mark "open osd"; q ipc call shell summon omarchy.osd '{"icon":"volume-high","value":"60","max":"100","progressText":"60%","duration":"6000"}' >/dev/null; sleep 1; q ipc call shell hide omarchy.osd >/dev/null;;
    emojis)    mark "open emojis"; q ipc call shell summon omarchy.emojis '{}' >/dev/null; sleep 1; q ipc call shell hide omarchy.emojis >/dev/null;;
    reminders) mark "open reminders"; q ipc call shell summon omarchy.reminders '{}' >/dev/null; sleep 1; q ipc call shell hide omarchy.reminders >/dev/null;;
    picker)    mark "open picker"; q ipc call shell summon omarchy.image-picker "$payload" >/dev/null; sleep 1.2; q ipc call shell hide omarchy.image-picker >/dev/null;;
    toast)     mark "open toast"; notify-send -a Quadrille "Toast" "A body line" ; sleep 1.2; notify-send -a Quadrille -u critical "Second" "Under the first"; sleep 1.2;;
    popup-menu) mark "open popup power"; q ipc call omarchy.power toggle >/dev/null 2>&1; sleep 0.3
                mark "open menu after popup"; q ipc call shell summon omarchy.menu '{"menu":"root"}' >/dev/null; sleep 1
                q ipc call shell hide omarchy.menu >/dev/null; q ipc call omarchy.power close >/dev/null 2>&1;;
  esac
  sleep 0.8
done
kill $qpid; sleep 0.5; kill -9 $qpid 2>/dev/null
echo finished
