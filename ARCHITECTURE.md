# Architecture

palmrest is the Magic Mouse 2 gesture daemon: it reads touch reports, turns them into scroll and
gesture streams, and delivers them over a socket to shell-host surfaces and, through virtual
input devices, to every other client. `README.md` has the data flow, the module table, the
socket protocol and the safety rules; quire's `design/12-BEHAVIOUR-gestures.md` is the
specification; `CONVENTIONS.md` holds the rules every repo shares.

## Crate map

One crate, `crates/palmrest` (library, the `palmrest` binary, `tests/`). It depends on no shell
crate and runs on any Linux.

- **Pure core:** `sensor`, `touch`, `stamp`, `feel`, `gestures`, `tracker`, `action`,
  `protocol`, `config`. It takes bytes and instants and returns values;
  `Tracker::step(self, &Report, now) -> (Self, Vec<Out>)` composes it.
- **Effects:** `daemon` (threads, socket, discovery, devices, clocks), `outputs`' uinput emitter
  and `sys`.
- **Seams:** `Sensor` (`MagicMouseHid`, `EvdevMultitouch`) and `Emitter` (uinput,
  `RecordingEmitter`).

## Repo rules

- **Only `daemon` owns threads, sockets, devices and clocks.** When the core seems to need an
  effect, it returns an `Out` for the daemon to carry out.
- **`unsafe` lives only in `sys.rs`** (the ioctls).
- **Units:** the core computes on an integer grid fixed in millimetres (the Magic Mouse 2's own,
  26 units/mm across, 70 along), in newtypes (`Um`, `Ms`, `Fraction`, `GestureId`), so every
  sensor feels the same per millimetre.
- **Device ids are parsed, never searched:** `HID_ID` is read as `bus:vendor:product`.
- **Feel is pinned by frozen goldens** (`tests/fixtures/reference/`): nothing regenerates them,
  so a change to a `feel_*` or `case_*` scenario breaks the suite by design.
- **Tests never create an input device:** every test writes through `RecordingEmitter`, and no
  test reads a real config directory or a running daemon (README, "Safety rules").
- **Gate** (no boundary script):

  ```bash
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-features
  cargo deny check licenses
  ```
