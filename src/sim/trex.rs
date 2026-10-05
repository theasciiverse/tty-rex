//! The T-Rex: jump physics, ducking and animation frames, after trex.ts.
//!
//! Positions are whole canvas pixels, as in the original (every move goes
//! through `Math.round`). One tick is one 60 Hz frame.

use super::rng::Rng;
use super::{js_round, CBox, MS_PER_FRAME};

pub const WIDTH: i32 = 44;
pub const HEIGHT: i32 = 47;
const START_X: i32 = 50;
const INTRO_DURATION: f64 = 1500.0;
const DROP_VELOCITY: f64 = -5.0;
const SPEED_DROP_COEFFICIENT: f64 = 3.0;
// normalJumpConfig
const GRAVITY: f64 = 0.6;
const MAX_JUMP_HEIGHT: i32 = 30;
const MIN_JUMP_HEIGHT: i32 = 30;
const INITIAL_JUMP_VELOCITY: f64 = -10.0;
const BLINK_TIMING: f64 = 7000.0;
/// DEFAULT_DIMENSIONS.height - HEIGHT - bottomPad
pub const GROUND_Y: i32 = 150 - HEIGHT - 10;

pub const BOXES_DUCKING: &[CBox] = &[CBox::new(1, 18, 55, 25)];
pub const BOXES_RUNNING: &[CBox] = &[
    CBox::new(22, 0, 17, 16),
    CBox::new(1, 18, 30, 9),
    CBox::new(10, 35, 14, 8),
    CBox::new(1, 24, 29, 5),
    CBox::new(5, 30, 21, 4),
    CBox::new(9, 34, 15, 4),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Crashed,
    Ducking,
    Jumping,
    Running,
    Waiting,
}

/// Sprite frames, named for what they show; the original addresses them by
/// x offset into the sprite sheet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame {
    Stand,
    Blink,
    Run1,
    Run2,
    Crash,
    Duck1,
    Duck2,
}

fn anim(status: Status) -> (&'static [Frame], f64) {
    match status {
        Status::Waiting => (&[Frame::Blink, Frame::Stand], 1000.0 / 3.0),
        Status::Running => (&[Frame::Run1, Frame::Run2], 1000.0 / 12.0),
        Status::Crashed => (&[Frame::Crash], 1000.0 / 60.0),
        Status::Jumping => (&[Frame::Stand], 1000.0 / 60.0),
        Status::Ducking => (&[Frame::Duck1, Frame::Duck2], 1000.0 / 8.0),
    }
}

#[derive(Clone)]
pub struct Trex {
    pub x: i32,
    pub y: i32,
    pub jumping: bool,
    pub ducking: bool,
    pub speed_drop: bool,
    pub jump_count: u32,
    pub blink_count: u32,
    pub playing_intro: bool,
    pub status: Status,
    x_initial: i32,
    min_jump_y: i32,
    jump_velocity: f64,
    reached_min_height: bool,
    timer: f64,
    frame: usize,
    ms_per_frame: f64,
    /// The waiting animation only redraws once a blink is due, so the frame
    /// on screen is kept separately from the animation cursor.
    shown: Frame,
    clock: f64,
    anim_start: f64,
    blink_delay: f64,
}

impl Trex {
    pub fn new(rng: &mut Rng) -> Trex {
        let mut t = Trex {
            x: 0,
            y: GROUND_Y,
            jumping: false,
            ducking: false,
            speed_drop: false,
            jump_count: 0,
            blink_count: 0,
            playing_intro: false,
            status: Status::Waiting,
            x_initial: 0,
            min_jump_y: GROUND_Y - MIN_JUMP_HEIGHT,
            jump_velocity: 0.0,
            reached_min_height: false,
            timer: 0.0,
            frame: 0,
            ms_per_frame: MS_PER_FRAME,
            shown: Frame::Stand,
            clock: 0.0,
            anim_start: 0.0,
            blink_delay: 0.0,
        };
        t.update(0.0, Some(Status::Waiting), rng);
        t
    }

