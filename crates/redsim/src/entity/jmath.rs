//! Java's `Math.sin` / `Math.cos` as the game runs them: HotSpot on x86-64 computes them with the
//! Intel LIBM intrinsic (`StubGenerator::generate_libmSin` / `generate_libmCos`, OpenJDK
//! `src/hotspot/cpu/x86/stubGenerator_x86_64_{sin,cos}.cpp`, its tables in
//! `stubGenerator_x86_64_constants.cpp`), whose result is not always the correctly rounded one:
//! for some arguments it is one ulp away from `StrictMath.sin` and from Rust's `f64::sin`. The
//! pick sweep (2026-10-21) found one: the 9th primed TNT of tnt-clocked-22b's judge run, its kick
//! angle `0x400388d384e3cab3`, `Math.sin` = `0x3fe49c1241087e59`, `f64::sin` = `...5a`, so the
//! TNT's motion was one ulp off at its first tick.
//!
//! [`sin`] and [`cos`] are the intrinsic's main path, operation for operation (SSE2 doubles, no
//! fused multiply-add): the range reduction `x = N pi/32 + r` (pi/32 in three pieces), the table
//! of `sin(B)`, `cos(B)` for `B = M pi/32` (`M = N mod 64`; cosine reads entry `M + 16`), the
//! degree-9/8 polynomials of `sin(r) - r` and `cos(r) - 1` and the compensated sums. The main path
//! covers `2^-252 <= |x| < 90112` (every TNT kick angle, every `Mth` sine table argument but 0);
//! other arguments (0, tiny, huge, NaN, infinities) fall back to Rust's `f64::sin` / `f64::cos`,
//! which agree there on the values the game uses (sin 0 = 0, cos 0 = 1).

/// `32/pi`.
const PI32INV: f64 = f64::from_bits(0x40245f306dc9c883);
/// `0.5`, its sign taken from x: the rounding of `N`.
const ONEHALF: f64 = 0.5;
/// pi/32 in three pieces (`P_1`, `P_2` with low zero bits: `N * P` is exact).
const P_1: f64 = f64::from_bits(0x3fb921fb54400000);
const P_2: f64 = f64::from_bits(0x3d90b4611a600000);
const P_3: f64 = f64::from_bits(0x3b63198a2e037073);
/// The polynomial coefficients, `[sin lane, cos lane]`.
const SC_1: [f64; 2] = [f64::from_bits(0xbfc5555555555555), f64::from_bits(0xbfe0000000000000)];
const SC_2: [f64; 2] = [f64::from_bits(0x3f81111111111111), f64::from_bits(0x3fa5555555555555)];
const SC_3: [f64; 2] = [f64::from_bits(0xbf2a01a01a01a01a), f64::from_bits(0xbf56c16c16c16c17)];
const SC_4: [f64; 2] = [f64::from_bits(0x3ec71de3a556c734), f64::from_bits(0x3efa01a01a01a01a)];

