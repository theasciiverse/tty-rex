//! Small seedable PRNG (SplitMix64), so the sim needs no dependencies and a
//! run can be replayed from its seed.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// Uniform in [0, 1), the stand-in for `Math.random()`.
    pub fn random(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }

    /// utils.ts `getRandomNum`: an integer in [min, max], both inclusive.
    pub fn num(&mut self, min: i32, max: i32) -> i32 {
        (self.random() * (max - min + 1) as f64).floor() as i32 + min
    }
}
