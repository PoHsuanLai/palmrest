//! Recorded-like sessions for the reference-feel comparison: jittered positions and report
//! intervals from a fixed seed, so they are reproducible.

use super::{Script, finger};

/// A deterministic jitter source, so "recorded-like" sessions are reproducible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    /// An integer in `-k..=k`.
    fn jitter(&mut self, k: i64) -> i32 {
        ((self.next() % (2 * k as u64 + 1)) as i64 - k) as i32
    }

    /// A report interval around 11 ms (the mouse's rate is unpublished; 90 Hz is assumed).
    fn interval(&mut self) -> f64 {
        9.0 + (self.next() % 5) as f64
    }
}

/// A one- or two-finger stroke with jittered timing, from `from` moving `v` units per report.
fn stroke(
    s: Script,
    rng: &mut Lcg,
    start: f64,
    n: usize,
    fingers: &[(u8, i32, i32)],
    v: (i32, i32),
) -> (Script, f64) {
    let mut t = start;
    let mut s = s;
    for i in 0..n {
        let i = i as i32;
        let touches = fingers
            .iter()
            .map(|&(id, x, y)| finger(id, x + v.0 * i + rng.jitter(1), y + v.1 * i + rng.jitter(1)))
            .collect();
        s = s.at(t, touches);
        t += rng.interval();
    }
    (s.lift(t), t)
}

pub fn feel_slow() -> Script {
    let mut rng = Lcg(1);
    let (s, t) = stroke(
        Script::default(),
        &mut rng,
        1000.0,
        120,
        &[(1, 0, -900)],
        (0, 2),
    );
    // A pause long enough to unlock, then the same finger moves on.
    let (s, t) = stroke(s, &mut rng, t + 40.0, 40, &[(2, 0, -600)], (0, 3));
    let s = s
        .at(t + 300.0, vec![finger(2, 0, -480)])
        .at(t + 311.0, vec![finger(2, 0, -470)]);
    let (s, _) = stroke(s, &mut rng, t + 322.0, 60, &[(2, 0, -460)], (0, -2));
    s
}

pub fn feel_fast() -> Script {
    let mut rng = Lcg(2);
    (0..4).fold(Script::default(), |s, k| {
        let (s, _) = stroke(
            s,
            &mut rng,
            1000.0 + 1000.0 * f64::from(k),
            18,
            &[(1, 100, -1200)],
            (0, 40 + 5 * k),
        );
        s
    })
}

pub fn feel_flicks() -> Script {
    let mut rng = Lcg(3);
    let mut t = 1000.0;
    let mut s = Script::default();
    for k in 0..7 {
        let (next, end) = stroke(
            s,
            &mut rng,
            t,
            10,
            &[(1 + (k % 3) as u8, -100, -1000)],
            (0, 25),
        );
        s = next;
        t = end + 150.0;
    }
    s
}

pub fn feel_diagonal() -> Script {
    let mut rng = Lcg(4);
    let (s, t) = stroke(
        Script::default(),
        &mut rng,
        1000.0,
        50,
        &[(1, -600, -1000)],
        (6, 5),
    );
    let (s, t) = stroke(s, &mut rng, t + 600.0, 50, &[(1, -600, -1000)], (7, 3));
    let (s, t) = stroke(s, &mut rng, t + 600.0, 50, &[(1, 600, 500)], (-4, -6));
    let (s, _) = stroke(s, &mut rng, t + 600.0, 60, &[(1, 800, 800)], (-9, 1));
    s
}

pub fn feel_fingers() -> Script {
    let mut rng = Lcg(5);
    let mut s = Script::default();
    let mut t = 1000.0;
    // One finger, then a second lands mid-stroke, then the first lifts.
    for i in 0..90 {
        let y = -1000 + 4 * i;
        let mut touches = vec![];
        if i < 60 {
            touches.push(finger(1, 0, y + rng.jitter(1)));
        }
        if i >= 30 {
            touches.push(finger(2, 400, y + 50 + rng.jitter(1)));
        }
        s = s.at(t, touches);
        t += rng.interval();
    }
    let s = s.lift(t);
    // Two fingers vertical, two fingers sideways (Blocked), three fingers.
    let (s, t) = stroke(
        s,
        &mut rng,
        t + 700.0,
        40,
        &[(3, -300, -800), (4, 200, -800)],
        (0, 6),
    );
    let (s, t) = stroke(
        s,
        &mut rng,
        t + 700.0,
        40,
        &[(5, -500, 0), (6, -100, 0)],
        (8, 1),
    );
    let (s, _) = stroke(
        s,
        &mut rng,
        t + 700.0,
        30,
        &[(7, -500, 0), (8, 0, 0), (9, 500, 0)],
        (0, 6),
    );
    s
}
