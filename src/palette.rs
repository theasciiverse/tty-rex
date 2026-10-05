//! Colours for the default theme: a day and a night palette, blended by the
//! night sky's fade (the moon's opacity), and mapped to whatever colour depth
//! the terminal has.

use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn lerp(self, o: Rgb, t: f64) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
        Rgb(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2))
    }

    pub fn mix(self, o: Rgb) -> Rgb {
        self.lerp(o, 0.5)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Depth {
    True,
    /// xterm 256 colours
    Indexed,
}

impl Depth {
    pub fn detect() -> Depth {
        match std::env::var("COLORTERM") {
            Ok(v) if v == "truecolor" || v == "24bit" => Depth::True,
            _ => Depth::Indexed,
        }
    }

    pub fn color(self, c: Rgb) -> Color {
        match self {
            Depth::True => Color::Rgb(c.0, c.1, c.2),
            Depth::Indexed => Color::Indexed(xterm256(c)),
        }
    }
}

/// Nearest xterm-256 colour over the 6x6x6 cube and the grey ramp, by the
/// "redmean" weighted distance (rounding each channel on its own turns sand
/// green and sky teal).
fn xterm256(c: Rgb) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let dist = |r: i32, g: i32, b: i32| {
        let rm = (c.0 as i32 + r) / 2;
        let (dr, dg, db) = (c.0 as i32 - r, c.1 as i32 - g, c.2 as i32 - b);
        (512 + rm) * dr * dr / 256 + 4 * dg * dg + (767 - rm) * db * db / 256
    };
    let cube = (0..216).map(|i| (16 + i as u8, dist(LEVELS[i / 36], LEVELS[(i / 6) % 6], LEVELS[i % 6])));
    let greys = (0..24).map(|i| {
        let v = 8 + i * 10;
        (232 + i as u8, dist(v, v, v))
    });
    cube.chain(greys).min_by_key(|&(_, d)| d).map_or(16, |(n, _)| n)
}

/// Everything the scene is coloured with.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub sky_top: Rgb,
    pub sky_mid: Rgb,
    pub sky_horizon: Rgb,
    pub dunes: Rgb,
    pub dunes_far: Rgb,
    pub sand: Rgb,
    pub pebble: Rgb,
    pub ground: Rgb,
    pub dino: Rgb,
    pub crashed: Rgb,
    pub cactus: Rgb,
    pub bird: Rgb,
    pub cloud: Rgb,
    pub moon: Rgb,
    pub star: Rgb,
    pub text: Rgb,
    pub score: Rgb,
    /// the plain background of the colour-Braille look, and its clouds
    /// (white ones vanish on a pale page)
    pub page: Rgb,
    pub page_cloud: Rgb,
}

pub const DAY: Palette = Palette {
    sky_top: Rgb(0x8e, 0xc5, 0xf2),
    sky_mid: Rgb(0xc4, 0xd6, 0xdf),
    sky_horizon: Rgb(0xfb, 0xe8, 0xcc),
    dunes: Rgb(0xe3, 0xb8, 0x86),
    dunes_far: Rgb(0xee, 0xd0, 0xa8),
    sand: Rgb(0xe9, 0xc4, 0x8a),
    pebble: Rgb(0xc4, 0x96, 0x5c),
    ground: Rgb(0x5e, 0x4b, 0x3c),
    // Chrome's own ink
    dino: Rgb(0x53, 0x53, 0x53),
    crashed: Rgb(0xc8, 0x32, 0x28),
    cactus: Rgb(0x2f, 0x7d, 0x3a),
    bird: Rgb(0x6a, 0x48, 0x8e),
    cloud: Rgb(0xff, 0xff, 0xff),
    moon: Rgb(0xff, 0xf2, 0xb8),
    star: Rgb(0xff, 0xff, 0xe0),
    text: Rgb(0x53, 0x53, 0x53),
    // Asciiverse amber
    score: Rgb(0xb0, 0x6a, 0x00),
    page: Rgb(0xf6, 0xf1, 0xe7),
    page_cloud: Rgb(0x9c, 0xc4, 0xe4),
};

