//! Game state and rules, after Chromium's offline.ts `Runner`.
//!
//! This layer knows nothing about terminals: no Ratatui, no crossterm, no
//! audio. Input arrives as the same key-down/key-up calls the browser makes,
//! time arrives one 60 Hz frame per `tick`, and what the outside world should
//! react to (sounds, crashes) comes back as `Event`s. Units are the
//! original's canvas pixels (the canvas is 150 px tall); turning those into
//! Braille dots is the renderer's business.
//!
//! Constants and update order follow the Chromium source
//! (components/neterror/resources/dino_game, BSD-3-Clause), fetched
//! 2026-10-04 at chromium/chromium a40910b0aaf2.

pub mod horizon;
mod rng;
pub mod trex;

use horizon::Horizon;
use rng::Rng;
use trex::{Status, Trex};

pub const FPS: f64 = 60.0;
pub const MS_PER_FRAME: f64 = 1000.0 / FPS;
pub const HEIGHT: i32 = 150;
/// DEFAULT_DIMENSIONS.width; a narrower terminal shows less of the road
/// ahead, as Chrome does in a narrow window.
pub const MAX_WIDTH: i32 = 600;

const SPEED: f64 = 6.0;
/// normalModeConfig.mobileSpeedCoefficient: a field narrower than 600 px
/// starts slower, in proportion
const MOBILE_SPEED_COEFFICIENT: f64 = 1.2;
const ACCELERATION: f64 = 0.001;
const MAX_SPEED: f64 = 13.0;
const CLEAR_TIME: f64 = 3000.0;
const GAMEOVER_CLEAR_TIME: f64 = 1200.0;
const INVERT_DISTANCE: u32 = 700;
const INVERT_FADE_DURATION: f64 = 12000.0;
const MAX_BLINK_COUNT: u32 = 3;
/// The canvas widens from the T-Rex's width to full width over this long
/// (CSS `intro .4s ease-out`) once the first jump lands.
const INTRO_MS: f64 = 400.0;

// distance_meter.ts
const COEFFICIENT: f64 = 0.025;
const ACHIEVEMENT_DISTANCE: u32 = 100;
const FLASH_DURATION: f64 = 1000.0 / 4.0;
const FLASH_ITERATIONS: u32 = 3;

/// Collision box, in pixels relative to its owner (or absolute once adjusted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl CBox {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> CBox {
        CBox { x, y, w, h }
    }

    fn offset(self, by: CBox) -> CBox {
        CBox::new(self.x + by.x, self.y + by.y, self.w, self.h)
    }

    fn hits(self, o: CBox) -> bool {
        self.x < o.x + o.w && self.x + self.w > o.x && self.y < o.y + o.h && self.h + self.y > o.y
    }
}

/// offline.ts setSpeed
fn start_speed(width: i32) -> f64 {
    if width < MAX_WIDTH {
        SPEED.min(SPEED * width as f64 / MAX_WIDTH as f64 * MOBILE_SPEED_COEFFICIENT)
    } else {
        SPEED
    }
}

/// JavaScript's `Math.round`: halves go up, also for negatives.
pub fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Press,
    Hit,
    Score,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    Sound(Sound),
    Started,
    Crashed { score: u32, high: u32 },
    Restarted,
    Night(bool),
}

/// distance_meter.ts: the score, and its flashing at every 100.
#[derive(Clone)]
pub struct Meter {
    pub shown: u32,
    pub achievement: bool,
    flash_timer: f64,
    flash_iterations: u32,
    /// false while the score blinks off
    pub paint: bool,
}

impl Meter {
    fn new() -> Meter {
        Meter { shown: 0, achievement: false, flash_timer: 0.0, flash_iterations: 0, paint: true }
    }

    /// Returns true when the achievement sound should play.
    fn update(&mut self, dt: f64, score: u32) -> bool {
        self.paint = true;
        let mut sound = false;
        if !self.achievement {
            if score > 0 && score % ACHIEVEMENT_DISTANCE == 0 {
                self.achievement = true;
                self.flash_timer = 0.0;
                sound = true;
            }
            self.shown = score;
        } else if self.flash_iterations <= FLASH_ITERATIONS {
            self.flash_timer += dt;
            if self.flash_timer < FLASH_DURATION {
                self.paint = false;
            } else if self.flash_timer > FLASH_DURATION * 2.0 {
                self.flash_timer = 0.0;
                self.flash_iterations += 1;
            }
        } else {
            self.achievement = false;
            self.flash_iterations = 0;
            self.flash_timer = 0.0;
        }
        sound
    }
}

