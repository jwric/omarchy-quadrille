#!/bin/bash
# An isolated Quickshell config root to try a component in, without touching the
# running shell: `qs.Commons` and `qs.Ui` resolve (they are the shell's own,
# linked), the theme is the live one, and the quadrille plugins are linked by
# name, so a test shell.qml can `import "quadrille.bar/Q"`.
#
#   plugins/tools/testroot.sh DIR        create DIR, then put a shell.qml in it
#   quickshell -n -p DIR                 run it (a window of the real session for as
#                                        long as it runs: keep it to seconds)
#
# Nothing here owns a D-Bus name the shell owns (no notification server, no
# polkit agent): it is only the components under test.
set -euo pipefail
dir=${1:?usage: testroot.sh DIR}
shell=${OMARCHY_PATH:-/usr/share/omarchy}/shell
here=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$dir"
for d in Commons Ui services plugins; do ln -sfn "$shell/$d" "$dir/$d"; done
for p in "$here"/quadrille.*; do ln -sfn "$p" "$dir/$(basename "$p")"; done
echo "$dir ready"
