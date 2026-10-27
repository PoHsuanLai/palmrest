//! Raw units to hi-res units with the remainder carried between samples of one locked stroke.
//!
//! A sample's raw units divide into whole hi-res units at the sample's rate; what is left over
//! is carried to the next sample, valued at the rate it was carried at, so a change of rate
//! never drops or repeats a fraction of a unit.

/// The remainder carried between samples of one locked stroke.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Quantizer {
    raw_remainder: f64,
    /// The raw remainder's value in hi-res units when it was carried: what the exact stream
    /// has already counted of it.
    carried_units: f64,
}

/// One sample through the quantiser: the exact hi-res amount the socket carries (as px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantized {
    pub exact_units: f64,
}

impl Quantizer {
    /// Adds `raw` units at `raw_per_hi_res` and returns the exact hi-res units due.
    pub fn step(self, raw: i32, raw_per_hi_res: f64) -> (Self, Quantized) {
        let raw_remainder = self.raw_remainder + f64::from(raw);
        let hi_res = (raw_remainder / raw_per_hi_res).trunc() as i32;
        let raw_remainder = raw_remainder - f64::from(hi_res) * raw_per_hi_res;
        let carried_units = raw_remainder / raw_per_hi_res;
        let exact_units = f64::from(hi_res) + carried_units - self.carried_units;
        let next = Self {
            raw_remainder,
            carried_units,
        };
        (next, Quantized { exact_units })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remainders_carry() {
        let (q, a) = Quantizer::default().step(5, 2.0);
        assert_eq!(a.exact_units, 2.5);
        let (_, b) = q.step(1, 2.0);
        assert_eq!(b.exact_units, 0.5, "the half unit carried over");
    }
}
