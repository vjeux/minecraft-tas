//! Extendable pulse families (`mctas circuit pulse family <name> --n N --out FILE`): one base
//! design and a repeated unit, the instance for any n, the `extend` card headers.

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct FamilyArgs {
    /// limiter-u, limiter-u4 (or `list`, or `limiter-for` with --l)
    pub name: String,
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// With `limiter-for`: the pulse length L (even, >= 4)
    #[arg(long)]
    pub l: Option<u32>,
    /// With `limiter-for`: every even L of this range (`a..b`), one design each, into --out-dir
    #[arg(long)]
    pub l_range: Option<String>,
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// Write the instance (or, with --card-max-n, the family card) here
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Write the family card (`extend` headers) judged up to this n
    #[arg(long)]
    pub card_max_n: Option<u32>,
}

pub struct Family {
    pub name: &'static str,
    pub kind: &'static str,
    pub l: &'static str,
    pub blocks: &'static str,
    pub rule: &'static str,
    pub how: &'static str,
}

pub const FAMILIES: &[Family] = &[
    Family {
        name: "limiter-u",
        kind: "pulse-limiter",
        l: "4 + 4n",
        blocks: "10 + 4n",
        rule: "lengthen the U by one row: one more delay-1 repeater (on stone) in each column",
        how: "a subtract comparator's rear sees the input at once and its side gets it back through a U of delay-1 repeaters (down one column, two dust across, up the other); the comparator outputs rear - side, so every input pulse is cut to the U's delay",
    },
    Family {
        name: "limiter-u4",
        kind: "pulse-limiter",
        l: "16 + 16n",
        blocks: "10 + 4n",
        rule: "lengthen the U by one row: one more delay-4 repeater (on stone) in each column",
        how: "the limiter-u with delay-4 repeaters: 16 gt per row",
    },
];

/// The U limiter with the given repeater delays, down column first (top to bottom) then up column
/// (bottom to top): 2k delays for k rows.
pub fn limiter_u_text(name: &str, delays: &[u8]) -> String {
    let k = delays.len() / 2;
    let mut palette: Vec<String> = vec!["  S minecraft:stone".into(), "  C minecraft:comparator[facing=west,mode=subtract,powered=false]".into(), "  W minecraft:redstone_wire".into()];
    let mut letters: Vec<(String, char)> = Vec::new();
    let mut letter = |state: String, palette: &mut Vec<String>| -> char {
        if let Some((_, c)) = letters.iter().find(|(s, _)| *s == state) {
            return *c;
        }
        let c = (b'a' + letters.len() as u8) as char;
        palette.push(format!("  {c} {state}"));
        letters.push((state, c));
        c
    };
    let mut l0 = vec!["...".to_string(), ".S.".to_string()];
    let mut l1 = vec!["...".to_string(), ".C.".to_string()];
    for row in 0..k {
        let down = letter(format!("minecraft:repeater[delay={},facing=north,locked=false,powered=false]", delays[row]), &mut palette);
        let up = letter(format!("minecraft:repeater[delay={},facing=south,locked=false,powered=false]", delays[2 * k - 1 - row]), &mut palette);
        l0.push("SS.".into());
        l1.push(format!("{down}{up}."));
    }
    l0.push("SS.".into());
    l1.push("WW.".into());
    format!(
        "name: {name}\ncircuit: pulse-limiter\norigin: 0 0 -1\nport input: 0 1 0\nport output: 2 1 0\npalette:\n{}\nlayer 0:\n{}\nlayer 1:\n{}\n",
        palette.join("\n"),
        l0.iter().map(|r| format!("  {r}")).collect::<Vec<_>>().join("\n"),
        l1.iter().map(|r| format!("  {r}")).collect::<Vec<_>>().join("\n")
    )
}

/// The smallest U limiter for an even L (4..): ceil(L/16) rows, delays filling L/2 redstone ticks.
pub fn limiter_u_for(l: u32) -> Option<String> {
    if l < 4 || l % 2 == 1 {
        return None;
    }
    let ticks = l / 2; // redstone ticks to share among 2k repeaters (1..4 each)
    let k = ticks.div_ceil(8) as usize;
    let mut delays = vec![1u8; 2 * k];
    let mut rest = ticks as i64 - 2 * k as i64;
    for d in delays.iter_mut() {
        let add = rest.min(3);
        *d += add as u8;
        rest -= add;
    }
    Some(limiter_u_text(&format!("pulse-limiter-u-L{l}"), &delays))
}

/// The design text of an instance: the U with n + 1 rows.
pub fn instance_text(name: &str, n: u32) -> Option<String> {
    let k = n as usize + 1;
    match name {
        "limiter-u" => Some(limiter_u_text(&format!("pulse-limiter-u-n{n}"), &vec![1; 2 * k])),
        "limiter-u4" => Some(limiter_u_text(&format!("pulse-limiter-u4-n{n}"), &vec![4; 2 * k])),
        _ => None,
    }
}

