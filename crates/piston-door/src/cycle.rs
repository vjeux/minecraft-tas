//! `mctas door cycle <design>`: flip a design's lever N times in redsim (the exact model) at the
//! judge's 4 positions and check that it alternates between two states: the state after every
//! even flip equals the rest state, the state after every odd flip equals the first flip's, and
//! one of the two has the 3x3 opening clear with the frame 12/12 while the other has the 9 door
//! cells filled. Unlike `door verify`, it accepts a door that rests OPEN (its lever's rest
//! polarity is whatever the design says), so a community design read from a screenshot of its
//! open state can be checked as built. Prints both states.

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

pub fn cli(path: &Path, flips: usize, write_state: Option<usize>, out: Option<&Path>) -> Result<()> {
    let (d, door) = crate::cosearch::load_door(path)?;
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
    let cells = spec.door_cells();
    let frame = frame_cells(spec.door);
    let offsets = crate::VerifyOptions::default().offsets;
    let mut first: Option<Vec<Snap>> = None;
    let mut ok_all = true;
    for (oi, off) in offsets.iter().enumerate() {
        let mv = |p: Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        let states = match flip_states(&door, *off, flips) {
            Ok(s) => s,
            Err(e) => {
                println!("position {oi}: BROKEN: {e}");
                ok_all = false;
                continue;
            }
        };
        let filled = |s: &Snap| cells.iter().all(|c| s.get(mv(*c)).is_some_and(|b| !b.is_air() && !crate::cosearch::is_moving(&b)));
        let clear = |s: &Snap| cells.iter().all(|c| s.get(mv(*c)).is_some_and(|b: Block| b.is_air()));
        let frame_n = |s: &Snap| frame.iter().filter(|p| s.get(mv(**p)).is_some_and(|b| crate::sheet::filled_frame(&b))).count();
        let mut problems = Vec::new();
        for k in 2..states.len() {
            if states[k] != states[k - 2] {
                problems.push(format!("the state after flip {k} differs from the state after flip {}", k - 2));
            }
        }
        let (a, b) = (&states[0], states.get(1).unwrap_or(&states[0]));
        let (open, closed) = if clear(a) { (a, b) } else { (b, a) };
        if !clear(open) {
            problems.push("neither state has the opening clear".into());
        }
        if !filled(closed) {
            problems.push("neither state has the 9 door cells filled".into());
        }
        let fr = frame_n(open);
        if fr != 12 {
            problems.push(format!("frame {fr}/12 when open"));
        }
        if problems.is_empty() {
            println!(
                "position {oi} {off:?}: CYCLES over {flips} flips (rest = {}, frame 12/12 when open)",
                if clear(a) { "open" } else { "closed" }
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
