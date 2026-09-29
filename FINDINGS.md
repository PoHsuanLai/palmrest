FINDINGS
========

Open items and standing facts. Design references are to quire's `design/NN` documents.

## Open items

**Manual acceptance, waiting for the user and the device.** Items 1-7 of 12.8 pass as tests
(`tests/acceptance_1_decode.rs`, `acceptance_2_reference_feel.rs`,
`acceptance_3_reference_cases.rs`, `acceptance_4_to_7.rs`), with the replay half of item 10
(suppression) in `acceptance_4_to_7.rs` and the socket half in `tests/socket.rs`. Item 11 is
measured on replay only (see Latency). Manual: **8** (virtual touchpad under
`libinput debug-events`), **9** (calibration of the 0.25 mm/px factor), **10 live** (O2/O4
silent while a real shell-host claims the pointer), **11 live** (HID read to socket / uinput
write on the device), **12** (timestamp fit residual on the device).

**No real recording.** Every fixture is synthesised from the documented layout; the five
reference-feel sessions are synthesised with seeded jitter, not recorded as 12.8 item 2 asks.
Unproven until recorded: the canonical frame's sign (+y toward the user), the rear band's side,
the report rate (90 Hz assumed), 0xF7 doubles actually arriving, and every P1-P5 threshold
(12.9 item 4).

**The shipped scroll defaults are 22 / 2.3.** `scroll.speed = 22` and
`scroll.repeat_gain_max = 2300` as 12.6 documents them. The Python daemon this replaces ran at
speed 32 and acceleration 1.0 on the development machine; the design defaults may want to
become 32 and 1000.

**The foreign outputs on KDE.** The default `foreign_output = "touchpad"` scrolls through a new
device, "palmrest touchpad", and `"wheel"` through "palmrest wheel"; Plasma has no saved
natural-scrolling setting for either, so the direction may need setting once in System
Settings. The touchpad path is unproven live (items 8, 9).

**The virtual touchpad is small for 0.25 mm/px.** A pair landing at the centre of a
100 x 70 mm pad has 30 mm (120 px of scroll) before the 5 mm edge margin vertically. palmrest
then lifts and re-lands the pair at the centre, which a toolkit sees as one stroke ending and
another starting (possibly a kinetic fling in between). Needs item 9 on the device: a larger
virtual pad, a smaller mm/px, or landing opposite the motion.

**Re-landing during a glide is two gestures to a toolkit.** A 2000 px/s glide at 0.25 mm/px is
~5000 pad units against 1200 from the centre to the edge margin, so it re-lands about four
times. The re-land is a Lift frame and a Land frame at the same instant, then that frame's move
from the centre. libinput ends the two-finger scroll at the lift and the new pair must pass its
scroll-start threshold again, so a toolkit sees scroll-stop at the glide's current speed and a
new scroll whose first millimetre or so libinput may swallow. Unproven without the device; the
mitigations are those of the item above. The *stroke's* re-land (`Pad::moved`) drops the delta
of the frame that crosses the margin; the glide adds it back after landing.

**P1 may eat real taps.** A quick tap can lift before its size reaches 8; with rejection on
such a touch never counts, so no tap. As specified; tune on the device.

**Not built: live workspace mode.** A synthetic four-contact swipe on the virtual touchpad so
the compositor tracks the workspace. Its model and keys are built together with the driver
that uses them. On COSMIC,
`WorkspacePrev/Next` spawn `sill workspace prev|next`, which sill-ipc does not have yet (12.7's
proposed addition), and `ShellIpc(s)` spawns `sill` with the words of `s`.

**Settings keys design/22 should gain** (all proposed; palmrest reads them with these defaults):

| Key | Type | Default | Range |
| --- | --- | --- | --- |
| `touchpad.mm_per_px` | `Fraction` | `250` | |
| `touchpad.contact_spacing_mm` | `Count` | `20` | |
| `touchpad.min_down_ms` | `Ms` | `200` | |
| `feel.curve` | `Curve::{Classic,MacLike}` | `classic` | |
| `feel.mac_like_threshold_units_s` | `Count` | `1500` | `0..20000` |
| `feel.mac_like_scale_units_s` | `Count` | `3000` | `100..20000` |
| `feel.mac_like_exponent` | `Fraction` | `1600` (1.6) | `1000..4000` |
| `feel.mac_like_gain_max` | `Fraction` | `6000` (6.0) | `1000..20000` |
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

