//! The commands.

use crate::model::Circuit;
use crate::rs::{Event, Prepared};
use crate::tff;
use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Cmd {
    /// The TFF metrics of each design on redsim: blocks, family, latency, min-pulse, min-period,
    /// edge.
    Eval {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
    },
    /// Print the output level(s) every gt under a stimulus (`pulse 4 every 40 x 6 [from 20]`, or
    /// edges `0+ 4- 20+ 22-` / `a 0+ 4-; b 10+ 30-`).
    Trace {
        design: PathBuf,
        #[arg(long, default_value = "pulse 4 every 40 x 4 from 20")]
        stimulus: String,
        #[arg(long)]
        frames: Option<u32>,
        /// Also every block that changed in each frame.
        #[arg(long)]
        blocks: bool,
    },
    /// The real game (stopgap until `mctas circuit verify`): the stimulus at the judge's positions
    /// x 4 rotations; every build's output changes compared with redsim's.
    Game {
        design: PathBuf,
        #[arg(long, default_value = "pulse 4 every 40 x 6 from 40")]
        stimulus: String,
        #[arg(long)]
        frames: Option<u32>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 16)]
        workers: usize,
    },
    /// Check the redsim TFF metrics in the real game: the trains behind min-pulse (its length, and
    /// one shorter) and min-period run at every build; every build must match redsim exactly.
    Verify {
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 16)]
        workers: usize,
    },
    /// The TFF census: every filling of k cells of a box (x 0..A, y 0..B, z 0..C) with palette
    /// states (clock census shorthands), the input a redstone block in an air cell next to the box
    /// (default: every cell of its west face, x = -1), judged on redsim (pulses of --len every
    /// --period, --count of them), every cell near the structure tried as the output; irreducible
    /// ones kept, one per type; designs + summary.tsv in --out.
    Census {
        /// `AxBxC`.
        #[arg(long = "box")]
        dims: String,
        /// Input cells `x,y,z;x,y,z` (each its own run).
        #[arg(long)]
        input: Option<String>,
        #[arg(long, default_value = "1..3")]
        k: String,
        #[arg(long, default_value = "stone,dust,torch,repeater,comparator")]
        palette: String,
        #[arg(long, default_value_t = 4)]
        len: u32,
        #[arg(long, default_value_t = 40)]
        period: u32,
        #[arg(long, default_value_t = 4)]
        count: u32,
        #[arg(long, default_value_t = 160)]
        jobs: usize,
        #[arg(long)]
        out: PathBuf,
        /// A skeleton circuit (its blocks fixed, its `port input:` the input, its `port output:`
        /// the only output tried when present) filled in the `--free` box instead of `--box`.
        #[arg(long)]
        skeleton: Option<PathBuf>,
        /// The free box for `--skeleton`: `x0,y0,z0:x1,y1,z1` (inclusive; skeleton cells and the
        /// input are skipped).
        #[arg(long)]
        free: Option<String>,
    },
}

/// Parse a stimulus for this circuit.
pub fn parse_stimulus(c: &Circuit, s: &str) -> Result<Vec<Event>> {
    let s = s.trim();
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.first() == Some(&"pulse") {
        // pulse L every T x K [from S]
        let num = |i: usize| -> Result<u32> { words.get(i).ok_or_else(|| anyhow!("stimulus {s:?}: missing number"))?.parse::<u32>().with_context(|| format!("stimulus {s:?}")) };
        if words.get(2) != Some(&"every") || words.get(4) != Some(&"x") {
            bail!("stimulus: pulse L every T x K [from S], got {s:?}");
        }
        let (l, t, k) = (num(1)?, num(3)?, num(5)?);
        let from = if words.get(6) == Some(&"from") { num(7)? } else { 0 };
        return Ok(crate::rs::pulses(0, from, l, t, k));
    }
    let mut out = Vec::new();
    for part in s.split(';') {
        let mut w: Vec<&str> = part.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let input = if w[0].ends_with('+') || w[0].ends_with('-') {
            0
        } else {
            let name = w.remove(0);
            c.input(name).ok_or_else(|| anyhow!("stimulus: no input {name:?}"))?
        };
        for e in w {
            let (t, on) = if let Some(t) = e.strip_suffix('+') {
                (t, true)
            } else if let Some(t) = e.strip_suffix('-') {
                (t, false)
            } else {
                bail!("stimulus: bad edge {e:?}")
            };
            out.push(Event { t: t.parse().with_context(|| format!("stimulus edge {e:?}"))?, input, on });
        }
    }
    Ok(out)
}

