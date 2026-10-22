//! Generator v2 of the 2-button sequence lock (`circuit display seqlock --v2`): tiles that no
//! bus has to cross (designs/circuits/lock/seqlock-plan.md, "Next layout").
//!
//! - Each button's pulse bulb is read into a lane, a straight dust row through every tile (lane A
//!   at z 6, lane B at z 8, y 1) with one booster per tile. The lane flips once per press.
//! - Each tile taps both lanes with observers (no electrical tap): lane B from its south side,
//!   lane A from above (carried over lane B at y 3). Delay-2 repeaters stretch the pulses to
//!   4 gt: these are PA and PB.
//! - G_a and G_b (y 1) feed Q under the token bulb B; TP_a and TP_b (y 3) feed Q' above it.
//!   G_x = PA/PB at its rear minus max(blocker, M_x). M_x = 15 when the code is not x (the frame
//!   reader f, then f - 1 or 2 - f, then a repeater). TP_x = PA/PB minus R'' (R'' = 15 when the
//!   token is not at s+1).
//! - R reads B into the block K. K feeds the R'' line (y 3, delay-1 repeaters into the TP gates),
//!   the next tile's blockers (delay-2 repeaters into the gates' north sides), and TE = 15 - R.
//!   TE chains west to tile 0, whose blocker it is.

use crate::abuild::{B, D, P};
use crate::rand8::plus;
use crate::seqgen::connect;
use anyhow::Result;
use std::collections::BTreeSet;
use std::path::Path;

const ZG: i32 = 15;
const STEP: i32 = 10;
const C0: i32 = 7;

struct Tile {
    blocker: [P; 2],
    r_src: P,
    te: P,
    te_in: P,
    ma: P,
    ma_to: P,
    mb: P,
    mb_to: P,
    frame: P,
}

