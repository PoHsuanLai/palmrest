FINDINGS
========

Numbered findings from the work, newest last.

## M8 core (branch `core`)

**F1. Acceptance status.** Items 1-7 of 12.8 pass as tests (`tests/acceptance_1_decode.rs`,
`acceptance_2_parity.rs`, `acceptance_3_python_tests.rs`, `acceptance_4_to_7.rs`), with the
replay half of item 10 (suppression) in `acceptance_4_to_7.rs` and the socket half in
`tests/socket.rs`. Item 11 is measured on replay only (F19). Manual, waiting for the user and
the device: **8** (virtual touchpad under `libinput debug-events`), **9** (O1/O2 calibration
of the 0.25 mm/px factor), **10 live** (O2/O4 silent while a real shell-host claims the
pointer), **11 live** (HID read to socket / uinput write on the device), **12** (timestamp fit
residual on the device).

**F2. Which file, which layout.** 22-SETTINGS §2 names `palmrest/gestures.toml` as the
authority; 12.3.12 and 12.6 still say `config.toml`. palmrest reads `gestures.toml` and falls
back to `config.toml`. 22-SETTINGS also disagrees with itself on shape: the §3.9/§3.10 key
catalogue uses dotted keys (`scroll.speed`, `palm_rejection.side_band_x_min`, top-level
`rejection`), §4.3 sketches a flat `GesturesSettings { scroll_speed, .. }`. palmrest follows the
catalogue literally, so every documented key is a TOML path: tables `[scroll]`, `[swipe]`,
`[tap]`, `[gestures]` (with `[gestures.gesture_action_map]`), `[palm_rejection]`, and top-level
`version`, `rejection`, `foreign_output`. Ratios are per-mille integers (`lock_ratio = 1500`),
enums snake_case. The Settings app should be written against this; §4.3 wants correcting.

**F3. Proposed values with no settings key.** 12.3.9's virtual-touchpad values are proposed but
absent from 22-SETTINGS; palmrest reads them from a new `[touchpad]` section (`mm_per_px = 250`,
`contact_spacing_mm = 20`, `min_down_ms = 200`) that 22-SETTINGS §3.9 should gain. Still
hard-coded, and also proposed: the timestamp calibration constants (10 s fit window, 60 s refit,
1 ms residual), the pad geometry (4000 x 2800 units at 40/mm) and the repeat window (0.5 s,
settled Python). The "2 mm" lift rule of 12.3.9 needs no key: a stroke older than
`min_down_ms` has outlived the tap timeout whatever it moved, so both conditions reduce to one.

**F4. Parity needed Python's float time.** With `dt` as exact integer nanoseconds, one report
of `parity_flicks` truncated to -50 hi-res units where Python got -51: Python's `dt` is a
difference of float seconds (0.0129999999999999, not 0.013), which moves the velocity gain in
the 15th digit and, through the carried remainder, one truncation two reports later. `feel`
now computes `dt`, the lock timeout and the repeat window as differences of float seconds
(`Mono::float_since`); lock decisions and cumulative v120 then agree on every report of all
five sessions.

**F5. "Cumulative px within one v120 unit" holds per stroke, not per session.** The Python lock
drops its sub-unit remainder at every lock and unlock; the socket's px have nothing to drop,
so over a session they drift from v120 x 0.5 by up to one unit per stroke even when every stroke
agrees. The test compares cumulatively from each `Began`. To make even that exact, the socket
px are defined as the wheel stream before truncation (whole units plus the carried remainder
at the current rate), so within a stroke they never differ from v120 x 0.5 by a unit; a plain
`raw / rate` px drifted past half a pixel when the velocity gain changed mid-stroke.

**F6. The running service is not the documented defaults.** `magic-mouse-kde.service` sets
`SCROLL_SPEED=32` and `SCROLL_ACCEL_MAX=1`; 12.6 calls 22 and 2.3 "settled (Python)". palmrest
ships 22/2.3 as documented; `install.sh` copies the unit's values into `gestures.toml` so the
switch does not change the feel. The design defaults may want to become 32 and 1.0.

**F7. The foreign output default would change direction on KDE.** The shipped default
`foreign_output = touchpad` puts scrolling on a new device, "palmrest touchpad", whose natural
scrolling Plasma has no setting for yet, while Plasma has `NaturalScroll=true` saved for the
Python daemon's wheel identity. `install.sh` therefore seeds `foreign_output = "wheel"` under
KDE (same identity, same v120 stream as the Python daemon, bit for bit). The touchpad path is
unproven live (items 8, 9).

**F8. The virtual touchpad is small for 0.25 mm/px.** A pair landing at the centre of a
100 x 70 mm pad has 30 mm (120 px of scroll) before the 5 mm edge margin vertically. palmrest
then lifts and re-lands the pair at the centre, which a toolkit sees as one stroke ending and
another starting (possibly a kinetic fling in between). Needs item 9 on the device: a larger
virtual pad, a smaller mm/px, or landing opposite the motion.

