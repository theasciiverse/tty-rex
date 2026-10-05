//! Everything that scrolls: obstacles, clouds, the ground line and the night
//! sky. After horizon.ts, obstacle.ts, cloud.ts, horizon_line.ts and
//! night_mode.ts.

use super::rng::Rng;
use super::CBox;

const GAP_COEFFICIENT: f64 = 0.6;
const MAX_GAP_COEFFICIENT: f64 = 1.5;
const MAX_OBSTACLE_LENGTH: i32 = 3;
const MAX_OBSTACLE_DUPLICATION: usize = 2;

const BG_CLOUD_SPEED: f64 = 0.2;
const CLOUD_FREQUENCY: f64 = 0.5;
const MAX_CLOUDS: usize = 6;
pub const CLOUD_WIDTH: i32 = 46;
const MIN_CLOUD_GAP: i32 = 100;
const MAX_CLOUD_GAP: i32 = 400;
const MAX_SKY_LEVEL: i32 = 30;
const MIN_SKY_LEVEL: i32 = 71;

/// The ground strip is two 600 px segments leapfrogging each other.
pub const LINE_WIDTH: i32 = 600;
pub const LINE_Y: i32 = 127;
const BUMPY_THRESHOLD: f64 = 0.3;

const MOON_PHASES: usize = 7;
/// moon sprite width; the full moon (phase 3) is twice this
const MOON_WIDTH: i32 = 20;
pub const MOON_Y: i32 = 30;
const MOON_SPEED: f64 = 0.25;
const STAR_SPEED: f64 = 0.3;
const STAR_MAX_Y: i32 = 70;
const NUM_STARS: usize = 2;
const FADE_SPEED: f64 = 0.035;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    CactusSmall,
    CactusLarge,
    Pterodactyl,
}

pub struct ObstacleType {
    pub kind: Kind,
    pub width: i32,
    pub height: i32,
    y: &'static [i32],
    multiple_speed: f64,
    min_gap: f64,
    min_speed: f64,
    speed_offset: f64,
    boxes: &'static [CBox],
    /// (frame count, ms per frame) for animated obstacles
    frames: Option<(usize, f64)>,
}

/// offline_sprite_definitions.ts, minus the alt-game-mode collectable.
pub const TYPES: [ObstacleType; 3] = [
    ObstacleType {
        kind: Kind::CactusSmall,
        width: 17,
        height: 35,
        y: &[105],
        multiple_speed: 4.0,
        min_gap: 120.0,
        min_speed: 0.0,
        speed_offset: 0.0,
        boxes: &[CBox::new(0, 7, 5, 27), CBox::new(4, 0, 6, 34), CBox::new(10, 4, 7, 14)],
        frames: None,
    },
    ObstacleType {
        kind: Kind::CactusLarge,
        width: 25,
        height: 50,
        y: &[90],
        multiple_speed: 7.0,
        min_gap: 120.0,
        min_speed: 0.0,
        speed_offset: 0.0,
        boxes: &[CBox::new(0, 12, 7, 38), CBox::new(8, 0, 7, 49), CBox::new(13, 10, 10, 38)],
        frames: None,
    },
    ObstacleType {
        kind: Kind::Pterodactyl,
        width: 46,
        height: 40,
        y: &[100, 75, 50],
        multiple_speed: 999.0,
        min_gap: 150.0,
        min_speed: 8.5,
        speed_offset: 0.8,
        boxes: &[
            CBox::new(15, 15, 16, 5),
            CBox::new(18, 21, 24, 6),
            CBox::new(2, 14, 4, 3),
            CBox::new(6, 10, 4, 7),
            CBox::new(10, 8, 6, 9),
        ],
        frames: Some((2, 1000.0 / 6.0)),
    },
];

#[derive(Clone)]
pub struct Obstacle {
    pub kind: Kind,
    pub ty: &'static ObstacleType,
    pub size: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub boxes: Vec<CBox>,
    pub frame: usize,
    gap: i32,
    speed_offset: f64,
    timer: f64,
    following_created: bool,
    remove: bool,
}

