//! EXTENDABLE clock families: a base design plus a repeated unit, `P = base + n x inc` (or
//! `base x 2^n` for the divider), each instance generated for any n and confirmed by redsim.
//! `mctas clock obs family <name> --n N [--out FILE]`, `--check K` checks n = 0..=K.

use crate::construct::{even_ring, note_ring, odd_ring};
use crate::sim::{self, Kick, Spec, Verdict};
use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use redsim::{Block, BlockPos, Dir};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct FamilyArgs {
    /// obs-ring | note-ring | odd-a | odd-b | divider (or `list`)
    pub name: String,
    /// The extension count n (>= 0)
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// Write the instance here
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Check every n from 0 to this with redsim (period and block count against the formula)
    #[arg(long)]
    pub check: Option<u32>,
    /// Write the family CARD (the base design, n = --n, with `extend` headers: family, period,
    /// blocks, rule, n range, base, generator) to --out, judged up to this n
    #[arg(long)]
    pub card_max_n: Option<u32>,
    /// With `members`: the designs (files or folders) to classify
    pub paths: Vec<PathBuf>,
}

/// `members`: which of these designs are instances of an extendable family (a ring of observers
/// with at most one relay in any layout whose period follows the formula, or exactly a generated
/// odd-stage or divider instance). Prints `<file>\t<family>\t<n>\t<P>` for each member.
fn members(paths: &[PathBuf]) -> Result<()> {
    use std::collections::HashMap;
    let mut exact: HashMap<u128, (&'static str, u32)> = HashMap::new();
    for name in ["odd-a", "odd-b", "divider"] {
        for n in 0..=if name == "divider" { 14 } else { 60 } {
            if let Some(s) = instance(name, n) {
                exact.insert(crate::census::canon_hash(&s.blocks), (name, n));
            }
        }
    }
    for f in crate::pick::files_of(paths)? {
        let Ok(spec) = Spec::load(&f) else { continue };
        if let Some((fam, n)) = exact.get(&crate::census::canon_hash(&spec.blocks)) {
            let (p, _) = formula(fam, *n).unwrap_or((0, 0));
            println!("{}\t{fam}\t{n}\t{p}", f.display());
            continue;
        }
        let r = sim::simulate(&spec, &sim::Opts { horizon: 20000, ..Default::default() });
        if r.verdict != Verdict::Clock {
            continue;
        }
        if let Some((fam, n)) = ring_member(&spec, r.period) {
            println!("{}\t{fam}\t{n}\t{}", f.display(), r.period);
        }
    }
    Ok(())
}

pub struct Family {
    pub name: &'static str,
    /// The period formula, as text
    pub period: &'static str,
    /// The block formula, as text
    pub blocks: &'static str,
    /// The extension rule, one line
    pub rule: &'static str,
    /// How the base works, one line
    pub how: &'static str,
}

pub const FAMILIES: &[Family] = &[
    Family {
        name: "obs-ring",
        period: "8 + 4n",
        blocks: "4 + 2n",
        rule: "insert 2 observers: one more column in the 2-wide ring (each observer watches the previous one)",
        how: "a single pulse runs around a ring of observers, 2 gt per observer; lattice rings have an even length",
    },
    Family {
        name: "note-ring",
        period: "10 + 4n",
        blocks: "6 + 2n",
        rule: "insert 2 observers: one more column in the 2-wide ring",
        how: "a note block after a straight observer follows its power at once and is watched by the next observer: it takes a lattice cell but no time, so an odd number of observers closes the ring",
    },
    Family {
        name: "odd-a",
        period: "11 + 4n",
        blocks: "8 + 2n",
        rule: "raise the observer tower by one: one more observer in each column",
        how: "the odd piston stage: observer O1 quasi-powers a sticky piston and O2 (watching O1) keeps it powered from above for 4 gt, so the redstone block it pushes LANDS under a normal piston that fires the next tick (+1 gt): the observer watching that piston's head fires 5 gt after O1; the tower carries the pulse back, 2 gt per observer",
    },
    Family {
        name: "odd-b",
        period: "13 + 4n",
        blocks: "10 + 2n",
        rule: "raise the observer tower by one: one more observer in each column",
        how: "the odd piston stage with a slime block that drags the redstone block one cell further, which moves the head-watching observer by one cell so the tower has an odd number of observers",
    },
    Family {
        name: "divider",
        period: "12 x 2^n",
        blocks: "3 + 3n",
        rule: "add a stage behind the last bulb: a comparator (facing the clock, reading the bulb, on a stone floor) and a bulb",
        how: "an observer pair (6 gt) toggles the copper bulb behind it on every pulse (12 gt); each comparator reading a bulb powers the next bulb only every other pulse, doubling the period",
    },
    Family {
        name: "trap-ring",
        period: "10 + 4n",
        blocks: "6 + 2n",
        rule: "insert 2 observers: one more column in the 2-wide ring",
        how: "an iron trapdoor after a straight observer opens and closes with its power at once and is watched by the next observer: a lattice cell without delay, so an odd number of observers closes the ring",
    },
    Family {
        name: "bulb-ring",
        period: "20 + 8n",
        blocks: "6 + 2n",
        rule: "insert 2 observers: one more column in the 2-wide ring",
        how: "a copper bulb after a straight observer toggles on each pulse and the next observer fires on each toggle, so one pulse makes the round twice before the bulb is back: P = 2 x 2 gt x (observers)",
    },
];

pub fn family(name: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|f| f.name == name)
}

