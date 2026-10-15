#!/usr/bin/env bash
# Build palmrest and install it.
#
#   ./install.sh [--dry-run] [--prefix P] [--destdir D] [--no-build]
#
#   (no flags)    a user install: the program in ~/.local/bin and a user service that is enabled
#                 and started. The two system files (hidraw and uinput access, kernel module
#                 options) and the module-load file need root; the script prints the commands for
#                 them instead of running them.
#   --prefix P    a packaged layout under P: P/bin/palmrest, the user unit in
#                 P/lib/systemd/user (its ExecStart names P/bin/palmrest), and the system files in
#                 P/lib/udev/rules.d, P/lib/modprobe.d and P/lib/modules-load.d. Nothing is
#                 enabled or started. Root is used for the copy only when needed ($SUDO, sudo,
#                 doas or pkexec, whichever is found first).
#   --destdir D   stage the packaged layout under D as you (no root, no systemd, no udev); the
#                 prefix defaults to /usr/local. $DESTDIR works too.
#   --no-build    use the binary that is already built (or $PALMREST_BIN)
#   --dry-run     print every command, change nothing (does not build)
#
# Never run by the build or the tests: it changes which program owns the mouse's scrolling.
set -euo pipefail

UNIT=palmrest.service
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESTDIR="${DESTDIR:-}"
PREFIX=""
BUILD=yes
DRY_RUN=no

say() { printf '%s\n' "$*"; }

