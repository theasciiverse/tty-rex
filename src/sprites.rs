//! Sprites as Braille dot bitmaps, downsampled at startup from the 1x pixel
//! bitmaps in assets/sprites.txt (made by tools/extract_sprites.py from
//! Chromium's sprite sheet).
//!
//! One dot covers `scale` x `scale` canvas pixels. Each sprite is sampled at
//! whichever pixel phase lines its art grid up with the dot grid (fewest
//! part-covered dots), and drawing snaps it to that same phase in world
//! space, so at scale 2 the 2x2-block pixel art keeps every art pixel.

use std::collections::HashMap;

const DATA: &str = include_str!("../assets/sprites.txt");

struct Bitmap {
    w: i32,
    h: i32,
    thin: bool,
    px: Vec<bool>,
}

impl Bitmap {
    fn px(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.px[(y * self.w + x) as usize]
    }

    /// Dot grid at `scale` and pixel phase (ox, oy), plus how many dots
    /// were only partly covered.
    fn sample(&self, scale: i32, ox: i32, oy: i32) -> (Sprite, usize) {
        let w = (self.w + ox + scale - 1) / scale;
        let h = (self.h + oy + scale - 1) / scale;
        let full = (scale * scale) as usize;
        // thin sprites light a dot for any pixel, pixel art needs half of it
        let need = if self.thin { 1 } else { full.div_ceil(2) };
        let mut dots = Vec::with_capacity((w * h) as usize);
        let mut partial = 0;
        for j in 0..h {
            for i in 0..w {
                let mut n = 0;
                for dy in 0..scale {
                    for dx in 0..scale {
                        n += self.px(i * scale + dx - ox, j * scale + dy - oy) as usize;
                    }
                }
                if n > 0 && n < full {
                    partial += 1;
                }
                dots.push(n >= need);
            }
        }
        (Sprite { phase: (ox, oy), w, h, dots }, partial)
    }

    fn best(&self, scale: i32) -> Sprite {
        let mut best: Option<(Sprite, usize)> = None;
        for oy in 0..scale {
            for ox in 0..scale {
                let cand = self.sample(scale, ox, oy);
                if best.as_ref().is_none_or(|b| cand.1 < b.1) {
                    best = Some(cand);
                }
            }
        }
        best.expect("scale >= 1").0
    }
}

fn parse() -> Vec<(String, Bitmap)> {
    let mut out: Vec<(String, Bitmap)> = Vec::new();
    for line in DATA.lines().filter(|l| !l.starts_with("# ") && !l.is_empty()) {
        if let Some(rest) = line.strip_prefix("sprite ") {
            let mut parts = rest.split_whitespace();
            let name = parts.next().expect("sprite name").to_string();
            let thin = parts.next() == Some("1");
            out.push((name, Bitmap { w: 0, h: 0, thin, px: Vec::new() }));
        } else if let Some((_, b)) = out.last_mut() {
            b.w = line.len() as i32;
            b.h += 1;
            b.px.extend(line.bytes().map(|c| c == b'#'));
        }
    }
    out
}

pub struct Sprite {
    /// pixel phase the sprite was sampled at
    pub phase: (i32, i32),
    pub w: i32,
    pub h: i32,
    dots: Vec<bool>,
}

impl Sprite {
    pub fn dot(&self, x: i32, y: i32) -> bool {
        self.dots[(y * self.w + x) as usize]
    }
}

/// Every sprite at one scale.
pub struct Sprites {
    pub scale: i32,
    map: HashMap<String, Sprite>,
}

impl Sprites {
    pub fn get(&self, name: &str) -> &Sprite {
        self.map.get(name).unwrap_or_else(|| panic!("no sprite {name}"))
    }
}

/// Hand-placed touches for the T-Rex, in sheet pixels, applied at every
/// scale up to MAX_TOUCH_SCALE after downsampling. The eye is a 2x2 pixel
/// hole, so from scale 3 on it falls between dots and the dino goes
/// blank-faced; and the snout keeps square pixel-art corners that look blunt
/// once coarse.
struct Touch {
    names: &'static [&'static str],
    /// centre of the eye: always a one-dot hole, with the head filled in
    /// around it wherever the sheet says the head is
    eye: Option<(f64, f64)>,
    /// snout corners to round off
    corners: &'static [(f64, f64)],
}

/// Past this scale the head is about three dots wide: there's no room for an
/// eye, and a forced hole or a trimmed corner only chips the silhouette.
const MAX_TOUCH_SCALE: i32 = 4;

const TOUCHES: &[Touch] = &[
    Touch { names: &["trex_stand", "trex_run1", "trex_run2"], eye: Some((27.0, 6.0)), corners: &[(41.0, 4.0), (41.0, 12.0)] },
    // eyes shut: just the snout
    Touch { names: &["trex_blink"], eye: None, corners: &[(41.0, 4.0), (41.0, 12.0)] },
    // the crash eye is a 4x4 ring; at any scale a hole reads as it
    Touch { names: &["trex_crash"], eye: Some((28.0, 7.0)), corners: &[(41.0, 4.0), (41.0, 14.0)] },
    Touch { names: &["trex_duck1", "trex_duck2"], eye: Some((42.0, 24.0)), corners: &[(56.0, 22.0), (56.0, 30.0)] },
];

