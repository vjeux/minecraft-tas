//! `mctas door rows-fire <ROWS.tsv>`: where the pistons of each strip-row mechanism FIRE.
//!
//! Replays every row mechanism of agent families' ROWS.tsv (columns id, arms, pistons, movers,
//! sym, open, close, pieces; `door families --rows`) in the same 1-D macro model as families.rs
//! (`P` = a 2 gt pulse from rest: push and spit, or, with air in front of a sticky piston, extend
//! into the gap and pull the block beyond; `L` = a level toggle: extend / retract with the sticky
//! pull) and reports, for every piston action of the open and close schedules, the cell the
//! piston stands on when it fires (schedules name pistons by their REST cell; walkers fire from
//! wherever they have been moved).
//!
//! Why it matters: in a 5-tall strip box (rows -1..3) wiring reaches a piston of row 0 from row -1
//! and a piston of row 2 from row 3, but the door columns x 0..2 of rows -1 and 3 are the floor and
//! ceiling ring (only door or plain blocks in the stable states), and row 1 is reached only by
//! quasi-connectivity through row 2's cells. So a row mechanism that fires a piston from x 0..2
//! (`inner`) cannot be wired in rows 0 and 1; row 2 can still fire at x 0 and 2 (quasi-connectivity
//! through the ceiling ring cell from the corner (-1,3) / (3,3)), never at x 1 (`x1`).
//! `--wirable` keeps the mechanisms with no inner firing at all.

use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Air,
    Door,
    Piston(usize),
    Head(usize),
}

#[derive(Clone, Copy, Debug)]
struct Pis {
    sticky: bool,
    f: i32,
    rest: i32,
    ext0: bool,
}

#[derive(Clone)]
struct Row {
    lo: i32,
    hi: i32,
    cells: Vec<Cell>,
    pos: Vec<i32>,
    pow: Vec<bool>,
}

impl Row {
    fn get(&self, x: i32) -> Option<Cell> {
        (x >= self.lo && x <= self.hi).then(|| self.cells[(x - self.lo) as usize])
    }
    fn set(&mut self, x: i32, c: Cell) {
        let i = (x - self.lo) as usize;
        self.cells[i] = c;
    }
    fn ext(&self, pis: &[Pis], id: usize) -> bool {
        self.get(self.pos[id] + pis[id].f) == Some(Cell::Head(id))
    }
    fn pushable(&self, pis: &[Pis], c: Cell) -> bool {
        match c {
            Cell::Door => true,
            Cell::Piston(id) => !self.ext(pis, id),
            _ => false,
        }
    }
    /// Ok(cells to move) / Err(true) = would leave the row box / Err(false) = blocked.
    fn push_list(&self, pis: &[Pis], start: i32, f: i32) -> std::result::Result<Vec<i32>, bool> {
        let mut list = Vec::new();
        let mut x = start;
        loop {
            let Some(c) = self.get(x) else { return Err(true) };
            if c == Cell::Air {
                return Ok(list);
            }
            if !self.pushable(pis, c) {
                return Err(false);
            }
            list.push(x);
            if list.len() > 12 {
                return Err(false);
            }
            x += f;
        }
    }
    fn move_list(&mut self, list: &[i32], f: i32) {
        for &x in list.iter().rev() {
            let c = self.get(x).unwrap();
            self.set(x + f, c);
            self.set(x, Cell::Air);
            if let Cell::Piston(id) = c {
                self.pos[id] = x + f;
            }
        }
    }
    fn pull(&mut self, pis: &[Pis], p: i32, f: i32) {
        let (one, two) = (p + f, p + 2 * f);
        let (Some(a), Some(b)) = (self.get(one), self.get(two)) else { return };
        if a != Cell::Air || b == Cell::Air || !self.pushable(pis, b) {
            return;
        }
        self.set(one, b);
        self.set(two, Cell::Air);
        if let Cell::Piston(id) = b {
            self.pos[id] = one;
        }
    }
    /// One action; Err(why) when the model refuses it (a schedule that does not replay).
    fn apply(&mut self, pis: &[Pis], id: usize, level: bool) -> Result<()> {
        let (sticky, f, p) = (pis[id].sticky, pis[id].f, self.pos[id]);
        let ext = self.ext(pis, id);
        if level {
            if self.pow[id] {
                self.pow[id] = false;
                if ext {
                    self.set(p + f, Cell::Air);
                    if sticky {
                        self.pull(pis, p, f);
                    }
                }
            } else {
                let list = self.push_list(pis, p + f, f).map_err(|_| anyhow::anyhow!("level-on blocked"))?;
                self.move_list(&list, f);
                self.set(p + f, Cell::Head(id));
                self.pow[id] = true;
            }
            return Ok(());
        }
        if self.pow[id] || ext {
            bail!("pulse on a powered or extended piston");
        }
        match self.push_list(pis, p + f, f) {
            Err(true) => bail!("push leaves the box"),
            Err(false) => bail!("push blocked"),
            Ok(list) if !list.is_empty() => self.move_list(&list, f),
            Ok(_) => {
                if !sticky {
                    bail!("normal piston pulsed into air");
                }
                self.pull(pis, p, f);
            }
        }
        Ok(())
    }
}

