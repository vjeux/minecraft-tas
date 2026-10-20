//! `mctas circuit pext cascade --n N`: generate a CASCADE extender of reach N (see NOTES.md).
//!
//! Machine (line y=1, z=0, travel +x): A (sticky) at x=1; unit k = 1..N-1 is a slime at x=2k, a
//! sticky piston at 2k+1 and an observer under the slime facing down (a moved unit fires its own
//! piston 2 gt after it lands); the payload (gold) at x=2N; target x=3N.
//!
//! Round starts: the pulse-toggle train T_N runs as N rounds (P_N-1 first, A last); round j starts
//! with piston j at its rest cell r_j = 2j+1 on the on-edge and at its extended cell e_j = 3j+1 on
//! the off-edge. Every such cell gets ONE static pulse (both edges): a pulser above-beside the line
//! at (x, 2, 1) facing south (quasi-connectivity, diagonal to the slime cells so never dragged),
//! fired by a trigger at (x, 2, 2) facing up that watches the spine node above it. The spine is a
//! westward observer chain at y=3, z=2 starting at the lever (x = e_N-1, lever at (x, 3, 3) on a
//! stone); where a pulse must come later than the chain's 2 gt per cell, a tower (2h+1 hops instead
//! of 1, +4h gt) is inserted between two neighbouring spine nodes, alternating up (y>3) and back
//! (z>2) so consecutive towers never share a column. A's pulser sits on A's top, fed through two
//! observers from the spine node at x=1. Required gaps: consecutive round starts of each edge at
//! least `gap` gt apart (6: a retracting piston's base is a moving block for 2 gt) and A after
//! both edges' round 1.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use mctas::design::Design;

pub type Pos = [i32; 3];

pub struct CascadeOptions {
    pub n: usize,
    /// Minimum gap between consecutive round starts of one edge (gt).
    pub gap: i32,
}

const OBS: &str = "minecraft:observer";

fn obs(f: &str) -> String {
    format!("{OBS}[facing={f}]")
}

/// The pulse cells (x >= 2) with the rounds they start: (on-edge round index, off-edge round index).
fn pulse_cells(n: usize) -> BTreeMap<i32, (Option<usize>, Option<usize>)> {
    let mut m: BTreeMap<i32, (Option<usize>, Option<usize>)> = BTreeMap::new();
    for j in 1..n {
        m.entry(2 * j as i32 + 1).or_default().0 = Some(j);
        m.entry(3 * j as i32 + 1).or_default().1 = Some(j);
    }
    m
}