/// Is this design (with this period) a member of one of the ring families, whatever its layout:
/// only observers plus at most one relay block (note block, iron trapdoor or copper bulb), with
/// the period of the family's formula for its size? Returns (family, n).
pub fn ring_member(spec: &Spec, period: u32) -> Option<(&'static str, u32)> {
    let mut obs = 0u32;
    let mut relay: Option<&'static str> = None;
    for (_, b) in &spec.blocks {
        match b {
            Block::Observer { .. } => obs += 1,
            _ => {
                if relay.is_some() {
                    return None;
                }
                let s = sim::short(b);
                relay = Some(match s.as_str() {
                    "note_block" => "note-ring",
                    "iron_trapdoor" => "trap-ring",
                    "waxed_copper_bulb" | "copper_bulb" => "bulb-ring",
                    _ => return None,
                });
            }
        }
    }
    let fam = relay.unwrap_or("obs-ring");
    let (p0, b0, inc) = match fam {
        "obs-ring" => (8, 4, 4),
        "note-ring" | "trap-ring" => (10, 6, 4),
        _ => (20, 6, 8),
    };
    let blocks = spec.blocks.len() as u32;
    if blocks < b0 || (blocks - b0) % 2 != 0 {
        return None;
    }
    let n = (blocks - b0) / 2;
    (period == p0 + inc * n).then_some((fam, n))
}

/// The formula's period and block count for n.
pub fn formula(name: &str, n: u32) -> Option<(u32, usize)> {
    let n_ = n as usize;
    Some(match name {
        "obs-ring" => (8 + 4 * n, 4 + 2 * n_),
        "note-ring" => (10 + 4 * n, 6 + 2 * n_),
        "odd-a" => (11 + 4 * n, 8 + 2 * n_),
        "odd-b" => (13 + 4 * n, 10 + 2 * n_),
        "divider" => (12u32.checked_mul(1u32.checked_shl(n)?)?, 3 + 3 * n_),
        "trap-ring" => (10 + 4 * n, 6 + 2 * n_),
        "bulb-ring" => (20 + 8 * n, 6 + 2 * n_),
        _ => return None,
    })
}