absolute_arg() {
  case "$2" in
    /*) local v="${2%/}"; printf '%s' "${v:-/}" ;;
    *) say "install.sh: $1 needs an absolute path"; exit 2 ;;
  esac
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=yes ;;
    --prefix) PREFIX="$(absolute_arg --prefix "${2:?--prefix needs a directory}")"; shift ;;
    --destdir) DESTDIR="$(absolute_arg --destdir "${2:?--destdir needs a directory}")"; shift ;;
    --no-build) BUILD=no ;;
    -h|--help) sed -n '2,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) say "install.sh: unknown argument $1 (try --help)"; exit 2 ;;
  esac
  shift
done
[[ -z "$DESTDIR" || -n "$PREFIX" ]] || PREFIX=/usr/local

# The words that put a command in front of root ($SUDO may hold arguments), empty when none.
ESCALATE=()
pick_escalator() {
  local candidate words
  for candidate in "${SUDO:-}" sudo doas pkexec; do
    [[ -n "$candidate" ]] || continue
    read -ra words <<<"$candidate"
    if command -v "${words[0]}" >/dev/null 2>&1; then
      ESCALATE=("${words[@]}")
      return 0
    fi
  done
  return 0
}
pick_escalator

# The escalation word for a printed command line: the helper found, or nothing (run as root).
printed_root() {
  if [[ "$(id -u)" == 0 ]]; then return 0; fi
  if (( ${#ESCALATE[@]} > 0 )); then printf '%s ' "${ESCALATE[@]}"; else printf ''; fi
}

# set_layout: where each file goes. The user install keeps the program and unit in the home and the
# system files in /etc (printed, not run); a prefix is a package layout with the vendor directories.
set_layout() {
  if [[ -z "$PREFIX" ]]; then
    LAYOUT=user
    BINDIR="$HOME/.local/bin"
    UNITDIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
    UDEVDIR=/etc/udev/rules.d
    MODPROBEDIR=/etc/modprobe.d
    MODULESDIR=/etc/modules-load.d
  else
    LAYOUT=package
    BINDIR="$PREFIX/bin"
    UNITDIR="$PREFIX/lib/systemd/user"
    UDEVDIR="$PREFIX/lib/udev/rules.d"
    MODPROBEDIR="$PREFIX/lib/modprobe.d"
    MODULESDIR="$PREFIX/lib/modules-load.d"
  fi
}
set_layout

# run DESCRIPTION CMD...: print, then run unless this is a dry run.
run() {
  local what="$1"; shift
  say "  $what"
  say "      \$ $*"
  [[ "$DRY_RUN" == yes ]] || "$@"
}

# put_file MODE SRC DST: copy SRC to DST (under DESTDIR), through the escalation helper when a
# machine path needs root and we are neither root nor staging.
put_file() {
  local mode="$1" src="$2" dst="$3"
  if [[ -n "$DESTDIR" || "$LAYOUT" == user || "$(id -u)" == 0 ]]; then
    run "install $dst" install -Dm"$mode" "$src" "$DESTDIR$dst"
  elif (( ${#ESCALATE[@]} > 0 )); then
    run "install $dst" "${ESCALATE[@]}" install -Dm"$mode" "$src" "$dst"
  else
    say "  install $dst"
    say "      (no sudo, doas or pkexec here: run this as root)"
    say "      \$ install -Dm$mode $src $dst"
  fi
}

BIN="${PALMREST_BIN:-$HERE/target/release/palmrest}"
if [[ "$BUILD" == yes ]]; then
  if [[ "$DRY_RUN" == yes ]]; then
    say "  would run: cargo build --release --locked (in $HERE)"
  else
    (cd "$HERE" && cargo build --release --locked)
  fi
fi

# The unit names the installed program, never the DESTDIR-prefixed path. The user install keeps the
# shipped %h/.local/bin form.
RENDERED="$(mktemp -d)"
trap 'rm -rf "$RENDERED"' EXIT
if [[ "$LAYOUT" == user ]]; then
  cp "$HERE/dist/palmrest.service" "$RENDERED/$UNIT"
else
  sed "s|^ExecStart=.*|ExecStart=$BINDIR/palmrest|" "$HERE/dist/palmrest.service" >"$RENDERED/$UNIT"
fi

put_file 755 "$BIN" "$BINDIR/palmrest"
put_file 644 "$RENDERED/$UNIT" "$UNITDIR/$UNIT"

if [[ "$LAYOUT" == package ]]; then
  put_file 644 "$HERE/dist/70-palmrest.rules" "$UDEVDIR/70-palmrest.rules"
  put_file 644 "$HERE/dist/99-palmrest.conf" "$MODPROBEDIR/99-palmrest.conf"
  put_file 644 "$HERE/dist/palmrest-modules.conf" "$MODULESDIR/palmrest.conf"
  say "Installed under $DESTDIR$PREFIX. Per person: systemctl --user enable --now $UNIT"
  exit 0
fi

run "reload the user manager" systemctl --user daemon-reload
run "enable and start palmrest" systemctl --user enable --now "$UNIT"
[[ "$DRY_RUN" == yes ]] || systemctl --user is-active "$UNIT"

if ! cmp -s "$HERE/dist/70-palmrest.rules" "$UDEVDIR/70-palmrest.rules" \
    || ! cmp -s "$HERE/dist/99-palmrest.conf" "$MODPROBEDIR/99-palmrest.conf" \
    || ! cmp -s "$HERE/dist/palmrest-modules.conf" "$MODULESDIR/palmrest.conf"; then
  ROOT="$(printed_root)"
  WHO=""
  [[ -n "$ROOT" || "$(id -u)" == 0 ]] || WHO=", as root (no sudo, doas or pkexec was found)"
  cat <<STEPS
The system files are not installed yet. Run once$WHO:
  ${ROOT}install -Dm644 "$HERE/dist/70-palmrest.rules" $UDEVDIR/70-palmrest.rules
  ${ROOT}install -Dm644 "$HERE/dist/99-palmrest.conf" $MODPROBEDIR/99-palmrest.conf
  ${ROOT}install -Dm644 "$HERE/dist/palmrest-modules.conf" $MODULESDIR/palmrest.conf
  ${ROOT}modprobe uinput
  echo 0 | ${ROOT}tee /sys/module/hid_magicmouse/parameters/emulate_scroll_wheel
  ${ROOT}udevadm control --reload-rules
  ${ROOT}udevadm trigger --subsystem-match=hidraw --subsystem-match=misc
  systemctl --user restart $UNIT
STEPS
fi