pub const NIGHT: Palette = Palette {
    sky_top: Rgb(0x07, 0x0b, 0x24),
    sky_mid: Rgb(0x18, 0x16, 0x3a),
    sky_horizon: Rgb(0x2b, 0x22, 0x4e),
    dunes: Rgb(0x3b, 0x2c, 0x4c),
    dunes_far: Rgb(0x30, 0x27, 0x4c),
    sand: Rgb(0x46, 0x37, 0x40),
    pebble: Rgb(0x2c, 0x22, 0x2c),
    ground: Rgb(0xa8, 0x98, 0x8a),
    dino: Rgb(0xd8, 0xd8, 0xd8),
    crashed: Rgb(0xff, 0x5a, 0x48),
    cactus: Rgb(0x5f, 0xb0, 0x6a),
    bird: Rgb(0xb8, 0x92, 0xe6),
    cloud: Rgb(0x5a, 0x5e, 0x80),
    moon: Rgb(0xff, 0xf2, 0xb8),
    star: Rgb(0xff, 0xff, 0xe0),
    text: Rgb(0xe0, 0xe0, 0xe0),
    score: Rgb(0xff, 0xb0, 0x00),
    page: Rgb(0x0d, 0x10, 0x2c),
    page_cloud: Rgb(0x5a, 0x5e, 0x80),
};

// The 256-colour cube has no real sand or dusk tones, and the nearest match
// to a blend comes out green or grey, so on 256-colour terminals the scenery
// uses hand-picked entries (exact cube colours, so they map to themselves)
// and the sky is three bands instead of a gradient. Sprites map fine as is.
const fn cube(r: u8, g: u8, b: u8) -> Rgb {
    const L: [u8; 6] = [0, 95, 135, 175, 215, 255];
    Rgb(L[r as usize], L[g as usize], L[b as usize])
}

pub const DAY_256: Palette = Palette {
    sky_top: cube(2, 4, 5),
    sky_mid: cube(3, 4, 5),
    sky_horizon: cube(5, 5, 4),
    dunes: cube(4, 2, 1),
    dunes_far: cube(4, 3, 2),
    sand: cube(5, 4, 2),
    pebble: cube(4, 3, 2),
    page: cube(5, 5, 4),
    ..DAY
};

pub const NIGHT_256: Palette = Palette {
    sky_top: Rgb(18, 18, 18),
    sky_mid: cube(0, 0, 1),
    sky_horizon: cube(1, 0, 2),
    dunes: cube(1, 0, 1),
    dunes_far: cube(1, 1, 2),
    sand: Rgb(58, 58, 58),
    pebble: Rgb(38, 38, 38),
    page: Rgb(18, 18, 18),
    ..NIGHT
};

impl Palette {
    /// The palette for this depth at night fade `t` (0 day, 1 night).
    pub fn for_depth(depth: Depth, t: f64) -> Palette {
        match depth {
            Depth::True => Palette::blend(&DAY, &NIGHT, t),
            // blends of hand-picked entries would land on muddy ones
            Depth::Indexed if t >= 0.5 => NIGHT_256,
            Depth::Indexed => DAY_256,
        }
    }

    pub fn blend(a: &Palette, b: &Palette, t: f64) -> Palette {
        Palette {
            sky_top: a.sky_top.lerp(b.sky_top, t),
            sky_mid: a.sky_mid.lerp(b.sky_mid, t),
            sky_horizon: a.sky_horizon.lerp(b.sky_horizon, t),
            dunes: a.dunes.lerp(b.dunes, t),
            dunes_far: a.dunes_far.lerp(b.dunes_far, t),
            sand: a.sand.lerp(b.sand, t),
            pebble: a.pebble.lerp(b.pebble, t),
            ground: a.ground.lerp(b.ground, t),
            dino: a.dino.lerp(b.dino, t),
            crashed: a.crashed.lerp(b.crashed, t),
            cactus: a.cactus.lerp(b.cactus, t),
            bird: a.bird.lerp(b.bird, t),
            cloud: a.cloud.lerp(b.cloud, t),
            moon: a.moon.lerp(b.moon, t),
            star: a.star.lerp(b.star, t),
            text: a.text.lerp(b.text, t),
            score: a.score.lerp(b.score, t),
            page: a.page.lerp(b.page, t),
            page_cloud: a.page_cloud.lerp(b.page_cloud, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm256_picks_sensible_entries() {
        assert_eq!(xterm256(Rgb(0, 0, 0)), 16);
        assert_eq!(xterm256(Rgb(255, 255, 255)), 231);
        assert_eq!(xterm256(Rgb(255, 0, 0)), 196);
        // the hand-picked 256-colour scenery maps to itself
        assert_eq!(xterm256(DAY_256.sand), 16 + 36 * 5 + 6 * 4 + 2);
        assert_eq!(xterm256(NIGHT_256.dunes_far), 16 + 36 + 6 + 2);
        // a mid grey lands on the grey ramp
        assert!((232..=255).contains(&xterm256(Rgb(0x53, 0x53, 0x53))));
    }
}
