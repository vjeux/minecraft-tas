//! `mctas circuit display seqlock --l L --code CODE --out FILE`: a button sequence lock of 2
//! buttons (`circuit: sequence-lock 2 L`, crate::seqlock's judge), from fixed component tiles
//! joined by routed dust (designs/circuits/lock/seqlock-plan.md has the mechanism).
//!
//! - Buttons a (x 0) and b (x 6) on the north face. Each press of x lights a pulse bulb; its
//!   reader's observer powers a block, whose delay-2 repeaters give 4 gt pulses into the inverter
//!   (NP_x: a subtract comparator with a redstone block behind it) and into the P_any net, inverted
//!   into NP_any.
//! - One tile per step s at x = 3 + 8 s, gates on row z = 14, the code frame on the south face.
//!   G_a (rear composter 2) reads NP_a and h_a = f; G_b reads NP_b and h_b = 3 - f (H, rear
//!   composter 3). Both point into Q, which toggles the bulb B_{s+1} above it. The repeaters Ra and
//!   Rb inject the step's blocker into h_a and h_b: R_s, the previous tile's reader of B_s, or for
//!   step 0 the TP-bus. R_{s+1} reads B_{s+1}. TP_{s+1} (rear composter 2, sides R_{s+1} and
//!   NP_any) toggles B_{s+1} back through Q' on top of it; Q' also feeds the TP-bus through a
//!   repeater. Step 0's NP taps go through delay-2 repeaters, so they line up with the TP-bus.
//! - Output: a subtract comparator with a redstone block behind it and R_L at its side (on while
//!   B_L is unlit).

use crate::abuild::{B, D, P};
use crate::rand8::{lay, plus, route_net};
use anyhow::{Result, anyhow};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

const ZG: i32 = 14;
const STEP: i32 = 11;

fn q_of(s: i32) -> i32 {
    3 + STEP * s
}

/// One tile's fixed blocks; returns (NP_a target, NP_b target, blocker targets, R_{s+1} source,
/// TP side for R_{s+1}, TP side for NP_any, TP-bus source, frame cell).
struct Tile {
    /// The NP taps' repeater inputs (a, b) and the chain boosters' outputs (to the next tile).
    np: [P; 2],
    np_next: [P; 2],
    /// The repeaters that inject the step's blocker into h_a and h_b: their inputs.
    blockers: [P; 2],
    /// R_{s+1}'s net source (on top of the block K it powers), TP_{s+1}'s boosters' inputs.
    r_src: P,
    tp_r: P,
    tp_np: P,
    /// The NP_any chain booster's output (to the next tile).
    np_any_next: P,
    /// TE_{s+1}'s dust (token at s+1, ORed with the next tiles' chain) and the chain booster's input.
    te: P,
    te_in: P,
    frame: P,
}