/// `NE(-4,0)`, `SW+(6,0)`, with an optional leading action letter (`P`/`L`).
fn parse_piece(tok: &str, action: bool) -> Result<(bool, Pis)> {
    let mut ch = tok.chars();
    let level = if action {
        match ch.next() {
            Some('P') => false,
            Some('L') => true,
            _ => bail!("bad action in {tok}"),
        }
    } else {
        false
    };
    let sticky = match ch.next() {
        Some('S') => true,
        Some('N') => false,
        _ => bail!("bad piston kind in {tok}"),
    };
    let f = match ch.next() {
        Some('E') => 1,
        Some('W') => -1,
        _ => bail!("bad facing in {tok} (rows only: E/W)"),
    };
    let rest: String = ch.collect();
    let ext0 = rest.starts_with('+');
    let inner = rest.trim_start_matches('+').trim_start_matches('(').trim_end_matches(')');
    let x: i32 = inner.split(',').next().context("no x")?.parse().with_context(|| format!("x in {tok}"))?;
    Ok((level, Pis { sticky, f, rest: x, ext0 }))
}

/// One mechanism's firings: (rest cell, current cell, level?) per action, open then close.
pub struct Fires {
    pub open: Vec<(i32, i32, bool)>,
    pub close: Vec<(i32, i32, bool)>,
}

fn replay(arms: &str, pieces: &str, open: &str, close: &str) -> Result<Fires> {
    let (a, b) = arms.split_once('+').context("arms a+b")?;
    let (lo, hi) = (-a.parse::<i32>()?, 2 + b.parse::<i32>()?);
    let pis: Vec<Pis> = pieces.split_whitespace().map(|t| parse_piece(t, false).map(|p| p.1)).collect::<Result<_>>()?;
    let mut row = Row { lo, hi, cells: vec![Cell::Air; (hi - lo + 1) as usize], pos: pis.iter().map(|p| p.rest).collect(), pow: pis.iter().map(|p| p.ext0).collect() };
    for x in 0..3 {
        row.set(x, Cell::Door);
    }
    for (id, p) in pis.iter().enumerate() {
        row.set(p.rest, Cell::Piston(id));
        if p.ext0 {
            row.set(p.rest + p.f, Cell::Head(id));
        }
    }
    let mut run = |sched: &str, row: &mut Row| -> Result<Vec<(i32, i32, bool)>> {
        let mut out = Vec::new();
        for tok in sched.split_whitespace() {
            let (level, q) = parse_piece(tok, true)?;
            let id = pis.iter().position(|p| p.rest == q.rest && p.f == q.f).with_context(|| format!("no piston for {tok}"))?;
            out.push((q.rest, row.pos[id], level));
            row.apply(&pis, id, level).with_context(|| format!("{tok}"))?;
        }
        Ok(out)
    };
    let o = run(open, &mut row)?;
    if (0..3).any(|x| row.get(x) != Some(Cell::Air)) {
        bail!("open schedule leaves the door cells occupied");
    }
    let c = run(close, &mut row)?;
    Ok(Fires { open: o, close: c })
}

pub fn cli(tsv: &Path, wirable: bool, only: &[String]) -> Result<()> {
    let text = std::fs::read_to_string(tsv).with_context(|| format!("reading {}", tsv.display()))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().context("empty tsv")?.split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).with_context(|| format!("no column {n}"));
    let (ci, ca, cp, co, cc, cpi) = (col("id")?, col("arms")?, col("pistons")?, col("open")?, col("close")?, col("pieces")?);
    println!("| id | arms | pistons | open/close actions | inner | x1 | firing cells | open (rest>cell) | close (rest>cell) |");
    println!("|---|---|---|---|---|---|---|---|---|");
    let (mut n, mut kept) = (0, 0);
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() <= cpi || (!only.is_empty() && !only.iter().any(|o| f[ci].starts_with(o.as_str()))) {
            continue;
        }
        n += 1;
        let fires = match replay(f[ca], f[cpi], f[co], f[cc]) {
            Ok(x) => x,
            Err(e) => {
                println!("| {} | {} | {} | - | - | - | replay failed: {e:#} | | |", f[ci], f[ca], f[cp]);
                continue;
            }
        };
        let all: Vec<&(i32, i32, bool)> = fires.open.iter().chain(fires.close.iter()).collect();
        let inner = all.iter().filter(|t| (0..3).contains(&t.1)).count();
        let x1 = all.iter().filter(|t| t.1 == 1).count();
        if wirable && inner > 0 {
            continue;
        }
        kept += 1;
        let cells: BTreeSet<i32> = all.iter().map(|t| t.1).collect();
        let fmt = |v: &[(i32, i32, bool)]| v.iter().map(|(r, c, l)| format!("{}{r}>{c}", if *l { "L" } else { "" })).collect::<Vec<_>>().join(" ");
        println!(
            "| {} | {} | {} | {}/{} | {inner} | {x1} | {} | {} | {} |",
            f[ci],
            f[ca],
            f[cp],
            fires.open.len(),
            fires.close.len(),
            cells.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
            fmt(&fires.open),
            fmt(&fires.close)
        );
    }
    eprintln!("{n} row mechanisms replayed, {kept} listed{}", if wirable { " (no firing from the door columns x 0..2)" } else { "" });
    Ok(())
}