**F9. No real recording.** `palmrest record` on `/dev/hidraw3` (read-only, alongside the
running Python daemon) captured 0 reads in 20 s: the mouse was not touched. Every fixture is
synthesised from the documented layout; the five parity "sessions" are synthesised with
seeded jitter, not recorded as 12.8 item 2 asks. Unproven until recorded: the canonical frame's
sign (+y toward the user), the rear band's side, the report rate (90 Hz assumed), 0xF7 doubles
actually arriving, and every P1-P5 threshold (12.9 item 4).

**F10. Rejection choices the spec left open.** P2 "moves < 30 units" is Euclidean distance from
the landing point; a band contact is *excluded* while it is being watched (so it never joins
then leaves the set) and becomes a finger the moment it moves 30 units within 150 ms. P3 turns a
counted contact Palm for life and Cancels the scroll. P5 is the Euclidean sum of mouse counts
in the window. P4 (click guard) stays on with `rejection = off`, as 12.3.3 lists only P1-P3 and
P5 for that switch; a Python comparison of a recording with clicks will differ.

**F11. P1 may eat real taps.** A quick tap can lift before its size reaches 8; with rejection on
such a touch never counts, so no tap. As specified; tune on the device.

**F12. Tap geometry.** "Each contact moves < 40" is Euclidean from each contact's own landing
point. The second tap's "within 300 units" compares centroids taken when each tap reached its
peak count: the first report of a two-finger tap usually has one finger, whose position is
~250 units from the pair's centroid.

**F13. Phases where 12.3.5 is silent.** Every `MayBegin` is closed by exactly one `Ended` or
`Cancelled` (checked over every fixture). A two-finger set that locks sideways (Blocked) emits
no further phase until it ends. After a `Cancelled` with fingers still down, the next lock
emits `MayBegin` then `Began` under a new id. `Touches{Down}` precedes a report's scroll phases,
`Touches{Up}` follows them.

**F14. Who gets taps and G4.** `Tap{kind}` goes to hosts only while one claims the pointer, so
the one-finger double-tap (SmartZoom) does nothing elsewhere; the two-finger double-tap's
action fires regardless. G4 fires nothing at all (not only keys) while a host claims the
pointer, and nothing in `g4_foreign_mode = native`. G4's travel is measured from the landing
point; its duration from the X lock to the lift.

**F15. Protocol decisions.** One JSON message per `SOCK_SEQPACKET` packet, adjacently tagged,
`PROTOCOL_VERSION = 1`, 4096-byte cap. `Filter` is `All | Only(Vec<Topic>)`; `Hello` and
`DeviceGone` bypass filters. With no mouse, a new subscriber gets `Hello` with an empty
`DeviceInfo` then `DeviceGone`. The `f32` fields keep `ToHost` from deriving `Eq`. A subscriber
whose queue is full misses that message rather than delaying the reader.

**F16. A dependency outside the pinned block.** `rustix` (poll, eventfd, `SOCK_SEQPACKET`,
`CLOCK_MONOTONIC`, typed ioctls) is not in `quire/docs/workspace-deps.toml`; §11 wants it added
there first. Its safe wrappers are why `sys.rs` is the only `unsafe`: six unsafe blocks, each
with a SAFETY comment, opcodes checked against the header values in a unit test. `toml`,
`serde_json` and `notify` use the pinned versions. Licences pass `cargo deny` with no change to
the allow-list.

**F17. Discovery matches fields, not substrings.** The Python daemon searched the uevent text
for "004c" and "0269"; palmrest parses `HID_ID` as bus:vendor:product. Only the Bluetooth
product id 0x0269 is known; the USB-C model's is unverified.

**F18. One Python test has no counterpart.** `test_removed_gestures_are_gone` asserts that
Python names do not exist. The other eleven cases are ported twice: against `AxisLock` /
`SwipeG` with the test's own parameters, and end to end through `replay` of a fixture.

**F19. Latency, replay only.** 10 000 synthesised reads (`tests/latency.rs`, release build, this
machine): read bytes to socket bytes p50 570 ns, p95 680 ns, p99 730 ns; to uinput event bytes
p95 810 ns. Debug build: p95 5.3 us / 5.9 us. The budget is 2 ms p95; the live figure adds the
eventfd hand-off, the send, and the kernel.

**F20. Not built.** Live mode's output (a synthetic four-contact swipe on the virtual touchpad
so the compositor tracks the workspace) is not implemented: `workspace_mode = live` only stops
the trigger; the model's formulas are in `gestures::live`. On COSMIC, `WorkspacePrev/Next` spawn
`sill workspace prev|next`, which sill-ipc does not have yet (12.7's proposed addition), and
`ShellIpc(s)` spawns `sill` with the words of `s`.

**F21. The Python daemon mis-reads 0xF7 doubles.** It never checks the report id, so a
Bluetooth double would be parsed as one report of garbage. palmrest decodes both halves; if the
mouse does send doubles, the two daemons differ there (the parity sessions contain none).
