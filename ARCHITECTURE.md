# Architecture

palmrest is the Magic Mouse 2 gesture daemon: it reads touch reports, turns them into scroll and
gesture streams, and delivers them over a socket to shell-host surfaces and, through virtual
input devices, to every other client. `README.md` has the data flow, the socket protocol and
the safety rules; quire's `design/12-BEHAVIOUR-gestures.md` is the specification;
`CONVENTIONS.md` holds the rules every repo shares; `FINDINGS.md` the open items and standing
facts. This file is the map: where each thing lives, what may depend on what, and how to add
the next thing. palmrest depends on no shell crate and runs on any Linux.

## 1. Crate map

| Crate | Path | Purpose | May depend on |
|---|---|---|---|
| `palmrest-protocol` | `crates/palmrest-protocol` | the socket contract with shell-host: `ToHost`, `ToDaemon`, their field types, `PROTOCOL_VERSION`, `MAX_PACKET`, `encode`, `decode`, `compat` | `serde`, `serde_json`, `thiserror` |
| `palmrest` | `crates/palmrest` | the pure core, the tools that replay recordings, and the daemon; library plus the `palmrest` binary | `palmrest-protocol`, `serde`, `serde_json`, `thiserror`, `toml`, `notify`, `rustix`, `turnstile-client`, `turnstile-wire` (git, tag `v0.1.0`; the daemon layer only) |

Edges: `palmrest` -> `palmrest-protocol` only. `palmrest-protocol` depends on no other workspace
crate, and has no `toml`, `notify` or `rustix`, so shell-host takes it without pulling the
daemon. shell-host imports `palmrest-protocol` as a path dependency; nothing else of palmrest
leaves the repo. There is no boundary script; `cargo tree -p palmrest-protocol` showing only
serde, serde_json and thiserror is the check, run in review.

The library surface of `palmrest` is the pure core plus the recording tools (section 2). The
daemon, `sys` and the CLI verbs are `pub(crate)`; the one public entry to them is
`command::run`.

### Moves

| From | To |
|---|---|
| `crates/palmrest/src/protocol.rs` | `crates/palmrest-protocol/src/lib.rs` split into `message.rs` (`ToHost`, `ToDaemon`, `Filter`, `Topic`, `DeviceInfo`), `vocab.rs` (`GestureId`, `Phase`, `Axis`, `TapKind`, `ActiveTouch`, `Over`), `wire.rs` (`encode`, `decode`, `Compat`, `compat`, constants), `tests.rs` |
| `crates/palmrest/src/lib.rs` re-export `pub use protocol::{ToDaemon, ToHost}` | deleted; callers import `palmrest_protocol` |
| `outputs/uinput.rs` | `daemon/uinput/` (`caps`, `direct`, `emitter`, `route`, `probe`) |
| `sys.rs` | `daemon/sys.rs` (the one `unsafe` module) |
| `main.rs` verbs (`record_cmd`, `replay_cmd`, `check_uinput`, usage, arguments) | `command/{mod.rs (Command, parse, run), record.rs, replay.rs, probe.rs}`; `main.rs` is `fn main() -> ExitCode { palmrest::command::run(std::env::args()) }` |
| `compare.rs` | `replay/compare.rs` |
| `gestures/live.rs`, the seven `gestures.live_workspace_*` keys | deleted (section 8) |
| `tests/socket.rs`, `tests/latency.rs` | unit tests beside `daemon/socket.rs` and `daemon/latency.rs` |
| `tests/acceptance_1_decode.rs`, `acceptance_2_reference_feel.rs`, `acceptance_3_reference_cases.rs`, `acceptance_4_to_7.rs` | `tests/decode.rs`, `tests/palmrest/reference_feel.rs`, `tests/palmrest/reference_cases.rs`, `tests/palmrest/gestures_replay.rs` (named after what they test) |

## 2. Modules and layers

`crates/palmrest/src`. A module imports from its own layer or a lower one only.