/// `_Ctable`: 64 entries of 4 doubles, each double two 32-bit words (low, high): `C_hl` (cos(B)
/// minus sigma), `S_hi`, `S_lo` (sin(B) as a double-double), `sigma` (the power of two closest
/// to cos(B)).
const CTABLE: [u32; 512] = [
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x3ff00000, 0x176d6d31, 0xbf73b92e,
    0xbc29b42c, 0x3fb917a6, 0xe0000000, 0xbc3e2718, 0x00000000,
    0x3ff00000, 0x011469fb, 0xbf93ad06, 0x3c69a60b, 0x3fc8f8b8,
    0xc0000000, 0xbc626d19, 0x00000000, 0x3ff00000, 0x939d225a,
    0xbfa60bea, 0x2ed59f06, 0x3fd29406, 0xa0000000, 0xbc75d28d,
    0x00000000, 0x3ff00000, 0x866b95cf, 0xbfb37ca1, 0xa6aea963,
    0x3fd87de2, 0xe0000000, 0xbc672ced, 0x00000000, 0x3ff00000,
    0x73fa1279, 0xbfbe3a68, 0x3806f63b, 0x3fde2b5d, 0x20000000,
    0x3c5e0d89, 0x00000000, 0x3ff00000, 0x5bc57974, 0xbfc59267,
    0x39ae68c8, 0x3fe1c73b, 0x20000000, 0x3c8b25dd, 0x00000000,
    0x3ff00000, 0x53aba2fd, 0xbfcd0dfe, 0x25091dd6, 0x3fe44cf3,
    0x20000000, 0x3c68076a, 0x00000000, 0x3ff00000, 0x99fcef32,
    0x3fca8279, 0x667f3bcd, 0x3fe6a09e, 0x20000000, 0xbc8bdd34,
    0x00000000, 0x3fe00000, 0x94247758, 0x3fc133cc, 0x6b151741,
    0x3fe8bc80, 0x20000000, 0xbc82c5e1, 0x00000000, 0x3fe00000,
    0x9ae68c87, 0x3fac73b3, 0x290ea1a3, 0x3fea9b66, 0xe0000000,
    0x3c39f630, 0x00000000, 0x3fe00000, 0x7f909c4e, 0xbf9d4a2c,
    0xf180bdb1, 0x3fec38b2, 0x80000000, 0xbc76e0b1, 0x00000000,
    0x3fe00000, 0x65455a75, 0xbfbe0875, 0xcf328d46, 0x3fed906b,
    0x20000000, 0x3c7457e6, 0x00000000, 0x3fe00000, 0x76acf82d,
    0x3fa4a031, 0x56c62dda, 0x3fee9f41, 0xe0000000, 0x3c8760b1,
    0x00000000, 0x3fd00000, 0x0e5967d5, 0xbfac1d1f, 0xcff75cb0,
    0x3fef6297, 0x20000000, 0x3c756217, 0x00000000, 0x3fd00000,
    0x0f592f50, 0xbf9ba165, 0xa3d12526, 0x3fefd88d, 0x40000000,
    0xbc887df6, 0x00000000, 0x3fc00000, 0x00000000, 0x00000000,
    0x00000000, 0x3ff00000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x0f592f50, 0x3f9ba165, 0xa3d12526, 0x3fefd88d,
    0x40000000, 0xbc887df6, 0x00000000, 0xbfc00000, 0x0e5967d5,
    0x3fac1d1f, 0xcff75cb0, 0x3fef6297, 0x20000000, 0x3c756217,
    0x00000000, 0xbfd00000, 0x76acf82d, 0xbfa4a031, 0x56c62dda,
    0x3fee9f41, 0xe0000000, 0x3c8760b1, 0x00000000, 0xbfd00000,
    0x65455a75, 0x3fbe0875, 0xcf328d46, 0x3fed906b, 0x20000000,
    0x3c7457e6, 0x00000000, 0xbfe00000, 0x7f909c4e, 0x3f9d4a2c,
    0xf180bdb1, 0x3fec38b2, 0x80000000, 0xbc76e0b1, 0x00000000,
    0xbfe00000, 0x9ae68c87, 0xbfac73b3, 0x290ea1a3, 0x3fea9b66,
    0xe0000000, 0x3c39f630, 0x00000000, 0xbfe00000, 0x94247758,
    0xbfc133cc, 0x6b151741, 0x3fe8bc80, 0x20000000, 0xbc82c5e1,
    0x00000000, 0xbfe00000, 0x99fcef32, 0xbfca8279, 0x667f3bcd,
    0x3fe6a09e, 0x20000000, 0xbc8bdd34, 0x00000000, 0xbfe00000,
    0x53aba2fd, 0x3fcd0dfe, 0x25091dd6, 0x3fe44cf3, 0x20000000,
    0x3c68076a, 0x00000000, 0xbff00000, 0x5bc57974, 0x3fc59267,
    0x39ae68c8, 0x3fe1c73b, 0x20000000, 0x3c8b25dd, 0x00000000,
    0xbff00000, 0x73fa1279, 0x3fbe3a68, 0x3806f63b, 0x3fde2b5d,
    0x20000000, 0x3c5e0d89, 0x00000000, 0xbff00000, 0x866b95cf,
    0x3fb37ca1, 0xa6aea963, 0x3fd87de2, 0xe0000000, 0xbc672ced,
    0x00000000, 0xbff00000, 0x939d225a, 0x3fa60bea, 0x2ed59f06,
    0x3fd29406, 0xa0000000, 0xbc75d28d, 0x00000000, 0xbff00000,
    0x011469fb, 0x3f93ad06, 0x3c69a60b, 0x3fc8f8b8, 0xc0000000,
    0xbc626d19, 0x00000000, 0xbff00000, 0x176d6d31, 0x3f73b92e,
    0xbc29b42c, 0x3fb917a6, 0xe0000000, 0xbc3e2718, 0x00000000,
    0xbff00000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0xbff00000, 0x176d6d31,
    0x3f73b92e, 0xbc29b42c, 0xbfb917a6, 0xe0000000, 0x3c3e2718,
    0x00000000, 0xbff00000, 0x011469fb, 0x3f93ad06, 0x3c69a60b,
    0xbfc8f8b8, 0xc0000000, 0x3c626d19, 0x00000000, 0xbff00000,
    0x939d225a, 0x3fa60bea, 0x2ed59f06, 0xbfd29406, 0xa0000000,
    0x3c75d28d, 0x00000000, 0xbff00000, 0x866b95cf, 0x3fb37ca1,
    0xa6aea963, 0xbfd87de2, 0xe0000000, 0x3c672ced, 0x00000000,
    0xbff00000, 0x73fa1279, 0x3fbe3a68, 0x3806f63b, 0xbfde2b5d,
    0x20000000, 0xbc5e0d89, 0x00000000, 0xbff00000, 0x5bc57974,
    0x3fc59267, 0x39ae68c8, 0xbfe1c73b, 0x20000000, 0xbc8b25dd,
    0x00000000, 0xbff00000, 0x53aba2fd, 0x3fcd0dfe, 0x25091dd6,
    0xbfe44cf3, 0x20000000, 0xbc68076a, 0x00000000, 0xbff00000,
    0x99fcef32, 0xbfca8279, 0x667f3bcd, 0xbfe6a09e, 0x20000000,
    0x3c8bdd34, 0x00000000, 0xbfe00000, 0x94247758, 0xbfc133cc,
    0x6b151741, 0xbfe8bc80, 0x20000000, 0x3c82c5e1, 0x00000000,
    0xbfe00000, 0x9ae68c87, 0xbfac73b3, 0x290ea1a3, 0xbfea9b66,
    0xe0000000, 0xbc39f630, 0x00000000, 0xbfe00000, 0x7f909c4e,
    0x3f9d4a2c, 0xf180bdb1, 0xbfec38b2, 0x80000000, 0x3c76e0b1,
    0x00000000, 0xbfe00000, 0x65455a75, 0x3fbe0875, 0xcf328d46,
    0xbfed906b, 0x20000000, 0xbc7457e6, 0x00000000, 0xbfe00000,
    0x76acf82d, 0xbfa4a031, 0x56c62dda, 0xbfee9f41, 0xe0000000,
    0xbc8760b1, 0x00000000, 0xbfd00000, 0x0e5967d5, 0x3fac1d1f,
    0xcff75cb0, 0xbfef6297, 0x20000000, 0xbc756217, 0x00000000,
    0xbfd00000, 0x0f592f50, 0x3f9ba165, 0xa3d12526, 0xbfefd88d,
    0x40000000, 0x3c887df6, 0x00000000, 0xbfc00000, 0x00000000,
    0x00000000, 0x00000000, 0xbff00000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x0f592f50, 0xbf9ba165, 0xa3d12526,
    0xbfefd88d, 0x40000000, 0x3c887df6, 0x00000000, 0x3fc00000,
    0x0e5967d5, 0xbfac1d1f, 0xcff75cb0, 0xbfef6297, 0x20000000,
    0xbc756217, 0x00000000, 0x3fd00000, 0x76acf82d, 0x3fa4a031,
    0x56c62dda, 0xbfee9f41, 0xe0000000, 0xbc8760b1, 0x00000000,
    0x3fd00000, 0x65455a75, 0xbfbe0875, 0xcf328d46, 0xbfed906b,
    0x20000000, 0xbc7457e6, 0x00000000, 0x3fe00000, 0x7f909c4e,
    0xbf9d4a2c, 0xf180bdb1, 0xbfec38b2, 0x80000000, 0x3c76e0b1,
    0x00000000, 0x3fe00000, 0x9ae68c87, 0x3fac73b3, 0x290ea1a3,
    0xbfea9b66, 0xe0000000, 0xbc39f630, 0x00000000, 0x3fe00000,
    0x94247758, 0x3fc133cc, 0x6b151741, 0xbfe8bc80, 0x20000000,
    0x3c82c5e1, 0x00000000, 0x3fe00000, 0x99fcef32, 0x3fca8279,
    0x667f3bcd, 0xbfe6a09e, 0x20000000, 0x3c8bdd34, 0x00000000,
    0x3fe00000, 0x53aba2fd, 0xbfcd0dfe, 0x25091dd6, 0xbfe44cf3,
    0x20000000, 0xbc68076a, 0x00000000, 0x3ff00000, 0x5bc57974,
    0xbfc59267, 0x39ae68c8, 0xbfe1c73b, 0x20000000, 0xbc8b25dd,
    0x00000000, 0x3ff00000, 0x73fa1279, 0xbfbe3a68, 0x3806f63b,
    0xbfde2b5d, 0x20000000, 0xbc5e0d89, 0x00000000, 0x3ff00000,
    0x866b95cf, 0xbfb37ca1, 0xa6aea963, 0xbfd87de2, 0xe0000000,
    0x3c672ced, 0x00000000, 0x3ff00000, 0x939d225a, 0xbfa60bea,
    0x2ed59f06, 0xbfd29406, 0xa0000000, 0x3c75d28d, 0x00000000,
    0x3ff00000, 0x011469fb, 0xbf93ad06, 0x3c69a60b, 0xbfc8f8b8,
    0xc0000000, 0x3c626d19, 0x00000000, 0x3ff00000, 0x176d6d31,
    0xbf73b92e, 0xbc29b42c, 0xbfb917a6, 0xe0000000, 0x3c3e2718,
    0x00000000, 0x3ff00000,
];

