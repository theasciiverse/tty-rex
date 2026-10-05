//! Drawing the sim into the terminal.
//!
//! The geometry decision behind the whole game: a Braille cell is 2 dots
//! wide and 4 tall, so on the usual 1:2 terminal cell its dots are square,
//! and one dot stands for `scale` x `scale` of the original canvas pixels.
//! Scale 2 is home: the 150 px canvas is 75 dots (19 rows), the full 600 px
//! road is 150 columns, and since the 1x sprite sheet is 2x2-block pixel art
//! every art pixel of the T-Rex, cacti and pterodactyl survives. Smaller
//! panes (a split at the bottom of the screen) get a coarser scale, down to
//! 7 rows at scale 6. The sim never sees any of this: physics stays in canvas
//! pixels, so the game plays the same at every scale.
//!
//! There are five looks (`Look`): plain Braille, colour Braille, all
//! half-blocks, the mixed default, and Chrome's classic greys. In the mixed
//! look the two kinds of glyph split the work: the
//! scenery behind the action (sky gradient, parallax dunes, the sand under
//! the ground line) is solid half-block colour, two pixels per cell, and
//! everything that moves or kills you stays in crisp Braille, coloured on
//! top of it. A Braille cell takes the scenery's colour as its background,
//! so the two blend instead of leaving holes.

use crate::sim::horizon::{Kind, LINE_WIDTH, LINE_Y, MOON_Y};
use crate::sim::trex::Frame;
use crate::sim::{self, Runner};
use crate::octants;
use crate::palette::{Depth, Palette, Rgb};
use crate::sprites::{self, Sprite, Sprites};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// 1: one dot per canvas pixel, Chrome's sprites exactly as drawn, 38 rows
pub const MIN_SCALE: i32 = 1;
pub const MAX_SCALE: i32 = 6;
/// Road to show, in canvas pixels, before trading detail for width: about
/// a phone-width Chrome window.
const PREFERRED_WIDTH: i32 = 360;
/// Below this the road ahead is too short to react to, at any scale.
const MIN_WIDTH: i32 = 240;

/// How the field sits in a terminal of a given size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    /// canvas pixels per Braille dot, both ways
    pub scale: i32,
    /// canvas width in pixels; the sim's field width
    pub width: i32,
    pub cols: u16,
    pub rows: u16,
    /// room for the help line under the field
    pub help: bool,
}

impl Layout {
    /// `wide`: the road runs the full terminal width instead of stopping at
    /// Chrome's 600 px.
    fn at(scale: i32, cols: u16, rows: u16, wide: bool) -> Option<Layout> {
        let dots_h = (sim::HEIGHT + scale - 1) / scale;
        let field_rows = ((dots_h + 3) / 4) as u16;
        let full = cols as i32 * 2 * scale;
        let width = if wide { full } else { full.min(sim::MAX_WIDTH) };
        if field_rows > rows || width < MIN_WIDTH {
            return None;
        }
        let field_cols = ((width + 2 * scale - 1) / (2 * scale)) as u16;
        Some(Layout { scale, width, cols: field_cols, rows: field_rows, help: rows >= field_rows + 2 })
    }

    /// The finest scale that fits and shows enough road, else the finest
    /// that fits at all. `forced` pins the scale.
    pub fn choose(cols: u16, rows: u16, forced: Option<i32>, wide: bool) -> Option<Layout> {
        if let Some(s) = forced {
            return Layout::at(s, cols, rows, wide);
        }
        let fits = || (MIN_SCALE..=MAX_SCALE).filter_map(|s| Layout::at(s, cols, rows, wide));
        fits().find(|l| l.width >= PREFERRED_WIDTH).or_else(|| fits().next())
    }

    /// Terminal row (within the field) of a canvas y.
    fn row(&self, y: i32) -> u16 {
        (y / self.scale / 4) as u16
    }

    /// Field row of the GAME OVER / PAUSED line: about (150 - 25) / 3 px, as
    /// in game_over_panel.ts, kept clear of the score row.
    fn text_row(&self) -> u16 {
        self.row(42).max(1)
    }

    /// Canvas y just below that line.
    fn text_bottom(&self) -> i32 {
        (self.text_row() as i32 + 1) * 4 * self.scale
    }
}

/// The smallest terminal that fits at all.
pub fn min_size() -> (u16, u16) {
    let s = MAX_SCALE;
    ((MIN_WIDTH / (2 * s)) as u16, ((((sim::HEIGHT + s - 1) / s) + 3) / 4) as u16)
}

// What each dot belongs to. Higher wins when sprites overlap in one dot,
// and a Braille cell is coloured by its highest.
const CLOUD: u8 = 1;
const STAR: u8 = 2;
const MOON: u8 = 3;
const GROUND: u8 = 4;
const CACTUS: u8 = 5;
const BIRD: u8 = 6;
const DINO: u8 = 7;
const ICON: u8 = 8;

/// Where the sand starts, just under the 1 px ground line at y = 131.
const SAND_Y: i32 = LINE_Y + 5;

/// How the game is drawn; `V` cycles through them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    /// Braille in the terminal's own colours; night inverts them
    Braille,
    /// coloured Braille sprites on a plain page
    ColorBraille,
    /// everything in colour half-blocks, sprites included
    Blocks,
    /// half-block scenery behind coloured Braille sprites
    Mixed,
    /// Chrome's greys on white, Braille (true colour)
    Classic,
}

impl Look {
    /// cycle order: the greyscale looks, then colour from least to most
    pub const ALL: [Look; 5] = [Look::Braille, Look::Classic, Look::ColorBraille, Look::Blocks, Look::Mixed];

    pub fn name(self) -> &'static str {
        match self {
            Look::Braille => "braille",
            Look::ColorBraille => "color",
            Look::Blocks => "blocks",
            Look::Mixed => "mixed",
            Look::Classic => "classic",
        }
    }

    pub fn parse(s: &str) -> Option<Look> {
        Look::ALL.into_iter().find(|l| l.name() == s)
    }

    pub fn next(self) -> Look {
        let i = Look::ALL.iter().position(|&l| l == self).unwrap_or(0);
        Look::ALL[(i + 1) % Look::ALL.len()]
    }
}

/// Hand-drawn restart arrows (↻): thick arc, corner-triangle head at the
/// top right, in three sizes.
const ARROW_LARGE: &[&str] = &[
    ".....####....#.",
    "...###..##..##.",
    "..##.....#.###.",
    ".##.......####.",
    ".##......#####.",
    "##......######.",
    "##.............",
    "##...........##",
    "##...........##",
    ".##.........##.",
    ".##.........##.",
    "..###.....###..",
    "...#########...",
    ".....#####.....",
];
const ARROW_MEDIUM: &[&str] = &[
    "...####.#..",
    "..##...##..",
    ".##...###..",
    "##...####..",
    "##.........",
    "##.......##",
    "##.......##",
    ".##.....##.",
    "..##...##..",
    "...#####...",
];
const ARROW_SMALL: &[&str] = &["..###.#.", ".#...##.", "#...###.", "#.......", "#......#", "#......#", ".#....#.", "..####.."];