| Layer | Modules | Nature | Visibility |
|---|---|---|---|
| L0 vocabulary | `time` (`Mono`), `keys` (`Key`, `KeyCombo`), `config/units`, `sensor/units` | newtypes | `pub` |
| L1 sensors | `sensor` (`Sensor`, `Frame`, `Surface`, `lattice`), `sensor/magic_mouse` (`MagicMouseHid`, `decode`, `encode`, `report`, `known`), `sensor/evdev` (`EvdevMultitouch`), `touch`, `stamp` | bytes to millimetre frames | `pub` |
| L2 settings | `config` (`Config`, sections, enums, `lenient`, `load`, `watch`) | pure except `load` and `watch` (file reads, inotify) | `pub` |
| L3 feel and gestures | `feel`, `gestures` (`contacts`, `bands`, `click`, `motion`, `scroll`, `swipe`, `one_swipe`, `tap`) | pure | `pub` |
| L4 tracker and actions | `tracker` (`Tracker::step`, `Out`), `action` (`Gesture`, `Action`, `plan`) | pure composition: `Tracker::step(self, &Frame, now) -> (Self, Vec<Out>)` | `pub` |
| L5 outputs model | `outputs` (`Emitter`, `RecordingEmitter`, `pad`, `events`) | the seam and the models of what is written | `pub` |
| L6 tools | `recording`, `replay` (`replay`, `feed`, `outs`, `compare`) | pure: feed recorded reads through the core | `pub` |
| L7 effects | `daemon` (`reader`, `socket`, `discover`, `hidraw`, `evdev`, `devices`, `dispatch`, `spawn`, `record`, `latency`, `source`, `uinput`, `sys`) | threads, sockets, nodes, clocks, uinput, ioctls | `pub(crate)` |
| L8 entry | `command` | argument parsing, one `run(args)`; reads `std::env` once into `daemon::Env` | `pub` (one fn) |

`recording` and `replay` stay public because the acceptance suite and the `palmrest replay`
verb are the same harness: a recording file in, the core's outputs out, no clock and no device.
`daemon` is where every thread, socket, device node and clock lives; the core never reaches
into it.

`lib.rs` holds declarations and these root re-exports only: `Config`, `Mono`, `Sensor`,
`Frame`, `Surface`, `Tracker`, `Out`.

### Files near the split line

`feel/lock.rs` (398), `gestures/contacts.rs` (375), `tests/acceptance_3_reference_cases.rs`
(370) and `tests/acceptance_4_to_7.rs` (359) split by concept when next touched: lock
decision vs lock parameters, contact tracking vs palm turn, one file per scenario family.

## 3. One home

| Concept | Home |
|---|---|
| Monotonic instants | `time::Mono` |
| Device timestamp fitted to monotonic time | `stamp` |
| Millimetre and lattice units, the reference grid | `sensor::units`, `sensor::lattice` |
| Frame, contact, surface of a sensor | `sensor::{frame, surface}` |
| Magic Mouse report codec, known devices | `sensor::magic_mouse::{decode, encode, report, known}` |
| evdev multitouch slots | `sensor::evdev` |
| Touch block and grid `Point` | `touch` |
| Palm and rest rejection, contact tracking | `gestures::contacts` |
| Scroll phases | `gestures::scroll` |
| One-finger swipe, two-finger swipe, double-taps | `gestures::{one_swipe, swipe, tap}` |
| Axis lock, gains, curves, wheel quantisation | `feel::*` |
| Natural direction | `feel::natural` |
| Gesture state of the daemon | `tracker::Tracker` |
| Foreign momentum (glide) | `tracker::coast`, `tracker::foreign` |
| What the tracker asks for | `tracker::Out` |
| Gesture to action table, per-desktop meaning | `action`, `keys` |
| Socket messages | `palmrest-protocol` |
| Every tunable value, default, range | `config::sections` |
| Lenient per-key reading | `config::lenient` |
| Config file location, reload | `config::{load, watch}` |
| The schema of the config for the Settings app (`palmrest --write-schema`) | `config::schema` (table in `keys.rs`), `command::schema` |
| Virtual device write seam | `outputs::Emitter` |
| Touchpad frame model, evdev event encoding | `outputs::{pad, events}` |
| Recording file format | `recording` |
| Replaying recordings, curve comparison | `replay`, `replay::compare` |
| Threads, sockets, discovery, backoff, nodes | `daemon` |
| Subscribers of the socket: ids, caps, budgets, the joint claim and lock | `daemon::socket::{server, client, limits}` |
| Programs a gesture runs, and their reaping | `daemon::spawn` |
| What the shell says about the lock screen | `palmrest_protocol::ToDaemon::SessionLock`, `Tracker::with_session_lock` |
| The virtual devices: what each is, asking the input service for them, the direct fallback | `daemon::uinput::{caps, emitter, direct, route}` |
| When to rebuild the devices after a cut, and when to give up | `daemon::devices` |
| ioctls and all `unsafe` | `daemon::sys` |
| Environment, clock, directories | `daemon::Env`, built once in `command::run` |
| Command-line verbs | `command` |

