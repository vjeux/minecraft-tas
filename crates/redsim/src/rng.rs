//! The game's `LegacyRandomSource` (Java's 48-bit LCG, `BitRandomSource`'s draws) and the drops the
//! bridge seeds with it (shulker study, docs/mechanics.md "Drops and the bridge's seeds").
//!
//! redsim does not mirror the LEVEL random (its draws depend on the sandbox's chunk count: 3
//! `nextInt(48)` per block-ticking chunk per tick, piston sound pitches, dispenser slots...). The
//! bridge instead gives each item entity that `Block.popResource` spawns during a tick its own
//! random, reseeded to `seed ^ (0x6d63746173 + n)` (n = the sandbox's n-th such drop), and draws
//! everything from it: the offsets in its cell (x, y, z), the yaw, the x and z drift. [`Drop`]
//! mirrors that: a drop is a function of the sandbox seed, its number and its cell.

use crate::pos::BlockPos;

/// `LegacyRandomSource`: `setSeed`, `next(bits)` and `BitRandomSource`'s `nextInt(bound)`,
/// `nextFloat` (float precision), `nextDouble`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LegacyRandom {
    seed: i64,
}

const MULTIPLIER: i64 = 25_214_903_917;
const MASK: i64 = (1 << 48) - 1;

impl LegacyRandom {
    pub fn new(seed: i64) -> LegacyRandom {
        let mut r = LegacyRandom { seed: 0 };
        r.set_seed(seed);
        r
    }

    /// `setSeed`: `(seed ^ 0x5DEECE66D) & (2^48 - 1)`.
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULTIPLIER) & MASK;
    }

    /// `next(bits)`.
    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(11) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `nextInt(bound)` (bound > 0): a power of two takes the high bits, anything else rejects the
    /// samples that would bias the modulo.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "nextInt bound must be positive");
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    /// `nextFloat`: `next(24) * 5.9604645E-8F`.
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    /// `nextDouble`: `((long) next(26) << 27) + next(27)`, times `DOUBLE_MULTIPLIER` (a double
    /// field initialized from the float literal `1.110223E-16F`, i.e. exactly 2^-53; decompilers
    /// print the float literal inline, but the product is a double one).
    pub fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        let combined = (upper << 27) + lower;
        combined as f64 * (1.110_223e-16_f32 as f64)
    }

    /// `Mth.nextDouble(random, min, max)`.
    pub fn next_double_between(&mut self, min: f64, max: f64) -> f64 {
        if min >= max { min } else { self.next_double() * (max - min) + min }
    }
}

/// A drop as the bridge places it (`Sandbox.DropSeeder`): where, its yaw, its motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    pub pos: [f64; 3],
    pub y_rot: f32,
    pub motion: [f64; 3],
}

/// Half the item entity's height (`EntityTypes.ITEM`: 0.25).
const HALF_HEIGHT: f64 = 0.125;

impl Drop {
    /// The `n`-th drop (from 0) of a sandbox seeded `seed`, out of the block at `cell`.
    pub fn seeded(seed: i64, n: u64, cell: BlockPos) -> Drop {
        let mut r = LegacyRandom::new(seed ^ 0x6d_6374_6173_i64.wrapping_add(n as i64));
        let x = cell.x as f64 + 0.5 + r.next_double_between(-0.25, 0.25);
        let y = cell.y as f64 + 0.5 + r.next_double_between(-0.25, 0.25) - HALF_HEIGHT;
        let z = cell.z as f64 + 0.5 + r.next_double_between(-0.25, 0.25);
        let y_rot = r.next_float() * 360.0;
        let mx = r.next_double() * 0.2 - 0.1;
        let mz = r.next_double() * 0.2 - 0.1;
        Drop { pos: [x, y, z], y_rot, motion: [mx, 0.2, mz] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Java's `new Random(42)`-style values, from `LegacyRandomSource(42)`: the LCG itself.
    #[test]
    fn lcg_matches_java() {
        let mut r = LegacyRandom::new(42);
        // java.util.Random(42).nextInt() = -1170105035, nextInt(10) after that = 3 (the same LCG)
        assert_eq!(r.next(32), -1_170_105_035);
        assert_eq!(r.next_int(10), 3);
    }

    /// The first drop of the real-game exact trace of shulker-ref-loader-1 (seed -5180098856, the
    /// box cell (0, 1, 0)): recorded after its first tick (gravity 0.04, then the drag: 0.98F
    /// horizontally, a float widened, and 0.98 vertically), its yaw as drawn.
    #[test]
    fn drop_matches_the_bridge() {
        let d = Drop::seeded(-5_180_098_856, 0, BlockPos::new(0, 1, 0));
        let drag = 0.98_f32 as f64;
        assert_eq!(d.y_rot, 219.419_204_711_914_06_f32);
        assert_eq!(d.motion[0] * drag, -0.042_815_966_586_209_03, "{:?}", d);
        assert_eq!(d.motion[2] * drag, -0.006_477_272_257_639_967, "{:?}", d);
        assert_eq!(d.pos[1] + (0.2 - 0.04), 1.409_805_512_992_943_4, "{:?}", d);
    }
}