impl Sprite {
    fn set(&mut self, x: i32, y: i32, on: bool) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.dots[(y * self.w + x) as usize] = on;
        }
    }

    fn is(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.dot(x, y)
    }

    fn apply(&mut self, t: &Touch, b: &Bitmap, scale: i32) {
        let (ox, oy) = self.phase;
        let dot = |x: f64, y: f64| (((x + ox as f64) / scale as f64).floor() as i32, ((y + oy as f64) / scale as f64).floor() as i32);
        for &(x, y) in t.corners {
            let (cx, cy) = dot(x, y);
            // only a real corner: open on the outside both ways, so a
            // coarse snout isn't cut in half
            let open_right = !self.is(cx + 1, cy);
            let open_vert = !self.is(cx, cy - 1) || !self.is(cx, cy + 1);
            let solid_inside = self.is(cx - 1, cy);
            if open_right && open_vert && solid_inside {
                self.set(cx, cy, false);
            }
        }
        // at scale 2 the sheet's own eyes survive (the crash X included),
        // so only the coarser scales get a placed one
        if let Some((x, y)) = t.eye.filter(|_| scale > 2) {
            let (ex, ey) = dot(x, y);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    // the dot's centre, back in sheet pixels
                    let px = (ex + dx) * scale - ox + scale / 2;
                    let py = (ey + dy) * scale - oy + scale / 2;
                    if b.px(px, py) {
                        self.set(ex + dx, ey + dy, true);
                    }
                }
            }
            self.set(ex, ey, false);
        }
    }
}

/// All scales from `min` to `max`, built once.
pub fn build(min: i32, max: i32) -> Vec<Sprites> {
    let bitmaps = parse();
    (min..=max)
        .map(|scale| {
            let map = bitmaps
                .iter()
                .map(|(n, b)| {
                    let mut sp = b.best(scale);
                    // scale 1 is the sheet itself: nothing to touch up
                    for t in TOUCHES.iter().filter(|t| (2..=MAX_TOUCH_SCALE).contains(&scale) && t.names.contains(&n.as_str())) {
                        sp.apply(t, b, scale);
                    }
                    (n.clone(), sp)
                })
                .collect();
            Sprites { scale, map }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprites_parse_and_scale() {
        let sets = build(2, 6);
        let s2 = &sets[0];
        // 44x47 px at 2 px per dot, plus room for the phase
        let t = s2.get("trex_run1");
        assert!((22..=23).contains(&t.w) && (24..=25).contains(&t.h));
        let h = s2.get("horizon_flat");
        assert_eq!(h.w, 300);
        // the ground line survives as a full row of dots at every scale
        for s in &sets {
            let h = s.get("horizon_flat");
            assert!((0..h.h).any(|y| (0..h.w).all(|x| h.dot(x, y))), "scale {}", s.scale);
        }
        // where the eye is placed (scales 3 and 4) it's a hole surrounded by head
        for s in &sets[1..3] {
            for name in ["trex_stand", "trex_run1", "trex_duck2", "trex_crash"] {
                let t = s.get(name);
                let eye = (1..t.h - 1).any(|y| (1..t.w - 1).any(|x| !t.dot(x, y) && t.dot(x - 1, y) && t.dot(x + 1, y) && t.dot(x, y - 1) && t.dot(x, y + 1)));
                assert!(eye, "{name} at scale {} has no eye", s.scale);
            }
        }
    }
}

#[cfg(test)]
mod look {
    use super::*;

    /// `cargo test show_sprites -- --ignored --nocapture` prints sprites as
    /// Braille at every scale (TTY_REX_SPRITES=name1,name2 picks them).
    #[test]
    #[ignore]
    fn show_sprites() {
        let names = std::env::var("TTY_REX_SPRITES").unwrap_or_else(|_| "trex_stand".into());
        for set in build(2, 12) {
            for name in names.split(',') {
                let s = set.get(name);
                println!("{name} scale {} phase {:?}", set.scale, s.phase);
                for cy in (0..s.h).step_by(4) {
                    let mut line = String::new();
                    for cx in (0..s.w).step_by(2) {
                        let mut bits = 0u32;
                        for (row, masks) in [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]].iter().enumerate() {
                            for (col, mask) in masks.iter().enumerate() {
                                let (x, y) = (cx + col as i32, cy + row as i32);
                                if x < s.w && y < s.h && s.dot(x, y) {
                                    bits |= mask;
                                }
                            }
                        }
                        line.push(char::from_u32(0x2800 + bits).unwrap());
                    }
                    println!("{line}");
                }
            }
        }
    }
}
