# palmrest

The Magic Mouse 2 gesture daemon of the desktop shell program. It reads the mouse's touch
reports from hidraw, turns them into scroll and gesture streams, and delivers them over a socket to shell-host surfaces and, through
virtual input devices, to every other client. Specified in quire's
`design/12-BEHAVIOUR-gestures.md` (and `design/11-BEHAVIOUR-scroll.md` for how the host uses
the stream; `design/22-SETTINGS.md` §3.9-3.10 for the settings). Portable to any Linux; no
shell dependency. Pointer motion and physical clicks stay in the kernel driver.

## Architecture

```
node ──read──► Sensor::decode ──► stamp ──► Tracker::step ──► Vec<Out> ──► dispatch
     (bytes)   (Vec<Frame>, mm)   (time)    (pure state)              │
                                                                   ├─ Host(ToHost) ─► socket thread ─► subscribers
                                                                   ├─ Pad / Wheel / Key ─► Emitter ─► input service (turnstile)
                                                                   └─ Action ─► action::plan ─► spawn
the shell ── PointerOver ──► socket thread ── claim ──► Tracker::claim (silences foreign outputs)
subscribers ── SessionLock ──► socket thread ── lock ──► Tracker::with_session_lock (no gesture keys or actions)
gestures.toml ── inotify, 30 ms debounce ──► Tracker::with_config
```