#[derive(Clone)]
pub struct Runner {
    pub width: i32,
    pub trex: Trex,
    pub horizon: Horizon,
    pub speed: f64,
    pub distance: f64,
    pub meter: Meter,
    /// best score, in displayed units
    pub high: u32,
    pub playing: bool,
    pub activated: bool,
    pub crashed: bool,
    pub paused: bool,
    pub playing_intro: bool,
    pub inverted: bool,
    running_time: f64,
    intro_time: f64,
    crash_time: f64,
    invert_timer: f64,
    invert_trigger: bool,
    rng: Rng,
    events: Vec<Event>,
}

impl Runner {
    pub fn new(width: i32, high: u32, seed: u64) -> Runner {
        let mut rng = Rng::new(seed);
        let trex = Trex::new(&mut rng);
        let horizon = Horizon::new(width, &mut rng);
        Runner {
            width,
            trex,
            horizon,
            speed: start_speed(width),
            distance: 0.0,
            meter: Meter::new(),
            high,
            playing: false,
            activated: false,
            crashed: false,
            paused: false,
            playing_intro: false,
            inverted: false,
            running_time: 0.0,
            intro_time: 0.0,
            crash_time: 0.0,
            invert_timer: 0.0,
            invert_trigger: false,
            rng,
            events: Vec::new(),
        }
    }

    /// distance_meter.ts getActualDistance
    pub fn score(&self) -> u32 {
        let d = self.distance.ceil();
        if d > 0.0 {
            js_round(d * COEFFICIENT) as u32
        } else {
            0
        }
    }

    /// How much of the field is revealed, in pixels: just the T-Rex before
    /// the first jump lands, then the intro widens it to the full width.
    pub fn visible_width(&self) -> i32 {
        if !self.activated {
            trex::WIDTH
        } else if self.playing_intro {
            let t = (self.intro_time / INTRO_MS).min(1.0);
            let ease = 1.0 - (1.0 - t) * (1.0 - t);
            trex::WIDTH + ((self.width - trex::WIDTH) as f64 * ease) as i32
        } else {
            self.width
        }
    }