/// The instance for n.
pub fn instance(name: &str, n: u32) -> Option<Spec> {
    let (p, _) = formula(name, n)?;
    let mut s = match name {
        "obs-ring" => even_ring(p),
        "note-ring" => note_ring(p),
        "odd-a" | "odd-b" => odd_ring(p)?,
        "divider" => {
            let a = BlockPos::new(0, 0, 0);
            let pair = Spec {
                name: "pair".into(),
                blocks: vec![(a, Block::Observer { facing: Dir::East, powered: false }), (BlockPos::new(1, 0, 0), Block::Observer { facing: Dir::West, powered: false })],
                kick: Kick::Last(BlockPos::new(1, 0, 0)),
                output: None,
            };
            crate::divider::build(&pair, a, Dir::West, n as usize + 1)?
        }
        "trap-ring" | "bulb-ring" => {
            let relay = if name == "trap-ring" { "minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false]" } else { "minecraft:waxed_copper_bulb[lit=false,powered=false]" };
            let cells = crate::construct::rectangle(3 + n as i32);
            crate::construct::ring_with(name, &cells, &[2], Block::parse(relay).unwrap())
        }
        _ => return None,
    };
    s.name = format!("clk-obs-{name}-n{n}");
    Some(s)
}

pub fn run(a: FamilyArgs) -> Result<()> {
    if a.name == "members" {
        return members(&a.paths);
    }
    if a.name == "list" {
        println!("| family | P | blocks | extension rule |\n|---|---|---|---|");
        for f in FAMILIES {
            println!("| {} | {} | {} | {} |", f.name, f.period, f.blocks, f.rule);
        }
        return Ok(());
    }
    let Some(f) = family(&a.name) else { bail!("unknown family {} (list: `mctas clock obs family list`)", a.name) };
    if let Some(k) = a.check {
        println!("{}: P = {}, blocks {}: {}", f.name, f.period, f.blocks, f.rule);
        println!("| n | P (formula) | P (redsim) | blocks | ok |\n|---|---|---|---|---|");
        for n in 0..=k {
            let Some((p, b)) = formula(f.name, n) else { break };
            let Some(s) = instance(f.name, n) else {
                println!("| {n} | {p} | (no instance) | | NO |");
                continue;
            };
            let r = sim::simulate(&s, &sim::Opts { horizon: (40 * p).max(20000), ..Default::default() });
            let ok = r.verdict == Verdict::Clock && r.period == p && s.blocks.len() == b;
            println!("| {n} | {p} | {} | {} | {} |", if r.verdict == Verdict::Clock { r.period.to_string() } else { format!("{:?}", r.verdict) }, s.blocks.len(), if ok { "yes" } else { "NO" });
        }
        return Ok(());
    }
    let (p, b) = formula(f.name, a.n).ok_or_else(|| anyhow::anyhow!("n too large"))?;
    let s = instance(f.name, a.n).ok_or_else(|| anyhow::anyhow!("no instance for n = {}", a.n))?;
    let r = sim::simulate(&s, &sim::Opts { horizon: (40 * p).max(20000), ..Default::default() });
    println!("{} n={}: P = {} = {p} (formula), redsim {:?} P={}, {} blocks (formula {b})", f.name, a.n, f.period, r.verdict, r.period, s.blocks.len());
    if let Some(out) = &a.out {
        let mut text = s.to_text(&format!(
            "Extendable family {} (n = {}): P = {} gt, {} blocks. {}. Extension: {}. Generator: mctas clock obs family {} --n N --out FILE.",
            f.name, a.n, f.period, f.blocks, f.how, f.rule, f.name
        ));
        if let Some(hi) = a.card_max_n {
            let period = f.period.replace(" x ", " * ");
            let header = format!(
                "extend family: clk-obs-{name}\nextend period: {period}\nextend blocks: {blocks}\nextend rule: {rule}\nextend n: {lo}..{hi}\nextend base: {lo}\nextend generator: mctas clock obs family {name} --n {{n}} --out {{out}}\n",
                name = f.name,
                blocks = f.blocks,
                rule = f.rule,
                lo = a.n,
            );
            let at = text.find("\norigin:").map(|i| i + 1).unwrap_or(0);
            text.insert_str(at, &header);
            text = text.replacen(&format!("name: clk-obs-{}-n{}", f.name, a.n), &format!("name: clk-obs-{}", f.name), 1);
        }
        std::fs::write(out, text)?;
    }
    Ok(())
}
