//! The sections of `palmrest/gestures.toml`, one struct per table, every key with the default
//! and range of 22-SETTINGS §3.9 / §3.10 (and `touchpad`, see FINDINGS).

use super::enums::{ContactNibblePolicy, G4Mode, SwipeRepeat, WorkspaceMode};
use super::units::{Count, Fraction, Ms, Percent, Px, Units};
use crate::action::ActionMap;
use crate::settings_struct;

settings_struct! {
    /// `[scroll]`: the scroll feel (12.3.8, 11.3.8).
    pub struct ScrollSettings {
        speed: Count = Count(22), in Count(0) => Count(63);
        lock_threshold: Units = Units(40), in Units(10) => Units(200);
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
        threshold: Units = Units(200), in Units(50) => Units(1000);
        vertical_max: Units = Units(150), in Units(20) => Units(1000);
        cooldown_ms: Ms = Ms(500), in Ms(0) => Ms(2000);
        workspace_mode: WorkspaceMode = WorkspaceMode::Trigger;
        repeat: SwipeRepeat = SwipeRepeat::AfterCooldown;
    }
}

settings_struct! {
    /// `[tap]`: the double-taps (G6, G7).
    pub struct TapSettings {
        max_ms: Ms = Ms(200);
        gap_ms: Ms = Ms(350);
        max_move_units: Units = Units(40);
        centroid_max_units: Units = Units(300);
    }
}

settings_struct! {
    /// `[gestures]`: the one-finger swipe, the live model, the action table.
    pub struct GestureSettings {
        g4_foreign_mode: G4Mode = G4Mode::InjectAndScroll;
        g4_travel_units: Units = Units(300);
        g4_max_duration_ms: Ms = Ms(400);
        g4_axis_ratio: Fraction = Fraction(2000);
        g4_settle_ms: Ms = Ms(50);
        host_swipe_d_units as "host_swipe_D_units": Units = Units(1000);
        live_workspace_d_units as "live_workspace_D_units": Units = Units(1000);
        live_workspace_overshoot_scale: Fraction = Fraction(250);
        live_workspace_overshoot_cap: Fraction = Fraction(125);
        live_workspace_velocity_window_ms: Ms = Ms(80);
        live_workspace_commit_velocity: Fraction = Fraction(1500);
        live_workspace_finish_min_ms: Ms = Ms(100);
        live_workspace_finish_max_ms: Ms = Ms(400);
        contact_nibble_policy: ContactNibblePolicy = ContactNibblePolicy::PythonParity;
        gesture_action_map: ActionMap = ActionMap::default();
    }
}

settings_struct! {
    /// `[palm_rejection]`: rules P1-P5 (12.3.3).
    pub struct PalmRejectionSettings {
        light_touch_size_max: Units = Units(8);
        side_band_x_min: Units = Units(-900);
        side_band_x_max: Units = Units(1060);
        rear_band_percent: Percent = Percent(15), in Percent(0) => Percent(100);
        resting_move_floor_units: Units = Units(30);
        resting_window_ms: Ms = Ms(150);
        large_contact_major_units: Units = Units(64);
        click_guard_ms: Ms = Ms(200);
        mouse_moving_threshold_counts_s: Count = Count(200);
        mouse_moving_window_ms: Ms = Ms(100);
        mouse_moving_lock_multiplier: Fraction = Fraction(2000);
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
