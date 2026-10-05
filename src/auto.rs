//! Auto mode's autopilot: the T-Rex runs on his own, as a strip of ambient
//! entertainment (say, the bottom pane of a tmux layout).
//!
//! The sim is deterministic and cheap to copy, so the bot doesn't guess at
//! timings: each frame it copies the game and plays a few options a little
//! way into the future (carry on, duck, jump with a short, medium or full
//! hold), then takes one that survives. It drives the sim through the same
//! key-down/key-up calls the keyboard does.
//!
//! It isn't meant to be perfect. Now and then, more often at speed, it
//! reacts a few frames late to an obstacle, and sometimes that's fatal;
//! auto mode then gets him back up after a pause.

use crate::sim::Runner;

/// How far ahead the bot looks, in frames (about 0.8 s).
const LOOKAHEAD: u32 = 50;
/// Jump-button holds to try, in frames: a short hop to a full jump.
const HOLDS: [u32; 3] = [3, 8, 20];
/// How long a duck lasts before the bot looks again, in frames.
const DUCK: u32 = 12;
/// Late reactions: the chance per obstacle (plus more at full speed), and
/// how late, in frames (min, min + span). Tuned over 20 headless runs from
/// a cold start: about 3 minutes on average (1.5 to 6), every run ending
/// in a crash eventually.
const LAPSE_CHANCE: f64 = 0.01;
const LAPSE_CHANCE_AT_SPEED: f64 = 0.03;
const LAPSE_MIN: u32 = 16;
const LAPSE_SPAN: u32 = 30;
/// An option gets past the danger if it lasts this many frames beyond the
/// crash that carrying on would cause.
const CLEAR: u32 = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan {
    Carry,
    Jump(u32),
    Duck(u32),
}

pub struct Autopilot {
    rng: u64,
    /// frames left before letting go of jump / duck
    hold_left: u32,
    duck_left: u32,
    /// the obstacle the bot last noticed (by spawn number), and how many
    /// frames late it's reacting to it
    noticed: u32,
    lapse: u32,
}

impl Autopilot {
    pub fn new(seed: u64) -> Autopilot {
        Autopilot { rng: seed | 1, hold_left: 0, duck_left: 0, noticed: u32::MAX, lapse: 0 }
    }

    /// xorshift64*: uniform in [0, 1)
    fn random(&mut self) -> f64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Decide and press for this frame; call before each `Runner::tick`.
    pub fn step(&mut self, r: &mut Runner) {
        if r.crashed || r.paused {
            return;
        }
        // let go of what's held
        if self.hold_left > 0 {
            self.hold_left -= 1;
            if self.hold_left == 0 {
                r.jump_up();
            }
        }
        if self.duck_left > 0 {
            self.duck_left -= 1;
            if self.duck_left == 0 {
                r.duck_up();
            }
        }
        if !r.playing {
            // the start screen: jump to start, as a player would
            r.jump_down();
            self.hold_left = HOLDS[2];
            return;
        }
        if r.trex.jumping || r.trex.ducking {
            return;
        }
        self.notice(r);
        if self.lapse > 0 {
            self.lapse -= 1;
            return;
        }
        let Some(crash) = survive(r, Plan::Carry) else { return };
        // try the options, shortest commitment first; take the first that
        // gets past this danger (the copy carries on blindly afterwards, so
        // "never crashes" would wrongly count the obstacle after next)
        let options = [Plan::Jump(HOLDS[0]), Plan::Jump(HOLDS[1]), Plan::Jump(HOLDS[2]), Plan::Duck(DUCK)];
        let clears = |f: Option<u32>| f.is_none_or(|f| f > crash + CLEAR);
        let mut best = (crash, Plan::Carry);
        let mut found = false;
        for plan in options {
            let f = survive(r, plan);
            if clears(f) {
                best = (u32::MAX, plan);
                found = true;
                break;
            }
            if let Some(f) = f {
                if f > best.0 {
                    best = (f, plan);
                }
            }
        }
        // nothing clears it yet and it's still a way off: wait, a later jump
        // may (jumping as early as possible lands on it)
        if !found && crash > 6 {
            return;
        }
        match best.1 {
            Plan::Carry => {}
            // released on exactly the frame the copy released on: Chrome
            // ignores a release before the jump has risen 30 px, so one
            // frame either way can turn a full jump into a short hop
            Plan::Jump(hold) => {
                r.jump_down();
                self.hold_left = hold;
            }
            Plan::Duck(d) => {
                r.duck_down();
                self.duck_left = d;
            }
        }
    }

    /// When a new obstacle becomes the next one, maybe react to it late:
    /// more likely the faster the game, a few frames each time.
    fn notice(&mut self, r: &Runner) {
        let h = &r.horizon;
        let Some((i, _)) = h.obstacles.iter().enumerate().find(|(_, o)| o.x + o.width > r.trex.x) else { return };
        let id = h.spawned - (h.obstacles.len() - i) as u32;
        if id == self.noticed {
            return;
        }
        self.noticed = id;
        let speed = ((r.speed - 6.0) / 7.0).clamp(0.0, 1.0);
        let (base, extra, min, span) = lapse_params();
        let chance = base + extra * speed;
        if self.random() < chance {
            self.lapse = min + (self.random() * span as f64) as u32;
        }
    }
}

#[cfg(not(test))]
fn lapse_params() -> (f64, f64, u32, u32) {
    (LAPSE_CHANCE, LAPSE_CHANCE_AT_SPEED, LAPSE_MIN, LAPSE_SPAN)
}

/// (tests can tune these through TTY_REX_LAPSE="base,extra,min,span")
#[cfg(test)]
fn lapse_params() -> (f64, f64, u32, u32) {
    let v: Vec<f64> = std::env::var("TTY_REX_LAPSE").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    if v.len() == 4 {
        (v[0], v[1], v[2] as u32, v[3] as u32)
    } else {
        (LAPSE_CHANCE, LAPSE_CHANCE_AT_SPEED, LAPSE_MIN, LAPSE_SPAN)
    }
}

/// Play `plan` on a copy of the game; the frame it crashes on, if it does
/// within the lookahead.
fn survive(r: &Runner, plan: Plan) -> Option<u32> {
    let mut c = r.clone();
    match plan {
        Plan::Carry => {}
        Plan::Jump(_) => c.jump_down(),
        Plan::Duck(_) => c.duck_down(),
    }
    for f in 0..LOOKAHEAD {
        match plan {
            Plan::Jump(hold) if f == hold => c.jump_up(),
            Plan::Duck(d) if f == d => c.duck_up(),
            _ => {}
        }
        c.tick();
        if c.crashed {
            return Some(f);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How long the bot survives, from a cold start, in seconds.
    fn survival(seed: u64, limit_s: u32) -> f64 {
        let mut r = Runner::new(600, 0, seed);
        let mut bot = Autopilot::new(seed.wrapping_mul(7919));
        for frame in 0..limit_s * 60 {
            bot.step(&mut r);
            r.tick();
            if r.crashed {
                return frame as f64 / 60.0;
            }
        }
        limit_s as f64
    }

    #[test]
    fn plays_well_but_not_forever() {
        let runs: Vec<f64> = (1..=20).map(|seed| survival(seed, 600)).collect();
        let mean = runs.iter().sum::<f64>() / runs.len() as f64;
        eprintln!("survival (s): {runs:.0?}, mean {mean:.0}");
        // good enough to be worth watching...
        assert!(mean > 45.0, "mean survival {mean:.0} s");
        // ...but it does die now and then
        assert!(runs.iter().filter(|&&s| s < 600.0).count() >= 10, "{runs:?}");
    }
}