    /// The frame to draw.
    pub fn frame(&self) -> Frame {
        match self.status {
            Status::Waiting => self.shown,
            _ => anim(self.status).0[self.frame],
        }
    }

    pub fn collision_boxes(&self) -> &'static [CBox] {
        if self.ducking {
            BOXES_DUCKING
        } else {
            BOXES_RUNNING
        }
    }

    /// Advance animation by `dt` ms, optionally switching status first.
    pub fn update(&mut self, dt: f64, status: Option<Status>, rng: &mut Rng) {
        self.timer += dt;
        self.clock += dt;
        if let Some(s) = status {
            self.status = s;
            self.frame = 0;
            self.ms_per_frame = anim(s).1;
            if s == Status::Waiting {
                self.anim_start = self.clock;
                self.blink_delay = (rng.random() * BLINK_TIMING).ceil();
            }
        }
        if self.playing_intro && self.x < START_X {
            self.x += js_round(START_X as f64 / INTRO_DURATION * dt) as i32;
            self.x_initial = self.x;
        }
        if self.status == Status::Waiting {
            if self.clock - self.anim_start >= self.blink_delay {
                self.shown = anim(Status::Waiting).0[self.frame];
                if self.frame == 1 {
                    self.blink_delay = (rng.random() * BLINK_TIMING).ceil();
                    self.anim_start = self.clock;
                    self.blink_count += 1;
                }
            }
        } else if self.status == Status::Crashed && self.ducking {
            // trex.ts draw(): a crash while ducking nudges the crash sprite
            // a pixel forward (it is drawn once, then the game freezes)
            self.x += 1;
        }
        if self.timer >= self.ms_per_frame {
            let n = anim(self.status).0.len();
            self.frame = if self.frame == n - 1 { 0 } else { self.frame + 1 };
            self.timer = 0.0;
        }
        if self.speed_drop && self.y == GROUND_Y {
            self.speed_drop = false;
            self.set_duck(true, rng);
        }
    }

    pub fn start_jump(&mut self, speed: f64, rng: &mut Rng) {
        if !self.jumping {
            self.update(0.0, Some(Status::Jumping), rng);
            self.jump_velocity = INITIAL_JUMP_VELOCITY - speed / 10.0;
            self.jumping = true;
            self.reached_min_height = false;
            self.speed_drop = false;
        }
    }

    pub fn end_jump(&mut self) {
        if self.reached_min_height && self.jump_velocity < DROP_VELOCITY {
            self.jump_velocity = DROP_VELOCITY;
        }
    }

    /// One frame of jump physics. Returns true on landing.
    pub fn update_jump(&mut self, rng: &mut Rng) -> bool {
        if self.speed_drop {
            self.y += js_round(self.jump_velocity * SPEED_DROP_COEFFICIENT) as i32;
        } else {
            self.y += js_round(self.jump_velocity) as i32;
        }
        self.jump_velocity += GRAVITY;
        if self.y < self.min_jump_y || self.speed_drop {
            self.reached_min_height = true;
        }
        if self.y < MAX_JUMP_HEIGHT || self.speed_drop {
            self.end_jump();
        }
        if self.y > GROUND_Y {
            self.reset(rng);
            self.jump_count += 1;
            return true;
        }
        false
    }

    pub fn set_speed_drop(&mut self) {
        self.speed_drop = true;
        self.jump_velocity = 1.0;
    }

    pub fn set_duck(&mut self, ducking: bool, rng: &mut Rng) {
        if ducking && self.status != Status::Ducking {
            self.update(0.0, Some(Status::Ducking), rng);
            self.ducking = true;
        } else if self.status == Status::Ducking {
            self.update(0.0, Some(Status::Running), rng);
            self.ducking = false;
        }
    }

    pub fn reset(&mut self, rng: &mut Rng) {
        self.x = self.x_initial;
        self.y = GROUND_Y;
        self.jump_velocity = 0.0;
        self.jumping = false;
        self.ducking = false;
        self.update(0.0, Some(Status::Running), rng);
        self.speed_drop = false;
        self.jump_count = 0;
    }
}