`momentum.*` duplicates sill's `scroll.momentum_*` keys for shell-host; the two should be one
table in design/22 if the Settings app is to keep them in step. Still hard-coded, and also
proposed: the timestamp calibration constants (10 s fit window, 60 s refit, 1 ms residual), the
pad geometry (4000 x 2800 units at 40/mm) and the repeat window (0.5 s). 22-SETTINGS §4.3
sketches a flat `GesturesSettings { scroll_speed, .. }`, which disagrees with its own §3.9/§3.10
dotted catalogue; palmrest follows the catalogue and §4.3 wants correcting.

**`rustix` is outside the pinned block.** It (poll, eventfd, `SOCK_SEQPACKET`,
`CLOCK_MONOTONIC`, typed ioctls) is not in quire's `docs/workspace-deps.toml`; it should be
added there. `toml`, `serde_json` and `notify` use the pinned versions.

**Every `TODO(untested)` left.** `sensor/evdev/mod.rs` (the whole sensor: fuzz, no contact
size, SYN_DROPPED without an `EVIOCGMTSLOTS` re-read, a node opened mid-touch, P5 counts);
`sensor/evdev/event.rs` (64-bit `input_event` layout only); `daemon/evdev.rs` (the capability
rule, and 64-bit sysfs mask words); `sensor/frame.rs` (`Shape::Unmeasured`: no mm major axis
for P3); `sensor/magic_mouse/mod.rs` (every model gets the Magic Mouse 2's surface);
`sensor/magic_mouse/known.rs` (Magic Mouse 2 on USB, the USB-C model's id and layout, the Magic
Mouse 1's report 0x29); `touch.rs` (12-bit wrap of deltas, now on every sensor's grid);
`config/sections.rs` (P5 in the sensor's own counts).

## Standing facts

**The settings file.** `$XDG_CONFIG_HOME/palmrest/gestures.toml` (22-SETTINGS §2). Every
documented key is a TOML path: tables `[scroll]`, `[swipe]`, `[tap]`, `[gestures]` (with
`[gestures.gesture_action_map]`), `[palm_rejection]`, `[feel]`, `[momentum]`, `[touchpad]`,
`[device]`, and top-level `rejection`, `foreign_output`, `foreign_momentum`. Ratios are
per-mille integers (`lock_ratio = 1500`), enums snake_case. The 2 mm lift rule of 12.3.9 needs
no key: a stroke older than `min_down_ms` has outlived the tap timeout whatever it moved.

**The reference feel is pinned by frozen goldens.** `tests/fixtures/reference/*.golden` were
produced once by the retired Python daemon and nothing regenerates them. The `Classic` curve
must match them per report: lock decisions and cumulative v120 on the wheel. Two consequences
for the code: `feel` computes `dt`, the lock timeout and the repeat window as differences of
float seconds (`Mono::float_since`), because integer-nanosecond `dt` moves a truncation by one
unit; and socket px are the wheel stream before truncation (whole units plus the carried
remainder at the current rate), so within a stroke they never differ from v120 x 0.5 by a unit.
The comparison is cumulative per stroke (from each `Began`), because the reference lock drops
its sub-unit remainder at every lock and unlock.

**A millimetre frame over the Magic Mouse's own grid.** The tracker consumes `Frame`s from a
`Sensor`, and every length threshold is configured in micrometres. The core does not compute in
float millimetres (that would move the truncations above); each frame is projected onto a fixed
integer grid, `sensor::lattice::REFERENCE`, the Magic Mouse 2's resolution as its kernel driver
advertises it: **26 units/mm across, 70 units/mm along** (`ABS_MT_POSITION_X` -1100..1258
res 26, `ABS_MT_POSITION_Y` -1589..2047 res 70). The physical pitch along y is ~70.6; the grid
uses the driver's number so the hidraw and evdev paths land on the same integers.
`MagicMouseHid` lifts each report at exactly the grid's resolution, and `units(mm(u)) == u` for
every integer, so the core sees the integers the mouse sent. Thresholds convert as
`round(um * units_per_mm / 1000)`. A rule that measures a Euclidean distance or compares both
axes against one number is anisotropic in millimetres and has an `_x_um` / `_y_um` pair; it
takes an `Ellipse`, which with equal radii on the grid is exactly `hypot(dx, dy) >= r`. Side
bands are widths from the surface's edges. Left in Magic Mouse units, because no other sensor
has them: `light_touch_size_max` (P1) and `large_contact_major_units` (P3), applied only to
`Shape::MagicMouse`; the `feel.mac_like_*` speeds, the velocity gain's 2000 units/s and the
speed ratio stay grid units per second (the transfer function, not recognition). P5's counts/s
is in the sensor's own mouse counts.

**Rejection choices the spec left open.** P2 "moves < 30 units" is Euclidean distance from the
landing point; a band contact is *excluded* while it is being watched and becomes a finger the
moment it moves 30 units within 150 ms. P3 turns a counted contact Palm for life and Cancels
the scroll. P5 is the Euclidean sum of mouse counts in the window. P4 (click guard) stays on
with `rejection = off`, as 12.3.3 lists only P1-P3 and P5 for that switch.

**Tap geometry.** "Each contact moves < 40" is Euclidean from each contact's own landing
point. The second tap's "within 300 units" compares centroids taken when each tap reached its
peak count: the first report of a two-finger tap usually has one finger, whose position is
~250 units from the pair's centroid.

**Phases where 12.3.5 is silent.** Every `MayBegin` is closed by exactly one `Ended` or
`Cancelled` (checked over every fixture). A two-finger set that locks sideways (Blocked) emits
no further phase until it ends. After a `Cancelled` with fingers still down, the next lock
emits `MayBegin` then `Began` under a new id. `Touches{Down}` precedes a report's scroll phases,
`Touches{Up}` follows them.

**Who gets taps and G4.** `Tap{kind}` goes to hosts only while one claims the pointer, so the
one-finger double-tap (SmartZoom) does nothing elsewhere; the two-finger double-tap's action
fires regardless. G4 fires nothing at all while a host claims the pointer, and nothing in
`g4_foreign_mode = native`. G4's travel is measured from the landing point; its duration from
the X lock to the lift.

**Protocol decisions.** One JSON message per `SOCK_SEQPACKET` packet, adjacently tagged,
`PROTOCOL_VERSION = 1`, 4096-byte cap. `Filter` is `All | Only(Vec<Topic>)`; `Hello` and
`DeviceGone` bypass filters. With no mouse, a new subscriber gets `Hello` with an empty
`DeviceInfo` then `DeviceGone`. The `f32` fields keep `ToHost` from deriving `Eq`. A subscriber
whose queue is full misses that message rather than delaying the reader.

**`unsafe` lives in `daemon/sys.rs` only.** Each block has a SAFETY comment; ioctl opcodes are checked
against the header values in a unit test. Licences pass `cargo deny`.

**Discovery matches fields, not substrings.** `HID_ID` is parsed as bus:vendor:product and
looked up in `sensor::magic_mouse::KNOWN`: Bluetooth Magic Mouse 2 (`0005:004c:0269`,
verified); Magic Mouse 2 on USB (`0003:05ac:0269`), the USB-C model (`004c`/`05ac`:`0323`,
the listed id, not read from a device) and the Magic Mouse 1 (`0005:05ac:030d`), all untested.
The Magic Mouse 1 is refused on hidraw with a pointer to `source = "evdev"`: it sends report
0x29 (different header, 6-byte touches), which `decode` does not read.

**0xF7 doubles.** palmrest checks the report id and decodes both halves of a Bluetooth double.
The reference sessions contain none.

**Latency, replay only.** 10 000 synthesised reads (`tests/latency.rs`, release build): read
bytes to socket bytes p50 570 ns, p95 680 ns, p99 730 ns; to uinput event bytes p95 810 ns.
Debug build: p95 5.3 us / 5.9 us. The budget is 2 ms p95; the live figure adds the eventfd
hand-off, the send, and the kernel.

**`feel.curve = "mac_like"`: the acceleration preset.** Opt-in; the default `classic` is the
reference feel. The shape is ours, fitted to the published *description* of the reference
desktop's scroll acceleration (slow scrolling is pixel-precise, faster motion gains
progressively, the gain saturates), not to its driver's table, which was neither read nor
copied. One function of the sample's finger speed `v` (grid units/s, the velocity gain's own
estimator: `|raw| / dt` for `0 < dt <= 50 ms`, else 0) replaces both classic gains:

