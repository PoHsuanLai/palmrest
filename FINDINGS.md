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

## Feel presets (branch `feel-presets`)

**F22. `feel.curve = "mac_like"`: the acceleration preset.** Opt-in; the default `current` is
the Python feel through the untouched code path (the parity suite runs unchanged; the only
edit to it is the new `curve: CurveParams::Current` field in `acceptance_3`'s literal
`LockParams`). The shape is ours, fitted to the published *description* of the reference
desktop's scroll acceleration (slow scrolling is pixel-precise, faster motion gains
progressively, the gain saturates), not to its driver's table, which was neither read nor
copied. One function of the sample's finger speed `v` (raw units/s, the velocity gain's own
estimator: `|raw| / dt` for `0 < dt <= 50 ms`, else 0) replaces both Python gains:

```
g(v) = 1                                   v <= v0
g(v) = min(g_max, 1 + ((v - v0) / s)^p)    v >  v0          px = base * raw * g(v)
```

`base` is the existing `detent_px / ((64 - speed) * 7)`, so "1:1" means the base ratio of
`scroll.speed` and `scroll.detent_px`, and the speed slider still works under both curves.
Monotone everywhere, continuous, and with `p > 1` its slope is continuous at `v0` (table and
sweep tests in `feel/curve.rs`). The repeat gain is not applied under `mac_like`: the
reference model has no per-stroke repeat divisor, and quick successive flicks are momentum's
business (11.3.6 boost). Shipped constants: `v0 = 1500` (~21 mm/s along y at ~70.6 units/mm),
`s = 3000`, `p = 1.6`, `g_max = 6.0` (reached at 9703 units/s): gain 1.33 at 3000, 2.0 at
4500, 2.91 at 6000 units/s. On the synthesised fixtures `palmrest replay --curve mac-like`
(defaults, no gestures.toml) gives: `parity_slow` 67.8 px at peak gain 1.000 against
Current's 84.2 px at 1.655 (velocity x repeat); `flick_two_vertical` (6667 units/s) 829 px at
3.39 against 490 px at 2.0; `parity_flicks` 282 px at 1.29 against 867 px at 4.6. The last is
the repeat gain: quick successive strokes reach 2.3x under Current and nothing replaces that
under `mac_like` except momentum, so repeated medium-speed flicks scroll a third as far. The
numbers are a starting point for the user's
bake-off on recorded sessions, not a measurement. New keys, for design/22 §3.9 (all
proposed):

| Key | Type | Default | Range |
| --- | --- | --- | --- |
| `feel.curve` | `Curve::{Current,MacLike}` | `current` | |
| `feel.mac_like_threshold_units_s` | `Count` | `1500` | `0..20000` |
| `feel.mac_like_scale_units_s` | `Count` | `3000` | `100..20000` |
| `feel.mac_like_exponent` | `Fraction` | `1600` (1.6) | `1000..4000` |
| `feel.mac_like_gain_max` | `Fraction` | `6000` (6.0) | `1000..20000` |

