//! The game's explosion particles (vjeux 2026-10-21: "the particules effect for the tnt explosion
//! looks broken": the viewer drew nine grey cubes). A TNT blast (radius 4, it breaks blocks) sends
//! the client an `explosion_emitter` (`HugeExplosionSeedParticle`): for its 8 ticks it spawns 6
//! `explosion` particles a tick (`HugeExplosionParticle`) at the centre plus `(r - r) * 4` blocks on
//! each axis (r uniform in 0..1). Each is a camera-facing square of the game's 16-frame sprite
//! (`particle/explosion_0..15`: a white puff that grows into a ring and breaks up), its half size
//! `2 * (1 - 0.5 * k / 8)` blocks (k: the emitter's tick when it spawned it), its life 6 to 9
//! ticks (`6 + nextInt(4)`), its frame `age * 15 / life`, its grey `0.4..1.0` times the sprite,
//! drawn at full brightness, still (no motion), cut out (the opaque particle sheet).
//!
//! The client's random numbers are its own: the viewer draws the same pattern from a seed of the
//! blast (its centre and tick), the game's sizes, spread, counts, lives and greys.

/// One puff on screen: its centre, half its size (blocks), the sprite frame (0..16), its grey.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    pub pos: [f32; 3],
    pub half: f32,
    pub frame: u32,
    pub grey: f32,
}

/// The emitter's life (ticks) and the puffs it spawns each of them.
pub const EMITTER_TICKS: u32 = 8;
pub const PER_TICK: u32 = 6;
/// The sprite's frames.
pub const FRAMES: u32 = 16;
/// How long after its blast an explosion still shows (the last puffs: spawned at tick 8, 9 ticks
/// of life).
pub const SHOWS_FOR: f32 = (EMITTER_TICKS + 1 + 9) as f32;

/// A small deterministic generator (splitmix64) standing in for the client's `RandomSource`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// Uniform in 0..1.
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

/// The puffs of a blast at `centre` (its tick `time` seeds the pattern) `age` ticks after it (the
/// packet arrives with the blast's tick; the emitter's first tick is the next one).
pub fn explosion(centre: [f32; 3], time: u64, age: f32) -> Vec<Puff> {
    let seed = time.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ (centre[0].to_bits() as u64) ^ ((centre[1].to_bits() as u64) << 21) ^ ((centre[2].to_bits() as u64) << 42);
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    for k in 0..EMITTER_TICKS {
        let born = (k + 1) as f32;
        for _ in 0..PER_TICK {
            let off = [(rng.f() - rng.f()) * 4.0, (rng.f() - rng.f()) * 4.0, (rng.f() - rng.f()) * 4.0];
            let life = 6 + rng.below(4);
            let grey = rng.f() * 0.6 + 0.4;
            let a = age - born;
            if a < 0.0 || a >= life as f32 {
                continue;
            }
            let half = 2.0 * (1.0 - 0.5 * k as f32 / EMITTER_TICKS as f32);
            let frame = (a.floor() as u32 * (FRAMES - 1) / life).min(FRAMES - 1);
            out.push(Puff { pos: [centre[0] + off[0], centre[1] + off[1], centre[2] + off[2]], half, frame, grey });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's emitter: 6 puffs a tick for 8 ticks from the tick after the blast, each living 6
    /// to 9 ticks within 4 blocks of the centre on every axis, the first ones the biggest (2
    /// blocks half size), all gone 18 ticks after the blast; the same blast, the same puffs.
    #[test]
    fn a_blast_puffs_like_the_game() {
        let c = [10.5, 64.0, -3.5];
        assert!(explosion(c, 100, 0.5).is_empty(), "nothing on the blast's own tick");
        let first = explosion(c, 100, 1.0);
        assert_eq!(first.len(), 6);
        assert!(first.iter().all(|p| p.half == 2.0 && p.frame == 0));
        let mid = explosion(c, 100, 6.0);
        assert!(mid.len() >= 30 && mid.len() <= 36, "{}", mid.len());
        for p in &mid {
            assert!((0..3).all(|i| (p.pos[i] - c[i]).abs() <= 4.0));
            assert!(p.half > 1.0 && p.half <= 2.0 && p.frame < FRAMES && (0.4..=1.0).contains(&p.grey));
        }
        assert!(explosion(c, 100, SHOWS_FOR).is_empty(), "gone");
        assert_eq!(explosion(c, 100, 6.0), mid, "the same blast, the same puffs");
        assert_ne!(explosion(c, 114, 6.0), mid, "another blast, another pattern");
        // the first tick's puffs (the biggest) are gone 10 ticks after the blast; the frames move
        // through the sprite
        let late = explosion(c, 100, 11.0);
        assert!(!late.is_empty() && late.iter().all(|p| p.half < 2.0), "{late:?}");
        assert!(late.iter().any(|p| p.frame > 5), "{:?}", late.iter().map(|p| p.frame).collect::<Vec<_>>());
    }
}