/// A census palette: clock census shorthands, plus `dropper1` / `hopper1` / `dispenser1`
/// (every facing, holding one stone) and `<state>{snbt}` entries (a container with contents).
pub fn parse_palette(spec: &str) -> Result<(Vec<redsim::Block>, Vec<Option<String>>)> {
    let mut items: Vec<String> = Vec::new();
    let (mut depth, mut cur) = (0i32, String::new());
    for ch in spec.chars() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            _ => {}
        }
        if ch == ',' && depth == 0 {
            items.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    items.push(cur);
    let stone = "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}".to_string();
    let mut blocks = Vec::new();
    let mut be = Vec::new();
    let mut push = |b: redsim::Block, n: Option<String>| {
        if !blocks.iter().zip(be.iter()).any(|(x, y): (&redsim::Block, &Option<String>)| *x == b && *y == n) {
            blocks.push(b);
            be.push(n);
        }
    };
    for item in items.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let (base, nbt) = match item {
            "dropper1" => ("dropper", Some(stone.clone())),
            "hopper1" => ("hopper", Some(stone.clone())),
            "dispenser1" => ("dispenser", Some(stone.clone())),
            s if s.contains('{') => {
                let i = s.find('{').unwrap();
                (&s[..i], Some(s[i..].to_string()))
            }
            s => (s, None),
        };
        for st in clock::census::expand_palette(base)? {
            push(redsim::Block::parse(&st).map_err(|e| anyhow!(e))?, nbt.clone());
        }
    }
    Ok((blocks, be))
}

/// FNV-1a of a key, as 16 hex digits (short ids for file names).
pub fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub fn parse_k(s: &str) -> Result<(usize, usize)> {
    if let Some((a, b)) = s.split_once("..") {
        let b = b.trim_start_matches('=');
        Ok((a.parse()?, b.parse()?))
    } else {
        let k = s.parse()?;
        Ok((k, k))
    }
}