fn tile(b: &mut B, s: i32) -> Tile {
    let q = q_of(s);
    let z = ZG;
    // the gates, Q, the step's h nets, the frame reader and H
    b.composter([q - 2, 1, z], 2);
    b.comp([q - 1, 1, z], D::E, true);
    b.stone([q, 1, z]);
    b.comp([q + 1, 1, z], D::W, true);
    b.composter([q + 2, 1, z], 2);
    b.stone([q, 1, z - 1]);
    b.dust([q - 1, 1, z + 1]);
    b.stone([q, 1, z + 1]);
    b.dust([q + 1, 1, z + 1]);
    b.stone([q - 1, 1, z + 2]);
    b.dust([q, 1, z + 2]);
    b.comp([q + 1, 1, z + 2], D::N, true);
    b.comp([q - 1, 1, z + 3], D::N, false);
    b.composter([q + 1, 1, z + 3], 3);
    b.stone([q - 1, 1, z + 4]);
    // the blockers come 4 gt late, after the press window of the tile they come from
    b.rep([q - 2, 1, z + 1], D::E, 2);
    b.rep([q + 2, 1, z + 1], D::W, 2);
    if s == 0 {
        // step 0's blocker (the TE chain) comes in through a relay into Rb's input dust, which also
        // feeds Ra (the TE dust is far up north)
        b.rep([q + 4, 1, z + 1], D::W, 1);
        b.rep([q - 4, 1, z + 1], D::E, 1);
        b.dust([q + 3, 1, z + 1]);
        b.dust([q - 3, 1, z + 1]);
    }
    // the NP taps (delay 1): their output dusts feed the gates' sides and the next tile's taps
    b.rep([q - 2, 1, z - 1], D::E, 1);
    b.dust([q - 1, 1, z - 1]);
    b.rep([q + 2, 1, z - 1], D::W, 1);
    b.dust([q + 1, 1, z - 1]);
    // the chain boosters: each bus goes on to the next tile from the taps' input dusts
    b.rep([q - 3, 1, z - 2], D::N, 1);
    b.rep([q + 4, 1, z - 1], D::E, 1);
    // the token bulb above Q; its reader R powers the block K, the R net starts on K
    b.set([q, 2, z], "waxed_copper_bulb[lit=true,powered=false]");
    b.comp([q, 2, z - 1], D::N, false);
    b.stone([q, 1, z - 2]);
    b.stone([q, 2, z - 2]);
    // TP_{s+1} on top: rear composter 2, boosters into its sides, toggles B_{s+1} through Q'
    b.stone([q - 1, 2, z]);
    b.stone([q, 3, z]);
    b.comp([q - 1, 3, z], D::E, true);
    b.composter([q - 2, 3, z], 2);
    b.stone([q - 1, 2, z - 1]);
    b.rep([q - 1, 3, z - 1], D::S, 1);
    b.stone([q - 1, 2, z + 1]);
    b.dust([q - 1, 3, z + 1]);
    b.stone([q - 1, 2, z + 2]);
    b.rep([q - 1, 3, z + 2], D::N, 1);
    b.stone([q - 1, 2, z + 3]);
    b.rep([q - 1, 3, z + 4], D::S, 1);
    b.stone([q - 1, 2, z + 4]);
    b.stone([q - 1, 2, z + 5]);
    b.stone([q - 1, 2, z - 2]);
    // TE_{s+1} = 15 - R_{s+1}: a booster reads K into TE's west side, TE's rear a redstone block;
    // TE's dust also takes the chain from the next tile through a booster
    b.stone([q + 1, 1, z - 2]);
    b.rep([q + 1, 2, z - 2], D::E, 1);
    b.stone([q + 2, 1, z - 2]);
    b.stone([q + 2, 1, z - 3]);
    b.comp([q + 2, 2, z - 2], D::N, true);
    b.set([q + 2, 2, z - 1], "redstone_block");
    b.dust([q + 2, 2, z - 3]);
    b.stone([q + 2, 1, z - 4]);
    b.rep([q + 2, 2, z - 4], D::S, 1);
    Tile {
        np: [[q - 3, 1, z - 1], [q + 3, 1, z - 1]],
        np_next: [[q - 3, 1, z - 3], [q + 5, 1, z - 1]],
        blockers: [[q - 3, 1, z + 1], [q + 3, 1, z + 1]],
        r_src: [q, 3, z - 2],
        tp_r: [q - 1, 3, z - 2],
        tp_np: [q - 1, 3, z + 3],
        np_any_next: [q - 1, 3, z + 5],
        te: [q + 2, 2, z - 3],
        te_in: [q + 2, 2, z - 5],
        frame: [q - 1, 1, z + 5],
    }
}