#[allow(dead_code)]
fn instance_text_v0(name: &str, n: u32) -> Option<String> {
    match name {
        "limiter-u" => {
            let k = n as usize + 1;
            // rows z = -1 (empty), 0 (input, comparator, output), 1..=k repeaters, k+1 dust
            let mut l0 = vec!["...".to_string(), ".S.".to_string()];
            let mut l1 = vec!["...".to_string(), ".C.".to_string()];
            for _ in 0..k {
                l0.push("SS.".into());
                l1.push("DU.".into());
            }
            l0.push("SS.".into());
            l1.push("WW.".into());
            Some(format!(
                "name: pulse-limiter-u-n{n}\ncircuit: pulse-limiter\norigin: 0 0 -1\nport input: 0 1 0\nport output: 2 1 0\npalette:\n  S minecraft:stone\n  C minecraft:comparator[facing=west,mode=subtract,powered=false]\n  D minecraft:repeater[delay=4,facing=north,locked=false,powered=false]\n  U minecraft:repeater[delay=4,facing=south,locked=false,powered=false]\n  W minecraft:redstone_wire\nlayer 0:\n{}\nlayer 1:\n{}\n",
                l0.iter().map(|r| format!("  {r}")).collect::<Vec<_>>().join("\n"),
                l1.iter().map(|r| format!("  {r}")).collect::<Vec<_>>().join("\n")
            ))
        }
        _ => None,
    }
}

pub fn run(a: FamilyArgs) -> Result<()> {
    if a.name == "limiter-for" {
        if let (Some(r), Some(dir)) = (&a.l_range, &a.out_dir) {
            let (lo, hi) = r.split_once("..").ok_or_else(|| anyhow::anyhow!("--l-range a..b"))?;
            let (lo, hi): (u32, u32) = (lo.trim().parse()?, hi.trim().parse()?);
            std::fs::create_dir_all(dir)?;
            for l in (lo..=hi).filter(|l| l % 2 == 0 && *l >= 4) {
                let mut text = limiter_u_for(l).unwrap_or_default();
                let at = text.find('\n').map(|i| i + 1).unwrap_or(0);
                text.insert_str(at, &format!("# A subtract comparator's rear sees the input at once, its side gets it back through a U of repeaters (down one column, two dust across, up the other) whose delays add up to {l} gt; it outputs rear - side, so every input pulse is cut to {l} gt (generator: mctas circuit pulse family limiter-for --l {l}).\n"));
                std::fs::write(dir.join(format!("pulse-limiter-u-L{l}.txt")), text)?;
            }
            return Ok(());
        }
        let l = a.l.ok_or_else(|| anyhow::anyhow!("limiter-for needs --l"))?;
        let Some(mut text) = limiter_u_for(l) else { bail!("L must be even and >= 4") };
        let at = text.find('\n').map(|i| i + 1).unwrap_or(0);
        text.insert_str(at, &format!("# A subtract comparator's rear sees the input at once, its side gets it back through a U of repeaters (down one column, two dust across, up the other) whose delays add up to {l} gt; it outputs rear - side, so every input pulse is cut to {l} gt (generator: mctas circuit pulse family limiter-for --l {l}).\n"));
        match &a.out {
            Some(o) => std::fs::write(o, text)?,
            None => print!("{text}"),
        }
        return Ok(());
    }
    if a.name == "list" {
        println!("| family | kind | L | blocks | extension rule |\n|---|---|---|---|---|");
        for f in FAMILIES {
            println!("| {} | {} | {} | {} | {} |", f.name, f.kind, f.l, f.blocks, f.rule);
        }
        return Ok(());
    }
    let Some(f) = FAMILIES.iter().find(|f| f.name == a.name) else { bail!("unknown family {} (`list`)", a.name) };
    let Some(mut text) = instance_text(f.name, a.n) else { bail!("no instance") };
    let caption = format!("# Extendable family {} (n = {}): {} with L = {} gt, {} blocks. {}. Extension: {}. Generator: mctas circuit pulse family {} --n N --out FILE.\n", f.name, a.n, f.kind, f.l, f.blocks, f.how, f.rule, f.name);
    let at = text.find('\n').map(|i| i + 1).unwrap_or(0);
    text.insert_str(at, &caption);
    if let Some(hi) = a.card_max_n {
        let header = format!(
            "extend family: pulse-{name}\nextend period: {l}\nextend blocks: {b}\nextend rule: {rule}\nextend n: {lo}..{hi}\nextend base: {lo}\nextend generator: mctas circuit pulse family {name} --n {{n}} --out {{out}}\n",
            name = f.name,
            l = f.l,
            b = f.blocks,
            rule = f.rule,
            lo = a.n
        );
        let at = text.find("origin:").unwrap_or(0);
        text.insert_str(at, &header);
        text = text.replacen(&format!("name: pulse-{}-n{}", f.name, a.n), &format!("name: pulse-{}", f.name), 1);
    }
    match &a.out {
        Some(o) => std::fs::write(o, text)?,
        None => print!("{text}"),
    }
    Ok(())
}