## 4. Traits

| Trait | Signature | Implementors |
|---|---|---|
| `Sensor` (`sensor`) | `fn surface(&self) -> Surface; fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, SensorError>` | `MagicMouseHid`, `EvdevMultitouch` |
| `Emitter` (`outputs`) | `fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError>; fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError>; fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError>` | `daemon::uinput::UinputEmitter` (over any `turnstile_client::Session`), `RecordingEmitter` |
| `Setting` (`config::lenient`) | `fn read(v: &toml::Value, key: &str, warn: &mut Vec<Warning>) -> Option<Self>` | every unit newtype, enum and `Action` (through `settings_struct!` and `serde_settings!`) |

Add an implementor of `Sensor` for a new device family, of `Emitter` for a new kind of output
device, of `Setting` for a new value type. Add a new trait only when two real implementations
are swapped at a boundary. The next candidate is a second source of the daemon's clock and
environment (`daemon::Env`); it is a plain struct until a fake exists.

**Closed enums that stay enums:** `Out`, `Action`, `Gesture`, `ToHost`, `ToDaemon`, `Phase`,
`Over`, `Kind` (the sensor kind the daemon opens), `RecordedSource`, `Curve`, `DeviceSource`,
`ForeignOutput`, `ForeignMomentum`, `G4Mode`.

## 5. Recipes

### Add a gesture

1. `gestures/<gesture>.rs`: the recogniser as a pure value, `fn step(self, input, now, &Config)
   -> (Self, Option<Event>)`, with its state as a small enum; no clock, no I/O. Declare it in
   `gestures/mod.rs` and export the type there.
2. `config/sections.rs`: its thresholds as keys (section below), lengths in micrometres.
3. `action.rs`: a `Gesture` variant when it fires an action, its default in the
   `gesture_action_map` table, and its `Action` mapping in `plan` if it needs a new action.
4. `tracker/mod.rs`: one field for the recogniser, one call in `step`, one arm turning its
   event into `Out`s (`tracker/out.rs`). The gesture never talks to the socket directly.
5. If the host must see it: a `ToHost` variant in `palmrest-protocol` (the protocol changes:
   bump `PROTOCOL_VERSION`, update the round-trip test and shell-host in the same change).
6. Tests: a table test of the recogniser beside it; a scenario in `tests/support/scenarios.rs`
   (a synthesised stroke), its fixture written by `PALMREST_REGEN_FIXTURES=1 cargo test --test
   fixtures_current`, and a replay assertion in `tests/palmrest/gestures_replay.rs`; the same recording
   replayed through `EvdevMultitouch` in `tests/evdev_parity.rs`.

### Add a sensor

1. `sensor/<family>/mod.rs`: the type, implementing `Sensor`; `decode` returns millimetre
   `Frame`s on the reference lattice and a `SensorError` for a malformed read; its `Surface`
   states size, resolution, buttons and clock.
2. `sensor/mod.rs`: declare and export it; add its variant to `SensorError`.
3. `config/enums.rs`: a `DeviceSource` variant (the `[device] source` key).
4. `daemon/source.rs`: a `Kind` variant and its arm in `Opened::sensor` and `Opened::recorded`;
   `daemon/<family>.rs` for opening and identifying the node (node facts parsed, never
   searched); `recording.rs`: a `RecordedSource` variant so recordings replay.
5. Tests: decode tables from bytes captured with `palmrest record`; a parity test in
   `tests/` replaying an existing fixture re-expressed through the new sensor to the same
   outputs.

### Add an output or emitter

1. A new kind of thing written: a variant in `Out` (`tracker/out.rs`) and a method on `Emitter`
   (`outputs/mod.rs`), with its `Emitted` variant and the `RecordingEmitter` arm.
2. `outputs/<name>.rs`: the pure model and its event encoding (tested by tables).
3. `daemon/uinput/emitter.rs` (or `daemon/<backend>.rs` for another backend): the implementor of
   `Emitter`; the kernel calls go through `daemon/sys.rs`.
4. `daemon/dispatch.rs`: the `Out` arm that calls the emitter; `daemon/devices.rs` when the
   device is created lazily.
5. Tests: encode tables; a replay test asserting the `RecordingEmitter`'s `Emitted` list. No
   test constructs a real device.

