//! `mctas door cycle <design>`: flip a design's lever N times in redsim (the exact model) at the
//! judge's 4 positions and check that it alternates at EVERY flip between an open state (the 3x3
//! opening clear, the frame 12/12 with the door block) and a closed one (the 9 door cells filled,
//! nothing moving), starting from whichever the rest state is, and that some closed state repeats
//! an earlier closed one (vjeux: internal state may drift as long as the door works repeatedly).
//! Unlike `door verify`, it accepts a door that rests OPEN (its lever's rest polarity is whatever
//! the design says), so a community design read from a screenshot of its open state can be
//! checked as built. Prints both states. `--vary x,y=a|b|...` (as in `door ideal-rs`) runs every
//! combination of alternatives for the uncertain cells of a transcription and prints the ones
//! that cycle.

use crate::DoorSpec;
use crate::cosearch::{RsDoor, Snap, flip_states};
use crate::sheet::frame_cells;
use anyhow::{Result, bail};
use mctas::sim::Pos;
use redsim::Block;
use std::path::Path;

fn draw(s: &Snap, z: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<String> {
    let mut out = Vec::new();
    for y in (y0..=y1).rev() {
        let mut line = format!("{y:>3} ");
        for x in x0..=x1 {
            let c = match s.get([x, y, z]) {
                None => ' ',
                Some(b) => crate::sheet::glyph_of(&b),
            };
            line.push(c);
        }
        out.push(line);
    }
    out
}

/// One position's flip sequence (`states[k]` = the settled state after k flips, `states[0]` the
/// rest state): whether it rests open, and its problems (empty = it cycles). Every state must
/// have the parity's kind (open / closed); the first wrong flip is reported.
fn position_problems(states: &[Snap], spec: &DoorSpec, off: Pos) -> (bool, Vec<String>) {
    let cells = spec.door_cells();
    let frame = frame_cells(spec.door);
    let mv = |p: Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
    let filled = |s: &Snap| cells.iter().all(|c| s.get(mv(*c)).is_some_and(|b| !b.is_air() && !crate::cosearch::is_moving(&b)));
    let clear = |s: &Snap| cells.iter().all(|c| s.get(mv(*c)).is_some_and(|b: Block| b.is_air()));
    let rest_open = clear(&states[0]);
    let closed_first = if rest_open { 1 } else { 0 };
    let door_block = states.get(closed_first).and_then(|s| s.get(mv(cells[0])));
    let mut problems = Vec::new();
    for (k, s) in states.iter().enumerate() {
        let want_open = (k % 2 == 0) == rest_open;
        if want_open {
            if !clear(s) {
                problems.push(format!("after flip {k}: the opening is not clear"));
                break;
            }
            let fr = door_block.map_or(0, |db| frame.iter().filter(|p| s.get(mv(**p)).is_some_and(|b| crate::sheet::filled_frame(&b, &db))).count());
            if fr != 12 {
                problems.push(format!("after flip {k}: frame {fr}/12 when open (frame cells must hold the door block)"));
                break;
            }
        } else if !filled(s) {
            problems.push(format!("after flip {k}: the 9 door cells are not filled"));
            break;
        }
    }
    let closed: Vec<&Snap> = states.iter().skip(closed_first).step_by(2).collect();
    let periodic = (1..closed.len()).any(|k| (0..k).any(|j| closed[j] == closed[k]));
    if problems.is_empty() && !periodic {
        problems.push(format!("no closed state repeats within {} flips (internal state drifts)", states.len() - 1));
    }
    (rest_open, problems)
}

/// Every --vary combination: (label, edits) with an edit `(cell, None)` = air.
fn variants(vary: &[String]) -> Result<Vec<(String, Vec<(Pos, Option<String>)>)>> {
    let mut out: Vec<(Vec<String>, Vec<(Pos, Option<String>)>)> = vec![(Vec::new(), Vec::new())];
    for v in vary {
        let (p, alts) = crate::ideal_rs::parse_vary(v)?;
        let mut next = Vec::new();
        for (label, edits) in &out {
            for a in &alts {
                let mut l2 = label.clone();
                let mut e2 = edits.clone();
                match a {
                    None => l2.push(format!("{},{}=keep", p[0], p[1])),
                    Some(st) => {
                        l2.push(format!("{},{}={}", p[0], p[1], st.as_deref().unwrap_or("air").trim_start_matches("minecraft:")));
                        e2.push((p, st.clone()));
                    }
                }
                next.push((l2, e2));
            }
        }
        out = next;
    }
    Ok(out.into_iter().map(|(l, e)| (l.join(" "), e)).collect())
}

pub fn cli(path: &Path, flips: usize, write_state: Option<usize>, out: Option<&Path>, vary: &[String], threads: usize, ok_only: bool) -> Result<()> {
    let (d, door) = crate::cosearch::load_door(path)?;
    let offsets = crate::VerifyOptions::default().offsets;
    if !vary.is_empty() {
        let vs = variants(vary)?;
        let results = crate::cosearch::par_map(&vs, threads, |(label, edits)| {
            let mut d2 = d.clone();
            for (p, st) in edits {
                match st {
                    None => {
                        d2.blocks.remove(p);
                    }
                    Some(s) => {
                        d2.blocks.insert(*p, s.clone());
                    }
                }
            }
            let door2 = RsDoor::from_design(&d2)?;
            for off in &offsets {
                let states = flip_states(&door2, *off, flips).map_err(|e| format!("at {off:?}: {e}"))?;
                let (_, pr) = position_problems(&states, &door2.spec, *off);
                if !pr.is_empty() {
                    return Err(format!("at {off:?}: {}", pr.join("; ")));
                }
            }
            Ok(())
        });
        let mut n = 0;
        for ((label, _), r) in vs.iter().zip(&results) {
            match r {
                Ok(()) => {
                    n += 1;
                    println!("{label}: CYCLES");
                }
                Err(e) if !ok_only => println!("{label}: {e}"),
                Err(_) => {}
            }
        }
        println!("{n} of {} variants cycle at the judge's {} positions over {flips} flips", vs.len(), offsets.len());
        return Ok(());
    }
    if let (Some(k), Some(o)) = (write_state, out) {
        // The settled state after k flips (position 0) as a design: e.g. the CLOSED state of a
        // door read from a screenshot of its open state, to judge it with `door verify`.
        let states = flip_states(&door, [0, 0, 0], k).map_err(|e| anyhow::anyhow!(e))?;
        let s = &states[k];
        let mut d2 = d.clone();
        d2.blocks.clear();
        for (p, b) in s.iter() {
            if !b.is_air() {
                d2.blocks.insert(p, b.to_string());
            }
        }
        d2.name = format!("{}-after-{k}-flips", d.name);
        d2.save(o)?;
        println!("wrote the state after {k} flips to {}", o.display());
    }
    let spec = door.spec;
    let mut first: Option<Vec<Snap>> = None;
    let mut ok_all = true;
    for (oi, off) in offsets.iter().enumerate() {
        let states = match flip_states(&door, *off, flips) {
            Ok(s) => s,
            Err(e) => {
                println!("position {oi}: BROKEN: {e}");
                ok_all = false;
                continue;
            }
        };
        let (rest_open, problems) = position_problems(&states, &spec, *off);
        if problems.is_empty() {
            println!(
                "position {oi} {off:?}: CYCLES over {flips} flips (rest = {}, frame 12/12 when open)",
                if rest_open { "open" } else { "closed" }
            );
        } else {
            ok_all = false;
            println!("position {oi} {off:?}: FAILS: {}", problems.join("; "));
        }
        if first.is_none() {
            first = Some(states);
        }
    }
    if let Some(states) = first {
        let z = spec.door[2];
        let (lo, hi) = d.bounds().map(|(a, b)| (a, b)).unwrap_or((spec.door, spec.door));
        let (x0, y0, x1, y1) = (lo[0] - 1, lo[1] - 1, hi[0] + 1, hi[1] + 1);
        let a = draw(&states[0], z, x0, y0, x1, y1);
        let b = draw(states.get(1).unwrap_or(&states[0]), z, x0, y0, x1, y1);
        let w = a.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        println!("{:<w$}    after flip 1", "rest");
        for (l, r) in a.iter().zip(b.iter()) {
            println!("{l:<w$}    {r}");
        }
    }
    if !ok_all {
        bail!("does not cycle at every position");
    }
    Ok(())
}