The `Sensor` seam mirrors `Emitter` on the output side: `MagicMouseHid` (the hidraw node,
decoded by palmrest) or `EvdevMultitouch` (a mouse's evdev node through its kernel multitouch
driver) turns reads into millimetre `Frame`s, and everything after it is device-generic (see
[Sensors](#sensors)).

Two threads and no async runtime: the **reader** (poll on the sensor's fd, read, decode, step,
emit, and every housekeeping step between reads) and the **socket** thread (accept, fan-out,
`PointerOver` and `SessionLock` aggregation), woken by an eventfd so the reader never waits on a
subscriber. palmrest is a system service (a dynamic user, as keycap is), so the socket is open
to every local account and the socket thread decides whom to serve: only the person at the seat
(logind's `seat0` state file, read when someone connects and once a second), and of their programs
only the shell (the process in `sill.service` under that person's own manager, held by its pidfd,
judged by peephole) is believed about the pointer and an unlocked session; any other program of
theirs may subscribe and may say the session is locked. A user switch closes the old user's
connections. At most 8 subscribers are served; each pass reads at most 16 packets from one, and a
subscriber is known by an id, never by its place in the list. If the socket thread cannot poll,
the process exits and systemd restarts it (release builds abort on any panic).

The pure core takes bytes and instants and returns values; nothing in it reads a clock or
touches a device, so a recording replays to exactly the outputs the live daemon produced.
The core computes on an integer grid fixed in millimetres, the Magic Mouse 2's own (26
units/mm across, 70 along), so the Magic Mouse keeps its reference feel exactly and any other
sensor's millimetres feel the same per millimetre.

| Module | What it is |
| --- | --- |
| `sensor` | the `Sensor` trait, `Frame`, `Surface`, millimetre units, the reference grid (`lattice`) |
| `sensor::magic_mouse` | `MagicMouseHid`; `decode`, `encode`, `report`: report 0x12 and 0xF7 doubles to `Report`s and back (12.3.1); the known-device list |
| `sensor::evdev` | `EvdevMultitouch`: `ABS_MT_*` slots and `SYN_REPORT` times (untested beyond the Magic Mouse) |
| `touch` | the Magic Mouse's touch block, and the grid `Point` the core works in |
| `stamp` | the 18-bit device timestamp fitted onto `CLOCK_MONOTONIC` (12.3.1, acceptance 12); evdev's kernel stamps |
| `feel` | axis lock, gains, px, wheel quantisation, natural direction (12.3.8) |
| `gestures` | contacts and palm/rest rejection P1-P5, scroll phases, G4 one-finger swipe, G5 workspace swipe, G6/G7 double-taps |
| `tracker` | `Tracker::step(self, &Report, now) -> (Self, Vec<Out>)`: all of the above composed (12.4) |
| `action`, `keys` | the gesture -> action table and what an action means on KDE |
| `palmrest-protocol` (crate) | the socket messages and their codec (12.3.10) |
| `config` | `gestures.toml`, lenient per key, the watch |
| `outputs` | the `Emitter` seam, `RecordingEmitter`, the virtual touchpad model, evdev event encoding |
| `recording`, `replay` | the `palmrest record` file (v1 hidraw, v2 any sensor) and the replay used by every acceptance test |
| `daemon` | threads, socket, discovery and backoff, the sensor node (hidraw or evdev), devices, record, latency, the virtual-device emitter over the input service (and the direct fallback), and `sys`: the ioctls, the only module allowed `unsafe` |

## Sensors

`[device] source` picks the sensor; it is read when the daemon looks for a device, so a change
applies at the next reconnect.

- **`"hidraw"`** (default): `MagicMouseHid`. The node is found by `HID_ID` in a list of known
  mice: the Bluetooth Magic Mouse 2 (`004c:0269`) is the one verified. Listed but **untested**:
  the Magic Mouse 2 on its cable (`05ac:0269`), the USB-C model (`0323`, id not read from a
  device), and the Magic Mouse 1 (`05ac:030d`), whose report 0x29 palmrest does not decode;
  the daemon refuses it and points at `"evdev"`.
- **`"evdev"`**: `EvdevMultitouch`, a mouse's `/dev/input/eventN` with multitouch slots, as the
  kernel's hid-magicmouse (or any other mouse's multitouch driver) exposes them. **Untested on
  anything but the Magic Mouse**, and there only with synthetic event sequences and a
  read-only open of the real node. The node is opened read-only, switched to
  `CLOCK_MONOTONIC` timestamps for this reader alone, and never grabbed. Touchpads are never
  read (udev's `ID_INPUT_TOUCHPAD`, or `BTN_TOOL_FINGER`): they stay with libinput, and
  shell-host already runs their scroll physics. A node that does not state its resolution is
  refused rather than guessed at. The Magic Mouse's node carries no contact size, so the
  light-touch and palm rules (P1, P3) do not apply through it, and the kernel's fuzz filter
  delays small motions.

### The socket protocol (O1)

`/run/palmrest/events.sock`, `SOCK_SEQPACKET`, mode 0666 (the daemon serves only the person at the seat; see Architecture), one JSON message per
packet, adjacently tagged (`{"kind": "...", "v": {...}}`). The first packet on a connection is
`Hello`; a client checks `version` (`PROTOCOL_VERSION = 1`) before reading on.

```rust
pub enum ToHost {
    Hello { version: u16, device: DeviceInfo },
    Scroll { gesture: GestureId, phase: Phase, dx_px: f32, dy_px: f32, fingers: u8, axis: Option<Axis>, t_ns: u64 },
    Swipe { gesture: GestureId, phase: Phase, fingers: u8, dx_units: f32, t_ns: u64 },
    Tap { kind: TapKind, t_ns: u64 },
    Touches { active: ActiveTouch },
    DeviceGone,
}
pub enum ToDaemon { PointerOver { over: Over }, Subscribe { filter: Filter }, SessionLock { state: LockState } }
pub enum Filter { All, Only(Vec<Topic>) }            // Topic: Scroll | Swipe | Tap | Touches
pub struct DeviceInfo { name: String, path: String, vendor: u16, product: u16 }
// Phase: MayBegin | Began | Changed | Ended | Cancelled;  Axis: X | Y;  TapKind: One | Two
// ActiveTouch: Down | Up;  Over: Ours | NotOurs;  LockState: Locked | Unlocked;  GestureId(u32)
```

`SessionLock` is the lock screen's state: the shell that draws it sends `Locked` when it locks and
`Unlocked` when it opens, and again after connecting. The session counts as locked while any
subscriber says so, and a subscriber that disconnects stops saying it. Only the shell's `Unlocked`
counts (another program of the person may say `Locked`, never `Unlocked`), and `PointerOver` counts
from the shell alone. While locked no gesture
runs a key or a desktop action; scrolling and the stream to subscribers go on.

Scroll px are in finger direction (+x right, +y toward the user) with every gain applied; the
host applies natural scrolling. Every `MayBegin` is closed by exactly one `Ended` or
`Cancelled`.

## Safety rules

palmrest drives the user's only pointing device, so:

- **Tests never create an input device.** Every test writes through `RecordingEmitter`, or
  through the input service's fake (`turnstile_client::fake::FakeSource`) for the code that
  talks to the service. The real service and the real uinput node are never opened by a test.
- **palmrest cannot type.** The virtual keyboard comes from the input service, which sends
  palmrest's key presses on only while ctrl, alt or super is held for a letter, digit,
  punctuation, Enter, Space, Tab, Backspace, Delete or Escape. A gesture mapped to such a key
  on its own (`enter`, `shift+t`) is warned about in the log when the settings are read and does
  nothing. Navigation and function keys go out as they are.
- `palmrest --check-uinput` asks the service for a touchpad and lets it go (it opens a session
  and makes a device; nothing else). `--direct-uinput` makes the devices without the service,
  for development on a machine that has none.
- `palmrest record` opens the hidraw node read-only and stops after 20 s without a touch.
- The daemon never changes kernel module parameters, udev rules or `/etc/modprobe.d`; only
  `install.sh` does, as root, and says each step first (`--dry-run` prints them all). With
  `emulate_scroll_wheel=0` the mouse scrolls only while palmrest runs.
- palmrest is a dynamic user with no capabilities, no network, a read-only file system and no
  access to `/dev/uinput`; the mouse's node comes through the `input` group.

## Install

```bash
./install.sh
```

palmrest is a system service, like keycap. `install.sh` builds a release binary and installs, through
`sudo`, `doas` or `pkexec` (whichever is found; `$SUDO` overrides; with none it prints the commands
for you to run as root): the program in `/usr/local/bin` (a root-owned file: the input service
serves only a palmrest that only root can change), `dist/palmrest.service` in `/etc/systemd/system`,
and the three system files below; then it enables and starts the service. The unit runs as a
dynamic user, passes no library-path variables (`UnsetEnvironment=`), and reads its settings from
`/etc/palmrest/gestures.toml`. `--prefix P` installs a packaged layout under P (the unit in
`P/lib/systemd/system`, the system files in `P/lib/udev/rules.d`, `P/lib/modprobe.d` and
`P/lib/modules-load.d`) and `--destdir D` stages it under D without root or systemd. An earlier
install ran palmrest as a service of the person; the script says how to remove that one.

- `dist/70-palmrest.rules` to `/etc/udev/rules.d/`: the Magic Mouse 2's hidraw node belongs to the
  `input` group, which the service is in, and to no person (the raw node, and the feature report
  written to it, are not reachable from the logged-in user's programs). `palmrest record` therefore
  needs root (or a person in `input`). There is no rule for `/dev/uinput`: the desktop installs the
  input service (turnstile), which owns that node and makes palmrest's devices. palmrest on its own,
  without the desktop, runs only with `--direct-uinput` and a `/dev/uinput` you have made writable
  for yourself.
- `dist/99-palmrest.conf` to `/etc/modprobe.d/`: `hid_magicmouse` keeps the pointer and clicks
  (`emulate_3button=1`) and stops its own wheel emulation (`emulate_scroll_wheel=0`), because
  palmrest scrolls.
- `dist/palmrest-modules.conf` to `/etc/modules-load.d/palmrest.conf`: loads `uinput` at boot, so
  `/dev/uinput` exists on Ubuntu and Arch too (the input service ships the same line).

## Retiring the Python daemon

Run once, in this order, on a machine that still has `magic-mouse-kde`:

```bash
~/.local/lib/magic-mouse-kde/uninstall.sh      # stops the service, removes its udev/modprobe files
sudo rm -f /etc/udev/rules.d/70-magic-mouse-kde.rules /etc/modprobe.d/99-magic-mouse-kde.conf
sudo install -Dm644 dist/70-palmrest.rules /etc/udev/rules.d/70-palmrest.rules
sudo install -Dm644 dist/99-palmrest.conf /etc/modprobe.d/99-palmrest.conf
echo 0 | sudo tee /sys/module/hid_magicmouse/parameters/emulate_scroll_wheel
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --subsystem-match=misc
./install.sh                                    # builds, installs and starts palmrest
```

The `rm` repeats what `uninstall.sh` does, in case it was already removed. Then
`rm -rf ~/.local/lib/magic-mouse-kde` if nothing else is wanted from it.

## Configuration

`/etc/palmrest/gestures.toml` for the service (the unit's `ConfigurationDirectory=`), and
`$XDG_CONFIG_HOME/palmrest/gestures.toml` for the tools a person runs. Every key of 22-SETTINGS §3.9/§3.10 with its
default; `palmrest --print-config` prints them all, and `palmrest --write-schema DIR` writes `palmrest.settings.toml`, the schema the Settings app reads. Lengths are micrometres (`_um`); a rule
that is anisotropic in millimetres on the mouse (a Euclidean distance, the lock threshold) has
an `_x_um` / `_y_um` pair. A bad value costs only its own key (logged, default used); an
unknown key is ignored. A change is re-read 30 ms after the last write and applied at the
reader's next wake (at most 100 ms later).

Two opt-in presets, both off by default (FINDINGS):

```toml
foreign_momentum = "ours"   # palmrest glides foreign apps after a flick (default "toolkit")
[feel]
curve = "mac_like"          # precise when slow, power-law gain when fast (default "classic")
```

## Tools

```bash
palmrest record FILE [SECONDS]              # raw reads of device.source + monotonic times
palmrest replay FILE                        # the outputs a recording produces, one JSON line each
palmrest replay FILE --latency              # decode-to-bytes latency percentiles
palmrest replay FILE --curve mac-like       # cumulative px and peak gain under both curves
palmrest --check-uinput                     # can palmrest get its virtual devices?
palmrest --direct-uinput                    # run the daemon without the input service (development)
```

## Manual checks

Waiting for the user and the device (FINDINGS has the core's list: items 8, 9, 10 live,
11 live, 12). The feel presets:

- **Curve bake-off:** record real sessions (`palmrest record`), compare them with
  `palmrest replay FILE --curve mac-like`, then scroll with `feel.curve = "mac_like"` and tune
  the four `feel.mac_like_*` keys.
- **Synthesized momentum, touchpad:** with `foreign_output = "touchpad"` and
  `foreign_momentum = "ours"`, `libinput debug-events` on "palmrest touchpad" during a flick
  shows the moves continue after the fingers lift and end with one lift at ~1 px/s; in a GTK4
  and a Qt app, check that no second fling follows the glide, what a re-land looks like, and
  whether a touch mid-glide stops dead or starts a short toolkit fling.
- **Synthesized momentum, wheel:** with `foreign_output = "wheel"`, a flick
  glides in Dolphin, Firefox and Konsole, and a touch stops it.
- **Suppression:** hovering a shell-host surface mid-glide stops the glide.

The sensor seam:

- **evdev on the Magic Mouse:** with `[device] source = "evdev"`, `palmrest record` a few
  scrolls, swipes and double-taps and `palmrest replay` them: the gestures fire as on hidraw
  (minus the palm and light-touch rules). Then run the daemon that way for a while and compare
  the feel; the kernel's fuzz may make slow scrolls start later.

## Tests

`cargo test` runs the unit tables and the acceptance tests of 12.8 items 1-7 (and the replay
half of 10), and `tests/evdev_parity.rs` (every fixture re-expressed as evdev events replays
to the same outputs through `EvdevMultitouch`), against the fixtures in
`crates/palmrest/tests/fixtures/`, which are synthesised from the documented report format by
`tests/support/scenarios.rs` and must match it byte for byte
(`PALMREST_REGEN_FIXTURES=1 cargo test --test fixtures_current` rewrites them).

The reference-feel suite (`reference_feel.rs`, `reference_cases.rs`) checks the default
scroll feel against the goldens in `tests/fixtures/reference/`. The goldens are frozen: they
were produced once by the retired Python daemon, and nothing regenerates them. A change
to a `feel_*` or `case_*` scenario therefore breaks the suite by design.

The latency measurement is `cargo test --release -p palmrest --lib latency -- --ignored --nocapture`.