```
g(v) = 1                                   v <= v0
g(v) = min(g_max, 1 + ((v - v0) / s)^p)    v >  v0          px = base * raw * g(v)
```

`base` is `detent_px / ((64 - speed) * 7)`, so the speed slider works under both curves.
Monotone, continuous, and with `p > 1` its slope is continuous at `v0` (tests in
`feel/curve.rs`). The repeat gain is not applied under `mac_like`: quick successive flicks are
momentum's business (11.3.6 boost). Shipped constants: `v0 = 1500` (~21 mm/s along y),
`s = 3000`, `p = 1.6`, `g_max = 6.0` (reached at 9703 units/s). On the synthesised fixtures
`palmrest replay --curve mac-like` gives: `feel_slow` 67.8 px at peak gain 1.000 against
Classic's 84.2 px at 1.655; `flick_two_vertical` 829 px at 3.39 against 490 px at 2.0;
`feel_flicks` 282 px at 1.29 against 867 px at 4.6 (the classic repeat gain reaches 2.3x on
quick successive strokes and nothing replaces it under `mac_like` except momentum). A starting
point for a bake-off on recorded sessions, not a measurement.

**`foreign_momentum = "ours"`: synthesized momentum for foreign apps.** Opt-in; the default
`toolkit` lifts at Ended and the app runs its own kinetic scroll. With `ours`, at a locked
stroke's Ended palmrest takes the release velocity of the stroke's socket px (design/11
§11.5.2) and runs the design/11 §11.5.1 drag (`v' = -a v^b`, the same closed form as
shell-host's engine, re-implemented in `feel/momentum.rs` since palmrest does not depend on
shell-host). The stop speed is 1 px/**s**, as design/11 has it (2000 px/s -> 501.5 px in
0.976 s). Each frame sends the closed-form travel so far minus what was already sent, so a late
wake sends one larger frame and the total never drifts. *Touchpad (O2):* the two contacts stay
down and move; at the glide's end they lift at ~1 px/s. *Wheel (O4):* a decaying run of hi-res
wheel frames, fractions carried. Any MayBegin during a glide stops it; a Cancelled, a claim
(`PointerOver{Ours}`) or a disconnect end it too; a flick under a claim never starts one.
Unproven: the touch-to-stop lift happens at glide speed, and toolkits may start a short fling
of their own from it. The 11.3.6 boost is not synthesized.

**The `Sensor` trait and the evdev sensor.** `fn surface(&self) -> Surface` (extent in mm,
resolution per axis, `ButtonZones`, `DeviceClock::{Ticks18, Kernel, ArrivalOnly}`) and
`fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, SensorError>`: `&mut self` and a
`Vec` because an evdev read is a run of events whose frames end at `SYN_REPORT` and can span
reads. `EvdevMultitouch` implements protocol B (`ABS_MT_SLOT`, `TRACKING_ID`,
`POSITION_X/Y`, `BTN_LEFT/RIGHT`, `REL_X/Y`), stamps each frame with its `SYN_REPORT` time (the
fd is switched to `CLOCK_MONOTONIC` with `EVIOCSCLOCKID`, which affects only palmrest's open
file), drops events after `SYN_DROPPED` until the next report and then treats every slot as
lifted, and refuses a node with no position resolution. The daemon picks a node that is a mouse
(`REL_X/Y`) with MT slots and is not a touchpad (`ID_INPUT_TOUCHPAD`, or `BTN_TOOL_FINGER`),
opens it `O_RDONLY` and never grabs it. `tests/evdev_parity.rs` replays every fixture through
both sensors: identical outputs except `palm_during_scroll` (evdev has no major axis, so no
P3). One read-only `palmrest record` of the real node returned the expected geometry and 16
slots; it has not tracked a real finger, and no other mouse has run it. Recordings: v1
(`PALMREC\x01`, hidraw) is what hidraw records write and every fixture is; v2 (`PALMREC\x02`)
adds a JSON header naming the source and, for evdev, the geometry. `recording::parse` refuses a
v2 evdev file rather than decoding it as hidraw.
