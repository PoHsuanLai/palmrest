# palmrest

The Magic Mouse 2 gesture daemon of the desktop shell program. It reads the mouse's touch
reports from hidraw, turns them into scroll and gesture streams with the scroll feel of the
Python daemon it replaces, and delivers them over a socket to shell-host surfaces and, through
virtual input devices, to every other client. Specified in quire's
`design/12-BEHAVIOUR-gestures.md` (and `design/11-BEHAVIOUR-scroll.md` for how the host uses
the stream; `design/22-SETTINGS.md` §3.9-3.10 for the settings). Portable to any Linux; no
shell dependency. Pointer motion and physical clicks stay in the kernel driver.

## Architecture

```
hidraw ──read(64)──► decode ──► stamp ──► Tracker::step ──► Vec<Out> ──► dispatch
                     (bytes)   (time)    (pure state)              │
                                                                   ├─ Host(ToHost) ─► socket thread ─► subscribers
                                                                   ├─ Pad / Wheel / Key ─► Emitter (uinput)
                                                                   └─ Action ─► action::plan ─► spawn
subscribers ── PointerOver ──► socket thread ── claim ──► Tracker::claim (silences foreign outputs)
gestures.toml ── inotify, 30 ms debounce ──► Tracker::with_config
```

Two threads and no async runtime: the **reader** (poll on the hidraw fd, read, decode, step,
emit, and every housekeeping step between reads) and the **socket** thread (accept, fan-out,
`PointerOver` aggregation), woken by an eventfd so the reader never waits on a subscriber.

The pure core takes bytes and instants and returns values; nothing in it reads a clock or
touches a device, so a recording replays to exactly the outputs the live daemon produced.

| Module | What it is |
| --- | --- |
| `decode`, `encode`, `report`, `touch` | report 0x12 and 0xF7 doubles to `Report`s and back (12.3.1) |
| `stamp` | the 18-bit device timestamp fitted onto `CLOCK_MONOTONIC` (12.3.1, acceptance 12) |
| `feel` | axis lock (the Python `ScrollAxisLock` as a value), gains, px, wheel quantisation, natural direction (12.3.8) |
| `gestures` | contacts and palm/rest rejection P1-P5, scroll phases, G4 one-finger swipe, G5 workspace swipe, G6/G7 double-taps, the live model formulas |
| `tracker` | `Tracker::step(self, &Report, now) -> (Self, Vec<Out>)`: all of the above composed (12.4) |
| `action`, `keys` | the gesture -> action table and what an action means on KDE / COSMIC |
| `protocol` | the socket messages (12.3.10) |
| `config` | `gestures.toml`, lenient per key, the Python environment variables, the watch |
| `outputs` | the `Emitter` seam, `RecordingEmitter`, the virtual touchpad model, evdev event encoding, the uinput emitter |
| `sys` | the ioctls: the only module allowed `unsafe` |
| `recording`, `replay` | the `palmrest record` file and the replay used by every acceptance test |
| `daemon` | threads, socket, discovery and backoff, hidraw, devices, record, latency |

### The socket protocol (O1)