fn tile(b: &mut B, s: i32) -> Tile {
    let c = C0 + STEP * s;
    let z = ZG;
    // lane boosters
    b.rep([c + 5, 1, 6], D::W, 1);
    b.rep([c + 5, 1, 8], D::W, 1);
    // taps: lane B from the south, lane A from above (over lane B at y 3)
    b.set([c + 2, 1, 9], "observer[facing=north,powered=false]");
    b.stone([c + 2, 1, 10]);
    b.rep([c + 2, 1, 11], D::S, 2);
    b.dust([c + 2, 1, 12]);
    b.set([c - 2, 2, 6], "observer[facing=down,powered=false]");
    b.stone([c - 2, 3, 6]);
    b.stone([c - 2, 2, 7]);
    b.rep([c - 2, 3, 7], D::S, 2);
    b.stone([c - 2, 2, 8]);
    b.dust([c - 2, 3, 8]);
    // the PA spine at y 1 down the west side (x c-3; the R line from the west passes over it at
    // y 3), the PB spine down the east side (x c+3); each feeds its G gate's rear at y 1, and climbs
    // (x c-4, c+4) to its TP gate's rear at y 3
    b.stone([c - 2, 1, 9]);
    b.dust([c - 2, 2, 9]);
    b.dust([c - 2, 1, 10]);
    b.dust([c + 3, 1, 12]);
    for zz in 10..=z {
        b.dust([c - 3, 1, zz]);
        if zz >= 12 {
            b.dust([c + 3, 1, zz]);
        }
    }
    b.dust([c - 2, 1, z]);
    b.dust([c + 2, 1, z]);
    for (x, dx) in [(c - 4, 1), (c + 4, -1)] {
        b.dust([x, 1, z - 2]);
        b.stone([x, 1, z - 1]);
        b.dust([x, 2, z - 1]);
        b.stone([x, 2, z]);
        b.dust([x, 3, z]);
        b.stone([x + dx, 2, z]);
        b.dust([x + dx, 3, z]);
        b.dust([x + 2 * dx, 3, z]);
    }
    // the gates and the token bulb
    b.stone([c, 1, z]);
    b.comp([c - 1, 1, z], D::E, true);
    b.comp([c + 1, 1, z], D::W, true);
    b.set([c, 2, z], "waxed_copper_bulb[lit=true,powered=false]");
    b.stone([c, 3, z]);
    b.stone([c - 1, 2, z]);
    b.stone([c + 1, 2, z]);
    b.comp([c - 1, 3, z], D::E, true);
    b.comp([c + 1, 3, z], D::W, true);
    b.stone([c - 2, 2, z]);
    b.stone([c + 2, 2, z]);
    // blockers into the gates' north sides (4 gt late), joined
    b.rep([c - 1, 1, z - 1], D::S, 2);
    b.rep([c + 1, 1, z - 1], D::S, 2);
    // the code into the gates' south sides
    b.rep([c - 1, 1, z + 1], D::N, 1);
    b.rep([c + 1, 1, z + 1], D::N, 1);
    // R reads B into K; the R'' line on K feeds the TP gates through delay-1 repeaters
    b.stone([c, 1, z + 1]);
    b.comp([c, 2, z + 1], D::S, false);
    b.stone([c, 2, z + 2]);
    for x in c - 1..=c + 1 {
        if x != c {
            b.stone([x, 2, z + 2]);
            b.stone([x, 2, z + 1]);
        }
        b.dust([x, 3, z + 2]);
    }
    b.rep([c - 1, 3, z + 1], D::N, 1);
    b.rep([c + 1, 3, z + 1], D::N, 1);
    // the R line leaves eastward through a booster
    b.stone([c + 2, 2, z + 2]);
    b.rep([c + 2, 3, z + 2], D::E, 1);
    // TE = 15 - R, chained west
    // (at y 3, off the R'' line, so the code lines at y 1 pass under it)
    b.stone([c, 2, z + 3]);
    b.rep([c, 3, z + 3], D::S, 1);
    b.stone([c, 2, z + 4]);
    b.comp([c, 3, z + 4], D::E, true);
    b.set([c - 1, 3, z + 4], "redstone_block");
    b.stone([c + 1, 2, z + 4]);
    b.dust([c + 1, 3, z + 4]);
    b.stone([c + 2, 2, z + 4]);
    b.rep([c + 2, 3, z + 4], D::W, 1);
    // the code: F reads the frame into D; Za = f - 1 (side: 1 from a composter at 1), Zb = 2 - f
    b.stone([c, 1, z + 8]);
    b.comp([c, 1, z + 7], D::N, false);
    b.dust([c, 1, z + 6]);
    b.comp([c - 1, 1, z + 6], D::W, true);
    b.rep([c - 2, 1, z + 6], D::W, 1);
    b.comp([c - 1, 1, z + 7], D::N, false);
    b.composter([c - 1, 1, z + 8], 1);
    b.comp([c + 1, 1, z + 6], D::S, true);
    b.composter([c + 1, 1, z + 5], 2);
    b.rep([c + 1, 1, z + 7], D::S, 1);
    Tile {
        blocker: [[c - 1, 1, z - 2], [c + 1, 1, z - 2]],
        r_src: [c + 3, 3, z + 2],
        te: [c + 1, 3, z + 4],
        te_in: [c + 3, 3, z + 4],
        ma: [c - 3, 1, z + 6],
        ma_to: [c - 1, 1, z + 2],
        mb: [c + 1, 1, z + 8],
        mb_to: [c + 1, 1, z + 2],
        frame: [c, 1, z + 9],
    }
}

