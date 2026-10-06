#!/bin/bash
# Disposable live preview. Run only after the nested suite has passed:
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/overlay-live.sh OUT
set -euo pipefail
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(readlink -f "$HERE/../../..")
exec timeout --kill-after=2s 55s python3 "$HERE/overlay_live.py" "$REPO" "${1:-/tmp/quadrille-overlay-live}"
