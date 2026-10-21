//! `mctas circuit material` (track clk-rt, vjeux 2026-10-21 15:26Z: "please make sure we only use
//! obsidian if we need to, otherwise use stone"): every obsidian block of a circuit tried as stone
//! on its redsim judge (`verify::sim`): all at once first, else one at a time, then the passing ones
//! together (greedy when the set does not pass as a whole). A replacement is kept only when the
//! judge still passes with the same kind, axis and metrics (and, for a TNT duper, the design still
//! intact). Each obsidian that stays gets its reason from its own stone trial: blast resistance
//! (the stone is destroyed by the design's own explosions) or immovable (anything else: pistons
//! cannot move obsidian).

use crate::{Circuit, archive, verify};
use anyhow::Result;
use clock::Pos;
use mctas::design::Design;

pub const STONE: &str = "minecraft:stone";

/// Why an obsidian block stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    BlastResistance,
    Immovable,
}

impl Need {
    pub fn text(&self) -> &'static str {
        match self {
            Need::BlastResistance => "blast resistance (the design keeps its verdict with a piston-movable netherite block there, not with stone)",
            Need::Immovable => "immovability (pistons must not move it: the design fails or changes even with a blast-proof netherite block there)",
        }
    }
}

pub struct Material {
    pub obsidian: Vec<Pos>,
    pub replaced: Vec<Pos>,
    pub kept: Vec<(Pos, Need)>,
    /// The seed's judge line, or why it does not pass on redsim.
    pub base: Result<String, String>,
    pub design: Option<Design>,
}

/// The judge's signature: kind, axis and metrics (what must not change), and the verdict text.
fn signature(d: &Design, vo: &verify::Options) -> Result<(String, String), String> {
    let c = Circuit::from_design(d.clone()).map_err(|e| format!("{e:#}"))?;
    let rep = verify::sim(&c, vo).map_err(|e| format!("{e:#}"))?;
    let (Ok(t), Some(out)) = (&rep.trace, &rep.outcome) else { return Err("no verdict".into()) };
    if !out.pass {
        return Err(out.why.chars().take(200).collect());
    }
    let kind = c.kind.clone().ok_or("no kind")?;
    let axis = archive::axis(&kind, out, &verify::family(&c, t));
    let metrics: String = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
    Ok((format!("{} | {axis} | {metrics}", kind.name()), out.why.clone()))
}

fn with_stone(d: &Design, cells: &[Pos]) -> Design {
    let mut d2 = d.clone();
    for p in cells {
        d2.blocks.insert(*p, STONE.to_string());
    }
    d2
}

pub fn obsidian_pass(d: &Design, vo: &verify::Options) -> Material {
    let cells: Vec<Pos> = d.blocks.iter().filter(|(_, s)| s.as_str() == "minecraft:obsidian").map(|(p, _)| *p).collect();
    let base = signature(d, vo);
    let mut m = Material { obsidian: cells.clone(), replaced: Vec::new(), kept: Vec::new(), base: base.clone().map(|b| b.0), design: None };
    let Ok((sig, why0)) = base else { return m };
    let intact = why0.contains("design intact");
    let same = |x: &Design| signature(x, vo).is_ok_and(|(s, w)| s == sig && (!intact || w.contains("design intact")));
    if cells.is_empty() {
        return m;
    }
    if same(&with_stone(d, &cells)) {
        m.replaced = cells.clone();
    } else {
        let mut singles = Vec::new();
        for p in &cells {
            let x = with_stone(d, &[*p]);
            if same(&x) {
                singles.push(*p);
            }
        }
        if !singles.is_empty() && same(&with_stone(d, &singles)) {
            m.replaced = singles;
        } else {
            for p in singles {
                let mut t = m.replaced.clone();
                t.push(p);
                if same(&with_stone(d, &t)) {
                    m.replaced = t;
                }
            }
        }
    }
    for p in cells.iter().filter(|p| !m.replaced.contains(p)) {
        // its own probe: a netherite block resists blasts like obsidian but pistons move it; if
        // the design keeps its verdict with one there, the obsidian is there for blast resistance,
        // else for being immovable
        let mut probe = d.clone();
        probe.blocks.insert(*p, "minecraft:netherite_block".to_string());
        let need = if same(&probe) { Need::BlastResistance } else { Need::Immovable };
        m.kept.push((*p, need));
    }
    if !m.replaced.is_empty() {
        m.design = Some(with_stone(d, &m.replaced));
    }
    m
}

/// `circuit material <designs> --out DIR [--judge]`: the changed designs (their comment lines, a
/// material line naming each kept obsidian's reason) and material.tsv; `--judge`: the game.
pub fn cli(designs: &[std::path::PathBuf], out: &std::path::Path, judge: bool, version: &str) -> Result<()> {
    let vo = verify::Options { version: version.to_string(), ..Default::default() };
    std::fs::create_dir_all(out)?;
    let pool = if judge { Some(verify::spawn_pool(&verify::Options { workers: 8, version: version.to_string(), ..Default::default() })?) } else { None };
    let gvo = verify::Options { workers: 8, version: version.to_string(), ..Default::default() };
    let mut tsv = String::from("design\tobsidian\tto stone\tkept (why)\tredsim\tgame\n");
    let (mut n, mut changed, mut stone, mut kept_blast, mut kept_imm) = (0, 0, 0, 0, 0);
    for p in designs {
        let text = std::fs::read_to_string(p)?;
        if !text.contains("minecraft:obsidian") {
            continue;
        }
        n += 1;
        let d = Design::load(p)?;
        let m = obsidian_pass(&d, &vo);
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let kept_s: String = m.kept.iter().map(|(q, k)| format!("{q:?} {}", if *k == Need::BlastResistance { "blast" } else { "immovable" })).collect::<Vec<_>>().join("; ");
        kept_blast += m.kept.iter().filter(|k| k.1 == Need::BlastResistance).count();
        kept_imm += m.kept.iter().filter(|k| k.1 == Need::Immovable).count();
        let mut game = String::new();
        if let Some(d2) = &m.design {
            changed += 1;
            stone += m.replaced.len();
            if let Some(pool) = &pool {
                let c = Circuit::from_design(d2.clone())?;
                let rep = verify::verify_in_pool(pool, &c, &gvo)?;
                game = rep.summary(&c);
            }
            let comments: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
            let why = if m.kept.is_empty() {
                "no obsidian left".to_string()
            } else {
                m.kept.iter().map(|(q, k)| format!("obsidian at {} {} {} stays for {}", q[0], q[1], q[2], k.text())).collect::<Vec<_>>().join("; ")
            };
            std::fs::write(out.join(&name), format!("{comments}# material (clk-rt 2026-10-21, `circuit material`): {} obsidian -> stone (the same kind, axis and metrics on its judge); {why}\n{}", m.replaced.len(), d2.to_text()))?;
        }
        let base = match &m.base {
            Ok(s) => format!("PASS {s}"),
            Err(e) => format!("not judged: {}", e.chars().take(120).collect::<String>()),
        };
        tsv += &format!("{name}\t{}\t{}\t{kept_s}\t{base}\t{game}\n", m.obsidian.len(), m.replaced.len());
        println!("{name}: {} obsidian, {} -> stone, kept: {}", m.obsidian.len(), m.replaced.len(), if kept_s.is_empty() { "none".into() } else { kept_s });
    }
    std::fs::write(out.join("material.tsv"), tsv)?;
    println!("{n} designs with obsidian: {changed} changed ({stone} obsidian -> stone); kept {kept_blast} for blast resistance, {kept_imm} immovable");
    Ok(())
}