pub fn generate(o: &CascadeOptions) -> Result<(Design, Vec<String>)> {
    let n = o.n;
    if n < 2 {
        bail!("--n must be at least 2");
    }
    let ni = n as i32;
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    let mut put = |b: &mut BTreeMap<Pos, String>, p: Pos, s: String| -> Result<()> {
        if let Some(old) = b.insert(p, s.clone()) {
            bail!("generator collision at {p:?}: {old} vs {s}");
        }
        Ok(())
    };
    // machine
    put(&mut b, [1, 1, 0], "minecraft:sticky_piston[facing=east]".into())?;
    for k in 1..ni {
        put(&mut b, [2 * k, 1, 0], "minecraft:slime_block".into())?;
        put(&mut b, [2 * k + 1, 1, 0], "minecraft:sticky_piston[facing=east]".into())?;
        put(&mut b, [2 * k, 0, 0], obs("down"))?;
    }
    put(&mut b, [2 * ni, 1, 0], "minecraft:gold_block".into())?;
    // lever + its stone
    let xs = 3 * (ni - 1) + 1;
    put(&mut b, [xs, 2, 3], "minecraft:stone".into())?;
    let lever = [xs, 3, 3];
    put(&mut b, lever, "minecraft:lever[face=floor,facing=north]".into())?;
    // schedule
    let cells = pulse_cells(n);
    let mut t_on: BTreeMap<usize, i32> = BTreeMap::new(); // round j -> pulse time on the on-edge
    let mut t_off: BTreeMap<usize, i32> = BTreeMap::new();
    let mut s: BTreeMap<i32, i32> = BTreeMap::new(); // spine node time per x
    let mut log = Vec::new();
    s.insert(xs, 2);
    put(&mut b, [xs, 3, 2], obs("south"))?;
    if let Some((on, off)) = cells.get(&xs) {
        put(&mut b, [xs, 2, 2], obs("up"))?;
        put(&mut b, [xs, 2, 1], obs("south"))?;
        if let Some(j) = on {
            t_on.insert(*j, 6);
        }
        if let Some(j) = off {
            t_off.insert(*j, 6);
        }
        log.push(format!("x{xs} at t6 (off-edge round of piston {})", n - 1));
    }
    let mut last_tower: Option<(i32, bool)> = None; // (x of the pair's high side, up?)
    for x in (1..xs).rev() {
        let base = s[&(x + 1)] + 2;
        // required spine time at x
        let mut req = base;
        let lead = if x == 1 { 6 } else { 4 };
        if x == 1 {
            let a = t_on.get(&1).copied().unwrap_or(0).max(t_off.get(&1).copied().unwrap_or(0)) + o.gap;
            req = req.max(a - lead);
        } else if let Some((on, off)) = cells.get(&x) {
            if let Some(j) = on
                && let Some(t) = t_on.get(&(j + 1))
            {
                req = req.max(t + o.gap - lead);
            }
            if let Some(j) = off
                && let Some(t) = t_off.get(&(j + 1))
            {
                req = req.max(t + o.gap - lead);
            }
        }
        let h = (req - base + 3).max(0) / 4;
        let sx = base + 4 * h;
        s.insert(x, sx);
        if h == 0 {
            put(&mut b, [x, 3, 2], obs("east"))?;
        } else {
            // a tower between x+1 and x; alternate planes for consecutive towers, never back at the
            // lever
            let up = match last_tower {
                Some((hx, up)) if hx == x + 2 => !up,
                _ => true,
            } || x + 1 == xs;
            if up {
                for i in 1..=h {
                    put(&mut b, [x + 1, 3 + i, 2], obs("down"))?;
                }
                put(&mut b, [x, 3 + h, 2], obs("east"))?;
                for i in (0..h).rev() {
                    put(&mut b, [x, 3 + i, 2], obs("up"))?;
                }
            } else {
                for i in 1..=h {
                    put(&mut b, [x + 1, 3, 2 + i], obs("north"))?;
                }
                put(&mut b, [x, 3, 2 + h], obs("east"))?;
                for i in (0..h).rev() {
                    put(&mut b, [x, 3, 2 + i], obs("south"))?;
                }
            }
            last_tower = Some((x + 1, up));
        }
        let t = sx + lead;
        if x == 1 {
            put(&mut b, [1, 3, 1], obs("south"))?;
            put(&mut b, [1, 3, 0], obs("south"))?;
            put(&mut b, [1, 2, 0], obs("up"))?;
            log.push(format!("A at t{t}"));
        } else if let Some((on, off)) = cells.get(&x) {
            put(&mut b, [x, 2, 2], obs("up"))?;
            put(&mut b, [x, 2, 1], obs("south"))?;
            if let Some(j) = on {
                t_on.insert(*j, t);
            }
            if let Some(j) = off {
                t_off.insert(*j, t);
            }
            log.push(format!(
                "x{x} at t{t}{}{}{}",
                on.map(|j| format!(" (on-edge round of piston {j})")).unwrap_or_default(),
                off.map(|j| format!(" (off-edge round of piston {j})")).unwrap_or_default(),
                if h > 0 { format!(", tower +{}", 4 * h) } else { String::new() }
            ));
        }
    }
    let mut d = Design { name: format!("pext-n{n}-cascade-gen"), ..Default::default() };
    d.meta.insert("circuit".into(), format!("piston-extender {n}"));
    d.meta.insert("travel".into(), "1 0 0".into());
    d.meta.insert("port input".into(), "lever".into());
    d.ports.insert("input".into(), lever);
    d.ports.insert("payload".into(), [2 * ni, 1, 0]);
    d.blocks = b;
    Ok((d, log))
}