### Add a config key

1. `config/sections.rs`: one line in the section's `settings_struct!`: `name: Type = default,
   in lo => hi;` with a doc comment. Lengths are `Um`, times `Ms`; a closed choice is an enum in
   `config/enums.rs` (with `serde_settings!`), never a `bool` or a string.
2. `config/file.rs`: nothing unless it is a new section (then the `Config` field, its
   `Default`, and its `section(...)` call).
3. Read it where it is used, from the `Config` value the caller holds (`tracker`, `gestures`,
   `feel`), never from a global.
4. `palmrest --print-config` prints it automatically; `README.md` "Configuration" gains a line
   only for a key with a non-obvious unit.
5. `config/schema/keys.rs`: one row (label, help, section, Basic or Advanced, its range or
   words). The default comes from `Config::default()`; a test fails until every key has a row,
   and reads each row's bounds and words back through the lenient reader.
6. Tests: the round-trip in `config/file.rs` tests, and a case in the lenient-reading table
   (a bad value keeps the default and yields one `Warning`).

### The schema and quire

`palmrest --write-schema DIR` writes `palmrest.settings.toml`, the file quire's Settings app
reads (quire `design/22-SETTINGS.md` section 9). palmrest takes no quire crate, so it does not use
quire's `SettingsSchema` derive: `config::schema` writes the same format from a table, with the
tests above as the guard that it cannot drift from `Config`.

### Add a reference-feel golden

The goldens in `tests/fixtures/reference/` are frozen: produced once by the retired Python
daemon, they define the default feel, and nothing regenerates them. A golden is added or changed
only with the user's approval, in a commit whose message says why the feel is meant to differ;
a change to a `feel_*` or `case_*` scenario breaks the suite by design, and is never fixed by
re-blessing the golden.

1. `tests/support/scenarios.rs`: the scenario, built from strokes, with a name that says what
   it exercises.
2. Its `.bin` fixture: `PALMREST_REGEN_FIXTURES=1 cargo test --test fixtures_current` (synthesised
   fixtures only, never a golden).
3. The golden file in `tests/fixtures/reference/`, from the user-approved source of truth.
4. A row in `tests/palmrest/reference_feel.rs` (`SESSIONS`) or `tests/palmrest/reference_cases.rs`.

## 6. Test harness

| Kind of thing | Test |
|---|---|
| Codecs, feel, gestures, config, protocol | table tests beside the code |
| Whole-core behaviour | replay: a recording through `replay::replay` with `RecordingEmitter`, asserting `Out`s and `Emitted`; strokes and fixtures from `tests/support` |
| Another sensor | the same recording re-expressed as evdev events, replayed to identical outputs (`tests/evdev_parity.rs`) |
| Reference feel | goldens in `tests/fixtures/reference/`, frozen |
| Socket | a private socket in a temp dir, beside `daemon/socket.rs` |
| Latency | ignored release test beside `daemon/latency.rs` |

The one harness is `replay` plus `tests/support`. No test creates an input device, opens a real
node for writing, or reads a real config directory or a running daemon.

## 7. Repo rules

- **A panic ends the process** (`panic = "abort"` in release) and a socket thread that cannot poll exits it, so
  systemd restarts the unit; a daemon with a dead socket thread never keeps running.
- **Everything a subscriber can make the socket thread store or read is capped** (`socket/limits.rs`):
  subscribers, packets per pass, queued messages; a subscriber is known by id, never by position.
- **Only `daemon` owns threads, sockets, devices, clocks and the environment.** When the core
  seems to need an effect, it returns an `Out` for the daemon to carry out.
- **`unsafe` lives only in `daemon/sys.rs`** (the ioctls), each block with a `SAFETY` comment.
- **Units:** the core computes on an integer grid fixed in millimetres (the Magic Mouse 2's own,
  26 units/mm across, 70 along), in newtypes (`Um`, `Ms`, `Fraction`, `GestureId`), so every
  sensor feels the same per millimetre. The only floats in public types are the socket's px.
- **The protocol is a crate:** a change to `ToHost` or `ToDaemon` is a change to
  `palmrest-protocol`, its round-trip tests, `PROTOCOL_VERSION` when incompatible, and
  shell-host, in one change.
- **Device ids are parsed, never searched:** `HID_ID` is read as `bus:vendor:product`.
- **No `bool` in a field, parameter or key.** A key is an enum.
- **Feel is pinned by frozen goldens** (section 5); nothing regenerates them.
- **Tests never create an input device:** every test writes through `RecordingEmitter` or
  `turnstile_client::fake::FakeSource`, and no test reads a real config directory, opens the
  service's socket or touches a running daemon (README, "Safety rules").
- **Gate** (no boundary script):

  ```bash
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-features
  cargo deny check licenses
  ```

## 8. The live model is deleted

`gestures::live` (progress, dampened overshoot, release velocity, commit or cancel, finish
duration) and the keys `gestures.live_workspace_D_um`, `live_workspace_overshoot_scale`,
`_overshoot_cap`, `_velocity_window_ms`, `_commit_velocity`, `_finish_min_ms`,
`_finish_max_ms` have no driver: nothing in the daemon, the tests outside `live.rs`, or
shell-host calls them (the host has no consumer of `Swipe` yet, and would own its own
page-swipe formulas). `host_swipe_D_um` stays: it configures the units of the `Swipe` stream
the host receives. A live workspace mode, when built, adds its model and keys with its driver.
The FINDINGS entry "Not built: live workspace mode" stays as the open item.

## 9. Migration order

Each step keeps master green (the gate above).

1. (done) `palmrest-protocol` crate: create it, move `protocol.rs` into it (split as in section 1), add
   it to the workspace, switch `palmrest` to import it, delete the re-export in `lib.rs`. The
   shell-host session then deletes its copy.
2. (done) Delete `gestures/live.rs`, the seven `live_workspace_*` keys, their `README` and default
   config text.
3. (done) Move `sys.rs` and `outputs/uinput.rs` into `daemon/`; make `daemon` and its `sys` `pub(crate)`.
4. (done) Move `tests/socket.rs` and `tests/latency.rs` in-crate; add `command/`, reduce `main.rs`;
   `daemon::Env` replaces the `std::env` reads in `main.rs` and `daemon/mod.rs`.
5. (done) `compare.rs` under `replay/`; rename the acceptance test files.
6. (done) Split `feel/lock.rs` and `gestures/contacts.rs` by concept.
7. (done) Trim `lib.rs` to declarations and the root list in section 2.

## 10. The virtual devices and the input service

palmrest does not own `/dev/uinput`. The input service (turnstile, a separate repository and a
root service) does, and makes palmrest's three devices on request after checking who asks.

| File (`daemon/uinput/`) | Holds |
|---|---|
| `emitter.rs` | `UinputEmitter<S: Session>`: asks for the touchpad or wheel `foreign_output` names, and for the keyboard only when a gesture presses keys (`Create::keys` with the deduplicated codes of `KeyCombo::codes()`). One `Session::send` per pad frame. Counts `Sent::Dropped` frames and goes on. Turns every `ClientError` into an `OutputError` in plain words, asking the session for the service's reason first |
| `route.rs` | `Route`: a `DeviceSource` over the service and, in this release, the direct node (`Origin`, `Either`) |
| `direct.rs` | `DirectSource`/`DirectSession`: the same `Session` over palmrest's own uinput node (`--direct-uinput`, and the fallback) |
| `caps.rs` | each device's identity and capabilities (used by `direct.rs`) |
| `probe.rs` | `--check-uinput`: open a session and make a touchpad |

Who rebuilds, and when to stop, is `daemon/devices.rs` and nothing else. `OutputError` says what
happened: `Unavailable` (the service is not there yet; retried without limit), `Write` (the
device failed; rebuilt) and `Cut` (the service closed the connection, refused palmrest or timed
out; counted). After a cut the devices are rebuilt with the 1 s, 2 s, 4 s, 5 s backoff, three
times; a fourth cut in a row, with no frame going through between, ends the virtual devices with
one line in the log that carries the service's reason. A change of settings that changes what the
devices must be tries again. The socket stream to shell-host is unaffected throughout.

A key combination the service denies (a key that types text with no ctrl, alt or super) is
reported when the settings are read (`Action::read`, `Key::needs_a_shortcut_modifier`). That
predicate mirrors the service's key classes (`turnstile-wire`'s `keytable`); a test here pins
which names are free, and the service's tests fail when `keys::NAMES` has a code it lacks.

Transition: this release prefers the service and falls back to the node when the service is
absent and the node opens (`Route::ServiceThenNode`). The next release deletes that arm and
`Origin::Service` becomes the service alone; `--direct-uinput` (`Route::Node`) stays.