/// Cheap integer hash for the sand's pebbles.
fn hash(a: i64, b: i64) -> u32 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h >> 32) as u32
}

fn trex_sprite(f: Frame) -> &'static str {
    match f {
        Frame::Stand => "trex_stand",
        Frame::Blink => "trex_blink",
        Frame::Run1 => "trex_run1",
        Frame::Run2 => "trex_run2",
        Frame::Crash => "trex_crash",
        Frame::Duck1 => "trex_duck1",
        Frame::Duck2 => "trex_duck2",
    }
}

/// The glyphs the blocks look fits cells with, as 8x8 masks (bit = row * 8
/// + col, from the top left) of where the foreground colour goes. Block
/// elements and box drawing only: terminals draw those themselves, so they
/// line up with the cell exactly.
fn glyph_masks() -> &'static [(char, u64)] {
    use std::sync::OnceLock;
    static MASKS: OnceLock<Vec<(char, u64)>> = OnceLock::new();
    MASKS.get_or_init(|| {
        let rows = |r: std::ops::Range<u64>| r.fold(0u64, |m, y| m | (0xFF << (y * 8)));
        let cols = |c: std::ops::Range<u64>| (0..8).fold(0u64, |m, y| m | (c.clone().fold(0u64, |row, x| row | 1 << x) << (y * 8)));
        let (tl, tr) = (rows(0..4) & cols(0..4), rows(0..4) & cols(4..8));
        let (bl, br) = (rows(4..8) & cols(0..4), rows(4..8) & cols(4..8));
        // no 1/8 or 2/8 wide vertical slivers (▏ ▎ ▕ ┃): on a coarse cell
        // they poke out of a silhouette as spikes
        let mut m = vec![(' ', 0), ('█', u64::MAX), ('▀', rows(0..4)), ('▔', rows(0..1)), ('▐', cols(4..8))];
        for (k, g) in "▁▂▃▄▅▆▇".chars().enumerate() {
            m.push((g, rows(7 - k as u64..8)));
        }
        for (k, g) in "▍▌▋▊▉".chars().enumerate() {
            m.push((g, cols(0..k as u64 + 3)));
        }
        m.extend([('▖', bl), ('▗', br), ('▘', tl), ('▝', tr), ('▙', tl | bl | br), ('▚', tl | br), ('▛', tl | tr | bl), ('▜', tl | tr | br), ('▞', tr | bl), ('▟', tr | bl | br)]);
        m.push(('━', rows(3..5)));
        m
    })
}