**F23. `foreign_momentum = "ours"`: synthesized momentum for foreign apps.** Opt-in; the
default `toolkit` is unchanged (lift at Ended, the app runs its own kinetic scroll). With
`ours`, at a locked stroke's Ended palmrest takes the release velocity of the stroke's socket
px (design/11 §11.5.2: least squares over the last 80 ms, >= 3 samples, 0 if the newest is
older than 50 ms, no glide under 100 px/s, cap 12 000 px/s) and runs the design/11 §11.5.1
drag (`v' = -a v^b`, a = 30, b = 0.7, stop 1 px/s; the same closed form as shell-host's
engine, re-implemented in `feel/momentum.rs` since palmrest does not depend on shell-host).
Note the stop speed is 1 px/**s** as design/11 has it: that is what gives the worked numbers
(2000 px/s -> 501.5 px in 0.976 s, tested to 0.5 %; end to end through the pad and the wheel,
within 5 % and one frame). Each frame (every `momentum.frame_ms`, driven by the reader's poll
deadline, and by any report that arrives) sends the closed-form travel so far minus what was
already sent, so a late wake sends one larger frame and the total never drifts.
*Touchpad (O2):* the two contacts stay down and move; at the glide's end they lift at ~1 px/s,
so a toolkit's own kinetic scroll starts from nothing. *Wheel (O4, the KDE choice of F7):* a
decaying run of hi-res wheel frames, fractions carried (`UnitCarry`). *Touch to stop:* any
MayBegin during a glide lifts the contacts in that report (wheel: the frames stop). A
Cancelled, a claim (`PointerOver{Ours}`) or a disconnect end it too; a flick under a claim
never starts one. `palmrest replay` now ticks the tracker at its deadlines between and after
records, as the reader does, so glides and delayed lifts replay frame by frame (records keep
one `Step` each; ticks add steps; every earlier test passed unchanged). Risk, unproven: the
touch-to-stop lift happens at glide speed, and toolkits (GTK computes its kinetic velocity
from the recent scroll deltas) may start a short fling of their own from it. New keys, for
design/22 §3.9 (proposed; the design/11 values):

| Key | Type | Default | Range |
| --- | --- | --- | --- |
| `foreign_momentum` | `ForeignMomentum::{Toolkit,Ours}` | `toolkit` | |
| `momentum.a` | `Fraction` | `30000` (30) | `1000..65000` |
| `momentum.b` | `Fraction` | `700` (0.7) | `100..950` (b < 1 or the glide never ends) |
| `momentum.stop_px_s` | `Count` | `1` | `1..1000` |
| `momentum.window_ms` | `Ms` | `80` | `20..200` |
| `momentum.held_still_ms` | `Ms` | `50` | `10..200` |
| `momentum.min_samples` | `Count` | `3` | `2..20` |
| `momentum.start_px_s` | `Count` | `100` | `0..5000` |
| `momentum.cap_px_s` | `Count` | `12000` | `100..60000` |
| `momentum.frame_ms` | `Ms` | `8` | `1..50` |

(`momentum.*` duplicates sill's `scroll.momentum_*` keys for shell-host; the two should be
one table in design/22 if the Settings app is to keep them in step. The 11.3.6 boost is not
synthesized.)

**F24. Re-landing during a glide is two gestures to a toolkit.** A 2000 px/s glide at
0.25 mm/px is ~5000 pad units against 1200 from the centre to the edge margin, so it re-lands
about four times (F8). The re-land is a Lift frame and a Land frame at the same instant, with
no stop of our own (no hold, no slow-down), then that frame's move from the centre. It cannot
be one gesture through libinput: the lift frame takes both tracking ids and `BTN_TOUCH` to 0,
libinput ends the two-finger scroll there and sends the axis stop, and the new pair must pass
libinput's scroll-start threshold before it scrolls again. So a toolkit sees scroll-stop at the
glide's current speed (GTK and Qt may begin their own kinetic scroll, which the next scroll
event cancels) and a new scroll, whose first millimetre or so libinput may swallow. Unproven
without the device; the mitigations are those of F8 (a larger virtual pad, a smaller
`mm_per_px`, landing opposite the motion, which would halve the re-lands). A tracking-id swap
inside one `SYN_REPORT` might keep the finger count at two throughout; how libinput treats
that is unknown and it is not attempted. Found on the way: the *stroke's* re-land (the core
`Pad::moved`, used by `toolkit` too) drops the delta of the frame that crosses the margin; the
glide adds it back after landing, the stroke path is left as it was.

## Sensor seam (branch `sensor`)

**F25. A millimetre frame over the Magic Mouse's own grid.** The tracker now consumes
`Frame`s (`buttons`, `motion`, `contacts` at `MmPoint`s, `stamp`) from a `Sensor`, and every
length threshold is configured in micrometres. The core does *not* compute in float
millimetres: the feel truncates the per-axis centroid and carries integer remainders exactly as
the Python daemon did (F4, F5), and float millimetres would move those truncations. Instead
each frame is projected onto a fixed integer grid, `sensor::lattice::REFERENCE`, which is the
Magic Mouse 2's resolution as its kernel driver advertises it: **26 units/mm across, 70
units/mm along**, read from the device with `EVIOCGABS` (`evtest`, read-only:
`ABS_MT_POSITION_X` -1100..1258 res 26, `ABS_MT_POSITION_Y` -1589..2047 res 70). The ~70.6 in
`touch.rs` and F22 was the physical pitch; the driver truncates it to 70, and the grid uses the
driver's number so that the mouse's hidraw and evdev paths land on the same integers. A mm value
printed by palmrest is therefore up to 0.8 % off the physical length along y, which changes
nothing: defaults are defined through the same factors. `MagicMouseHid` lifts each report at
exactly the grid's resolution, and `units(mm(u)) == u` for every integer (tested over
-5000..5000 at several resolutions, and over every x of the mouse's range), so the core sees the
integers the mouse sent. Thresholds convert as `round(um * units_per_mm / 1000)`:

| Old key (raw units) | Rule | New key(s), default (grid units) | Anisotropic in mm? |
| --- | --- | --- | --- |
| `scroll.lock_threshold` 40 | each axis vs one number | `lock_threshold_x_um` 1538 (40), `_y_um` 571 (40) | yes: 1.54 mm across, 0.57 along |
| `swipe.threshold` 200 | x travel | `threshold_um` 7692 (200) | |
| `swipe.vertical_max` 150 | y travel, then \|y\| > \|x\| in grid units | `vertical_max_um` 2143 (150) | the \|y\| > \|x\| comparison is (kept) |
| `tap.max_move_units` 40 | Euclidean | `max_move_x_um` 1538, `_y_um` 571 (40, 40) | yes: an ellipse |
| `tap.centroid_max_units` 300 | Euclidean | `centroid_max_x_um` 11538, `_y_um` 4286 (300, 300) | yes |
| `gestures.g4_travel_units` 300 | x travel | `g4_travel_um` 11538 (300) | |
| `gestures.host_swipe_D_units`, `live_workspace_D_units` 1000 | x travel | `host_swipe_D_um`, `live_workspace_D_um` 38462 (1000) | |
| `palm_rejection.side_band_x_min` -900 | absolute x | `side_band_left_um` 7692: 200 from the left edge (-1100) | |
| `palm_rejection.side_band_x_max` 1060 | absolute x | `side_band_right_um` 7615: 198 from the right edge (1258) | |
| `palm_rejection.resting_move_floor_units` 30 | Euclidean | `resting_move_floor_x_um` 1154, `_y_um` 429 (30, 30) | yes |

Euclidean rules take an `Ellipse` of an x and a y radius; with equal radii on the grid (every
default) the scale factors are exactly 1.0 and the test is the old `hypot(dx, dy) >= r` float
for float. Side bands became widths from the surface's edges so they mean something on another
surface; the rear band was already a share of the length. Left in Magic Mouse units, because no
other sensor has them: `light_touch_size_max` (size 0..63, P1) and `large_contact_major_units`
(P3), applied only to `Shape::MagicMouse`; and the `feel.mac_like_*` speeds, the velocity gain's
2000 units/s and the speed ratio, which are the transfer function rather than recognition
thresholds and are applied to whichever axis locked, so they stay grid units per second (the
same per millimetre on every sensor). P5's counts/s is in the sensor's own mouse counts.

**F26. Old settings files keep working, and the Magic Mouse is unchanged.** Every old raw-unit
key is still read (`config::legacy`) with its old range, converted onto its replacement(s),
and reported as a `Warning` with `Outcome::Converted` ("... is in raw Magic Mouse units, which
are deprecated; read as scroll.lock_threshold_x_um = 1538, ..."); a replacement set in the same
table wins (`Outcome::Ignored`); a bad old value keeps the default (`Outcome::DefaultUsed`, the
existing wording). `SCROLL_LOCK_THRESHOLD`, `SWIPE_THRESHOLD` and `SWIPE_VERTICAL_MAX` convert
the same way (now clamped at 1 000 000 units rather than `i32::MAX`, which a micrometre key
could not hold). Parity: every existing test passes; edits to tests were API only
(`LockParams.threshold` is a `Threshold { x, y }` in `acceptance_3`'s literal; the config tests
name the new keys; `contacts`' band table moved to `gestures/bands.rs` with the same cases).
Beyond the suite, `palmrest replay` of all 43 fixtures under four configurations (defaults;
wheel + `foreign_momentum = "ours"` + `mac_like`; `rejection = "off"`; a file of *non-default
old raw keys*), with arrival and calibrated timing, 344 outputs, is byte-identical between
master (e923d19) and this branch.

**F27. The `Sensor` trait and the evdev sensor.** `fn surface(&self) -> Surface` (extent in mm,
resolution per axis, `ButtonZones`, `DeviceClock::{Ticks18, Kernel, ArrivalOnly}`) and
`fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, SensorError>`. `&mut self` and a
`Vec` because an evdev read is a run of events whose frames end at `SYN_REPORT` and can span
reads (and the kernel resends only changed values, so slot positions outlive their contacts);
a `Result` because a malformed read is logged and counted, as before. `EvdevMultitouch`
implements protocol B (`ABS_MT_SLOT`, `TRACKING_ID`, `POSITION_X/Y`, `BTN_LEFT/RIGHT`,
`REL_X/Y`), stamps each frame with its `SYN_REPORT` time (the fd is switched to
`CLOCK_MONOTONIC` with `EVIOCSCLOCKID`, which affects only palmrest's open file), drops events
after `SYN_DROPPED` until the next report and then treats every slot as lifted, and refuses a
node with no position resolution. The daemon picks a node that is a mouse (`REL_X/Y`) with MT
slots and is not a touchpad (`ID_INPUT_TOUCHPAD` in udev's database, or `BTN_TOOL_FINGER`),
opens it `O_RDONLY` and never grabs it. Two new ioctls (`EVIOCGABS`, `EVIOCSCLOCKID`) put
`sys.rs` at eight unsafe blocks, opcodes checked against the header values. Evidence, and its
limit: table tests over synthetic sequences; `tests/evdev_parity.rs` re-expresses every
fixture as the events the driver would send and replays it through both sensors, identical
outputs except `palm_during_scroll` (evdev has no major axis, so no P3); and one read-only
`palmrest record` of the real node with `source = "evdev"`, whose v2 header came back as
x -1100..1258 res 26, y -1589..2047 res 70, 16 slots (the ioctls work), with 0 reads because
nobody touched the mouse. It has not tracked a real finger, and no other mouse has run it.
Recordings: v1 (`PALMREC\x01`, hidraw) is still what hidraw records write and every fixture
is; v2 (`PALMREC\x02`) adds a JSON header naming the source and, for evdev, the geometry the
bytes do not carry. `recording::parse` refuses a v2 evdev file rather than decoding it as
hidraw.

**F28. Known devices.** Discovery looks `HID_ID` up in `sensor::magic_mouse::KNOWN`, verified
first: Bluetooth Magic Mouse 2 (`0005:004c:0269`, verified); Magic Mouse 2 on USB
(`0003:05ac:0269`), the USB-C model (`004c`/`05ac`:`0323`) and the Magic Mouse 1
(`0005:05ac:030d`), all untested. The Magic Mouse 1 is refused on hidraw with a pointer to
`source = "evdev"`: it sends report 0x29 (different header, 6-byte touches), which `decode`
does not read. The USB-C product id is the one listed for it, not read from a device.

**F29. Every `TODO(untested)` left.** `sensor/evdev/mod.rs` (the whole sensor: fuzz, no contact
size, SYN_DROPPED without an `EVIOCGMTSLOTS` re-read, a node opened mid-touch, P5 counts);
`sensor/evdev/event.rs` (64-bit `input_event` layout only); `daemon/evdev.rs` (the capability
rule, and 64-bit sysfs mask words); `sensor/frame.rs` (`Shape::Unmeasured`: no mm major axis
for P3); `sensor/magic_mouse/mod.rs` (every model gets the Magic Mouse 2's surface);
`sensor/magic_mouse/known.rs` (Magic Mouse 2 on USB, the USB-C model's id and layout, the Magic
Mouse 1's report 0x29); `touch.rs` (12-bit wrap of deltas, now on every sensor's grid);
`config/sections.rs` (P5 in the sensor's own counts).