/// Entry `m` of [`CTABLE`]: `[C_hl, S_hi, S_lo, sigma]`.
fn entry(m: usize) -> [f64; 4] {
    let w = |k: usize| f64::from_bits(((CTABLE[8 * m + 2 * k + 1] as u64) << 32) | CTABLE[8 * m + 2 * k] as u64);
    [w(0), w(1), w(2), w(3)]
}

/// Whether `x` takes the intrinsic's main path: the high word's exponent bits minus 0x3030_0000
/// at most 0x10c5_0000, unsigned (`2^-252 <= |x| < 90112`, roughly).
fn main_path(x: f64) -> bool {
    let hi = (x.to_bits() >> 32) as u32;
    (hi & 0x7fff_0000).wrapping_sub(0x3030_0000) <= 0x10c5_0000
}

/// The main path of both stubs: `sin(x)` for `offset` 0, `cos(x)` for 16 (the cosine stub adds
/// 1865232 to N, which is 16 mod 64: `cos(x) = sin(x + pi/2)` read from the table).
fn main(x: f64, offset: i32) -> f64 {
    // y = 32/pi x; N = (int)(y + copysign(0.5, x)) (cvttsd2si: toward zero)
    let y = x * PI32INV;
    let n = (y + if x.is_sign_negative() { -ONEHALF } else { ONEHALF }) as i32;
    let nf = n as f64;
    let [c_hl, s_hi, s_lo, sigma] = entry((n.wrapping_add(offset) & 63) as usize);
    // the reduction: r_1 = x - N P_1, r = r_1 - N P_2, -c = N P_3 - ((r_1 - r) - N P_2)
    let m1 = P_1 * nf;
    let m2 = P_2 * nf;
    let m3 = nf * P_3;
    let r1 = x - m1;
    let r = r1 - m2;
    let c2 = (r1 - r) - m2;
    let neg_c = m3 - c2;
    // the polynomials, both lanes: ((SC_4 r_1) r + SC_3) r^4 + (SC_2 r^2 + SC_1)
    let r2 = r * r;
    let r4 = r2 * r2;
    let lane = |k: usize| {
        let msc4 = (SC_4[k] * r1) * r;
        let psc3 = msc4 + SC_3[k];
        let psc1 = SC_2[k] * r2 + SC_1[k];
        psc1 + psc3 * r4
    };
    let cs = c_hl + sigma;
    let neg_d = s_hi * r - cs;
    let pols = [lane(0) * ((cs * r) * r2), lane(1) * (s_hi * r2)];
    // the compensated sums
    let rs = sigma * r;
    let res_int = rs + s_hi;
    let med = r * c_hl;
    let res_hi = med + res_int;
    let k2 = (s_hi - res_int) + rs;
    let k3 = (res_int - res_hi) + med;
    let corr = neg_c * neg_d + s_lo;
    let lo = (((corr + k2) + k3) + pols[0]) + pols[1];
    res_hi + lo
}

