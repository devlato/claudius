//! Deterministic randomness (xoshiro256++). Every creative choice is seeded, so a blueprint always renders the same
//! song; `fork` derives independent streams per part, so changing the bassline does not reshuffle the drums.

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        Rng { s: [splitmix(&mut x), splitmix(&mut x), splitmix(&mut x), splitmix(&mut x)] }
    }

    /// an independent stream named `label`, derived from this one's state without advancing it
    pub fn fork(&self, label: &str) -> Rng {
        Rng::new(self.s[0] ^ self.s[2].rotate_left(17) ^ hash_str(label))
    }

    pub fn seed_from(seed: u64, label: &str) -> Rng {
        Rng::new(seed ^ hash_str(label).rotate_left(9))
    }

    pub fn next_u64(&mut self) -> u64 {
        let r = (self.s[0].wrapping_add(self.s[3])).rotate_left(23).wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn f32(&mut self) -> f32 {
        self.f64() as f32
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// log-uniform in [lo, hi] (frequencies, times)
    pub fn log_range(&mut self, lo: f64, hi: f64) -> f64 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.f64()).exp()
    }

    /// uniform integer in [lo, hi] inclusive
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i64
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as usize }
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    pub fn normal(&mut self) -> f64 {
        let u1 = self.f64().max(1e-300);
        let u2 = self.f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }

    /// index chosen with probability proportional to `w` (all-zero weights pick uniformly)
    pub fn weighted(&mut self, w: &[f64]) -> usize {
        let total: f64 = w.iter().map(|v| v.max(0.0)).sum();
        if total <= 0.0 {
            return self.below(w.len());
        }
        let mut t = self.f64() * total;
        for (i, v) in w.iter().enumerate() {
            t -= v.max(0.0);
            if t <= 0.0 {
                return i;
            }
        }
        w.len() - 1
    }

    pub fn pick_weighted<'a, T>(&mut self, items: &'a [(T, f64)]) -> &'a T {
        let w: Vec<f64> = items.iter().map(|(_, w)| *w).collect();
        &items[self.weighted(&w)].0
    }

    pub fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = self.below(i + 1);
            xs.swap(i, j);
        }
    }
}