impl Obstacle {
    fn new(ty: &'static ObstacleType, field_width: i32, speed: f64, rng: &mut Rng) -> Obstacle {
        let mut size = rng.num(1, MAX_OBSTACLE_LENGTH);
        if size > 1 && ty.multiple_speed > speed {
            size = 1;
        }
        let width = ty.width * size;
        let y = ty.y[rng.num(0, ty.y.len() as i32 - 1) as usize];
        let mut boxes = ty.boxes.to_vec();
        if size > 1 {
            // stretch the middle box across the cluster, pin the last to the end
            boxes[1].w = width - boxes[0].w - boxes[2].w;
            boxes[2].x = width - boxes[2].w;
        }
        let speed_offset = if ty.speed_offset != 0.0 {
            if rng.random() > 0.5 {
                ty.speed_offset
            } else {
                -ty.speed_offset
            }
        } else {
            0.0
        };
        let min_gap = (width as f64 * speed + ty.min_gap * GAP_COEFFICIENT).round() as i32;
        let max_gap = (min_gap as f64 * MAX_GAP_COEFFICIENT).round() as i32;
        Obstacle {
            kind: ty.kind,
            ty,
            size,
            x: field_width + ty.width,
            y,
            width,
            boxes,
            frame: 0,
            gap: rng.num(min_gap, max_gap),
            speed_offset,
            timer: 0.0,
            following_created: false,
            remove: false,
        }
    }

    fn update(&mut self, dt: f64, speed: f64) {
        if self.remove {
            return;
        }
        let speed = speed + self.speed_offset;
        // obstacle.ts: xPos -= floor(speed * FPS / 1000 * deltaTime), which
        // at one 60 Hz frame is floor(speed)
        self.x -= (speed * dt / super::MS_PER_FRAME).floor() as i32;
        if let Some((n, rate)) = self.ty.frames {
            self.timer += dt;
            if self.timer >= rate {
                self.frame = if self.frame == n - 1 { 0 } else { self.frame + 1 };
                self.timer = 0.0;
            }
        }
        if !self.visible() {
            self.remove = true;
        }
    }

    fn visible(&self) -> bool {
        self.x + self.width > 0
    }
}

#[derive(Clone)]
pub struct Cloud {
    pub x: i32,
    pub y: i32,
    gap: i32,
}

#[derive(Clone)]
pub struct Star {
    pub x: f64,
    pub y: i32,
    /// which of the two star sprites
    pub variant: usize,
}

#[derive(Clone)]
pub struct NightSky {
    pub moon_x: f64,
    pub phase: usize,
    pub opacity: f64,
    pub stars: Vec<Star>,
    draw_stars: bool,
}

impl NightSky {
    fn new(field_width: i32, rng: &mut Rng) -> NightSky {
        let mut n = NightSky { moon_x: 0.0, phase: 0, opacity: 0.0, stars: Vec::new(), draw_stars: false };
        n.place_stars(field_width, rng);
        n
    }

    pub fn stars_visible(&self) -> bool {
        self.draw_stars
    }

    fn place_stars(&mut self, field_width: i32, rng: &mut Rng) {
        let segment = (field_width as f64 / NUM_STARS as f64).round() as i32;
        self.stars = (0..NUM_STARS)
            .map(|i| Star { x: rng.num(segment * i as i32, segment * (i as i32 + 1)) as f64, y: rng.num(0, STAR_MAX_Y), variant: i })
            .collect();
    }

    fn update(&mut self, activated: bool, field_width: i32, rng: &mut Rng) {
        if activated && self.opacity == 0.0 {
            self.phase = (self.phase + 1) % MOON_PHASES;
        }
        if activated && (self.opacity < 1.0 || self.opacity == 0.0) {
            self.opacity += FADE_SPEED;
        } else if self.opacity > 0.0 {
            self.opacity -= FADE_SPEED;
        }
        let wrap = |pos: f64, speed: f64| if pos < -MOON_WIDTH as f64 { field_width as f64 } else { pos - speed };
        if self.opacity > 0.0 {
            self.moon_x = wrap(self.moon_x, MOON_SPEED);
            if self.draw_stars {
                for s in &mut self.stars {
                    s.x = wrap(s.x, STAR_SPEED);
                }
            }
        } else {
            self.opacity = 0.0;
            self.place_stars(field_width, rng);
        }
        self.draw_stars = true;
    }

    fn reset(&mut self, field_width: i32, rng: &mut Rng) {
        self.phase = 0;
        self.opacity = 0.0;
        self.update(false, field_width, rng);
    }
}

#[derive(Clone)]
pub struct Horizon {
    pub width: i32,
    pub obstacles: Vec<Obstacle>,
    pub clouds: Vec<Cloud>,
    /// x of each ground segment and whether it uses the bumpy variant
    pub lines: [(i32, bool); 2],
    pub night: NightSky,
    /// obstacles created so far
    pub spawned: u32,
    /// total pixels the ground has moved, for scenery that must stay locked
    /// to it (the renderer's sand texture)
    pub scrolled: i64,
    history: Vec<Kind>,
}

