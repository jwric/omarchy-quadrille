#!/bin/bash
# A nested Hyprland to test input in, so that nothing here ever clicks or types
# on the real session.
#
#   tools/nested.sh up           start it (a window of the real session, for as
#                                long as it runs), wait for its socket, and add
#                                two headless outputs: QA, 2560x1600 at 1.666667
#                                like the laptop, and QB, 3440x1440 at 1 like the
#                                ultrawide (the window's own output is a third,
#                                of whatever size the real session gave it)
#   tools/nested.sh run CMD...   run CMD against it: WAYLAND_DISPLAY and
#                                HYPRLAND_INSTANCE_SIGNATURE point at it, and
#                                QUADRILLE_NESTED marks the display as safe for
#                                vptr; refuses if it is not up
#   tools/nested.sh ctl ARGS...  hyprctl, on it
#   tools/nested.sh bar-ctl ... isolated test host only (socket env required)
#   tools/nested.sh down         stop it and remove what it left
#
# Hyprland has no headless-only mode in 0.56 (a nested one is a window of the
# parent compositor), so `up` makes one briefly visible. Keep it short.
set -u

STATE=${QUADRILLE_NESTED_DIR:-${TMPDIR:-/tmp}/quadrille-nested}
RUNTIME=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}

die() { echo "nested.sh: $*" >&2; exit 1; }

read_state() {
  [ -f "$STATE/display" ] || die "not up (tools/nested.sh up)"
  DISPLAY_NAME=$(cat "$STATE/display"); SIGNATURE=$(cat "$STATE/signature"); PID=$(cat "$STATE/pid")
  kill -0 "$PID" 2>/dev/null || die "the nested compositor is gone"
  [ -n "$DISPLAY_NAME" ] && [ -n "$SIGNATURE" ] || die "incomplete state"
  [ "$DISPLAY_NAME" != "$(cat "$STATE/live")" ] || die "refusing: that is the real display"
  [ "$SIGNATURE" != "$(cat "$STATE/live_signature" 2>/dev/null)" ] || die "refusing: that is the real instance"
}

case "${1:-}" in
  bar-ctl)
    shift
    [ -n "${QUADRILLE_BAR_SOCKET:-}" ] || die "bar-ctl requires an isolated QUADRILLE_BAR_SOCKET"
    case "$QUADRILLE_BAR_SOCKET" in
      /*) control_path=$QUADRILLE_BAR_SOCKET ;;
      *) control_path=$RUNTIME/$QUADRILLE_BAR_SOCKET ;;
    esac
    control_path=$(readlink -m "$control_path")
    [ "$control_path" != /run/user/1000/quadrille-bar.sock ] &&
      [ "$control_path" != "$(readlink -m "$RUNTIME/quadrille-bar.sock")" ] || die "refusing the live host socket"
    QUADRILLE_BAR_SOCKET=$control_path exec "$(dirname "$0")/../target/release/quadrille-bar" ctl "$@"
    ;;
  up)
    [ -f "$STATE/pid" ] && kill -0 "$(cat "$STATE/pid")" 2>/dev/null && die "already up"
    rm -rf "$STATE"; mkdir -p "$STATE"
    [ -n "${WAYLAND_DISPLAY:-}" ] || die "no WAYLAND_DISPLAY to nest under"
    echo "$WAYLAND_DISPLAY" > "$STATE/live"
    echo "${HYPRLAND_INSTANCE_SIGNATURE:-}" > "$STATE/live_signature"
    cat > "$STATE/hl.lua" <<LUA
hl.config({
  general = { gaps_in = 0, gaps_out = 0, border_size = 0 },
  misc = { disable_hyprland_logo = true, disable_splash_rendering = true,
           disable_watchdog_warning = true },
  input = { follow_mouse = 1 },
})
hl.monitor({ output = "QA", mode = "2560x1600@60", position = "20000x0", scale = 1.666667 })
hl.monitor({ output = "QB", mode = "3440x1440@60", position = "30000x0", scale = 1 })
LUA
    before_sockets=$(ls "$RUNTIME" | grep -E '^wayland-[0-9]+$' | sort)
    before_instances=$(ls "$RUNTIME/hypr" 2>/dev/null | sort)
    # The Intel GPU, never the discrete one. The nested compositor must not
    # inherit the real one's identity.
    ( env -u HYPRLAND_INSTANCE_SIGNATURE HYPRLAND_NO_CRASHREPORTER=1 \
        AQ_DRM_DEVICES=/dev/dri/by-path/pci-0000:00:02.0-card \
        nohup Hyprland -c "$STATE/hl.lua" > "$STATE/hyprland.log" 2>&1 &
      echo $! > "$STATE/pid" )
    for _ in $(seq 1 100); do
      after_sockets=$(ls "$RUNTIME" | grep -E '^wayland-[0-9]+$' | sort)
      new=$(comm -13 <(echo "$before_sockets") <(echo "$after_sockets") | head -1)
      [ -n "$new" ] && break
      kill -0 "$(cat "$STATE/pid")" 2>/dev/null || die "Hyprland exited: $(tail -3 "$STATE/hyprland.log")"
      sleep 0.1
    done
    [ -n "${new:-}" ] || die "no new wayland socket"
    echo "$new" > "$STATE/display"
    for _ in $(seq 1 50); do
      sig=$(comm -13 <(echo "$before_instances") <(ls "$RUNTIME/hypr" 2>/dev/null | sort) | head -1)
      [ -n "$sig" ] && break
      sleep 0.1
    done
    [ -n "${sig:-}" ] || die "no new Hyprland instance"
    echo "$sig" > "$STATE/signature"
    sleep 1
    HYPRLAND_INSTANCE_SIGNATURE=$sig hyprctl output create headless QA >/dev/null
    HYPRLAND_INSTANCE_SIGNATURE=$sig hyprctl output create headless QB >/dev/null
    sleep 0.5
    echo "nested: display $new, instance $sig"
    ;;
  run)
    shift; read_state
    WAYLAND_DISPLAY=$DISPLAY_NAME QUADRILLE_NESTED=$DISPLAY_NAME HYPRLAND_INSTANCE_SIGNATURE=$SIGNATURE "$@"
    ;;
  ctl)
    shift; read_state
    HYPRLAND_INSTANCE_SIGNATURE=$SIGNATURE WAYLAND_DISPLAY=$DISPLAY_NAME hyprctl "$@"
    ;;
  env)
    read_state; echo "WAYLAND_DISPLAY=$DISPLAY_NAME HYPRLAND_INSTANCE_SIGNATURE=$SIGNATURE"
    ;;
  down)
    if [ -f "$STATE/pid" ]; then
      PID=$(cat "$STATE/pid")
      if kill -0 "$PID" 2>/dev/null; then
        # Never hyprctl here: with a missing signature it would talk to the
        # real session. SIGTERM is how Hyprland is asked to leave.
        kill "$PID"
        for _ in $(seq 1 50); do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
        kill -0 "$PID" 2>/dev/null && kill -9 "$PID"
      fi
      [ -f "$STATE/signature" ] && rm -rf "$RUNTIME/hypr/$(cat "$STATE/signature")"
    fi
    rm -rf "$STATE"
    echo "nested: down"
    ;;
  *) die "usage: nested.sh up | run CMD... | ctl ARGS... | bar-ctl ARGS... | env | down" ;;
esac
