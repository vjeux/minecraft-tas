//! `mctas clock sculk climb`: a long-period hunt. Random walkers start from seed clocks and mutate
//! one cell at a time (a palette block in any facing, or air) inside the design's box grown by one,
//! never touching a sensor or the kick; a mutant that is still a clock with no more than
//! --max-blocks blocks and a period at least the walker's current one is accepted (worse ones now
//! and then, to leave plateaus). Every distinct clock met is kept for the report.

use crate::build::DIRS;
use crate::{report, Found, Out};
use anyhow::Result;
use clock::{Clock, Kick, Pos};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(clap::Args)]
pub struct ClimbArgs {
    /// Seed clock designs.
    seeds: Vec<PathBuf>,
    /// Most blocks a mutant may have.
    #[arg(long, default_value_t = 8)]
    max_blocks: usize,
    /// Mutations per walker.
    #[arg(long, default_value_t = 5000)]
    iters: u32,
    /// Independent walkers (threads).
    #[arg(long, default_value_t = 48)]
    walkers: usize,
    /// Mutation palette: observer,sticky,piston,note,trapdoor,stone,repeater,dispenser.
    #[arg(long, default_value = "observer,sticky,piston,note,trapdoor,stone")]
    palette: String,
    /// Chance (percent) of accepting a shorter period.
    #[arg(long, default_value_t = 5)]
    worse: u32,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[command(flatten)]
    out: Out,
}

fn palette(spec: &str) -> Vec<String> {
    let mut v = Vec::new();
    for w in spec.split(',') {
        for d in DIRS {
            let f = crate::build::dir_name(d);
            match w {
                "observer" => v.push(format!("minecraft:observer[facing={f},powered=false]")),
                "sticky" => v.push(format!("minecraft:sticky_piston[extended=false,facing={f}]")),
                "piston" => v.push(format!("minecraft:piston[extended=false,facing={f}]")),
                "dispenser" => v.push(format!("minecraft:dispenser[facing={f},triggered=false]")),
                _ => {}
            }
        }
        for f in ["north", "east", "south", "west"] {
            if w == "repeater" {
                for delay in 1..=4 {
                    v.push(format!("minecraft:repeater[delay={delay},facing={f},locked=false,powered=false]"));
                }
            }
        }
        match w {
            "note" => v.push("minecraft:note_block[instrument=harp,note=0,powered=false]".into()),
            "trapdoor" => v.push("minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]".into()),
            "stone" => v.push("minecraft:stone".into()),
            _ => {}
        }
    }
    v
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn is_sensor(s: &str) -> bool {
    s.contains("sculk_sensor")
}

pub fn climb(a: &ClimbArgs) -> Result<()> {
    let pal = palette(&a.palette);
    anyhow::ensure!(!pal.is_empty(), "empty palette");
    let seeds: Vec<Clock> = a.seeds.iter().map(|p| Clock::load(p)).collect::<Result<_>>()?;
    anyhow::ensure!(!seeds.is_empty(), "no seed clocks");
    let opts = clock::sim::SimOptions { horizon: a.out.horizon, margin: 4 };
    let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
    let seen: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let best_p = std::sync::atomic::AtomicU32::new(0);
    std::thread::scope(|s| {
        for w in 0..a.walkers {
            let (pal, seeds, found, seen, opts, best_p) = (&pal, &seeds, &found, &seen, &opts, &best_p);
            s.spawn(move || {
                let mut rng = Rng((a.seed * 1_000_003 + w as u64 + 1) * 0x9E37_79B9_7F4A_7C15);
                let mut cur = seeds[w % seeds.len()].clone();
                let r0 = clock::sim::simulate(&cur, opts);
                let mut cur_p = if r0.verdict == clock::sim::Verdict::Clock { r0.period } else { 0 };
                let kick_cell: Option<Pos> = cur.kick.as_ref().map(|k| k.pos());
                for _ in 0..a.iters {
                    let Some((lo, hi)) = cur.design.bounds() else { break };
                    let cell = [
                        lo[0] - 1 + rng.below((hi[0] - lo[0] + 3) as usize) as i32,
                        lo[1] - 1 + rng.below((hi[1] - lo[1] + 3) as usize) as i32,
                        lo[2] - 1 + rng.below((hi[2] - lo[2] + 3) as usize) as i32,
                    ];
                    if Some(cell) == kick_cell || cur.design.blocks.get(&cell).is_some_and(|s| is_sensor(s)) {
                        continue;
                    }
                    let mut next = cur.clone();
                    if rng.below(100) < 30 {
                        if next.design.blocks.remove(&cell).is_none() {
                            continue;
                        }
                    } else {
                        next.design.blocks.insert(cell, pal[rng.below(pal.len())].clone());
                    }
                    if next.design.blocks.len() > a.max_blocks {
                        continue;
                    }
                    // The kick block must still be there for `use` / `last`.
                    if let Some(Kick::Use { pos, .. }) | Some(Kick::Last { pos }) = &next.kick {
                        if !next.design.blocks.contains_key(pos) {
                            continue;
                        }
                    }
                    let r = clock::sim::simulate(&next, opts);
                    if r.verdict != clock::sim::Verdict::Clock {
                        continue;
                    }
                    let key = crate::build::structure_key(&next.design.blocks);
                    if r.period >= a.out.min_p && r.period <= a.out.max_p && seen.lock().unwrap().insert(format!("{}|{key}", r.period)) {
                        found.lock().unwrap().push(Found {
                            period: r.period,
                            blocks: r.blocks,
                            volume: r.volume(),
                            family: r.analysis.family.join("+"),
                            summary: r.summary(),
                            key,
                            clock: next.clone(),
                            how: format!("climb from {} (walker {w})", seeds[w % seeds.len()].name()),
                        });
                        let prev = best_p.fetch_max(r.period, std::sync::atomic::Ordering::Relaxed);
                        if r.period > prev {
                            eprintln!("climb: new longest P={} ({} blocks)", r.period, r.blocks);
                        }
                    }
                    if r.period >= cur_p || rng.below(100) < a.worse as usize {
                        cur = next;
                        cur_p = r.period;
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    println!("climb: {} distinct clocks", found.len());
    report(found, &a.out)
}