/// One button's pulse chain at column x; returns (NP_x source, P_any source).
fn button(b: &mut B, x: i32) -> (P, P) {
    b.set([x, 1, 0], "stone_button[face=wall,facing=north,powered=false]");
    b.stone([x, 1, 1]);
    // a cross: the dust powers the bulb in front of it (a dot, which a lone dust becomes, powers nothing)
    b.set([x, 1, 2], "redstone_wire[east=side,north=side,south=side,west=side]");
    b.needs.push([x, 1, 2]);
    b.set([x, 1, 3], "waxed_copper_bulb[lit=false,powered=false]");
    b.comp([x, 1, 4], D::S, false);
    b.set([x, 1, 5], "observer[facing=north,powered=false]");
    b.stone([x, 1, 6]);
    b.rep([x, 1, 7], D::S, 2);
    b.comp([x, 1, 8], D::E, true);
    b.set([x - 1, 1, 8], "redstone_block");
    b.rep([x + 1, 1, 6], D::E, 2);
    b.dust([x + 1, 1, 8]);
    ([x + 1, 1, 8], [x + 2, 1, 6])
}

/// Grow `net` (its cells already dust) to every target, in order.
pub(crate) fn connect(b: &mut B, mut net: BTreeSet<P>, targets: &[P], what: &str, keep: &BTreeSet<P>) -> Result<BTreeSet<P>> {
    let start: Vec<P> = net.iter().copied().collect();
    for t in targets {
        if net.contains(t) {
            continue;
        }
        let path = route_net(b, &net, *t, &[], keep).ok_or_else(|| anyhow!("{what}: no route to {t:?}"))?;
        lay(b, &path);
        net.extend(path);
    }
    // every target (a repeater input, or the output comparator's side against a composter at 1)
    // within 14 dust of the source: it arrives at 1 or more
    let mut dist: BTreeMap<P, u32> = start.iter().map(|p| (*p, 0)).collect();
    let mut q: VecDeque<P> = start.into_iter().collect();
    while let Some(c) = q.pop_front() {
        let d = dist[&c];
        for n in net.iter() {
            if !dist.contains_key(n) && (n[0] - c[0]).abs() + (n[2] - c[2]).abs() == 1 && (n[1] - c[1]).abs() <= 1 {
                dist.insert(*n, d + 1);
                q.push_back(*n);
            }
        }
    }
    for t in targets {
        let d = dist.get(t).copied().unwrap_or(99);
        anyhow::ensure!(d <= 14, "{what}: {t:?} is {d} dust from the source");
    }
    Ok(net)
}

