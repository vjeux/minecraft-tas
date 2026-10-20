//! `mctas circuit fly gen ...`: generators for extendable families (the `extend generator:` of
//! their base design builds any member).
//!
//! * `payload --base <design> --from x,y,z --dir up --n N [--cap C --from2 x,y,z --dir2 down]
//!   --out f`: the base machine carrying N payload blocks: a column (slime, so each drags the
//!   next) growing from the cell next to `from` in `dir`, at most `cap` blocks, the rest in a
//!   second column from `from2` in `dir2` (on the other part); the top block of each column is
//!   `--block`, the last one `port payload`. A part pushes at most 12 blocks, so a 3-block part
//!   carries at most 9.

use crate::machine::Machine;
use anyhow::{Result, bail};
use clock::Pos;
use std::path::Path;

fn dir_vec(d: &str) -> Result<Pos> {
    Ok(match d {
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => bail!("--dir up|down|north|south|west|east"),
    })
}

/// One payload column of `n` blocks from the cell next to `from` in `dir`; the last cell.
fn column(m: &mut Machine, from: Pos, dir: &str, n: usize, block: &str) -> Result<Option<Pos>> {
    if !m.design().blocks.contains_key(&from) {
        bail!("--from {from:?}: no machine block there");
    }
    let d = dir_vec(dir)?;
    let mut p = from;
    for i in 0..n {
        p = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        if m.design().blocks.contains_key(&p) {
            bail!("payload cell {p:?} is taken");
        }
        let state = if i + 1 == n { block.to_string() } else { "minecraft:slime_block".to_string() };
        m.clock.design.blocks.insert(p, state);
    }
    Ok((n > 0).then_some(p))
}

/// The base machine with `n` payload blocks (see the module doc).
pub fn payload(base: &Path, first: (Pos, &str), second: Option<(Pos, &str)>, cap: usize, n: usize, block: &str) -> Result<Machine> {
    let mut m = Machine::load(base)?;
    let n1 = if second.is_some() { n.min(cap) } else { n };
    let mut last = column(&mut m, first.0, first.1, n1, block)?;
    if let Some((from2, dir2)) = second
        && n > n1
    {
        last = column(&mut m, from2, dir2, n - n1, block)?;
    }
    if let Some(p) = last {
        m.clock.design.ports.insert("payload".into(), p);
    }
    m.clock.design.name = format!("{}-payload{n}", m.name());
    // re-parse so the derived fields (payload list) are current
    Machine::parse(&m.to_text())
}

/// The train family on the engine fly-horizontal-6b-10gt-e2cf52 (travel +x, rear slime at
/// 1,0,0): a sticky piston on the rear slime (1,0,1, facing back) reach-pulls trailer 1 each cycle
/// (fired with the pusher by the rear observer); trailer k = a slime at (1 - 2k, 0, 1) and, unless
/// it is the last, a sticky piston facing back right behind it and a rider observer on it (facing
/// up, firing into the slime 2 gt after the trailer lands) that pulls trailer k + 1, which starts
/// against that piston's face. Each trailer is a move of its own (12 blocks), so the reach is
/// unbounded: 5 + 3n blocks, 10 gt/block. The last trailer's slime is `port payload`.
pub fn train(base: &Path, n: usize) -> Result<Machine> {
    let mut m = Machine::load(base)?;
    let b = &mut m.clock.design.blocks;
    let need = |b: &std::collections::BTreeMap<Pos, String>, p: Pos, what: &str| -> Result<()> {
        if b.get(&p).map(|s| s.as_str()) != Some(what) {
            bail!("train: the base must be e2cf52 ({what} at {p:?})");
        }
        Ok(())
    };
    need(b, [1, 0, 0], "minecraft:slime_block")?;
    let sticky = "minecraft:sticky_piston[extended=false,facing=west]".to_string();
    let take = |b: &mut std::collections::BTreeMap<Pos, String>, p: Pos, s: String| -> Result<()> {
        if b.insert(p, s).is_some() {
            bail!("train: cell {p:?} is taken");
        }
        Ok(())
    };
    take(b, [1, 0, 1], sticky.clone())?;
    for k in 1..=n as i32 {
        let x = 1 - 2 * k;
        take(b, [x, 0, 1], "minecraft:slime_block".into())?;
        if k < n as i32 {
            take(b, [x - 1, 0, 1], sticky.clone())?;
            take(b, [x, 1, 1], "minecraft:observer[facing=up,powered=false]".into())?;
        }
    }
    if n > 0 {
        m.clock.design.ports.insert("payload".into(), [1 - 2 * n as i32, 0, 1]);
    }
    m.clock.design.name = format!("{}-train{n}", m.name());
    Machine::parse(&m.to_text())
}