/// Fit one cell: reduce its samples to two colours, then pick the glyph
/// (either way round) whose mask disagrees with the fewest samples. A
/// sprite's colour is always one of the two, even when it covers less than
/// either scenery colour (the ground line between dune and sand), and the
/// colour left out folds into the nearer of the two.
fn fit_cell(samples: &[Rgb; 64], kinds: &[u8; 64]) -> (char, Rgb, Rgb) {
    let mut counts: Vec<(Rgb, u32, bool)> = Vec::with_capacity(4);
    for (&c, &k) in samples.iter().zip(kinds) {
        match counts.iter_mut().find(|(x, _, _)| *x == c) {
            Some(e) => e.1 += 1,
            None => counts.push((c, 1, k != 0)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let a = counts.iter().find(|e| e.2).unwrap_or(&counts[0]).0;
    let Some(&(b, _, _)) = counts.iter().find(|e| e.0 != a) else { return (' ', a, a) };
    let dist = |p: Rgb, q: Rgb| (p.0 as i32 - q.0 as i32).pow(2) + (p.1 as i32 - q.1 as i32).pow(2) + (p.2 as i32 - q.2 as i32).pow(2);
    // bit set where the sample is (nearer) colour b
    let mask = samples.iter().enumerate().fold(0u64, |m, (i, &c)| if c != a && (c == b || dist(c, b) < dist(c, a)) { m | 1 << i } else { m });
    let sprite_a = counts.iter().any(|e| e.0 == a && e.2);
    // a sprite with real presence in the cell must stay visible (a thin line
    // one row off a glyph's line would otherwise tie with "nothing")
    let must_show = sprite_a && (!mask).count_ones() >= 4;
    // the ground is a horizontal line: only horizontal glyphs, or a bump
    // pixel turns it into dashes of quadrants
    let horizontal = sprite_a && kinds.iter().filter(|&&k| k == GROUND).count() * 2 >= kinds.iter().filter(|&&k| k != 0).count();
    let mut best = (u32::MAX, ' ', a, a);
    for &(g, gm) in glyph_masks() {
        if horizontal && !matches!(g, ' ' | '█' | '▀' | '▔' | '▁' | '▂' | '▃' | '▄' | '▅' | '▆' | '▇' | '━') {
            continue;
        }
        for (predicted_b, fg, bg) in [(gm, b, a), (!gm, a, b)] {
            if must_show && predicted_b == u64::MAX {
                continue;
            }
            let wrong = (predicted_b ^ mask).count_ones();
            if wrong < best.0 {
                best = (wrong, g, fg, bg);
            }
        }
    }
    (best.1, best.2, best.3)
}

/// Colour of a sprite kind in the colour looks.
fn kind_color(kind: u8, r: &Runner, p: &Palette) -> Rgb {
    match kind {
        CLOUD => p.cloud,
        STAR => p.star,
        MOON => p.moon,
        GROUND => p.ground,
        CACTUS => p.cactus,
        BIRD => p.bird,
        DINO if r.crashed => p.crashed,
        DINO => p.dino,
        _ => p.text,
    }
}

fn classic(grey: u8, inverted: bool) -> Color {
    let g = if inverted { 255 - grey } else { grey };
    Color::Rgb(g, g, g)
}

/// The half-block scenery of the colour theme.
struct Scenery {
    p: Palette,
    scale: i32,
    scrolled: i64,
    /// sky in three bands rather than a gradient (256 colours)
    banded: bool,
}

impl Scenery {
    /// Colour of the scenery at canvas pixel (x, y).
    fn at(&self, x: i32, y: i32) -> Rgb {
        let p = &self.p;
        if y >= SAND_Y {
            // pebbles move in whole cells, locked to the ground's scroll
            let cell = 2 * self.scale as i64;
            let col = (x as i64 + self.scrolled).div_euclid(cell);
            let row = (y / (2 * self.scale)) as i64;
            return if hash(col, row) % 7 == 0 { p.pebble } else { p.sand };
        }
        // two dune ridges, scrolling at a third and a sixth of the ground
        let near = x as f64 + self.scrolled as f64 / 3.0;
        let far = x as f64 + self.scrolled as f64 / 6.0;
        let near_top = 121.0 + 5.0 * (near / 37.0 + 0.4).sin() + 3.0 * (near / 17.0).sin();
        let far_top = 110.0 + 7.0 * (far / 53.0).sin() + 4.0 * (far / 23.0 + 1.7).sin();
        if y as f64 >= near_top {
            p.dunes
        } else if y as f64 >= far_top {
            p.dunes_far
        } else {
            // one shade per cell row, not per half-row: then both halves of
            // a sky cell match and a Braille sprite over the sky (whose cell
            // has one background) leaves no box around itself
            let row_h = 4 * self.scale;
            let t = ((y / row_h) * row_h + row_h / 2) as f64 / SAND_Y as f64;
            if self.banded {
                [p.sky_top, p.sky_mid, p.sky_horizon][((t * 3.0) as usize).min(2)]
            } else if t < 0.5 {
                p.sky_top.lerp(p.sky_mid, t * 2.0)
            } else {
                p.sky_mid.lerp(p.sky_horizon, t * 2.0 - 1.0)
            }
        }
    }

    /// Colour of a cell's top and bottom half.
    fn halves(&self, cx: u16, cy: u16) -> (Rgb, Rgb) {
        let s = self.scale;
        let x = cx as i32 * 2 * s + s;
        let y = cy as i32 * 4 * s;
        (self.at(x, y + s), self.at(x, y + 3 * s))
    }
}

struct Dots {
    scale: i32,
    w: i32,
    h: i32,
    clip_w: i32,
    layer: Vec<u8>,
}

impl Dots {
    fn set(&mut self, x: i32, y: i32, layer: u8) {
        if x >= 0 && y >= 0 && x < self.clip_w.min(self.w) && y < self.h {
            let i = (y * self.w + x) as usize;
            self.layer[i] = self.layer[i].max(layer);
        }
    }

    /// The restart arrow (↻), centred on canvas x and in the free band
    /// between canvas y `top` and `bottom`. Hand-drawn pixel art rather than
    /// Chrome's button, whose arrow is a 3 px cut-out that turns to mush
    /// past scale 2 (and a computed ring's arrowhead came out as a lump).
    /// The large one is about Chrome's 26 px at scale 2; coarser scales take
    /// the largest that fits, since small panes need it more readable, not
    /// more faithful.
    fn restart_arrow(&mut self, x: i32, top: i32, bottom: i32, layer: u8) {
        let s = self.scale as f64;
        let room = (bottom - top) as f64 / s;
        let (cx, cy) = (x as f64 / s, (top + bottom) as f64 / 2.0 / s);
        let arts: &[&[&str]] = if s <= 2.0 { &[ARROW_LARGE, ARROW_MEDIUM, ARROW_SMALL] } else { &[ARROW_MEDIUM, ARROW_SMALL] };
        // at scale 1 the large art would be half Chrome's size: double it
        let k = if s < 2.0 { 2 } else { 1 };
        for &art in arts {
            let (w, h) = (art[0].len() as i32 * k, art.len() as i32 * k);
            if room >= h as f64 + 1.0 {
                let (ox, oy) = ((cx - w as f64 / 2.0).round() as i32, (cy - h as f64 / 2.0).round() as i32);
                for (j, row) in art.iter().enumerate() {
                    for (i, c) in row.bytes().enumerate() {
                        if c == b'#' {
                            for (dx, dy) in (0..k).flat_map(|dx| (0..k).map(move |dy| (dx, dy))) {
                                self.set(ox + i as i32 * k + dx, oy + j as i32 * k + dy, layer);
                            }
                        }
                    }
                }
                return;
            }
        }
        // no room at all; the help line still says how to restart
    }

    /// Blit a sprite whose top-left is at canvas pixel (x, y).
    fn sprite(&mut self, s: &Sprite, x: i32, y: i32, layer: u8) {
        let ox = (x - s.phase.0).div_euclid(self.scale);
        let oy = (y - s.phase.1).div_euclid(self.scale);
        for j in 0..s.h {
            for i in 0..s.w {
                if s.dot(i, j) {
                    self.set(ox + i, oy + j, layer);
                }
            }
        }
    }
}

pub struct Renderer {
    /// sprites at every scale from 1 (the sheet's own pixels, for the blocks
    /// look) to twice MAX_SCALE
    sprites: Vec<Sprites>,
    pub look: Look,
    pub depth: Depth,
    /// --scale N
    pub forced: Option<i32>,
    /// --wide
    pub wide: bool,
    /// the blocks look draws the T-Rex in octants (2x4 per cell, the Braille
    /// dots as solid blocks); otherwise in quadrants
    pub octants: bool,
}

impl Renderer {
    pub fn new(look: Look, depth: Depth, forced: Option<i32>, wide: bool, octants: bool) -> Renderer {
        // scale 1 (the sheet's own pixels) feeds the blocks look's glyph fitting
        Renderer { sprites: sprites::build(1, 2 * MAX_SCALE), look, depth, forced, wide, octants }
    }

    pub fn layout(&self, area: Rect) -> Option<Layout> {
        Layout::choose(area.width, area.height, self.forced, self.wide)
    }

    fn sprites_at(&self, scale: i32) -> &Sprites {
        self.sprites.iter().find(|s| s.scale == scale).expect("sprites built for every scale")
    }

    /// `help` lists the help line from longest to shortest; the first that
    /// fits is shown, if there is room for one. `note` is a short message
    /// for the field's top-left corner.
    pub fn draw(&self, r: &Runner, area: Rect, buf: &mut Buffer, help: &[&str], note: Option<&str>) {
        let Some(l) = self.layout(area) else {
            let (w, h) = min_size();
            let msg = format!("tty-rex needs {w}x{h}, this is {}x{}", area.width, area.height);
            let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
            buf.set_stringn(x, area.y + area.height / 2, &msg, area.width as usize, Style::default());
            return;
        };
        let total = l.rows + if l.help { 2 } else { 0 };
        let field = Rect::new(area.x + (area.width - l.cols) / 2, area.y + (area.height - total) / 2, l.cols, l.rows);
        let pal = Palette::for_depth(self.depth, r.horizon.night.opacity);
        let scenery = Scenery { p: pal, scale: l.scale, scrolled: r.horizon.scrolled, banded: self.depth == Depth::Indexed };

        if self.look == Look::Blocks {
            self.draw_blocks(r, &l, field, buf, &pal, &scenery);
        } else {
            self.draw_braille(r, &l, field, buf, &pal, &scenery);
        }

        let style = self.text_style(r, &pal);
        self.text(r, &l, field, buf, style, &scenery);
        if let Some(note) = note {
            let n = buf.set_stringn(field.x + 1, field.y, note, field.width.saturating_sub(2) as usize, style).0 - field.x - 1;
            self.back_text(buf, field, field.x + 1, field.y, n, &scenery);
        }
        if l.help {
            if let Some(help) = help.iter().find(|h| h.chars().count() <= area.width as usize) {
                let x = area.x + (area.width - help.chars().count() as u16) / 2;
                buf.set_string(x, field.bottom() + 1, help, Style::default().add_modifier(Modifier::DIM));
            }
        }
    }

    fn draw_braille(&self, r: &Runner, l: &Layout, field: Rect, buf: &mut Buffer, pal: &Palette, scenery: &Scenery) {
        let (w, h) = (l.cols as i32 * 2, l.rows as i32 * 4);
        let clip_w = (r.visible_width() + l.scale - 1) / l.scale;
        let base = self.base(r.inverted);
        let depth = self.depth;
        for cy in 0..l.rows {
            for cx in 0..l.cols {
                let cell = &mut buf[(field.x + cx, field.y + cy)];
                cell.reset();
                // colour backgrounds only where the canvas is revealed (the
                // intro widens it)
                let revealed = (cx as i32) * 2 < clip_w;
                match self.look {
                    Look::Mixed if revealed => {
                        let (top, bottom) = scenery.halves(cx, cy);
                        cell.set_char('▀');
                        cell.set_style(Style::default().fg(depth.color(top)).bg(depth.color(bottom)));
                    }
                    Look::ColorBraille if revealed => {
                        cell.set_style(Style::default().bg(depth.color(pal.page)));
                    }
                    Look::Mixed | Look::ColorBraille => {}
                    _ => {
                        cell.set_style(base);
                    }
                }
            }
        }

        let mut d = Dots { scale: l.scale, w, h, clip_w, layer: vec![0; (w * h) as usize] };
        self.paint(r, &mut d, self.sprites_at(l.scale), l.text_bottom(), false, true);

        for cy in 0..l.rows as i32 {
            for cx in 0..l.cols as i32 {
                let mut bits = 0u8;
                let mut top = 0u8;
                for (row, masks) in [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]].iter().enumerate() {
                    for (col, mask) in masks.iter().enumerate() {
                        let k = d.layer[((cy * 4 + row as i32) * d.w + cx * 2 + col as i32) as usize];
                        if k > 0 {
                            bits |= mask;
                            top = top.max(k);
                        }
                    }
                }
                if bits == 0 {
                    continue;
                }
                let style = self.sprite_style(top, bits, r, pal, scenery, cx as u16, cy as u16);
                let cell = &mut buf[(field.x + cx as u16, field.y + cy as u16)];
                cell.set_char(char::from_u32(0x2800 + bits as u32).unwrap_or(' '));
                cell.set_style(style);
            }
        }
    }

    /// Everything in block glyphs, fitted per cell. The sprites are painted
    /// at the sheet's own resolution, each cell (2 * scale canvas px wide,
    /// twice that tall) is sampled 8x8, and the cell becomes whichever block
    /// glyph, in two colours, matches the samples best: eighth blocks for
    /// the top of a head or a foot, quadrants for corners, a middle line for
    /// a gap like the T-Rex's open mouth, the half blocks for the scenery.
    /// The glyphs are block and box-drawing characters, which terminals
    /// draw themselves rather than from the font, so they stay crisp. The
    /// eye, a 2 px hole no glyph can hold, gets a placed ▪.
    fn draw_blocks(&self, r: &Runner, l: &Layout, field: Rect, buf: &mut Buffer, pal: &Palette, scenery: &Scenery) {
        let cell_w = 2 * l.scale;
        let cell_h = 2 * cell_w;
        let (w, h) = (l.cols as i32 * cell_w, l.rows as i32 * cell_h);
        let clip_px = r.visible_width();
        let mut px = Dots { scale: 1, w, h, clip_w: clip_px, layer: vec![0; (w * h) as usize] };
        // the T-Rex isn't fitted: he's drawn after, from the Braille dots
        self.paint(r, &mut px, self.sprites_at(1), l.text_bottom(), true, false);
        // the restart arrow is pixel art meant for cell-sized pixels: draw it
        // at that size and blow it up into the canvas
        if r.crashed {
            let (aw, ah) = (l.cols as i32, l.rows as i32 * 2);
            let mut art = Dots { scale: cell_w, w: aw, h: ah, clip_w: aw, layer: vec![0; (aw * ah) as usize] };
            art.restart_arrow(r.width / 2, l.text_bottom(), LINE_Y - 2, ICON);
            for (i, &k) in art.layer.iter().enumerate().filter(|(_, &k)| k > 0) {
                let (ax, ay) = (i as i32 % aw, i as i32 / aw);
                for y in ay * cell_w..(ay + 1) * cell_w {
                    for x in ax * cell_w..(ax + 1) * cell_w {
                        px.set(x, y, k);
                    }
                }
            }
        }
        // the 1 px ground line would cover too few samples to survive the
        // fit: thicken it downwards to a quarter of a cell
        // (within its own cell row: spilling into the next one left a row of
        // dashes under the line)
        let mut thick = px.layer.clone();
        for x in 0..w {
            for y in 0..h {
                if px.layer[(y * w + x) as usize] == GROUND {
                    let cell_top = y / cell_h * cell_h;
                    let band = cell_h / 4;
                    // downwards if it fits in the cell, else up to the cell's bottom
                    let start = y.min(cell_top + cell_h - band).max(cell_top);
                    for yy in start..start + band {
                        if thick[(yy * w + x) as usize] == 0 {
                            thick[(yy * w + x) as usize] = GROUND;
                        }
                    }
                }
            }
        }
        px.layer = thick;
        for cy in 0..l.rows {
            for cx in 0..l.cols {
                let cell = &mut buf[(field.x + cx, field.y + cy)];
                cell.reset();
                if cx as i32 * cell_w >= clip_px {
                    continue;
                }
                let (sky_top, sky_bottom) = scenery.halves(cx, cy);
                let (x0, y0) = (cx as i32 * cell_w, cy as i32 * cell_h);
                // 8x8 samples, row-major from the top left
                let mut colors = [Rgb(0, 0, 0); 64];
                let mut kinds = [0u8; 64];
                for sr in 0..8 {
                    for sc in 0..8 {
                        let i = (sr * 8 + sc) as usize;
                        let (sx, sy) = (x0 + sc * cell_w / 8, y0 + sr * cell_h / 8);
                        kinds[i] = px.layer[(sy * w + sx) as usize];
                        colors[i] = match kinds[i] {
                            0 if sr < 4 => sky_top,
                            0 => sky_bottom,
                            k => kind_color(k, r, pal),
                        };
                    }
                }
                let (glyph, fg, bg) = fit_cell(&colors, &kinds);
                cell.set_char(glyph);
                cell.set_style(Style::default().fg(self.depth.color(fg)).bg(self.depth.color(bg)));
            }
        }
        self.draw_dino_blocks(r, l, field, buf, pal, scenery);
    }

    /// The T-Rex in the blocks look: exactly the Braille look's dots (Chrome's
    /// pixels at this scale, with the same hand touches), drawn as solid
    /// blocks. With octants (2x4 per cell, Unicode 16; VTE draws them itself
    /// since 0.78) that's one block pixel per Braille dot. Without, quadrants
    /// (2x2), a quadrant lit if either of its two dots is. As in the Braille
    /// look, a cell whose top layer is the T-Rex takes all its dots in his
    /// colour (his feet and the ground line share cells).
    fn draw_dino_blocks(&self, r: &Runner, l: &Layout, field: Rect, buf: &mut Buffer, pal: &Palette, scenery: &Scenery) {
        let (w, h) = (l.cols as i32 * 2, l.rows as i32 * 4);
        let clip_w = (r.visible_width() + l.scale - 1) / l.scale;
        let mut d = Dots { scale: l.scale, w, h, clip_w, layer: vec![0; (w * h) as usize] };
        self.paint(r, &mut d, self.sprites_at(l.scale), l.text_bottom(), false, true);
        let dino = self.depth.color(kind_color(DINO, r, pal));
        // without octants: quadrants sampled from Chrome's own pixels, a
        // quadrant lit if over half of it is dino, with the T-Rex snapped
        // to the alignment that best fits his pixel grid (fewest half-lit
        // quadrants); from the Braille dots, two per quadrant, either rule
        // (one dot or both) loses the eye or the mouth
        let quads = (!self.octants).then(|| self.dino_quadrants(r, l));
        for cy in 0..l.rows as i32 {
            for cx in 0..l.cols as i32 {
                let mut bits = 0u8;
                let mut top = 0u8;
                for row in 0..4 {
                    for col in 0..2 {
                        let k = d.layer[((cy * 4 + row) * w + cx * 2 + col) as usize];
                        if k > 0 {
                            bits |= 1 << (row * 2 + col);
                            top = top.max(k);
                        }
                    }
                }
                // the night stars too: a 9 px sparkle chopped into fragments
                // by the fit, exact as octants
                let star = self.octants && top == STAR;
                let glyph = match &quads {
                    None if top == DINO => octants::OCTANT[bits as usize],
                    _ if star => octants::OCTANT[bits as usize],
                    Some(q) if q[(cy * l.cols as i32 + cx) as usize] != 0 => {
                        bits = q[(cy * l.cols as i32 + cx) as usize];
                        octants::OCTANT[bits as usize]
                    }
                    _ => continue,
                };
                // one background for the cell: the scenery of the half that
                // shows more of it
                let (st, sb) = scenery.halves(cx as u16, cy as u16);
                let bg = if (bits & 0x0F).count_ones() <= (bits & 0xF0).count_ones() { st } else { sb };
                let cell = &mut buf[(field.x + cx as u16, field.y + cy as u16)];
                cell.set_char(glyph);
                let fg = if star { self.depth.color(kind_color(STAR, r, pal)) } else { dino };
                cell.set_style(Style::default().fg(fg).bg(self.depth.color(bg)));
            }
        }
    }

    /// The quadrant T-Rex, as an octant-style pattern per cell (each
    /// quadrant setting its two octant bits), row-major over the field.
    fn dino_quadrants(&self, r: &Runner, l: &Layout) -> Vec<u8> {
        let t = &r.trex;
        let s = self.sprites_at(1).get(trex_sprite(t.frame()));
        let (qw, qh) = (l.scale, 2 * l.scale);
        let lit = |ox: i32, oy: i32, i: i32, j: i32| {
            let mut n = 0;
            for y in j * qh - oy..(j + 1) * qh - oy {
                for x in i * qw - ox..(i + 1) * qw - ox {
                    n += (x >= 0 && y >= 0 && x < s.w && y < s.h && s.dot(x, y)) as i32;
                }
            }
            n
        };
        let full = qw * qh;
        let (gw, gh) = ((s.w + 2 * qw) / qw, (s.h + 2 * qh) / qh);
        let partial = |ox: i32, oy: i32| (0..gh).flat_map(|j| (0..gw).map(move |i| (i, j))).filter(|&(i, j)| (1..full).contains(&lit(ox, oy, i, j))).count();
        let (ox, oy) = (0..qh).flat_map(|oy| (0..qw).map(move |ox| (ox, oy))).min_by_key(|&(ox, oy)| partial(ox, oy)).unwrap_or((0, 0));
        // the sprite's quadrant (i, j) lands at field quadrant (q0x + i, q0y + j)
        let q0x = (t.x - ox).div_euclid(qw);
        let q0y = (t.y - oy).div_euclid(qh);
        let mut out = vec![0u8; l.cols as usize * l.rows as usize];
        for j in 0..gh {
            for i in 0..gw {
                if lit(ox, oy, i, j) * 2 <= full {
                    continue;
                }
                let (qx, qy) = (q0x + i, q0y + j);
                if qx < 0 || qy < 0 || qx >= 2 * l.cols as i32 || qy >= 2 * l.rows as i32 {
                    continue;
                }
                let (cx, cy) = (qx / 2, qy / 2);
                // quadrant to its two octant bits: left/right column, top/bottom pair
                let (col, half) = (qx % 2, qy % 2);
                let bits = (1u8 << (half * 4 + col)) | (1u8 << (half * 4 + 2 + col));
                out[(cy * l.cols as i32 + cx) as usize] |= bits;
            }
        }
        out
    }

    /// Style of an empty cell in the Braille and classic looks.
    fn base(&self, inverted: bool) -> Style {
        match self.look {
            Look::Classic => Style::default().fg(classic(0x53, inverted)).bg(classic(0xff, inverted)),
            _ if inverted => Style::default().add_modifier(Modifier::REVERSED),
            _ => Style::default(),
        }
    }

    /// `bits`: the cell's lit Braille dots.
    fn sprite_style(&self, kind: u8, bits: u8, r: &Runner, p: &Palette, sc: &Scenery, cx: u16, cy: u16) -> Style {
        let fg = self.depth.color(kind_color(kind, r, p));
        match self.look {
            Look::Mixed | Look::Blocks => {
                // a Braille cell has one background for its two scenery
                // halves (they differ where dunes meet sky): take the half
                // that shows more of it, i.e. has fewer lit dots. A blend
                // matched neither and showed as a faint box.
                let (top, bottom) = sc.halves(cx, cy);
                let unlit = |mask: u8| 4 - (bits & mask).count_ones();
                let bg = if unlit(0x1B) >= unlit(0xE4) { top } else { bottom };
                Style::default().fg(fg).bg(self.depth.color(bg))
            }
            Look::ColorBraille => {
                let fg = if kind == CLOUD { self.depth.color(p.page_cloud) } else { fg };
                Style::default().fg(fg).bg(self.depth.color(p.page))
            }
            // faint clouds by day; not at night, where everything is
            // reversed and dim would grey the cell's background into a box
            Look::Braille if kind == CLOUD && !r.inverted => self.base(r.inverted).add_modifier(Modifier::DIM),
            Look::Braille => self.base(r.inverted),
            Look::Classic => {
                // sprite sheet greys: #535353 ink, #dadada clouds, page #fff;
                // night is the page's CSS invert(1)
                let fg = match kind {
                    CLOUD => classic(0xda, r.inverted),
                    STAR | MOON => Color::Rgb(0xf7, 0xf7, 0xf7),
                    _ => classic(0x53, r.inverted),
                };
                Style::default().fg(fg).bg(classic(0xff, r.inverted))
            }
        }
    }

    fn text_style(&self, r: &Runner, p: &Palette) -> Style {
        match self.look {
            Look::Mixed | Look::Blocks | Look::ColorBraille => Style::default().fg(self.depth.color(p.score)).add_modifier(Modifier::BOLD),
            _ => self.base(r.inverted).add_modifier(Modifier::BOLD),
        }
    }

    /// `text_bottom`: canvas y just below the GAME OVER line.
    /// `blocks`: painting the blocks look's full-resolution canvas, which
    /// takes solid clouds instead of Chrome's outlines (a 1 px outline can't
    /// survive a cell-wide fit) and draws its own cell-sized restart arrow.
    /// `draw_dino`: false when the caller draws the T-Rex itself.
    fn paint(&self, r: &Runner, d: &mut Dots, s: &Sprites, text_bottom: i32, blocks: bool, draw_dino: bool) {
        let h = &r.horizon;

        let night = &h.night;
        if night.opacity > 0.0 {
            // (the blocks look draws stars as octants when it can, after the fit)
            if night.stars_visible() && !(blocks && self.octants) {
                for st in &night.stars {
                    d.sprite(s.get(if st.variant == 0 { "star1" } else { "star2" }), st.x.round() as i32, st.y, STAR);
                }
            }
            d.sprite(s.get(&format!("moon{}", night.phase)), night.moon_x.round() as i32, MOON_Y, MOON);
        }
        for c in &h.clouds {
            d.sprite(s.get(if blocks { "cloud_filled" } else { "cloud" }), c.x, c.y, CLOUD);
        }
        // Chrome's two leapfrogging 600 px strips cover a 600 px canvas; a
        // --wide field repeats the pair across its width
        // (the pair starts up to a strip's width left of the edge, so one more)
        for k in 0..=(r.width / (2 * LINE_WIDTH) + 1) {
            for &(x, bumpy) in &h.lines {
                let x = x + k * 2 * LINE_WIDTH;
                if x < r.width && x + LINE_WIDTH > 0 {
                    d.sprite(s.get(if bumpy { "horizon_bumpy" } else { "horizon_flat" }), x, LINE_Y, GROUND);
                }
            }
        }
        for o in &h.obstacles {
            let name = match o.kind {
                Kind::CactusSmall => format!("cactus_small{}", o.size),
                Kind::CactusLarge => format!("cactus_large{}", o.size),
                Kind::Pterodactyl => format!("ptero{}", o.frame + 1),
            };
            d.sprite(s.get(&name), o.x, o.y, if o.kind == Kind::Pterodactyl { BIRD } else { CACTUS });
        }
        let t = &r.trex;
        if draw_dino {
            d.sprite(s.get(trex_sprite(t.frame())), t.x, t.y, DINO);
        }

        if r.crashed && !blocks {
            // between the GAME OVER line and the ground, centred, about where
            // game_over_panel.ts puts its button
            d.restart_arrow(r.width / 2, text_bottom, LINE_Y - 2, ICON);
        }

    }

    /// Over half-block scenery, text cells keep whatever was behind them as
    /// their background: a cloud's white, a cactus's green. Back them with
    /// the plain scenery colour instead, so text knocks out the sprites.
    fn back_text(&self, buf: &mut Buffer, field: Rect, x: u16, y: u16, len: u16, sc: &Scenery) {
        if !matches!(self.look, Look::Mixed | Look::Blocks) {
            return;
        }
        for cx in x..(x + len).min(field.right()) {
            let (top, bottom) = sc.halves(cx - field.x, y - field.y);
            let bg = if sc.banded { top } else { top.mix(bottom) };
            buf[(cx, y)].set_bg(self.depth.color(bg));
        }
    }

    fn text(&self, r: &Runner, l: &Layout, field: Rect, buf: &mut Buffer, style: Style, sc: &Scenery) {
        if r.activated || r.crashed {
            let digits = 5.max(r.meter.shown.to_string().len());
            let now = if r.meter.paint { format!("{:0digits$}", r.meter.shown) } else { " ".repeat(digits) };
            let line = if r.high > 0 { format!("HI {:0digits$} {now}", r.high) } else { now };
            let x = field.right().saturating_sub(line.len() as u16 + 1);
            buf.set_string(x, field.y, &line, style);
            self.back_text(buf, field, x, field.y, line.len() as u16, sc);
        }
        // spaced out like the original's pixel lettering, where it fits
        let center = |wide: &str, narrow: &str, buf: &mut Buffer| {
            let s = if wide.len() + 2 <= field.width as usize { wide } else { narrow };
            let x = field.x + field.width.saturating_sub(s.len() as u16) / 2;
            let y = field.y + l.text_row();
            buf.set_string(x, y, s, style);
            self.back_text(buf, field, x, y, s.len() as u16, sc);
        };
        if r.crashed {
            center("G A M E   O V E R", "GAME OVER", buf);
        } else if r.paused {
            center("P A U S E D", "PAUSED", buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn screen(r: &Runner, w: u16, h: u16) -> String {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        Renderer::new(Look::Braille, Depth::True, None, false, true).draw(r, area, &mut buf, &[], None);
        (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n")
    }

    /// With TTY_REX_SNAP=<dir>, write a colour-theme frame there as ANSI
    /// text (feed it to any ANSI-to-image tool to look at it).
    fn snap(r: &Runner, name: &str) {
        for look in [Look::ColorBraille, Look::Blocks, Look::Mixed] {
            snap_look(r, name, look);
        }
    }

    fn snap_look(r: &Runner, name: &str, look: Look) {
        let Ok(dir) = std::env::var("TTY_REX_SNAP") else { return };
        let (w, h) = (152u16, 22u16);
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        Renderer::new(look, Depth::True, None, false, true).draw(r, area, &mut buf, &["help"], None);
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => Some(format!("{r};{g};{b}")),
            _ => None,
        };
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                let c = &buf[(x, y)];
                out.push_str("\x1b[0m");
                if let Some(f) = rgb(c.fg) {
                    out.push_str(&format!("\x1b[38;2;{f}m"));
                }
                if let Some(b) = rgb(c.bg) {
                    out.push_str(&format!("\x1b[48;2;{b}m"));
                }
                if c.modifier.contains(Modifier::BOLD) {
                    out.push_str("\x1b[1m");
                }
                out.push_str(c.symbol());
            }
            out.push_str("\x1b[0m\n");
        }
        let _ = std::fs::write(format!("{dir}/{name}-{}.ans", look.name()), out);
    }

    #[test]
    fn text_knocks_out_sprites_behind_it() {
        let mut r = Runner::new(600, 0, 7);
        r.jump_down();
        r.jump_up();
        while !r.crashed {
            r.tick();
        }
        // park a cloud right behind GAME OVER
        r.horizon.clouds[0].x = 260;
        r.horizon.clouds[0].y = 36;
        let white = Color::Rgb(255, 255, 255);
        for look in [Look::Mixed, Look::Blocks] {
            let area = Rect::new(0, 0, 150, 21);
            let mut buf = Buffer::empty(area);
            Renderer::new(look, Depth::True, None, false, true).draw(&r, area, &mut buf, &[], None);
            let row = (0..21).find(|&y| (0..150).map(|x| buf[(x, y)].symbol()).collect::<String>().contains("G A M E")).expect("GAME OVER drawn");
            let start = (0..150).find(|&x| buf[(x, row)].symbol() == "G").unwrap();
            for x in start..start + 17 {
                assert_ne!(buf[(x, row)].bg, white, "{look:?}: cloud white behind GAME OVER at column {x}");
            }
        }
    }

    #[test]
    fn wide_ground_reaches_the_right_edge() {
        // 500 columns at scale 2 is a 2000 px road. The tiling once came up a
        // strip short on roads more than 600 px past a multiple of 1200
        // (yours was ~1976 px), for part of every scroll cycle
        let mut r = Runner::new(2000, 0, 4);
        r.jump_down();
        r.jump_up();
        let renderer = Renderer::new(Look::Braille, Depth::True, None, true, true);
        let area = Rect::new(0, 0, 500, 21);
        // until the intro has opened the road to full width
        while !r.activated || r.playing_intro {
            r.tick();
        }
        for _ in 0..200 {
            r.tick();
            if r.crashed {
                break;
            }
            let mut buf = Buffer::empty(area);
            renderer.draw(&r, area, &mut buf, &[], None);
            // the ground line row: the one with the most Braille cells
            let braille = |y: u16| (0..500).filter(|&x| buf[(x, y)].symbol().chars().all(|c| ('\u{2801}'..='\u{28ff}').contains(&c))).count();
            let row = (0..21).max_by_key(|&y| braille(y)).unwrap();
            // bumps dip the line into the next row, so look a row either
            // side; the bug left up to 150 bare columns at the right edge
            let is_braille = |x: u16, y: u16| buf[(x, y)].symbol().chars().all(|c| ('\u{2801}'..='\u{28ff}').contains(&c));
            let bare: Vec<u16> = (300..500).filter(|&x| !(row - 1..=row + 1).any(|y| is_braille(x, y))).collect();
            assert!(bare.is_empty(), "no ground at columns {bare:?} (lines at {:?})", r.horizon.lines);
        }
    }

    /// The colour a blocks-look cell shows at a sample position (row, col
    /// of the 8x8 grid), reading the glyph back through its mask.
    fn shown(buf: &Buffer, x: u16, y: u16, sr: u32, sc: u32) -> Color {
        let cell = &buf[(x, y)];
        let ch = cell.symbol().chars().next().unwrap_or(' ');
        let bit = 1u64 << (sr * 8 + sc);
        let fg = match ch {
            ' ' => false,
            '▪' => (3..5).contains(&sr) && (2..6).contains(&sc),
            _ => glyph_masks().iter().find(|(g, _)| *g == ch).is_some_and(|(_, m)| m & bit != 0),
        };
        if fg {
            cell.fg
        } else {
            cell.bg
        }
    }

    #[test]
    fn blocks_dino_is_the_braille_dino_in_octants() {
        // the octant T-Rex lights exactly the Braille T-Rex's dots, at every
        // size, frame and x offset
        let octant_bits = |c: char| octants::OCTANT.iter().position(|&o| o == c).map(|i| i as u8);
        let braille_bits = |c: char| -> Option<u8> {
            let b = (c as u32).checked_sub(0x2800).filter(|&b| b < 256)? as u8;
            // Braille dot n (1..8) to octant bit: rows of 2, top to bottom
            let map = [(0x01, 0), (0x08, 1), (0x02, 2), (0x10, 3), (0x04, 4), (0x20, 5), (0x40, 6), (0x80, 7)];
            Some(map.iter().fold(0u8, |m, &(dot, bit)| if b & dot != 0 { m | 1 << bit } else { m }))
        };
        for (cols, rows) in [(100u16, 13u16), (150, 21), (220, 45), (80, 10)] {
            let area = Rect::new(0, 0, cols, rows);
            let l = Layout::choose(cols, rows, None, false).unwrap();
            let mut r = Runner::new(l.width, 0, 3);
            r.jump_down();
            r.jump_up();
            while !r.activated || r.playing_intro {
                r.tick();
            }
            r.horizon.obstacles.clear();
            let blocks = Renderer::new(Look::Blocks, Depth::True, None, false, true);
            let braille = Renderer::new(Look::Mixed, Depth::True, None, false, true);
            let dino = Color::Rgb(crate::palette::DAY.dino.0, crate::palette::DAY.dino.1, crate::palette::DAY.dino.2);
            let mut cells = 0;
            r.jump_down();
            for frame in 0..80 {
                r.tick();
                if frame == 55 {
                    r.duck_down();
                }
                for x in 20..20 + 2 * l.scale {
                    r.trex.x = x;
                    let (mut a, mut b) = (Buffer::empty(area), Buffer::empty(area));
                    blocks.draw(&r, area, &mut a, &[], None);
                    braille.draw(&r, area, &mut b, &[], None);
                    for y in 0..rows {
                        for x in 0..cols {
                            let bc = &b[(x, y)];
                            if bc.fg != dino {
                                continue;
                            }
                            let want = braille_bits(bc.symbol().chars().next().unwrap()).unwrap();
                            let got = octant_bits(a[(x, y)].symbol().chars().next().unwrap());
                            assert_eq!(got, Some(want), "{cols}x{rows} {:?} cell ({x},{y})", r.trex.frame());
                            cells += 1;
                        }
                    }
                }
            }
            assert!(cells > 1000, "{cols}x{rows}: {cells} cells checked");
        }
    }

    #[test]
    fn layout_picks_finest_scale_that_fits() {
        let pick = |w, h| Layout::choose(w, h, None, false).map(|l| (l.scale, l.width, l.rows));
        assert_eq!(pick(160, 30), Some((2, 600, 19)));
        // a big window gets scale 1: Chrome's pixels, 38 rows
        assert_eq!(pick(220, 45), Some((1, 440, 38)));
        assert_eq!(pick(179, 45), Some((2, 600, 19)));
        // 100 columns at scale 2 is 400 px, enough road to keep the detail
        assert_eq!(pick(100, 24), Some((2, 400, 19)));
        // a short split pane drops to a scale that fits its height
        assert_eq!(pick(80, 12), Some((4, 600, 10)));
        assert_eq!(pick(200, 14), Some((3, 600, 13)));
        // narrow and short: coarser still, for the road ahead
        assert_eq!(pick(40, 10), Some((5, 400, 8)));
        assert_eq!(pick(20, 7), Some((6, 240, 7)));
        assert_eq!(pick(19, 7), None);
        assert_eq!(min_size(), (20, 7));
        // the help line only shows with two spare rows
        assert!(!Layout::choose(80, 11, None, false).unwrap().help);
        assert!(Layout::choose(80, 12, None, false).unwrap().help);
        // --wide takes the whole width
        assert_eq!(Layout::choose(400, 10, None, true).map(|l| (l.scale, l.width, l.cols)), Some((4, 3200, 400)));
    }

    #[test]
    fn every_scale_draws_the_dino_and_ground() {
        let mut r = Runner::new(600, 0, 2);
        r.jump_down();
        r.jump_up();
        for _ in 0..200 {
            r.tick();
        }
        for (w, h) in [(150, 21), (100, 13), (75, 10), (60, 8), (50, 7)] {
            let s = screen(&r, w, h);
            println!("{w}x{h}:\n{s}");
            assert!(s.chars().filter(|c| ('\u{2801}'..='\u{28ff}').contains(c)).count() > 20, "{w}x{h} drew nothing");
        }
    }

    /// A simple autopilot: jump cacti and low birds, duck the middle ones.
    /// Run with --nocapture to see frames from deep into a run.
    #[test]
    fn autopilot_reaches_birds_and_night() {
        use crate::sim::horizon::Kind;
        let mut r = Runner::new(600, 0, 5);
        r.jump_down();
        r.jump_up();
        let (mut saw_bird, mut saw_night, mut printed) = (false, false, 0);
        let mut ducking = false;
        let mut release_in = 0;
        let mut day_snapped = false;
        for _ in 0..60 * 240 {
            if release_in > 0 {
                release_in -= 1;
                if release_in == 0 {
                    r.jump_up();
                }
            }
            let next = r.horizon.obstacles.iter().find(|o| o.x + o.width > r.trex.x).map(|o| (o.kind, o.x, o.y));
            let mut want_duck = false;
            if let Some((kind, x, y)) = next {
                let dist = x - (r.trex.x + 44);
                let lead = (r.speed * 7.0) as i32;
                let mut jump = |r: &mut Runner| {
                    if !r.trex.jumping {
                        r.jump_down();
                        release_in = 7;
                    }
                };
                if kind == Kind::Pterodactyl {
                    saw_bird = true;
                    if y == 75 && dist < lead * 2 && dist > -50 {
                        want_duck = true;
                    } else if y == 100 && dist < lead && dist > 0 {
                        jump(&mut r);
                    }
                } else if dist < lead && dist > 0 {
                    jump(&mut r);
                }
            }
            if want_duck != ducking {
                if want_duck { r.duck_down() } else { r.duck_up() }
                ducking = want_duck;
            }
            r.tick();
            if saw_bird && r.distance < 27_000.0 && r.activated {
                // skip ahead to just short of 700, where night falls
                r.distance = 27_900.0;
            }
            if !day_snapped && r.score() >= 150 && r.horizon.obstacles.iter().any(|o| o.x < 300) {
                day_snapped = true;
                snap(&r, "day");
            }
            if r.inverted && !saw_night {
                saw_night = true;
                for _ in 0..90 { r.tick(); }
                println!("night at {}:\n{}", r.score(), screen(&r, 150, 21));
                snap(&r, "night");
            }
            if saw_bird && printed == 0 && r.horizon.obstacles.iter().any(|o| o.kind == Kind::Pterodactyl && o.x < 400) {
                printed += 1;
                println!("bird at {}:\n{}", r.score(), screen(&r, 150, 21));
                snap(&r, "bird");
            }
            if r.crashed {
                println!("crashed at {}:\n{}", r.score(), screen(&r, 150, 21));
                snap(&r, "crash");
                break;
            }
        }
        println!("score {} speed {:.2} bird {saw_bird} night {saw_night}", r.score(), r.speed);
        assert!(saw_bird, "birds appear once speed passes 8.5");
        assert!(saw_night, "night falls at 700");
    }
}
#[cfg(test)]
mod contact {
    use super::*;
    /// TTY_REX_SNAP=<dir> cargo test --release dino_gallery -- --ignored:
    /// the blocks-look T-Rex running (both frames), jumping, ducking and
    /// crashed, at the 13 and 21 row sizes, through the real draw path, as
    /// one ANSI file to look at.
    #[test]
    #[ignore]
    fn dino_gallery() {
        let Ok(dir) = std::env::var("TTY_REX_SNAP") else { return };
        let renderer = Renderer::new(Look::Blocks, Depth::True, None, false, true);
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => format!("{r};{g};{b}"),
            _ => "0;0;0".into(),
        };
        let mut out = String::new();
        for (cols, rows) in [(100u16, 13u16), (150, 21)] {
            let l = Layout::choose(cols, rows, None, false).unwrap();
            let area = Rect::new(0, 0, cols, rows);
            let mut tiles: Vec<Vec<String>> = Vec::new();
            let mut grab = |r: &Runner| {
                let mut buf = Buffer::empty(area);
                renderer.draw(r, area, &mut buf, &[], None);
                let fx = (cols - l.cols) / 2;
                let (x0, y0) = (fx + (r.trex.x / (2 * l.scale)) as u16, ((r.trex.y / (4 * l.scale)) as u16).saturating_sub(1));
                let tile: Vec<String> = (y0..(y0 + 9).min(rows))
                    .map(|y| (x0.saturating_sub(1)..x0 + 17).map(|x| { let c = &buf[(x, y)]; format!("\x1b[38;2;{}m\x1b[48;2;{}m{}", rgb(c.fg), rgb(c.bg), c.symbol()) }).collect::<String>() + "\x1b[48;2;255;0;255m ")
                    .collect();
                tiles.push(tile);
            };
            let mut r = Runner::new(l.width, 0, 3);
            r.jump_down();
            r.jump_up();
            while !r.activated || r.playing_intro {
                r.tick();
            }
            r.horizon.obstacles.clear();
            for _ in 0..3 { r.tick(); }
            grab(&r); // running
            for _ in 0..5 { r.tick(); }
            grab(&r); // the other run frame
            r.jump_down();
            for _ in 0..8 { r.tick(); }
            grab(&r); // jumping
            while r.trex.jumping { r.tick(); }
            r.jump_up();
            r.duck_down();
            r.tick();
            grab(&r); // ducking
            r.duck_up();
            while !r.crashed { r.tick(); }
            grab(&r); // crashed
            let h = tiles.iter().map(|t| t.len()).max().unwrap_or(0);
            for row in 0..h {
                for t in &tiles {
                    out.push_str(t.get(row).map_or("", |s| s.as_str()));
                }
                out.push_str("\x1b[0m\n");
            }
            out.push_str("\x1b[0m\n");
        }
        std::fs::write(format!("{dir}/gallery.ans"), out).unwrap();
    }
}
