#!/usr/bin/env bash
# Build palmrest and install it as a user service.
#
# Never run by the build or the tests: it changes which program owns the mouse's scrolling.
# The two system files (hidraw access, kernel module options) need root; this script prints the
# commands for them instead of running sudo.
set -euo pipefail

UNIT=palmrest.service
BIN="$HOME/.local/bin/palmrest"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

(cd "$HERE" && cargo build --release)
install -Dm755 "$HERE/target/release/palmrest" "$BIN"
install -Dm644 "$HERE/dist/palmrest.service" "$UNIT_DIR/$UNIT"
systemctl --user daemon-reload
systemctl --user enable --now "$UNIT"
systemctl --user is-active "$UNIT"

if ! cmp -s "$HERE/dist/70-palmrest.rules" /etc/udev/rules.d/70-palmrest.rules \
    || ! cmp -s "$HERE/dist/99-palmrest.conf" /etc/modprobe.d/99-palmrest.conf; then
    cat <<STEPS
The system files are not installed yet. Run once:
  sudo install -Dm644 "$HERE/dist/70-palmrest.rules" /etc/udev/rules.d/70-palmrest.rules
  sudo install -Dm644 "$HERE/dist/99-palmrest.conf" /etc/modprobe.d/99-palmrest.conf
  echo 0 | sudo tee /sys/module/hid_magicmouse/parameters/emulate_scroll_wheel
  sudo udevadm control --reload-rules
  sudo udevadm trigger --subsystem-match=hidraw
  systemctl --user restart $UNIT
STEPS
fi
