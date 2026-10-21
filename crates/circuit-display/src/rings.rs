//! Three ring stacks feeding the display face (`mctas circuit display rings`, FACE-NOTES plan 1).
//! Each segment is one analog cross ring (astack: four windows W at distance 2 lighting four
//! stones round a centre, constants in hoppers, s from radius-3 arcs bridged one level down), set
//! ONE LEVEL ABOVE its lamp. The centre holds dust (the OR of its four stones); its output is taken
//! under the front (north) axis: the dust powers its support block, a repeater reads that block and
//! powers the north W's support block, a second repeater (where the north bridge was) reads it and
//! starts the lane to the lamp at the lamp's own height. Column x = 1 stacks a, g, d (rings at
//! 5, 3, 1) straight behind their lamps; the f/e and b/c stacks stand beside it and reach their
//! lamps along the face plane. s comes from the thermometer's chain (astack::build_chain, code
//! neg 1111) behind the stacks, by lossless lines into one back arc cell of each top ring; it
//! flows down each stack through the rings' bridges (astack::coupling).

use crate::abuild::{B, D, P, add};
use crate::analog::{Code, masks, runs};
use crate::astack::{build_ring, coupling, solve_ring_f};
use crate::therm::{chain_back, walk};
use anyhow::{Result, anyhow, ensure};

/// One stack: centre x, the rings' segments top down with their lamp rows, the top ring's feed
/// cell (ring-relative), and how its lanes reach the lamps (0: straight, -1/+1: along the face
/// plane toward -x / +x).
struct Stack {
    cx: i32,
    rings: Vec<(char, i32, i32)>,
    feed: (i32, i32, i32),
    turn: i32,
}

