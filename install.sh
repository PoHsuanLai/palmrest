#!/usr/bin/env bash
# Switch the Magic Mouse from the Python service (magic-mouse-kde) to palmrest, or back.
#
#   ./install.sh             build, install, carry the current feel over, switch services
#   ./install.sh --rollback  stop palmrest and start the Python service again
#
# Never run by the build or the tests: it changes which program owns the mouse's scrolling.
# It does not touch /etc/modprobe.d, /sys/module/hid_magicmouse or the udev rules. The kernel
# keeps emulate_scroll_wheel=0 (palmrest scrolls instead), and the existing hidraw rule already
# gives this user the access palmrest needs.
set -euo pipefail

PY_UNIT=magic-mouse-kde.service
UNIT=palmrest.service
BIN="$HOME/.local/bin/palmrest"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
CONF_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/palmrest"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

rollback() {
    echo "Rolling back to $PY_UNIT"
    systemctl --user disable --now "$UNIT" >/dev/null 2>&1 || true
    systemctl --user enable --now "$PY_UNIT"
    systemctl --user is-active "$PY_UNIT"
}

if [[ "${1:-}" == "--rollback" ]]; then
    rollback
    exit 0
fi

# 1. Build and install the binary.
(cd "$HERE" && cargo build --release)
install -Dm755 "$HERE/target/release/palmrest" "$BIN"

# 2. Carry the running feel over, once. The Python unit sets SCROLL_* in its environment;
#    palmrest reads the same values from gestures.toml. Under KDE the wheel output keeps the
#    device identity Plasma already has settings for (NaturalScroll), so nothing changes
#    direction on the switch; the virtual touchpad can be tried later by editing the file.
if [[ ! -e "$CONF_DIR/gestures.toml" ]]; then
    mkdir -p "$CONF_DIR"
    env_of() { systemctl --user show "$PY_UNIT" -p Environment --value 2>/dev/null | tr ' ' '\n' | sed -n "s/^$1=//p"; }
    speed="$(env_of SCROLL_SPEED)"
    accel="$(env_of SCROLL_ACCEL_MAX)"
    {
        echo "version = 1"
        if [[ ":${XDG_CURRENT_DESKTOP:-}:" == *":KDE:"* ]]; then echo 'foreign_output = "wheel"'; fi
        echo "[scroll]"
        if [[ -n "$speed" ]]; then echo "speed = $speed"; fi
        if [[ -n "$accel" ]]; then echo "repeat_gain_max = $(awk -v a="$accel" 'BEGIN { printf "%d", a * 1000 + 0.5 }')"; fi
    } > "$CONF_DIR/gestures.toml.tmp"
    mv "$CONF_DIR/gestures.toml.tmp" "$CONF_DIR/gestures.toml"
    echo "Wrote $CONF_DIR/gestures.toml:"
    cat "$CONF_DIR/gestures.toml"
fi

# 3. Switch: the Python service stops before palmrest starts, so exactly one scrolls.
install -Dm644 "$HERE/dist/palmrest.service" "$UNIT_DIR/$UNIT"
systemctl --user daemon-reload
systemctl --user disable --now "$PY_UNIT"
systemctl --user enable --now "$UNIT"

# 4. If palmrest does not stay up, give the mouse back to the Python service.
sleep 3
if ! systemctl --user is-active --quiet "$UNIT"; then
    echo "palmrest did not stay up:"
    journalctl --user -u "$UNIT" -n 20 --no-pager || true
    rollback
    exit 1
fi
echo "palmrest is running. Undo with: $0 --rollback"