impl Horizon {
    pub fn new(width: i32, rng: &mut Rng) -> Horizon {
        let mut h = Horizon {
            width,
            obstacles: Vec::new(),
            clouds: Vec::new(),
            lines: [(0, false), (LINE_WIDTH, true)],
            night: NightSky::new(width, rng),
            spawned: 0,
            scrolled: 0,
            history: Vec::new(),
        };
        h.add_cloud(rng);
        h
    }

    /// `dt` is ms, 0 while the ground stands still (before the first jump
    /// lands, during the intro).
    pub fn update(&mut self, dt: f64, speed: f64, update_obstacles: bool, show_night: bool, rng: &mut Rng) {
        self.update_lines(dt, speed, rng);
        self.night.update(show_night, self.width, rng);
        self.update_clouds(dt, speed, rng);
        if update_obstacles {
            self.update_obstacles(dt, speed, rng);
        }
    }

    fn update_lines(&mut self, dt: f64, speed: f64, rng: &mut Rng) {
        let inc = (speed * dt / super::MS_PER_FRAME).floor() as i32;
        self.scrolled += inc as i64;
        let a = if self.lines[0].0 <= 0 { 0 } else { 1 };
        let b = 1 - a;
        self.lines[a].0 -= inc;
        self.lines[b].0 = self.lines[a].0 + LINE_WIDTH;
        if self.lines[a].0 <= -LINE_WIDTH {
            self.lines[a].0 += LINE_WIDTH * 2;
            self.lines[b].0 = self.lines[a].0 - LINE_WIDTH;
            self.lines[a].1 = rng.random() > BUMPY_THRESHOLD;
        }
    }

    fn add_cloud(&mut self, rng: &mut Rng) {
        let gap = rng.num(MIN_CLOUD_GAP, MAX_CLOUD_GAP);
        let y = rng.num(MAX_SKY_LEVEL, MIN_SKY_LEVEL);
        self.clouds.push(Cloud { x: self.width, y, gap });
    }

    fn update_clouds(&mut self, dt: f64, speed: f64, rng: &mut Rng) {
        let step = (BG_CLOUD_SPEED / 1000.0 * dt * speed).ceil() as i32;
        let Some(last) = self.clouds.last() else {
            self.add_cloud(rng);
            return;
        };
        let room = self.width - (last.x - step) > last.gap;
        for c in &mut self.clouds {
            c.x -= step;
        }
        if self.clouds.len() < MAX_CLOUDS && room && CLOUD_FREQUENCY > rng.random() {
            self.add_cloud(rng);
        }
        self.clouds.retain(|c| c.x + CLOUD_WIDTH > 0);
    }

    fn update_obstacles(&mut self, dt: f64, speed: f64, rng: &mut Rng) {
        for o in &mut self.obstacles {
            o.update(dt, speed);
        }
        self.obstacles.retain(|o| !o.remove);
        match self.obstacles.last_mut() {
            Some(last) => {
                if !last.following_created && last.visible() && last.x + last.width + last.gap < self.width {
                    last.following_created = true;
                    self.add_obstacle(speed, rng);
                }
            }
            None => self.add_obstacle(speed, rng),
        }
    }

    fn add_obstacle(&mut self, speed: f64, rng: &mut Rng) {
        loop {
            let ty = &TYPES[rng.num(0, TYPES.len() as i32 - 1) as usize];
            if self.duplicate(ty.kind) || speed < ty.min_speed {
                continue;
            }
            self.obstacles.push(Obstacle::new(ty, self.width, speed, rng));
            self.spawned += 1;
            self.history.insert(0, ty.kind);
            self.history.truncate(MAX_OBSTACLE_DUPLICATION);
            return;
        }
    }

    /// horizon.ts duplicateObstacleCheck: the run length is counted to the
    /// end of the (newest-first) history.
    fn duplicate(&self, kind: Kind) -> bool {
        let mut count = 0;
        for &k in &self.history {
            count = if k == kind { count + 1 } else { 0 };
        }
        count >= MAX_OBSTACLE_DUPLICATION
    }

    pub fn reset(&mut self, rng: &mut Rng) {
        self.obstacles.clear();
        self.lines = [(0, self.lines[0].1), (LINE_WIDTH, self.lines[1].1)];
        self.night.reset(self.width, rng);
    }
}
