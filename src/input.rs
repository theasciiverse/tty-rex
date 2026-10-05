//! Terminal keys to the sim's browser-style key-down/key-up calls.
//!
//! - Jump: Space, Up, W, K. Hold for a higher jump; held through a landing
//!   it jumps again (the browser's key repeat does the same).
//! - Duck: Down, S, J. Mid-air it drops fast.
//! - Enter restarts after a crash, P pauses, V changes the look, M mutes,
//!   Q / Esc / Ctrl-C quit.
//!
//! With the kitty keyboard protocol presses and releases are exact. Without
//! it a terminal only sends presses (repeats included), so a release is
//! inferred when the presses stop: a key counts as held for a while after a
//! fresh press (long enough to bridge the terminal's initial repeat delay),
//! then for a short gap after each repeat.

use crate::sim::Runner;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

const PRE_REPEAT: Duration = Duration::from_millis(550);
const REPEAT_GAP: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Quit,
    Pause,
    Mute,
    Restart,
    Look,
}

#[derive(Default)]
struct Hold {
    down: bool,
    last: Option<Instant>,
    repeats: u32,
}

impl Hold {
    fn expired(&self, now: Instant) -> bool {
        let Some(last) = self.last else { return false };
        let window = if self.repeats == 0 { PRE_REPEAT } else { REPEAT_GAP };
        now.duration_since(last) > window
    }
}

#[derive(Clone, Copy)]
enum Key {
    Jump,
    Duck,
}

pub struct Input {
    exact: bool,
    jump: Hold,
    duck: Hold,
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input { exact, jump: Hold::default(), duck: Hold::default() }
    }

    fn hold(&mut self, k: Key) -> &mut Hold {
        match k {
            Key::Jump => &mut self.jump,
            Key::Duck => &mut self.duck,
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant, r: &mut Runner) -> Option<Cmd> {
        let key = match k.code {
            KeyCode::Char(' ') | KeyCode::Up | KeyCode::Char('w' | 'W' | 'k' | 'K') => Some(Key::Jump),
            KeyCode::Down | KeyCode::Char('s' | 'S' | 'j' | 'J') => Some(Key::Duck),
            _ => None,
        };
        if let Some(key) = key {
            let exact = self.exact;
            let h = self.hold(key);
            let release = k.kind == KeyEventKind::Release;
            if !exact {
                h.repeats = if h.down { h.repeats + 1 } else { 0 };
                h.last = Some(now);
            }
            h.down = !release;
            match (key, release) {
                (Key::Jump, false) => r.jump_down(),
                (Key::Jump, true) => r.jump_up(),
                (Key::Duck, false) => r.duck_down(),
                (Key::Duck, true) => r.duck_up(),
            }
            return None;
        }
        if k.kind == KeyEventKind::Release {
            return None;
        }
        match k.code {
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Cmd::Quit),
            KeyCode::Char('q' | 'Q') | KeyCode::Esc => Some(Cmd::Quit),
            KeyCode::Char('p' | 'P') => Some(Cmd::Pause),
            KeyCode::Char('m' | 'M') => Some(Cmd::Mute),
            KeyCode::Char('v' | 'V') => Some(Cmd::Look),
            KeyCode::Enter => Some(Cmd::Restart),
            _ => None,
        }
    }

    /// Without release events, release keys whose presses have stopped.
    /// Returns which were released, for the log.
    pub fn poll(&mut self, now: Instant, r: &mut Runner) -> Vec<&'static str> {
        let mut released = Vec::new();
        if self.exact {
            return released;
        }
        if self.jump.down && self.jump.expired(now) {
            self.jump.down = false;
            r.jump_up();
            released.push("jump");
        }
        if self.duck.down && self.duck.expired(now) {
            self.duck.down = false;
            r.duck_up();
            released.push("duck");
        }
        released
    }

    /// Drop held keys, e.g. on pause.
    pub fn clear(&mut self) {
        self.jump = Hold::default();
        self.duck = Hold::default();
    }
}