`$XDG_RUNTIME_DIR/palmrest/events.sock`, `SOCK_SEQPACKET`, mode 0600, one JSON message per
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
pub enum ToDaemon { PointerOver { over: Over }, Subscribe { filter: Filter } }
pub enum Filter { All, Only(Vec<Topic>) }            // Topic: Scroll | Swipe | Tap | Touches
pub struct DeviceInfo { name: String, path: String, vendor: u16, product: u16 }
// Phase: MayBegin | Began | Changed | Ended | Cancelled;  Axis: X | Y;  TapKind: One | Two
// ActiveTouch: Down | Up;  Over: Ours | NotOurs;  GestureId(u32)
```

Scroll px are in finger direction (+x right, +y toward the user) with every gain applied; the
host applies natural scrolling. Every `MayBegin` is closed by exactly one `Ended` or
`Cancelled`.

## Safety rules

palmrest drives the user's only pointing device, so:

- **Tests never create an input device.** Every test writes through `RecordingEmitter`. The
  uinput emitter is compiled and reviewed, and its event sequences are the tested `events`
  functions, but nothing in the suite constructs it.
- `palmrest --check-uinput` opens and closes `/dev/uinput` without `UI_DEV_CREATE`, and only
  when `PALMREST_ALLOW_UINPUT_PROBE=1` is set.
- `palmrest record` opens the hidraw node read-only and stops after 20 s without a touch.
- palmrest never changes kernel module parameters, udev rules or `/etc/modprobe.d`. The kernel
  keeps `emulate_scroll_wheel=0`, which is only safe while palmrest or the Python daemon runs.
- Exactly one of palmrest and the Python service may run: `palmrest.service` declares
  `Conflicts=magic-mouse-kde.service`.

## Switching from the Python service

Nothing switches until you run it:

```bash
./install.sh             # build, install to ~/.local/bin, seed gestures.toml, switch services
./install.sh --rollback  # stop palmrest, start magic-mouse-kde again
```

`install.sh` builds a release binary, writes `~/.config/palmrest/gestures.toml` once from the
Python unit's `SCROLL_SPEED` / `SCROLL_ACCEL_MAX` (so the feel does not change), selects the
wheel output under KDE (the device identity Plasma already has settings for), installs
`dist/palmrest.service`, stops and disables the Python service, then enables palmrest. If
palmrest is not running three seconds later it rolls back by itself.

## Configuration

`$XDG_CONFIG_HOME/palmrest/gestures.toml` (`config.toml` is read if that is absent). Every key
of 22-SETTINGS §3.9/§3.10 with its default; `palmrest --print-config` prints them all. A bad
value costs only its own key (logged, default used). A change is re-read 30 ms after the last
write and applied at the reader's next wake (at most 100 ms later). The Python
daemon's variables (`SCROLL_SPEED`, `SCROLL_LOCK_RATIO`, `SCROLL_LOCK_TIMEOUT`,
`SCROLL_LOCK_THRESHOLD`, `SCROLL_VELOCITY_GAIN_MAX`, `SCROLL_ACCEL_MAX`, `SWIPE_THRESHOLD`,
`SWIPE_VERTICAL_MAX`, `SWIPE_COOLDOWN`) override the file, for parity runs.

Two opt-in presets, both off by default (FINDINGS F22-F24):

```toml
foreign_momentum = "ours"   # palmrest glides foreign apps after a flick (default "toolkit")
[feel]
curve = "mac_like"          # precise when slow, power-law gain when fast (default "current")
```

## Tools

```bash
palmrest record FILE [SECONDS]              # raw reads + monotonic times
palmrest replay FILE                        # the outputs a recording produces, one JSON line each
palmrest replay FILE --latency              # decode-to-bytes latency percentiles
palmrest replay FILE --curve mac-like       # cumulative px and peak gain under both curves
PALMREST_ALLOW_UINPUT_PROBE=1 palmrest --check-uinput
```

## Manual checks

Waiting for the user and the device (F1 has the core's list: items 8, 9, 10 live, 11 live,
12). Added by the feel presets:

- **Curve bake-off (F22):** record real sessions (`palmrest record`), compare them with
  `palmrest replay FILE --curve mac-like`, then scroll with `feel.curve = "mac_like"` and tune
  the four `feel.mac_like_*` keys.
- **Synthesized momentum, touchpad (F23, F24):** with `foreign_output = "touchpad"` and
  `foreign_momentum = "ours"`, `libinput debug-events` on "palmrest touchpad" during a flick
  shows the moves continue after the fingers lift and end with one lift at ~1 px/s; in a GTK4
  and a Qt app, check that no second fling follows the glide, what a re-land looks like, and
  whether a touch mid-glide stops dead or starts a short toolkit fling.
- **Synthesized momentum, wheel (F23):** under KDE with `foreign_output = "wheel"`, a flick
  glides in Dolphin, Firefox and Konsole, and a touch stops it.
- **Suppression (F23):** hovering a shell-host surface mid-glide stops the glide.

## Tests

`cargo test` runs the unit tables and the acceptance tests of 12.8 items 1-7 (and the replay
half of 10) against the fixtures in `crates/palmrest/tests/fixtures/`, which are synthesised
from the documented report format by `tests/support/scenarios.rs` and must match it byte for
byte (`PALMREST_REGEN_FIXTURES=1 cargo test --test fixtures_current` rewrites them). The Python
parity goldens in `tests/fixtures/parity/` come from running those fixtures through the Python
daemon, read-only: `python3 crates/palmrest/tests/parity/python_reference.py`. The latency
measurement is `cargo test --release --test latency -- --ignored --nocapture`.
