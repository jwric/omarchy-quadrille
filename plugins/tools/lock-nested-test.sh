#!/bin/bash
# Test the lock clone (plugins/quadrille.lock) end to end without touching the
# session: its Service.qml and LockView.qml run in a Quickshell of their own inside
# a nested Hyprland (layershell/tools/nested.sh), which takes the *session lock of
# the nested compositor* and nothing else. The real session is never locked.
#
#   plugins/tools/lock-nested-test.sh [OUTDIR]
#
# What is stubbed in the copy of the service (the rest is byte for byte the clone's):
#   * PAM: `configDirectory` points at a directory of this script's own, whose
#     `test` service is pam_deny.so, then pam_permit.so. The real password is never
#     asked for, no failed attempt is ever counted against the account (pam_faillock),
#     and the fingerprint reader is not touched (`echo no`).
#   * the commands the service runs while locked (wake, and blanking the screen and
#     the keyboard backlight with `omarchy-brightness-*`) are `true`.
# Input goes to the nested compositor only: nested.sh points WAYLAND_DISPLAY at it.
#
# The nested compositor is a window of the real session while it runs, so this takes
# the shared desktop lock (flock) and gives up after 150 s. Screenshots and the log
# are left in OUTDIR.
set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
REPO=$(cd "$HERE/.." && pwd)
N=$REPO/layershell/tools/nested.sh
LOCK=/tmp/quadrille-live.lock
if [ "${1:-}" != "--inner" ]; then
  OUT=${1:-${TMPDIR:-/tmp}/quadrille-lock-test}
  mkdir -p "$OUT"
  exec flock -w 900 "$LOCK" timeout -k 5 150 "$0" --inner "$OUT"
fi
OUT=$2
ROOT=$(mktemp -d /tmp/qlock-root.XXXXXX)
PAM=$(mktemp -d /tmp/qlock-pam.XXXXXX)
FAILS=0
pass() { echo "  ok    $*"; }
fail() { echo "  FAIL  $*"; FAILS=$((FAILS + 1)); }
cleanup() {
  "$N" run quickshell kill -p "$ROOT" >/dev/null 2>&1
  "$N" down >/dev/null 2>&1
  rm -rf "$ROOT" "$PAM"
}
trap cleanup EXIT

shell=${OMARCHY_PATH:-/usr/share/omarchy}/shell
for d in Commons Ui services; do ln -s "$shell/$d" "$ROOT/$d"; done
mkdir "$ROOT/svc"
cp -rL "$HERE/quadrille.lock/Q" "$ROOT/svc/Q" 2>/dev/null
cp "$HERE/quadrille.lock/LockView.qml" "$ROOT/svc/"
python3 - "$HERE/quadrille.lock/Service.qml" "$ROOT/svc/Service.qml" "$PAM" <<'PY'
import sys
src, dst, pam = sys.argv[1:4]
s = open(src).read()
def sub(a, b, count=1):
    global s
    assert a in s, a
    s = s.replace(a, b)
sub('command: ["bash", "-c", "omarchy-system-wake"]', 'command: ["true"]')
sub('command: ["bash", "-c", "omarchy-brightness-keyboard off; omarchy-brightness-display off"]', 'command: ["true"]')
sub('config: "omarchy-lock-password"', 'config: "test"\n    configDirectory: "%s"' % pam)
sub('config: "omarchy-lock-fingerprint"', 'config: "test"\n    configDirectory: "%s"' % pam)
sub('"if [[ -f /etc/pam.d/omarchy-lock-fingerprint ]]', '"echo no; exit 0; if [[ -f /etc/pam.d/omarchy-lock-fingerprint ]]')
sub('path: "/etc/pam.d/omarchy-lock-password"', 'path: "%s/test"' % pam)
open(dst, "w").write(s)
PY
printf 'auth required pam_deny.so\naccount required pam_permit.so\n' > "$PAM/test"
cat > "$ROOT/shell.qml" <<'QML'
import Quickshell
import "svc"
ShellRoot { Service { omarchyPath: "/usr/share/omarchy" } }
QML
# the clone's own Service imports qs.Commons; Q is the kit
ipc() { quickshell ipc -p "$ROOT" call lock "$@" 2>&1; }

echo "== nested compositor"
"$N" up >"$OUT/nested.log" 2>&1 || { fail "nested compositor did not start"; cat "$OUT/nested.log"; exit 1; }
"$N" run bash -c "nohup quickshell -p '$ROOT' >'$OUT/lock.log' 2>&1 &"
for i in $(seq 1 40); do grep -q "Configuration Loaded" "$OUT/lock.log" 2>/dev/null && break; sleep 0.25; done
grep -q "Configuration Loaded" "$OUT/lock.log" && pass "the lock clone loaded" || { fail "the lock clone did not load"; tail -20 "$OUT/lock.log"; exit 1; }
sleep 1.5
shot() { "$N" ctl dismissnotify >/dev/null 2>&1; sleep 0.3; "$N" run grim -o "$1" "$OUT/$2.png"; }

echo "== lock"
[ "$(ipc isLocked)" = false ] && pass "starts unlocked" || fail "starts unlocked: $(ipc isLocked)"
ipc lock >/dev/null
sleep 2.5
st=$(ipc status)
echo "$st" | grep -q '"secure":true' && pass "the nested compositor is locked and secure" || fail "not locked: $st"
shot QA lock-QA; shot QB lock-QB

echo "== typing, then a wrong password (pam_deny)"
"$N" run wtype "hunter2"; sleep 0.8
shot QA typed-QA
"$N" run wtype -k Return; sleep 2
st=$(ipc status)
echo "$st" | grep -q '"locked":true' && pass "still locked after a wrong password" || fail "unlocked by a wrong password: $st"
shot QA failed-QA
grep -q "Authentication failed" "$OUT/lock.log" && true
echo "== the right password (pam_permit): unlock"
printf 'auth required pam_permit.so\naccount required pam_permit.so\n' > "$PAM/test"
sleep 0.5
"$N" run wtype "x"; sleep 0.3; "$N" run wtype -k Return; sleep 2.5
[ "$(ipc isLocked)" = false ] && pass "unlocked by an accepted password" || fail "still locked: $(ipc status)"
shot QA unlocked-QA

echo
[ "$FAILS" = 0 ] && echo "lock clone: all checks passed" || echo "lock clone: $FAILS check(s) FAILED"
exit "$FAILS"