fn parse_pos(s: &str) -> Result<crate::model::Pos> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().with_context(|| format!("bad position {s:?}"))?;
    if v.len() != 3 {
        bail!("bad position {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

/// A found circuit as a design file.
pub fn found_design(kind: &str, name: &str, f: &crate::census::Found, inputs: &[(String, redsim::BlockPos)]) -> mctas::design::Design {
    let mut d = mctas::design::Design { name: name.to_string(), ..Default::default() };
    d.meta.insert("circuit".into(), kind.to_string());
    for (n, p) in inputs {
        d.ports.insert(if n.is_empty() { "input".into() } else { format!("input.{n}") }, [p.x, p.y, p.z]);
    }
    d.ports.insert("output".into(), [f.output.x, f.output.y, f.output.z]);
    for (p, b) in &f.blocks {
        d.blocks.insert([p.x, p.y, p.z], b.to_string());
    }
    for (p, s) in &f.be {
        d.block_entities.insert([p.x, p.y, p.z], s.clone());
    }
    d
}

/// The family signature of blocks (clock judge names).
pub fn family_of(blocks: &[(redsim::BlockPos, redsim::Block)]) -> String {
    let names: std::collections::BTreeSet<String> = blocks.iter().filter_map(|(_, b)| clock::wave::family_name(&b.to_string())).collect();
    names.into_iter().collect::<Vec<_>>().join("+")
}

/// The on/off change frames of a level sequence.
pub fn changes(levels: &[u8]) -> Vec<usize> {
    (1..levels.len()).filter(|&t| (levels[t] > 0) != (levels[t - 1] > 0)).collect()
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Game { design, stimulus, frames, positions, no_rotations, workers } => {
            let c = Circuit::load(&design)?;
            let p = Prepared::new(&c, 3).map_err(|e| anyhow!("{e}"))?;
            let ev = parse_stimulus(&c, &stimulus)?;
            let n = frames.unwrap_or_else(|| ev.iter().map(|e| e.t).max().unwrap_or(0) + 60);
            let rs = p.run(&ev, n).map_err(|e| anyhow!("redsim: {e}"))?;
            let builds = positions * if no_rotations { 1 } else { 4 };
            let pool = crate::game::spawn_pool(workers.min(builds), mctas::DEFAULT_VERSION)?;
            let res = crate::game::run_all(&pool, &c, &ev, n, positions, !no_rotations);
            let want: Vec<Vec<usize>> = rs.iter().map(|v| changes(v)).collect();
            println!("redsim: output changes at {want:?}");
            let mut same = 0;
            for (t, off, r) in &res {
                match r {
                    Ok(lv) => {
                        let got: Vec<Vec<usize>> = lv.iter().map(|v| changes(v)).collect();
                        if got == want {
                            same += 1;
                        } else {
                            println!("  rotation {t} at {off:?}: changes at {got:?}");
                        }
                    }
                    Err(e) => println!("  rotation {t} at {off:?}: {e:#}"),
                }
            }
            println!("{same} of {} builds match redsim", res.len());
        }
        Cmd::Verify { designs, positions, workers } => {
            let pool = crate::game::spawn_pool(workers.min(positions * 4), mctas::DEFAULT_VERSION)?;
            for d in designs {
                let c = Circuit::load(&d)?;
                let p = Prepared::new(&c, 3).map_err(|e| anyhow!("{}: {e}", d.display()))?;
                let input = c.input("").unwrap_or(0);
                let m = tff::metrics(&p, input, 0).map_err(|e| anyhow!("{}: {e}", d.display()))?;
                let Some(lmin) = m.min_pulse else {
                    println!("{}: NOT A TFF on redsim ({})", d.display(), m.why);
                    continue;
                };
                let mut trains: Vec<(u32, u32, u32)> = vec![(lmin, lmin + 40, 6), (lmin.max(4), 40, 6)];
                if lmin > 1 {
                    trains.push((lmin - 1, lmin + 39, 6));
                }
                if let Some((t, l)) = m.min_period {
                    trains.push((l, t, 12));
                    if t > l + 1 {
                        trains.push((l, t - 1, 12));
                    }
                }
                let mut bad: Vec<String> = Vec::new();
                for (l, t, k) in &trains {
                    let ev = crate::rs::pulses(input, tff::SETTLE, *l, *t, *k);
                    let n = tff::SETTLE + (k - 1) * t + l.max(t) + tff::TAIL;
                    let rs = p.run(&ev, n).map_err(|e| anyhow!("{e}"))?;
                    let want = changes(&rs[0]);
                    for (turns, off, r) in crate::game::run_all(&pool, &c, &ev, n, positions, true) {
                        match r {
                            Ok(lv) => {
                                if changes(&lv[0]) != want {
                                    bad.push(format!("pulse {l} every {t}: rotation {turns} at {off:?} differs"));
                                }
                            }
                            Err(e) => bad.push(format!("pulse {l} every {t}: {e:#}")),
                        }
                    }
                }
                let verdict = if bad.is_empty() { format!("GAME AGREES ({} trains x {} builds)", trains.len(), positions * 4) } else { format!("GAME DIFFERS: {}", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) };
                println!("{}: {} blocks, family {}: {}; {verdict}", d.display(), c.blocks(), c.family(), m.summary());
            }
        }
        Cmd::Census { dims, input, k, palette, len, period, count, jobs, out, skeleton, free: free_box } => {
            let d: Vec<i32> = dims.split('x').map(|t| t.parse::<i32>()).collect::<Result<_, _>>().context("--box AxBxC")?;
            if d.len() != 3 {
                bail!("--box AxBxC");
            }
            let mut free = Vec::new();
            for y in 0..d[1] {
                for z in 0..d[2] {
                    for x in 0..d[0] {
                        free.push(redsim::BlockPos::new(x, y, z));
                    }
                }
            }
            // a skeleton: fixed blocks, its input and output, the free box
            let mut fixed: Vec<(redsim::BlockPos, redsim::Block)> = Vec::new();
            let mut fixed_be: Vec<(redsim::BlockPos, String)> = Vec::new();
            let mut fixed_out: Option<Vec<redsim::BlockPos>> = None;
            let mut sk_input: Option<redsim::BlockPos> = None;
            if let Some(sk) = &skeleton {
                let c = Circuit::load(sk)?;
                for (p, s) in &c.design.blocks {
                    fixed.push((redsim::BlockPos::new(p[0], p[1], p[2]), redsim::Block::parse(s).map_err(|e| anyhow!(e))?));
                }
                for (p, s) in &c.design.block_entities {
                    fixed_be.push((redsim::BlockPos::new(p[0], p[1], p[2]), s.clone()));
                }
                let ip = c.inputs[0].pos;
                sk_input = Some(redsim::BlockPos::new(ip[0], ip[1], ip[2]));
                if c.design.ports.contains_key("output") {
                    let op = c.outputs[0].1;
                    fixed_out = Some(vec![redsim::BlockPos::new(op[0], op[1], op[2])]);
                }
                let fb = free_box.as_deref().ok_or_else(|| anyhow!("--skeleton needs --free x0,y0,z0:x1,y1,z1"))?;
                let (a, b) = fb.split_once(':').ok_or_else(|| anyhow!("--free x0,y0,z0:x1,y1,z1"))?;
                let (a, b) = (parse_pos(a)?, parse_pos(b)?);
                free.clear();
                for y in a[1].min(b[1])..=a[1].max(b[1]) {
                    for z in a[2].min(b[2])..=a[2].max(b[2]) {
                        for x in a[0].min(b[0])..=a[0].max(b[0]) {
                            let p = redsim::BlockPos::new(x, y, z);
                            if !fixed.iter().any(|(q, _)| *q == p) && Some(p) != sk_input {
                                free.push(p);
                            }
                        }
                    }
                }
            }
            let inputs: Vec<redsim::BlockPos> = match (&input, sk_input) {
                (_, Some(p)) => vec![p],
                (Some(s), None) => s.split(';').filter(|t| !t.trim().is_empty()).map(|t| parse_pos(t).map(|p| redsim::BlockPos::new(p[0], p[1], p[2]))).collect::<Result<_>>()?,
                (None, None) => {
                    let mut v = Vec::new();
                    for y in 0..d[1] {
                        for z in 0..d[2] {
                            v.push(redsim::BlockPos::new(-1, y, z));
                        }
                    }
                    v
                }
            };
            let (pal, pal_be) = parse_palette(&palette)?;
            let (k_min, k_max) = parse_k(&k)?;
            let spec = crate::census::TffSpec { len, period, count };
            let mut all: std::collections::BTreeMap<String, (redsim::BlockPos, crate::census::Found)> = std::collections::BTreeMap::new();
            for inp in &inputs {
                eprintln!("input {},{},{}: {} free cells, {} palette states, k {k_min}..={k_max}", inp.x, inp.y, inp.z, free.len(), pal.len());
                let c = crate::census::Census { inputs: vec![*inp], fixed: fixed.clone(), fixed_be: fixed_be.clone(), free: free.clone(), palette: pal.clone(), palette_be: pal_be.clone(), k_min, k_max, outputs: fixed_out.clone(), jobs, margin: 3 };
                for f in c.run(&spec, true) {
                    all.entry(f.key.clone()).or_insert((*inp, f));
                }
            }
            std::fs::create_dir_all(&out)?;
            let mut rows: Vec<(String, usize, u32, String)> = Vec::new();
            for (key, (inp, f)) in &all {
                let fam = family_of(&f.blocks);
                let h = fnv(key);
                let name = format!("tff-{}-{}b-{}", if fam.is_empty() { "none".to_string() } else { fam.replace('+', "-") }, f.blocks.len(), &h[..8]);
                let des = found_design("tff", &name, f, &[(String::new(), *inp)]);
                des.save(&out.join(format!("{name}.txt")))?;
                rows.push((fam, f.blocks.len(), f.score, name));
            }
            rows.sort();
            let mut tsv = String::from("family\tblocks\tlatency\tdesign\n");
            for (fam, b, s, n) in &rows {
                tsv.push_str(&format!("{fam}\t{b}\t{s}\t{n}\n"));
            }
            std::fs::write(out.join("summary.tsv"), &tsv)?;
            // per family: the smallest
            let mut best: std::collections::BTreeMap<&str, (usize, usize)> = std::collections::BTreeMap::new();
            for (fam, b, _, _) in &rows {
                let e = best.entry(fam.as_str()).or_insert((*b, 0));
                if *b < e.0 {
                    *e = (*b, 0);
                }
                if *b == e.0 {
                    e.1 += 1;
                }
            }
            println!("{} TFF types; smallest per family:", rows.len());
            for (fam, (b, n)) in best {
                println!("  {fam}: {b} blocks ({n} types)");
            }
        }
        Cmd::Eval { designs } => {
            for d in designs {
                let c = Circuit::load(&d)?;
                let p = Prepared::new(&c, 3).map_err(|e| anyhow!("{}: {e}", d.display()))?;
                let input = c.input("").unwrap_or(0);
                let m = tff::metrics(&p, input, 0).map_err(|e| anyhow!("{}: {e}", d.display()))?;
                println!("{}: {} blocks, family {}: {}", d.display(), c.blocks(), c.family(), m.summary());
            }
        }
        Cmd::Trace { design, stimulus, frames, blocks } => {
            let c = Circuit::load(&design)?;
            let p = Prepared::new(&c, 3).map_err(|e| anyhow!("{e}"))?;
            let ev = parse_stimulus(&c, &stimulus)?;
            let n = frames.unwrap_or_else(|| ev.iter().map(|e| e.t).max().unwrap_or(0) + 40);
            let (lv, changes) = p.run_traced(&ev, n).map_err(|e| anyhow!("{e}"))?;
            for t in 0..n as usize {
                let ins: Vec<String> = (0..c.inputs.len())
                    .map(|i| {
                        let on = ev.iter().filter(|e| e.input == i && e.t as usize <= t).max_by_key(|e| e.t).is_some_and(|e| e.on);
                        if on { "+".to_string() } else { ".".to_string() }
                    })
                    .collect();
                let outs: Vec<String> = lv.iter().map(|v| v[t].to_string()).collect();
                if blocks {
                    println!("t={t:>4} in {} out {} {}", ins.join(""), outs.join(" "), changes[t]);
                } else {
                    println!("t={t:>4} in {} out {}", ins.join(""), outs.join(" "));
                }
            }
        }
    }
    Ok(())
}
