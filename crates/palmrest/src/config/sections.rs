//! The sections of `palmrest/gestures.toml`, one struct per table, every key with the default
//! and range of 22-SETTINGS §3.9 / §3.10 (and `touchpad`, see FINDINGS).
//!
//! Lengths are micrometres (`_um`). The defaults sit on the Magic Mouse 2 grid (26 units/mm
//! across, 70 units/mm along), so the core rounds them back to whole sensor units. A rule that
//! measures a Euclidean distance, or compares both axes against one number, is anisotropic in
//! millimetres; it has an `_x_um` / `_y_um` pair.

use super::enums::{ContactNibblePolicy, Curve, DeviceSource, G4Mode, SwipeRepeat};
use super::units::{Count, Fraction, Ms, Percent, Px, Um, Units};
use crate::action::ActionMap;
use crate::settings_struct;

settings_struct! {
    /// `[scroll]`: the scroll feel (12.3.8, 11.3.8).
    pub struct ScrollSettings {
        speed: Count = Count(22), in Count(0) => Count(63);
        /// The lock needs this much accumulated motion on an axis (was one raw number for both).
        lock_threshold_x_um: Um = Um(1538), in Um(385) => Um(7692);
        lock_threshold_y_um: Um = Um(571), in Um(143) => Um(2857);
        lock_ratio: Fraction = Fraction(1500), in Fraction(1000) => Fraction(4000);
        lock_timeout_ms: Ms = Ms(250), in Ms(0) => Ms(2000);
        velocity_gain_max: Fraction = Fraction(2000), in Fraction(1000) => Fraction(4000);
        repeat_gain_max: Fraction = Fraction(2300), in Fraction(1000) => Fraction(4000);
        detent_px: Px = Px(60), in Px(20) => Px(200);
    }
}

settings_struct! {
    /// `[swipe]`: the two-finger workspace swipe (G5).
    pub struct SwipeSettings {
        /// Horizontal travel that fires.
        threshold_um: Um = Um(7692), in Um(1923) => Um(38462);
        /// Vertical travel past which a pair counts as scrolling.
        vertical_max_um: Um = Um(2143), in Um(286) => Um(14286);
        cooldown_ms: Ms = Ms(500), in Ms(0) => Ms(2000);
        repeat: SwipeRepeat = SwipeRepeat::AfterCooldown;
    }
}

settings_struct! {
    /// `[tap]`: the double-taps (G6, G7).
    pub struct TapSettings {
        max_ms: Ms = Ms(200);
        gap_ms: Ms = Ms(350);
        /// No contact of a tap moves this far from where it landed (an ellipse).
        max_move_x_um: Um = Um(1538);
        max_move_y_um: Um = Um(571);
        /// The second tap lands this close to the first (an ellipse).
        centroid_max_x_um: Um = Um(11538);
        centroid_max_y_um: Um = Um(4286);
    }
}

settings_struct! {
    /// `[gestures]`: the one-finger swipe, the host swipe stream, the action table.
    pub struct GestureSettings {
        g4_foreign_mode: G4Mode = G4Mode::InjectAndScroll;
        g4_travel_um: Um = Um(11538);
        g4_max_duration_ms: Ms = Ms(400);
        g4_axis_ratio: Fraction = Fraction(2000);
        g4_settle_ms: Ms = Ms(50);
        host_swipe_d_um as "host_swipe_D_um": Um = Um(38462);
        contact_nibble_policy: ContactNibblePolicy = ContactNibblePolicy::OneToFour;
        gesture_action_map: ActionMap = ActionMap::default();
    }
}

settings_struct! {
    /// `[palm_rejection]`: rules P1-P5 (12.3.3).
    pub struct PalmRejectionSettings {
        /// P1, on the Magic Mouse's own 0..=63 contact size (other sensors have none).
        light_touch_size_max: Units = Units(8);
        /// P2's side bands: their width from the left and right edges of the surface.
        side_band_left_um: Um = Um(7692);
        side_band_right_um: Um = Um(7615);
        rear_band_percent: Percent = Percent(15), in Percent(0) => Percent(100);
        /// P2: a band contact that moves this far is a finger (an ellipse).
        resting_move_floor_x_um: Um = Um(1154);
        resting_move_floor_y_um: Um = Um(429);
        resting_window_ms: Ms = Ms(150);
        /// P3, on the Magic Mouse's own major axis units (other sensors have none).
        large_contact_major_units: Units = Units(64);
        click_guard_ms: Ms = Ms(200);
        // TODO(untested): P5 is in the sensor's own mouse counts per second, so it means
        // whatever another mouse's CPI makes it; only the Magic Mouse's counts are known.
        mouse_moving_threshold_counts_s: Count = Count(200);
        mouse_moving_window_ms: Ms = Ms(100);
        mouse_moving_lock_multiplier: Fraction = Fraction(2000);
    }
}

settings_struct! {
    /// `[device]`: which sensor the daemon reads. Chosen when the daemon looks
    /// for a device, so a change applies at the next reconnect.
    pub struct DeviceSettings {
        source: DeviceSource = DeviceSource::Hidraw;
    }
}

settings_struct! {
    /// `[touchpad]`: the virtual touchpad's contact rules (12.3.9 O2). Proposed values that
    /// 22-SETTINGS does not list yet; see FINDINGS.
    pub struct TouchpadSettings {
        mm_per_px: Fraction = Fraction(250);
        contact_spacing_mm: Count = Count(20);
        min_down_ms: Ms = Ms(200);
    }
}

settings_struct! {
    /// `[feel]`: the acceleration curve and the `MacLike` shape's constants.
    /// Speeds are grid units per second of one sample (26/mm across, 70/mm along: the Magic
    /// Mouse 2's raw units), as the velocity gain measures them. They shape the transfer
    /// function rather than recognise a gesture, and are applied to whichever axis locked, so
    /// they stay in grid units rather than becoming x/y millimetre pairs.
    pub struct FeelSettings {
        curve: Curve = Curve::Classic;
        mac_like_threshold_units_s: Count = Count(1500), in Count(0) => Count(20000);
        mac_like_scale_units_s: Count = Count(3000), in Count(100) => Count(20000);
        mac_like_exponent: Fraction = Fraction(1600), in Fraction(1000) => Fraction(4000);
        mac_like_gain_max: Fraction = Fraction(6000), in Fraction(1000) => Fraction(20000);
    }
}

settings_struct! {
    /// `[momentum]`: the glide palmrest synthesizes with `foreign_momentum = "ours"`: the
    /// design/11 §11.3.6 values (drag `v' = -a v^b`, release window, thresholds) and the
    /// frame interval of the synthesized contacts.
    pub struct MomentumSettings {
        a: Fraction = Fraction(30000), in Fraction(1000) => Fraction(65000);
        b: Fraction = Fraction(700), in Fraction(100) => Fraction(950);
        stop_px_s: Count = Count(1), in Count(1) => Count(1000);
        window_ms: Ms = Ms(80), in Ms(20) => Ms(200);
        held_still_ms: Ms = Ms(50), in Ms(10) => Ms(200);
        min_samples: Count = Count(3), in Count(2) => Count(20);
        start_px_s: Count = Count(100), in Count(0) => Count(5000);
        cap_px_s: Count = Count(12000), in Count(100) => Count(60000);
        frame_ms: Ms = Ms(8), in Ms(1) => Ms(50);
    }
}