pub fn build(l: u32, code: &str, name: &str, out: &Path) -> Result<()> {
    let code: Vec<u8> = code.bytes().map(|c| c - b'a').collect();
    anyhow::ensure!(code.len() == l as usize && code.iter().all(|c| *c < 2), "--code: {l} letters of a, b");
    let mut b = B::default();
    let (np_a, pa) = button(&mut b, 0);
    let (np_b, pb) = button(&mut b, 6);
    // NP_any: an inverter whose south side is the P_any net
    b.comp([3, 1, 4], D::E, true);
    b.rep([3, 1, 5], D::N, 1);
    b.set([2, 1, 4], "redstone_block");
    b.dust([4, 1, 4]);
    let tiles: Vec<Tile> = (0..l as i32).map(|s| tile(&mut b, s)).collect();
    // the output: on while B_L is unlit
    let ql = q_of(l as i32 - 1);
    let o = [ql + 6, 2, ZG - 1];
    b.comp(o, D::E, true);
    // rear: a composter at level 1, so any R_L at the side (however far it came) shuts it; the
    // repeater after it is the tap (15)
    b.composter([ql + 5, 2, ZG - 1], 1);
    b.rep([ql + 7, 2, ZG - 1], D::E, 1);
    b.supports();
    // every net: (name, sources, targets); routed in order, each keeping clear of the others'
    // targets (a halo of one cell around each)
    // the nets: (name, sources, targets). The NP chains and the NP_any chain go tile to tile
    // (each tile's booster output feeds the next); the TE chain goes back to tile 0, whose
    // blocker it is (a token at any s >= 1)
    let mut nets: Vec<(String, Vec<P>, Vec<P>)> = vec![("P_any".into(), vec![pa, pb], vec![[3, 1, 6]])];
    for s in (0..l as usize).rev() {
        let targets = if s == 0 { vec![plus(tiles[0].blockers[1], [2, 0, 0]), plus(tiles[0].blockers[0], [-2, 0, 0])] } else { vec![tiles[s - 1].te_in] };
        nets.push((format!("TE_{}", s + 1), vec![tiles[s].te], targets));
    }
    for (k, src) in [(1, np_b), (0, np_a)] {
        for s in 0..l as usize {
            let from = if s == 0 { src } else { tiles[s - 1].np_next[k] };
            nets.push((format!("NP_{}{s}", ["a", "b"][k]), vec![from], vec![tiles[s].np[k]]));
        }
    }
    for s in 0..l as usize {
        let from = if s == 0 { [4, 1, 4] } else { tiles[s - 1].np_any_next };
        nets.push((format!("NP_any{s}"), vec![from], vec![tiles[s].tp_np]));
    }
    for s in 0..l as usize {
        let t = &tiles[s];
        let mut targets = vec![t.tp_r];
        match tiles.get(s + 1) {
            Some(n) => targets.extend(n.blockers),
            None => targets.push([o[0], 2, o[2] - 1]),
        }
        nets.push((format!("R_{}", s + 1), vec![t.r_src], targets));
    }
    for (_, src, _) in &nets {
        // only the first source starts the net: the others are reached by routing (two separate
        // dust cells are not one net)
        b.dust(src[0]);
    }
    b.supports();
    // no dust may power a bulb or a block next to one: a dust pointing into a block (or lying on
    // it) gives it a direct signal, which a bulb beside that block takes as power and toggles
    let mut protect: Vec<P> = vec![[0, 1, 3], [6, 1, 3]];
    for s in 0..l as i32 {
        let q = q_of(s);
        protect.extend([[q, 2, ZG], [q, 1, ZG], [q, 3, ZG], [q - 1, 2, ZG], [q + 1, 2, ZG], [q, 2, ZG + 1]]);
    }
    let mut fence = BTreeSet::new();
    for x in &protect {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
            fence.insert(plus(*x, d));
        }
        fence.insert(*x);
    }
    // nor run next to a redstone block (it would power the dust)
    for (p, st) in &b.blocks {
        if st.ends_with("redstone_block") {
            for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]] {
                fence.insert(plus(*p, d));
            }
        }
    }
    // greedy routing in an order; a net that finds no route moves to the front and the routing
    // starts over (up to 100 orders)
    let keeps: Vec<BTreeSet<P>> = (0..nets.len())
        .map(|i| {
            let mut keep = fence.clone();
            for (j, (_, s2, t2)) in nets.iter().enumerate() {
                if j == i {
                    continue;
                }
                // an approach corridor round every other target: every cell within 2 (across) and
                // 1 (up or down) of it (sources are laid already: the near rule keeps dust off them)
                for p in t2.iter().chain(s2[1..].iter()) {
                    for dx in -3..=3i32 {
                        for dz in -3..=3i32 {
                            for dy in -1..=1 {
                                // (a cell at least as close to one of this net's own targets stays free)
                                let c = plus(*p, [dx, dy, dz]);
                                let own = nets[i].2.iter().map(|t| (t[0] - c[0]).abs() + (t[2] - c[2]).abs()).min().unwrap_or(99);
                                if dx.abs() + dz.abs() <= 3 && dx.abs() + dz.abs() < own {
                                    keep.insert(c);
                                }
                            }
                        }
                    }
                }
            }
            keep
        })
        .collect();
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
            let all: Vec<P> = targets.iter().chain(src[1..].iter()).copied().collect();
            if let Err(e) = connect(&mut b, [src[0]].into_iter().collect(), &all, what, &keeps[i]) {
                last = e.to_string();
                failed = Some(i);
                break;
            }
        }
        match failed {
            None => {
                routed = true;
                break;
            }
            Some(i) => {
                if order[0] == i {
                    // first and still failing: shuffle the others (a seeded Fisher-Yates)
                    for k in (2..order.len()).rev() {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        let j = 1 + (seed >> 33) as usize % k;
                        order.swap(k, j);
                    }
                    continue;
                }
                order.retain(|&x| x != i);
                order.insert(0, i);
            }
        }
    }
    anyhow::ensure!(routed, "no routing order found (last: {last})");
    b.port("input.a", [0, 1, 0]);
    b.port("input.b", [6, 1, 0]);
    for (s, t) in tiles.iter().enumerate() {
        b.port(&format!("input.k{s}"), t.frame);
    }
    b.port("output", [o[0] + 1, o[1], o[2]]);
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
    let gates: Vec<P> = tiles.iter().flat_map(|t| [[t.frame[0], 3, ZG], [t.frame[0], 1, ZG], [t.frame[0] + 2, 1, ZG]]).collect();
    write_at_rest(d, out, &gates, &[[0, 1, 2], [6, 1, 2]])
}