/// `Math.sin(x)` as HotSpot computes it on x86-64.
pub fn sin(x: f64) -> f64 {
    if main_path(x) { main(x, 0) } else { x.sin() }
}

/// `Math.cos(x)` as HotSpot computes it on x86-64.
pub fn cos(x: f64) -> f64 {
    if main_path(x) { main(x, 16) } else { x.cos() }
}

#[cfg(test)]
mod tests {
    /// Ground truth: `Math.sin` / `Math.cos` of the game's JVM (jdk-25.0.4.1, the bridge's) on 3000
    /// arguments: the TNT kick that diverged, 1500 kick angles `nextDouble() * (float)(2 pi)`, 1000
    /// in [-200, 200], 500 `Mth` table arguments; one line each `x sin cos` as hex bits (jshell:
    /// `Long.toHexString(Double.doubleToLongBits(Math.sin(x)))`). JSINCOS_FULL=<file> checks a
    /// bigger dump of the same form (every `Mth` argument and 200000 kick angles: all identical
    /// when this module was written).
    #[test]
    fn matches_the_game_jvm() {
        let full = std::env::var("JSINCOS_FULL").ok().and_then(|p| std::fs::read_to_string(p).ok());
        let text = full.as_deref().unwrap_or(include_str!("jmath_samples.txt"));
        let hex = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
        let (mut n, mut bad) = (0usize, Vec::new());
        for l in text.lines() {
            let v: Vec<&str> = l.split_whitespace().collect();
            if v.len() != 3 {
                continue;
            }
            let (x, s, c) = (hex(v[0]), hex(v[1]), hex(v[2]));
            n += 1;
            if super::sin(x).to_bits() != s.to_bits() || super::cos(x).to_bits() != c.to_bits() {
                bad.push(format!("{} sin {:x}/{:x} cos {:x}/{:x}", v[0], super::sin(x).to_bits(), s.to_bits(), super::cos(x).to_bits(), c.to_bits()));
            }
        }
        assert!(n >= 3000, "{n} samples");
        assert!(bad.is_empty(), "{} of {n} differ from the game's JVM: {:?}", bad.len(), &bad[..bad.len().min(5)]);
        // the one the sweep found: one ulp below the correctly rounded sine
        let x = f64::from_bits(0x400388d384e3cab3);
        assert_eq!(super::sin(x).to_bits(), 0x3fe49c1241087e59);
    }
}
