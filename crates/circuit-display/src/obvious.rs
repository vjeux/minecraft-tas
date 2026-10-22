//! `mctas circuit display obvious` (clk-obs, 2026-10-21): THE OBVIOUS 7-segment display for one 0–15
//! signal strength (designs/briefs/display-7seg.md, layout 1, gate G2): decode every value, OR the
//! decoded values per segment, light each corner from its segments.
//!
//! - Level chain (y = 0, z = ZC): the chest, then compare comparators and stones along +x; each
//!   column's stone G holds the level L and a dust on it (the contact) carries L exactly.
//! - B unit: a subtract comparator facing west beside the contact, a hopper of constant k behind it:
//!   its output block O_k = k - L > 0 iff L < k.
//! - One-hot decoder per value: E_0 = O_1; E_v (1..14) = a subtract comparator W reading O_{v+1} (north
//!   B) on its rear and O_v (south B, through two repeaters over the chain) on its side; E_15 = a
//!   torch on O_15. Each E_v leaves its column northward (-z) at full strength.
//! - ROM: the E lines cross seven collectors (one per segment, along x, flowing west). Where the
//!   segment shows v, a repeater in the E line powers a stone that the collector's dust stands on;
//!   elsewhere the collector bridges over the line on a stone.
//! - Face: each collector's lane ends in a socket stone, a repeater from it strongly powers the
//!   segment's stone two cells behind its lamp; a dust cross in front of it lights the lamp and the
//!   corner stones beside it; a dust cross in each corner cell beside the segment stones reads their
//!   strong power (the OR of its segments) and powers the corner's stone (crate::corners).

use crate::abuild::{B, D, P};
use crate::route::{Cell, Kind, Lane};
use anyhow::{Result, anyhow};
use circuit::compute::{CORNERS, FACE, INTERIOR, SEGMENTS};

const ZC: i32 = 40;
const PITCH: i32 = 6;
const CROSS: &str = "redstone_wire[east=side,north=side,south=side,west=side]";

fn bunit(b: &mut B, x: i32, south: bool, k: u8) -> P {
    let z = if south { ZC + 1 } else { ZC - 1 };
    b.comp([x, 1, z], D::W, true);
    b.hopper([x + 1, 1, z], k);
    let o = [x - 1, 1, z];
    b.stone(o);
    o
}