pub fn build(l: u32, code: &str, name: &str, out: &Path) -> Result<()> {
    let code: Vec<u8> = code.bytes().map(|c| c - b'a').collect();
    anyhow::ensure!(code.len() == l as usize && code.iter().all(|c| *c < 2), "--code: {l} letters of a, b");
    let mut b = B::default();
    // buttons at the east end, so the lanes run west and the last tile hears a press first (a
    // delay that falls along the chain is race-free, and the output is quick): a at xa (lane A at
    // z 6), b at xa + 2 (its lane goes south to z 8, then west, outside lane A)
    let xa = C0 + STEP * (l as i32 - 1) + 7;
    for (x, lane) in [(xa + 2, 8), (xa, 6)] {
        b.set([x, 1, 0], "stone_button[face=wall,facing=north,powered=false]");
        b.stone([x, 1, 1]);
        b.set([x, 1, 2], "redstone_wire[east=side,north=side,south=side,west=side]");
        b.needs.push([x, 1, 2]);
        b.set([x, 1, 3], "waxed_copper_bulb[lit=false,powered=false]");
        b.comp([x, 1, 4], D::S, false);
        for z in 5..=lane {
            b.dust([x, 1, z]);
        }
    }
    let tiles: Vec<Tile> = (0..l as i32).map(|s| tile(&mut b, s)).collect();
    for (lane, east) in [(6, xa - 1), (8, xa + 1)] {
        for x in C0 - 3..=east {
            if !b.blocks.contains_key(&[x, 1, lane]) {
                b.dust([x, 1, lane]);
            }
        }
    }
    // the output: on while the last bulb is unlit
    let cl = C0 + STEP * (l as i32 - 1);
    let o = [cl + 5, 2, ZG + 2];
    b.comp(o, D::E, true);
    b.composter([cl + 4, 2, ZG + 2], 1);
    b.supports();
    let mut nets: Vec<(String, P, Vec<P>)> = Vec::new();
    for (s, t) in tiles.iter().enumerate() {
        nets.push((format!("Ma{s}"), t.ma, vec![t.ma_to]));
        nets.push((format!("Mb{s}"), t.mb, vec![t.mb_to]));
        let to = match tiles.get(s + 1) {
            Some(n) => n.blocker.to_vec(),
            None => vec![[o[0], 2, o[2] - 1]],
        };
        nets.push((format!("R{}", s + 1), t.r_src, to));
        let te_to = if s == 0 { t.blocker.to_vec() } else { vec![tiles[s - 1].te_in] };
        nets.push((format!("TE{}", s + 1), t.te, te_to));
    }
    for (_, src, _) in &nets {
        b.dust(*src);
    }
    b.supports();
    let mut fence = BTreeSet::new();
    for (p, st) in &b.blocks {
        let bulb = st.contains("copper_bulb");
        if bulb || st.ends_with("redstone_block") {
            for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]] {
                fence.insert(plus(*p, d));
                if bulb {
                    // nor next to a block beside a bulb
                    for e in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
                        fence.insert(plus(plus(*p, d), e));
                    }
                }
            }
        }
    }
    // the output's line out stays free
    for k in 1..=4 {
        for dy in -1..=1 {
            fence.insert([o[0] + k, o[1] + dy, o[2]]);
        }
    }
    let base = b.clone();
    let mut order: Vec<usize> = (0..nets.len()).collect();
    let mut last = String::new();
    let mut routed = false;
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..400 {
        b = base.clone();
        let mut failed = None;
        for &i in &order {
            let (what, src, targets) = &nets[i];
            let mut keep = fence.clone();
            for (j, (_, _, t2)) in nets.iter().enumerate() {
                if j != i {
                    for p in t2 {
                        for dx in -2..=2i32 {
                            for dz in -2..=2i32 {
                                for dy in -1..=1 {
                                    if dx.abs() + dz.abs() <= 2 {
                                        keep.insert(plus(*p, [dx, dy, dz]));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for t in targets {
                keep.remove(t);
            }
            if let Err(e) = connect(&mut b, [*src].into_iter().collect(), targets, what, &keep) {
                last = e.to_string();
                failed = Some(i);
                break;
            }
        }
        // every net still joined once all are laid (a later support can cut an earlier climb)
        if failed.is_none() {
            for (i, (what, src, targets)) in nets.iter().enumerate() {
                if let Some(t) = targets.iter().find(|t| !joined(&b, *src, **t)) {
                    last = format!("{what}: cut from {t:?} ({})", why_cut(&b, *src, *t));
                    failed = Some(i);
                    break;
                }
            }
        }
        match failed {
            None => {
                routed = true;
                break;
            }
            Some(i) if order[0] == i => {
                for k in (2..order.len()).rev() {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    order.swap(k, 1 + (seed >> 33) as usize % k);
                }
            }
            Some(i) => {
                order.retain(|&x| x != i);
                order.insert(0, i);
            }
        }
    }
    anyhow::ensure!(routed, "no routing order found (last: {last})");
    b.port("input.a", [xa, 1, 0]);
    b.port("input.b", [xa + 2, 1, 0]);
    for (s, t) in tiles.iter().enumerate() {
        b.port(&format!("input.k{s}"), t.frame);
    }
    b.port("output", o);
    let mut d = b.design(name, Some(&format!("sequence-lock 2 {l}")));
    d.meta.insert("port input.a".into(), "button".into());
    d.meta.insert("port input.b".into(), "button".into());
    for (s, t) in tiles.iter().enumerate() {
        d.meta.insert(format!("port input.k{s}"), "frame".into());
        let f = t.frame;
        d.entities.push(mctas::design::EntitySpec {
            kind: "minecraft:item_frame".into(),
            pos: [f[0] as f64 + 0.5, f[1] as f64 + 0.5, f[2] as f64 + 0.5],
            nbt: Some(format!("{{Facing:3b,Item:{{id:\"minecraft:arrow\",count:1}},ItemRotation:{}b}}", code[s])),
        });
    }
    crate::seqgen::write_at_rest(d, out, &tiles.iter().flat_map(|t| {
        let c = t.frame[0];
        [[c - 1, 1, ZG], [c + 1, 1, ZG], [c - 1, 3, ZG], [c + 1, 3, ZG]]
    }).collect::<Vec<P>>(), &[[xa, 1, 2], [xa + 2, 1, 2]])
}

/// Whether the dust at `to` is joined to the dust at `from` by dust connections as the game makes
/// them: side by side, or a level apart when the block over the lower dust is no conductor (a
/// support placed later can cut a climb laid earlier).
fn joined(b: &B, from: P, to: P) -> bool {
    let dust = |p: &P| b.blocks.get(p).is_some_and(|s| s.contains("redstone_wire"));
    let solid = |p: &P| {
        b.blocks.get(p).is_some_and(|s| {
            !s.contains("redstone_wire") && !s.contains("repeater") && !s.contains("comparator") && !s.contains("glass") && !s.contains("observer") && !s.contains("button") && !s.contains("lever")
        }) || b.blocks.get(p).is_some_and(|s| s.contains("observer"))
    };
    let mut seen: BTreeSet<P> = [from].into_iter().collect();
    let mut q = vec![from];
    while let Some(c) = q.pop() {
        if c == to {
            return true;
        }
        for h in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for dy in -1..=1 {
                let n = [c[0] + h[0], c[1] + dy, c[2] + h[2]];
                if seen.contains(&n) || !dust(&n) {
                    continue;
                }
                let ok = match dy {
                    0 => true,
                    1 => !solid(&[c[0], c[1] + 1, c[2]]),
                    _ => !solid(&[n[0], n[1] + 1, n[2]]),
                };
                if ok {
                    seen.insert(n);
                    q.push(n);
                }
            }
        }
    }
    false
}

/// The dust cells next to `to` and what lies over each (for the cut report).
fn why_cut(b: &B, _from: P, to: P) -> String {
    let mut v = Vec::new();
    for h in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
        for dy in -1..=1 {
            let n = [to[0] + h[0], to[1] + dy, to[2] + h[2]];
            if b.blocks.get(&n).is_some_and(|s| s.contains("redstone_wire")) {
                let over = if dy > 0 { [to[0], to[1] + 1, to[2]] } else { [n[0], n[1] + 1, n[2]] };
                v.push(format!("{n:?} over {:?}={}", over, b.blocks.get(&over).map(|s| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string()).unwrap_or_default()));
            }
        }
    }
    v.join(", ")
}