/// Write a design at rest: settled on redsim without its gates (from blank states every gate would
/// fire once at placement and toggle its bulb), then the gates put back, off. The settled states
/// drop dust shapes: the dusts in `crosses` stay crosses (a button's dust powering a bulb).
pub(crate) fn write_at_rest(mut d: mctas::design::Design, out: &Path, gates: &[P], crosses: &[P]) -> Result<()> {
    let mut pre = d.clone();
    for g in gates {
        pre.blocks.remove(g);
    }
    let tmp = out.with_extension("pre.txt");
    std::fs::write(&tmp, pre.to_text())?;
    std::fs::write(out.with_extension("raw.txt"), d.to_text())?;
    crate::settle::run(&tmp, &tmp, 200)?;
    let settled = mctas::design::Design::load(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    for (p, s) in &settled.blocks {
        d.blocks.insert(*p, s.clone());
    }
    for p in crosses {
        d.blocks.insert(*p, "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into());
    }
    d.block_entities = settled.block_entities.clone();
    std::fs::write(out, d.to_text())?;
    println!("wrote {} ({} blocks)", out.display(), d.blocks.len());
    Ok(())
}


/// Primitive 4 (generator v2's tap): button a's toggle level runs down a lane; an observer beside
/// the lane gives one pulse per press, a delay-2 repeater stretches it to 4 gt into a dust, which
/// is the rear of a subtract comparator G whose side is a lever's repeater (15 blocks it).
pub fn prim4(out: &Path) -> Result<()> {
    let mut b = B::default();
    b.set([0, 1, 0], "stone_button[face=wall,facing=north,powered=false]");
    b.stone([0, 1, 1]);
    b.set([0, 1, 2], "redstone_wire[east=side,north=side,south=side,west=side]");
    b.needs.push([0, 1, 2]);
    b.set([0, 1, 3], "waxed_copper_bulb[lit=false,powered=false]");
    b.comp([0, 1, 4], D::S, false);
    for z in 5..12 {
        b.dust([0, 1, z]);
    }
    // the observer watches the lane cell (0,1,9) from the east, output east
    b.set([1, 1, 9], "observer[facing=west,powered=false]");
    b.stone([2, 1, 9]);
    b.rep([3, 1, 9], D::E, 2);
    b.dust([4, 1, 9]);
    b.comp([5, 1, 9], D::E, true);
    b.set([5, 1, 7], "lever[face=floor,facing=north,powered=false]");
    b.needs.push([5, 1, 7]);
    b.rep([5, 1, 8], D::S, 1);
    b.set([6, 1, 9], "redstone_lamp[lit=false]");
    b.supports();
    b.port("input.a", [0, 1, 0]);
    b.port("input.l", [5, 1, 7]);
    b.port("output", [5, 1, 9]);
    let mut d = b.design("prim4", None);
    d.meta.insert("port input.a".into(), "button".into());
    d.meta.insert("port input.l".into(), "lever".into());
    d.meta.insert("stimulus".into(), "a pulse 1 every 40 x 4 from 20; l 95+ 135-".into());
    std::fs::write(out, d.to_text())?;
    Ok(())
}