    /// Milliseconds since the crash (auto mode gets back up after a pause).
    pub fn crash_ms(&self) -> f64 {
        self.crash_time
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    // ---------------------------------------------------------------- input

    /// Space or Up went down (or auto-repeated).
    pub fn jump_down(&mut self) {
        if self.crashed || self.paused {
            return;
        }
        if !self.playing {
            self.playing = true;
            self.events.push(Event::Started);
        }
        if !self.trex.jumping && !self.trex.ducking {
            self.events.push(Event::Sound(Sound::Press));
            self.trex.start_jump(self.speed, &mut self.rng);
        }
    }

    pub fn jump_up(&mut self) {
        if self.running() {
            self.trex.end_jump();
        } else if self.crashed && self.crash_time >= GAMEOVER_CLEAR_TIME {
            self.restart();
        } else if self.paused {
            self.trex.reset(&mut self.rng);
            self.paused = false;
            self.playing = true;
        }
    }

    pub fn duck_down(&mut self) {
        if self.crashed || self.paused || !self.playing {
            return;
        }
        if self.trex.jumping {
            self.trex.set_speed_drop();
        } else if !self.trex.ducking {
            self.trex.set_duck(true, &mut self.rng);
        }
    }

    pub fn duck_up(&mut self) {
        self.trex.speed_drop = false;
        self.trex.set_duck(false, &mut self.rng);
    }

    /// Enter: restart straight away after a crash.
    pub fn restart_key(&mut self) {
        if self.crashed {
            self.restart();
        }
    }

    /// offline.ts onVisibilityChange: losing focus stops the game. Unlike
    /// Chrome, getting focus back doesn't restart it on its own; Space or P
    /// does, so nobody is dropped back in mid-cactus.
    pub fn pause(&mut self) {
        if self.activated && self.playing && !self.crashed {
            self.paused = true;
            self.playing = false;
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.activated && !self.crashed {
            self.paused = !self.paused;
            self.playing = !self.paused;
        }
    }

    fn running(&self) -> bool {
        self.playing && !self.crashed && !self.paused
    }

    // ---------------------------------------------------------------- time

    /// Advance one 60 Hz frame.
    pub fn tick(&mut self) {
        let mut dt = MS_PER_FRAME;
        if self.crashed {
            self.crash_time += dt;
            return;
        }
        if self.playing {
            if self.trex.jumping {
                self.trex.update_jump(&mut self.rng);
            }
            self.running_time += dt;
            let has_obstacles = self.running_time > CLEAR_TIME;

            if self.trex.jump_count == 1 && !self.playing_intro && !self.activated {
                self.playing_intro = true;
                self.trex.playing_intro = true;
                self.activated = true;
                self.intro_time = 0.0;
            }
            if self.playing_intro {
                self.intro_time += dt;
                self.horizon.update(0.0, self.speed, has_obstacles, false, &mut self.rng);
            } else {
                if !self.activated {
                    dt = 0.0;
                }
                self.horizon.update(dt, self.speed, has_obstacles, self.inverted, &mut self.rng);
            }

            let collision = has_obstacles && self.horizon.obstacles.first().is_some_and(|o| self.collides(o));
            if !collision {
                self.distance += self.speed * dt / MS_PER_FRAME;
                if self.speed < MAX_SPEED {
                    self.speed += ACCELERATION;
                }
            } else {
                self.game_over();
                return;
            }

            let score = self.score();
            if self.meter.update(dt, score) {
                self.events.push(Event::Sound(Sound::Score));
            }
            self.update_night(dt, score);

        }
        if self.playing || (!self.activated && self.trex.blink_count < MAX_BLINK_COUNT) {
            self.trex.update(dt, None, &mut self.rng);
        }
        // the intro's CSS animation ending starts the game proper
        if self.playing_intro && self.intro_time >= INTRO_MS {
            self.playing_intro = false;
            self.trex.playing_intro = false;
            self.running_time = 0.0;
        }
    }

    fn update_night(&mut self, dt: f64, score: u32) {
        if self.invert_timer > INVERT_FADE_DURATION {
            self.invert_timer = 0.0;
            self.invert_trigger = false;
            self.set_inverted(false);
        } else if self.invert_timer > 0.0 {
            self.invert_timer += dt;
        } else if score > 0 {
            self.invert_trigger = score % INVERT_DISTANCE == 0;
            if self.invert_trigger && self.invert_timer == 0.0 {
                self.invert_timer += dt;
                self.set_inverted(true);
            }
        }
    }

    fn set_inverted(&mut self, on: bool) {
        if self.inverted != on {
            self.inverted = on;
            self.events.push(Event::Night(on));
        }
    }

    /// offline.ts checkForCollision: outer boxes first (inset by the
    /// sprites' 1 px transparent border), then box against box.
    fn collides(&self, o: &horizon::Obstacle) -> bool {
        let t = &self.trex;
        let tbox = CBox::new(t.x + 1, t.y + 1, trex::WIDTH - 2, trex::HEIGHT - 2);
        let obox = CBox::new(o.x + 1, o.y + 1, o.ty.width * o.size - 2, o.ty.height - 2);
        if !tbox.hits(obox) {
            return false;
        }
        t.collision_boxes().iter().any(|tb| o.boxes.iter().any(|ob| tb.offset(tbox).hits(ob.offset(obox))))
    }

    fn game_over(&mut self) {
        self.events.push(Event::Sound(Sound::Hit));
        self.playing = false;
        self.crashed = true;
        self.crash_time = 0.0;
        self.meter.achievement = false;
        self.meter.paint = true;
        self.trex.update(100.0, Some(Status::Crashed), &mut self.rng);
        let score = self.score();
        if score > self.high {
            self.high = score;
        }
        self.events.push(Event::Crashed { score, high: self.high });
    }

    fn restart(&mut self) {
        self.running_time = 0.0;
        self.playing = true;
        self.paused = false;
        self.crashed = false;
        self.distance = 0.0;
        self.speed = start_speed(self.width);
        self.meter = Meter::new();
        self.horizon.reset(&mut self.rng);
        self.trex.reset(&mut self.rng);
        self.events.push(Event::Sound(Sound::Press));
        self.invert_timer = 0.0;
        self.set_inverted(false);
        self.events.push(Event::Restarted);
    }

    /// Change the field width (terminal resize). Things already on screen
    /// keep their positions; new ones enter at the new right edge.
    pub fn resize(&mut self, width: i32) {
        self.width = width;
        self.horizon.width = width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(r: &mut Runner, frames: usize) {
        for _ in 0..frames {
            r.tick();
        }
    }

    #[test]
    fn js_round_matches_javascript() {
        assert_eq!(js_round(-10.5), -10.0);
        assert_eq!(js_round(10.5), 11.0);
        assert_eq!(js_round(-10.6), -11.0);
    }

    #[test]
    fn first_jump_lands_then_intro_activates() {
        let mut r = Runner::new(600, 0, 1);
        r.jump_down();
        assert!(r.playing && r.trex.jumping && !r.activated);
        let mut peak = trex::GROUND_Y;
        for _ in 0..120 {
            r.tick();
            peak = peak.min(r.trex.y);
            if r.activated {
                break;
            }
        }
        assert!(r.activated, "first landing should start the intro");
        // a held jump from speed 6 tops out near the canvas top (y < 30 stops the rise)
        assert!(peak < 40 && peak >= 0, "peak {peak}");
        run(&mut r, 30);
        assert!(!r.playing_intro);
        // the intro slides the dino 1 px a frame for 0.4 s, and it stays there
        assert!((23..=25).contains(&r.trex.x), "x {}", r.trex.x);
    }

    #[test]
    fn early_release_gives_a_short_hop() {
        let mut tall = Runner::new(600, 0, 1);
        tall.jump_down();
        let mut short = Runner::new(600, 0, 1);
        short.jump_down();
        let (mut peak_tall, mut peak_short) = (trex::GROUND_Y, trex::GROUND_Y);
        for i in 0..80 {
            // a release before minJumpHeight is reached (~4 frames) is
            // ignored, as in the original
            if i == 6 {
                short.jump_up();
            }
            tall.tick();
            short.tick();
            peak_tall = peak_tall.min(tall.trex.y);
            peak_short = peak_short.min(short.trex.y);
        }
        assert!(peak_short > peak_tall + 10, "short {peak_short} tall {peak_tall}");
        // ...but never lower than minJumpHeight
        assert!(peak_short <= trex::GROUND_Y - 30);
    }

    #[test]
    fn standing_still_eventually_crashes_and_restart_resets() {
        let mut r = Runner::new(600, 0, 7);
        r.jump_down();
        r.jump_up();
        for _ in 0..60 * 30 {
            r.tick();
            if r.crashed {
                break;
            }
        }
        assert!(r.crashed, "an idle dino should hit something within 30 s");
        let score = r.score();
        assert!(score > 0);
        assert_eq!(r.high, score);
        r.jump_up();
        assert!(r.crashed, "jump can't restart before gameoverClearTime");
        run(&mut r, 80);
        r.jump_up();
        assert!(!r.crashed && r.playing && r.score() == 0 && r.speed == SPEED);
    }

    #[test]
    fn multi_cactus_boxes_stretch() {
        let mut rng = Rng::new(3);
        let mut h = horizon::Horizon::new(600, &mut rng);
        let mut seen = false;
        for _ in 0..2000 {
            h.update(MS_PER_FRAME, 10.0, true, false, &mut rng);
            for o in &h.obstacles {
                if o.size == 3 {
                    seen = true;
                    assert_eq!(o.boxes[2].x + o.boxes[2].w, o.width);
                    assert_eq!(o.boxes[0].w + o.boxes[1].w + o.boxes[2].w, o.width);
                }
            }
        }
        assert!(seen);
    }

    #[test]
    fn no_three_in_a_row() {
        let mut rng = Rng::new(11);
        let mut h = horizon::Horizon::new(600, &mut rng);
        let mut kinds = Vec::new();
        for _ in 0..20000 {
            let before = h.spawned;
            h.update(MS_PER_FRAME, 9.0, true, false, &mut rng);
            if h.spawned > before {
                kinds.push(h.obstacles.last().unwrap().kind);
            }
        }
        assert!(kinds.len() > 50);
        assert!(kinds.windows(3).all(|w| !(w[0] == w[1] && w[1] == w[2])));
    }
}