pub fn build(name: &str, out: &std::path::Path, expand: usize) -> Result<()> {
    let mut b = B::default();
    // the chain and the 16 columns (value v at G = X0 + PITCH * v)
    let x0 = 0;
    let chest = [x0 - 3, 0, ZC];
    b.set(chest, "chest[facing=north,type=single,waterlogged=false]");
    b.port("input.a", chest);
    b.comp([x0 - 2, 0, ZC], D::E, false);
    let mut prev = x0 - 2;
    let mut eline: Vec<i32> = Vec::new();
    for v in 0..16u8 {
        let g = x0 + PITCH * v as i32;
        for x in prev + 1..g {
            if (g - x) % 2 == 1 {
                b.comp([x, 0, ZC], D::E, false);
            } else {
                b.stone([x, 0, ZC]);
            }
        }
        b.stone([g, 0, ZC]);
        b.set([g, 1, ZC], CROSS);
        b.needs.push([g, 1, ZC]);
        prev = g;
        let xe = g - 1;
        // debugging: OBVIOUS_ONLY=v[,w...] builds only those values' columns (the chain stays whole)
        let only: Option<Vec<u8>> = std::env::var("OBVIOUS_ONLY").ok().map(|s| s.split(',').filter_map(|w| w.parse().ok()).collect());
        if only.as_ref().is_some_and(|o| !o.contains(&v)) {
            eline.push(xe);
            continue;
        }
        match v {
            0 => {
                bunit(&mut b, g, false, 1);
                b.rep([xe, 1, ZC - 2], D::N, 1);
                b.dust([xe, 1, ZC - 3]);
            }
            15 => {
                bunit(&mut b, g, false, 15);
                b.set([xe, 1, ZC - 2], "redstone_wall_torch[facing=north,lit=true]");
                b.rep([xe, 1, ZC - 3], D::N, 1);
            }
            _ => {
                bunit(&mut b, g, false, v + 1);
                let os = bunit(&mut b, g, true, v);
                b.comp([xe, 1, ZC - 2], D::N, true);
                b.stone([xe, 1, ZC - 3]);
                b.rep([os[0] - 1, 1, ZC + 1], D::W, 1);
                b.dust([g - 3, 1, ZC + 1]);
                b.stone([g - 3, 1, ZC]);
                b.dust([g - 3, 2, ZC]);
                b.dust([g - 3, 1, ZC - 1]);
                b.dust([g - 3, 1, ZC - 2]);
                b.rep([g - 2, 1, ZC - 2], D::E, 1);
            }
        }
        // full strength northward from z = ZC - 4
        if v != 0 && v != 15 {
            b.rep([xe, 1, ZC - 4], D::N, 1);
        } else {
            b.dust([xe, 1, ZC - 4]);
        }
        eline.push(xe);
    }
    // the ROM: collector j (segment "abcdefg"[j]) at z = ZC - 6 - 2j; E lines from z = ZC - 5
    let segs = ['a', 'b', 'c', 'd', 'e', 'f', 'g'];
    let zc = |j: usize| ZC - 6 - 3 * j as i32;
    let xw = eline[0] - 3;
    let xe_max = *eline.last().unwrap();
    let only: Option<Vec<usize>> = std::env::var("OBVIOUS_ONLY").ok().map(|s| s.split(',').filter_map(|w| w.parse().ok()).collect());
    for (v, &xe) in eline.iter().enumerate() {
        if only.as_ref().is_some_and(|o| !o.contains(&v)) {
            continue;
        }
        b.dust([xe, 1, ZC - 5]);
        for (j, s) in segs.iter().enumerate() {
            let z = zc(j);
            // three cells per crossing: [z + 2] dust (it ignores the weak power a collector's dust
            // puts into the tap stone before it), [z + 1] the tap's repeater or dust, [z] the crossing
            if j > 0 {
                b.dust([xe, 1, z + 2]);
            }
            if SEGMENTS[v].contains(*s) {
                b.rep([xe, 1, z + 1], D::N, 1);
                b.stone([xe, 1, z]);
                b.dust([xe, 2, z]);
                // a cap over the E dust past the tap stone: without it the collector's dust on the
                // stone connects down to it (the diagonal step) and drives this value's line
                b.stone([xe, 2, z - 1]);
            } else {
                b.dust([xe, 1, z]);
                b.stone([xe, 2, z]);
                b.dust([xe, 3, z]);
                if !b.blocks.contains_key(&[xe, 1, z + 1]) || j > 0 {
                    b.dust([xe, 1, z + 1]);
                }
            }
        }
    }
    // the collectors between E lines: dust at h = 3 on stones, a repeater (west) every 12 cells
    for j in 0..7 {
        let z = zc(j);
        for x in xw..=xe_max + 1 {
            if eline.contains(&x) {
                continue;
            }
            b.stone([x, 2, z]);
            if (x - 2).rem_euclid(12) == 0 {
                b.rep([x, 3, z], D::W, 1);
            } else {
                b.dust([x, 3, z]);
            }
        }
        // the lane source: a repeater at the west end
        b.blocks.remove(&[xw, 3, z]);
        b.rep([xw, 3, z], D::W, 1);
    }
    // the face: lamps at z = F facing -z, col c at x = fx - c; segment stones at z = F + 2
    let (fx, fy, fz) = (xw - 8, 3, ZC - 6 - 21 - 10);
    let at = |col: i32, row: i32, out: i32| -> P { [fx - col, fy + 4 - row, fz + out] };
    let mut lanes: Vec<Lane> = Vec::new();
    for (j, s) in segs.iter().enumerate() {
        let (_, col, row) = FACE.iter().find(|(n, _, _)| n.starts_with(*s)).copied().unwrap();
        b.stone(at(col, row, 2));
        b.set(at(col, row, 1), CROSS);
        b.needs.push(at(col, row, 1));
        b.rep(at(col, row, 3), D::N, 1);
        let socket = at(col, row, 4);
        // the socket dust's support (the dust itself goes in after routing: the router aims at a
        // lamp-like cell, and a dust reads both a lane pointing into it and a powered block beside it)
        b.stone([socket[0], socket[1] - 1, socket[2]]);
        // walled on every side but the back (+z), so no other lane can run beside its dust
        for p in [[socket[0] - 1, socket[1], socket[2]], [socket[0] + 1, socket[1], socket[2]], [socket[0], socket[1] + 1, socket[2]]] {
            b.blocks.entry(p).or_insert_with(|| "minecraft:stone".into());
        }
        lanes.push(Lane { src: [xw, 3, zc(j)], src_dir: D::W, lamp: socket });
    }
    for (_, col, row) in CORNERS {
        b.stone(at(col, row, 1));
        b.set(at(col, row, 2), CROSS);
        b.needs.push(at(col, row, 2));
    }
    for (col, row) in INTERIOR {
        b.stone(at(col, row, 1));
    }
    b.stone(at(1, 5, 1));
    // the face repeaters' side cells (a lane's repeater there would lock them)
    for (col, row) in [(0, 0), (2, 0), (0, 2), (2, 2), (0, 4), (2, 4), (1, 1), (1, 3), (-1, 1), (3, 1), (-1, 3), (3, 3)] {
        b.blocks.entry(at(col, row, 3)).or_insert_with(|| "minecraft:stone".into());
    }
    for (s, col, row) in FACE.iter().chain(CORNERS.iter()) {
        b.port(&format!("output.{s}"), at(*col, *row, 0));
    }
    if std::env::var("OBVIOUS_PROBE").is_ok() {
        for (v, &xe) in eline.iter().enumerate() {
            b.port(&format!("output.probeE{v:02}"), [xe, 1, ZC - 5]);
            b.port(&format!("output.probeL{v:02}"), [xe + 1, 1, ZC]);
            if (7..=10).contains(&v) {
                for dx in 1..=5 {
                    b.port(&format!("output.probeC{v:02}x{dx}"), [xe + 1 + dx, 0, ZC]);
                }
                b.port(&format!("output.probeN{v:02}"), [xe, 1, ZC - 1]);
                b.port(&format!("output.probeS{v:02}"), [xe, 1, ZC + 1]);
                b.port(&format!("output.probeW{v:02}"), [xe, 1, ZC - 3]);
            }
        }
        // the collector ends (west of the lane source), the sockets and the segment stones
        for (j, s) in segs.iter().enumerate() {
            let (_, col, row) = FACE.iter().find(|(n, _, _)| n.starts_with(*s)).copied().unwrap();
            b.port(&format!("output.probe{s}1col"), [xw + 1, 3, zc(j)]);
            b.port(&format!("output.probe{s}2sock"), at(col, row, 4));
            b.port(&format!("output.probe{s}3T"), at(col, row, 2));
            b.port(&format!("output.probe{s}4rep"), at(col, row, 3));
        }
    }
    b.supports();
    // route the seven lanes; the lamps and the plane in front are obstacles
    let lo: P = [fx - 6, -2, fz - 1];
    let hi: P = [xw - 1, 8, ZC - 4];
    let base = Cell { kind: Kind::Base, lane: -1, idx: 0, support: false, source: false };
    let mut order: Vec<usize> = (0..lanes.len()).collect();
    let mut rng: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut routed = None;
    for attempt in 0..60 {
        let mut w = crate::route::world_from(&b, &lanes, lo, hi);
        for (s, col, row) in FACE.iter().chain(CORNERS.iter()) {
            let _ = s;
            w.cells.insert(at(*col, *row, 0), base);
        }
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                w.cells.entry([x, y, fz - 1]).or_insert(base);
            }
        }
        match crate::route::route_all(&mut w, &lanes, &order, expand) {
            Ok(n) => {
                routed = Some((w, n));
                break;
            }
            Err(e) => {
                eprintln!("order {attempt}: {e}");
                for i in (1..order.len()).rev() {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    order.swap(i, (rng % (i as u64 + 1)) as usize);
                }
            }
        }
    }
    let (w, n) = routed.ok_or_else(|| anyhow!("no routing order routes the seven lanes"))?;
    crate::route::write_lanes(&w, &mut b);
    for l in &lanes {
        b.blocks.insert(l.lamp, format!("minecraft:{CROSS}"));
    }
    let mut d = b.design(name, Some("seven-segment 4 hex"));
    d.meta.insert("port input.a".into(), "strength".into());
    std::fs::write(out, format!("# {name} (clk-obs, 2026-10-21): the obvious display, designs/briefs/display-7seg.md layout 1 (`mctas circuit display obvious`)\n{}", d.to_text()))?;
    println!("{}: {} blocks ({n} lane blocks)", out.display(), d.blocks.len());
    Ok(())
}