pub fn build(code: &Code, allowed: u8, bridges_off: u8, lane: i32, out: &std::path::Path) -> Result<()> {
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    // the rings' front arcs at z = 2: an arc next to a lamp (z = 1) would take the lamp's power
    let cz = 5;
    let mut b = B::default();
    let split = chain_back(&mut b, [-7, 2, 18], code);
    // every ring is fed at its slot-S O contact (ring-relative (-1, 3): distance 0 there) by a
    // comparator at (cx - 1, level, 8) whose rear block is reached by lossless lines from the split
    let s2 = walk(&mut b, split, "N");
    let s3 = walk(&mut b, s2, "N");
    let f_rear = walk(&mut b, s2, "E E E E N N N W");
    let b_rear = walk(&mut b, s3, "W W W W W W N N E");
    let a_rear = walk(&mut b, s3, "uN N W");
    // descents: two levels down per "dS dE N W"
    // descents: a step sideways first, so the last block does not sit under the first dust (the
    // dust above a strongly powered block reads it: the descent would feed itself and latch)
    let g_rear = walk(&mut b, a_rear, "W dS dE N");
    let d_rear = walk(&mut b, g_rear, "W dS dE N");
    let e_rear = walk(&mut b, f_rear, "W dS dE N");
    let c_rear = walk(&mut b, b_rear, "E dS dW N");
    for (rear, want) in [(a_rear, [0, 5, 10]), (g_rear, [0, 3, 10]), (d_rear, [0, 1, 10]), (f_rear, [8, 4, 10]), (e_rear, [8, 2, 10]), (b_rear, [-8, 4, 10]), (c_rear, [-8, 2, 10])] {
        ensure!(rear == want, "feed line ends at {rear:?}, want {want:?}");
        b.comp(add(rear, [0, 0, -1]), D::N, false);
    }
    let direct = |_seg: char, _cx: i32| -> (i32, i32, i32) { (-1, 0, 3) };
    let stacks = [
        Stack { cx: 1, rings: vec![('a', 4, 5), ('g', 2, 3), ('d', 0, 1)], feed: (-1, 0, 3), turn: 0 },
        Stack { cx: 9, rings: vec![('f', 3, 4), ('e', 1, 2)], feed: (-1, 0, 3), turn: -1 },
        Stack { cx: -7, rings: vec![('b', 3, 4), ('c', 1, 2)], feed: (-1, 0, 3), turn: 1 },
    ];
    let lamp_x = |seg: char| match seg {
        'f' | 'e' => 2,
        'b' | 'c' => 0,
        _ => 1,
    };
    for st in &stacks {
        let mut feed: Vec<((i32, i32, i32), u8)> = Vec::new();
        let _ = st.feed;
        for &(seg, row, level) in &st.rings {
            feed.push((direct(seg, st.cx), 0));
            let si = "abcdefg".find(seg).unwrap();
            let rs = runs(m[si]);
            // the north bridge (bit 3) is where the lane passes; when the direct feed is not
            // enough, a second feed cell (an arc cell or a W contact) is tried
            let mut res = solve_ring_f(&rs, &feed, allowed, bridges_off);
            if res.is_none() {
                let mut cands: Vec<(i32, i32, i32)> = Vec::new();
                for i in -3..=3 {
                    for (x, z) in [(3, i), (-3, i), (i, 3), (i, -3)] {
                        if x != 0 && z != 0 && !cands.contains(&(x, 0, z)) {
                            cands.push((x, 0, z));
                        }
                    }
                }
                cands.extend([(2, 0, -1), (-1, 0, -2), (-2, 0, 1), (1, 0, 2)]);
                // wired: the back arc cell beside the direct feed, by a dust taking s from the
                // direct feed's rear block and a second feed comparator
                cands.retain(|c| *c == (-2, 0, 3));
                let mut found = None;
                for c2 in cands {
                    let mut f2 = feed.clone();
                    f2.push((c2, 0));
                    if let Some(r) = solve_ring_f(&rs, &f2, allowed, bridges_off) {
                        println!("  {seg}: a second feed works at {c2:?}");
                        if found.is_none() {
                            found = Some((f2, r));
                        }
                    }
                }
                if let Some((f2, r)) = found {
                    feed = f2;
                    res = Some(r);
                }
            }
            let (slots, mask) = res.ok_or_else(|| anyhow!("no ring for {seg} {rs:?} with feed {feed:?}"))?;
            if feed.iter().any(|(c, d)| *c == (-2, 0, 3) && *d == 0) {
                b.dust([st.cx - 2, level, cz + 5]);
                b.comp([st.cx - 2, level, cz + 4], D::N, false);
            }
            let c = [st.cx, level, cz];
            build_ring(&mut b, c, &slots, &feed, mask);
            b.dust(c);
            let y = level - 1;
            ensure!(y == row, "ring {seg} at {level}, lamp row {row}");
            // the lane: `lane` cells east of the ring's axis
            let lane_x = st.cx + lane;
            if lane != 0 {
                b.rep([st.cx + lane.signum(), y, cz], if lane > 0 { D::E } else { D::W }, 1);
                let mut x = st.cx + 2 * lane.signum();
                while x != lane_x {
                    b.rep([x, y, cz], if lane > 0 { D::E } else { D::W }, 1);
                    x += lane.signum();
                }
            }
            b.rep([lane_x, y, cz - 1], D::N, 1);
            b.stone([lane_x, y, cz - 2]);
            b.rep([lane_x, y, cz - 3], D::N, 1);
            for z in 1..cz - 3 {
                b.rep([lane_x, y, z], D::N, 1);
            }
            let lx = lamp_x(seg);
            if st.turn != 0 {
                // the lane turns in the face plane: a block, then repeaters toward the lamp
                b.rep([lane_x, y, 1], D::N, 1);
                b.stone([lane_x, y, 0]);
                let dir = if st.turn < 0 { D::W } else { D::E };
                let mut x = lane_x + st.turn;
                while x != lx {
                    b.rep([x, y, 0], dir, 1);
                    x += st.turn;
                }
            } else {
                ensure!(lane_x == lx, "straight lane off its lamp");
            }
            b.port(&format!("output.{seg}"), [lx, row, 0]);
            // every ring has its own feed line: what this ring couples into the one below is
            // IGNORED (a second source into the same dust net can latch a bridge the wrong way).
            let _ = coupling(&slots, &feed, mask);
            feed.clear();
        }
    }
    let bad = crate::agen::bad_supports(&b);
    let d = b.design("seven-segment-rings", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: {} blocks; bad supports {:?}", out.display(), d.blocks.len(), bad);
    let _: P = [0, 0, 0];
    Ok(())
}
