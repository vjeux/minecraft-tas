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
    /// With `oneshot-for`: the kind to declare (rising-edge, pulse-extender, pulse-limiter)
    #[arg(long, default_value = "rising-edge")]
    pub kind: String,
    /// With `oneshot-mode`: rise, fall or dual
    #[arg(long, default_value = "rise")]
    pub mode: String,
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
    Family {
        name: "oneshot-u",
        kind: "rising-edge",
        l: "?",
        blocks: "17 + 4n",
        rule: "lengthen the U by one row: one more delay-4 repeater (on stone) in each column",
        how: "a sticky piston pushes an observer whose back then faces a copper bulb; the observer fires on landing and lights the bulb; a comparator reads the bulb into a U of repeaters that comes back into the bulb and turns it off (a bulb toggles only on rising edges of its power); the falling edge pulls the observer back where it faces nothing",
    },
    Family {
        name: "oneshot-rise",
        kind: "rising-edge",
        l: "18 + 16n",
        blocks: "17 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "a sticky piston pushes an observer whose back then faces a copper bulb; the observer fires on landing and lights the bulb; a comparator reads the bulb into a U of delay-4 repeaters that comes back into the bulb and turns it off one loop later (a bulb toggles only on rising edges of its power, so the loop's own falling edge does nothing); on the falling edge the observer is pulled back and fires into nothing",
    },
    Family {
        name: "oneshot-ext",
        kind: "pulse-extender",
        l: "18 + 16n",
        blocks: "17 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "the rising-edge bulb one-shot (oneshot-rise) judged as an extender: every input pulse, short or long, gives one output pulse of L from its rising edge",
    },
    Family {
        name: "oneshot-fall",
        kind: "falling-edge",
        l: "18 + 16n",
        blocks: "17 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "a sticky piston pushes an observer away on the rising edge and pulls it back on the falling edge, where its back faces a copper bulb: it fires on landing and lights the bulb; a comparator reads the bulb into a U of delay-4 repeaters that comes back into the bulb and turns it off one loop later (a bulb toggles only on rising edges of its power)",
    },
    Family {
        name: "dual-lock",
        kind: "dual-edge",
        l: "?",
        blocks: "23 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "an observer watching the input fires on both edges through a repeater into a copper bulb; a comparator reads the bulb and feeds a lock (a repeater pointing into the trigger repeater's side: triggers are ignored while the bulb is lit) and a U of delay-4 repeaters back into the bulb, which turns it off one loop later",
    },
    Family {
        name: "dual-lock1",
        kind: "dual-edge",
        l: "?",
        blocks: "23 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-1 repeater (on stone) in each column",
        how: "the dual-lock with delay-1 repeaters",
    },
    Family {
        name: "oneshot-u1",
        kind: "rising-edge",
        l: "?",
        blocks: "17 + 4n",
        rule: "lengthen the U by one row: one more delay-1 repeater (on stone) in each column",
        how: "the oneshot-u with delay-1 repeaters",
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

/// The bulb one-shot (a rising-edge detector of any L, also an extender and a limiter): a sticky
/// piston pushes an observer whose back then faces a copper bulb; the observer fires on landing and
/// lights the bulb; a comparator reads the bulb into a U of repeaters (k rows, delay `d`) that comes
/// back into the bulb and turns it off. Only the landing at the pushed position faces the bulb, so
/// the falling edge (the observer pulled back) does nothing.
/// A bulb one-shot of any mode: `rise` (the observer's back faces the bulb when pushed), `fall`
/// (when back at rest: it fires there on the falling edge), `dual` (a fixed observer watching the
/// input). Delays: the U's down column (top to bottom) then up column (bottom to top), 2k values.
pub fn oneshot_text(name: &str, kind: &str, mode: &str, delays: &[u8]) -> String {
    let k = delays.len() / 2;
    // (bulb x, row of the bulb/comparator line, has piston)
    let (bx, z0, piston) = match mode {
        "fall" => (1i32, 1usize, true),
        "dual" => (1, 0, false),
        _ => (2, 1, true),
    };
    let w = (bx + 4) as usize; // x from -1 to bx + 2
    let rows = z0 + k + 2;
    let mut l0 = vec![vec!['.'; w]; rows];
    let mut l1 = vec![vec!['.'; w]; rows];
    let at = |x: i32| (x + 1) as usize;
    if piston {
        l1[0][at(0)] = 'P';
        l1[0][at(1)] = 'O';
    } else {
        l1[0][at(0)] = 'V';
    }
    l1[z0][at(bx)] = 'B';
    l1[z0][at(bx + 1)] = 'C';
    l0[z0][at(bx + 1)] = 'S';
    l1[z0][at(bx + 2)] = 'W';
    l0[z0][at(bx + 2)] = 'S';
    let mut palette = vec![
        "  S minecraft:stone".to_string(),
        "  P minecraft:sticky_piston[extended=false,facing=east]".into(),
        "  O minecraft:observer[facing=north,powered=false]".into(),
        "  V minecraft:observer[facing=west,powered=false]".into(),
        "  B minecraft:waxed_copper_bulb[lit=false,powered=false]".into(),
        "  C minecraft:comparator[facing=west,mode=compare,powered=false]".into(),
        "  W minecraft:redstone_wire".into(),
    ];
    for j in 0..k {
        let z = z0 + 1 + j;
        let (cd, cu) = ((b'a' + j as u8) as char, (b'm' + j as u8) as char);
        palette.push(format!("  {cd} minecraft:repeater[delay={},facing=north,locked=false,powered=false]", delays[j]));
        palette.push(format!("  {cu} minecraft:repeater[delay={},facing=south,locked=false,powered=false]", delays[2 * k - 1 - j]));
        l1[z][at(bx + 2)] = cd;
        l0[z][at(bx + 2)] = 'S';
        l1[z][at(bx)] = cu;
        l0[z][at(bx)] = 'S';
    }
    for x in bx..=bx + 2 {
        l1[z0 + k + 1][at(x)] = 'W';
        l0[z0 + k + 1][at(x)] = 'S';
    }
    let rowtext = |l: &Vec<Vec<char>>| l.iter().map(|r| format!("  {}", r.iter().collect::<String>())).collect::<Vec<_>>().join("\n");
    format!(
        "name: {name}\ncircuit: {kind}\norigin: -1 0 0\nport input: -1 1 0\nport output: {} 1 {z0}\npalette:\n{}\nlayer 0:\n{}\nlayer 1:\n{}\n",
        bx + 1,
        palette.join("\n"),
        rowtext(&l0),
        rowtext(&l1)
    )
}

/// The 2k delays (1..4 each) adding up to `ticks` redstone ticks, k as small as possible.
pub fn split_delays(ticks: u32) -> Vec<u8> {
    let k = ticks.div_ceil(8).max(1) as usize;
    let mut delays = vec![1u8; 2 * k];
    let mut rest = ticks as i64 - 2 * k as i64;
    for d in delays.iter_mut() {
        let add = rest.clamp(0, 3);
        *d += add as u8;
        rest -= add;
    }
    delays
}

pub fn oneshot_u_text(name: &str, k: usize, d: u8) -> String {
    // x from -1 (input) to 4; z from 0 to k + 2
    let w = 6usize;
    let rows = k + 3;
    let mut l0 = vec![vec!['.'; w]; rows];
    let mut l1 = vec![vec!['.'; w]; rows];
    let at = |x: i32| (x + 1) as usize;
    l1[0][at(0)] = 'P';
    l1[0][at(1)] = 'O';
    l1[1][at(2)] = 'B';
    l1[1][at(3)] = 'C';
    l0[1][at(3)] = 'S';
    l1[1][at(4)] = 'W';
    l0[1][at(4)] = 'S';
    for j in 0..k {
        let z = 2 + j;
        l1[z][at(4)] = 'D';
        l0[z][at(4)] = 'S';
        l1[z][at(2)] = 'U';
        l0[z][at(2)] = 'S';
    }
    for x in 2..=4 {
        l1[k + 2][at(x)] = 'W';
        l0[k + 2][at(x)] = 'S';
    }
    let rowtext = |l: &Vec<Vec<char>>| l.iter().map(|r| format!("  {}", r.iter().collect::<String>())).collect::<Vec<_>>().join("\n");
    format!(
        "name: {name}\ncircuit: rising-edge\norigin: -1 0 0\nport input: -1 1 0\nport output: 3 1 1\npalette:\n  S minecraft:stone\n  P minecraft:sticky_piston[extended=false,facing=east]\n  O minecraft:observer[facing=north,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\n  C minecraft:comparator[facing=west,mode=compare,powered=false]\n  D minecraft:repeater[delay={d},facing=north,locked=false,powered=false]\n  U minecraft:repeater[delay={d},facing=south,locked=false,powered=false]\n  W minecraft:redstone_wire\nlayer 0:\n{}\nlayer 1:\n{}\n",
        rowtext(&l0),
        rowtext(&l1)
    )
}

/// The smallest bulb one-shot for an even L >= 6: L = 2 + 2 x (sum of the 2k repeater delays).
pub fn oneshot_for(l: u32, kind: &str) -> Option<String> {
    if l < 6 || l % 2 == 1 {
        return None;
    }
    let ticks = (l - 2) / 2;
    let k = ticks.div_ceil(8) as usize;
    let mut delays = vec![1u8; 2 * k];
    let mut rest = ticks as i64 - 2 * k as i64;
    for d in delays.iter_mut() {
        let add = rest.min(3);
        *d += add as u8;
        rest -= add;
    }
    // the U's down column takes the first k delays, the up column the rest
    let mut text = oneshot_u_text(&format!("pulse-oneshot-{kind}-L{l}"), k, 1);
    let mut palette_extra = String::new();
    let mut l1_rows: Vec<String> = Vec::new();
    let mut in_l1 = false;
    for line in text.lines() {
        if line.starts_with("layer 1:") {
            in_l1 = true;
            continue;
        }
        if in_l1 {
            l1_rows.push(line.to_string());
        }
    }
    // rewrite the repeaters of each row with their own delays: letters a.. for down, A.. for up
    for j in 0..k {
        let (dd, du) = (delays[j], delays[2 * k - 1 - j]);
        let (cd, cu) = ((b'a' + j as u8) as char, (b'm' + j as u8) as char);
        palette_extra.push_str(&format!("  {cd} minecraft:repeater[delay={dd},facing=north,locked=false,powered=false]\n  {cu} minecraft:repeater[delay={du},facing=south,locked=false,powered=false]\n"));
        let row = &mut l1_rows[2 + j];
        *row = row.replacen('D', &cd.to_string(), 1).replacen('U', &cu.to_string(), 1);
    }
    let head = text.split("layer 1:").next().unwrap_or("").to_string();
    text = format!("{}layer 1:\n{}\n", head.replacen("palette:\n", &format!("palette:\n{palette_extra}"), 1), l1_rows.join("\n"));
    text = text.replacen("circuit: rising-edge", &format!("circuit: {kind}"), 1);
    Some(text)
}

/// The locked dual-edge one-shot: an observer watching the input fires on both edges through a
/// repeater T into a copper bulb; a comparator reads the bulb and feeds a lock (a repeater pointing
/// into T's side, so triggers are ignored while the bulb is lit) and a U of k rows of repeaters
/// (delay d) back into the bulb, which turns it off one loop later.
pub fn dual_lock_text(name: &str, k: usize, d: u8) -> String {
    // x from 1 to 4; z from -(k+1) to 3 (rows listed from the smallest z)
    let zmin = -(k as i32 + 1);
    let mut l0: Vec<String> = Vec::new();
    let mut l1: Vec<String> = Vec::new();
    for z in zmin..=3 {
        let (r0, r1) = match z {
            3 => ("....", "...."),
            2 => ("....", ".V.."),
            1 => (".SSS", ".TLW"),
            0 => (".SSS", ".BCW"),
            z if z == zmin => (".SSS", ".WWW"),
            _ => (".S.S", ".N.M"),
        };
        l0.push(format!("  {r0}"));
        l1.push(format!("  {r1}"));
    }
    format!(
        "name: {name}\ncircuit: dual-edge\norigin: 1 0 {zmin}\nport input: 2 1 3\nport output: 3 1 0\npalette:\n  S minecraft:stone\n  V minecraft:observer[facing=south,powered=false]\n  T minecraft:repeater[delay=1,facing=south,locked=false,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\n  C minecraft:comparator[facing=west,mode=compare,powered=false]\n  L minecraft:repeater[delay=1,facing=east,locked=false,powered=false]\n  N minecraft:repeater[delay={d},facing=north,locked=false,powered=false]\n  M minecraft:repeater[delay={d},facing=south,locked=false,powered=false]\n  W minecraft:redstone_wire\nlayer 0:\n{}\nlayer 1:\n{}\n",
        l0.join("\n"),
        l1.join("\n")
    )
}

/// The design text of an instance: the U with n + 1 rows.
pub fn instance_text(name: &str, n: u32) -> Option<String> {
    let k = n as usize + 1;
    match name {
        "oneshot-u" => Some(oneshot_u_text(&format!("pulse-oneshot-u-n{n}"), k, 4)),
        "oneshot-u1" => Some(oneshot_u_text(&format!("pulse-oneshot-u1-n{n}"), k, 1)),
        "oneshot-rise" => Some(oneshot_text(&format!("pulse-oneshot-rise-n{n}"), "rising-edge", "rise", &vec![4; 2 * k])),
        "oneshot-ext" => Some(oneshot_text(&format!("pulse-oneshot-ext-n{n}"), "pulse-extender", "rise", &vec![4; 2 * k])),
        "oneshot-fall" => Some(oneshot_text(&format!("pulse-oneshot-fall-n{n}"), "falling-edge", "fall", &vec![4; 2 * k])),
        "dual-lock" => Some(dual_lock_text(&format!("pulse-dual-lock-n{n}"), k, 4)),
        "dual-lock1" => Some(dual_lock_text(&format!("pulse-dual-lock1-n{n}"), k, 1)),
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
    if a.name == "oneshot-mode" {
        // any mode, any even L: rise / fall / dual, the U delays split to fit
        let (Some(r), Some(dir)) = (&a.l_range, &a.out_dir) else { bail!("oneshot-mode needs --mode M --l-range a..b --out-dir D [--kind K]") };
        let (lo, hi) = r.split_once("..").ok_or_else(|| anyhow::anyhow!("--l-range a..b"))?;
        let (lo, hi): (u32, u32) = (lo.trim().parse()?, hi.trim().parse()?);
        std::fs::create_dir_all(dir)?;
        let base = match a.mode.as_str() {
            "fall" | "rise" => 2,
            _ => 2,
        };
        for l in (lo..=hi).filter(|l| l % 2 == 0) {
            if l < base + 4 {
                continue;
            }
            let delays = split_delays((l - base) / 2);
            let name = format!("pulse-oneshot-{}-{}-L{l}", a.mode, a.kind);
            let mut text = oneshot_text(&name, &a.kind, &a.mode, &delays);
            let how = match a.mode.as_str() {
                "fall" => "a sticky piston pushes an observer away and pulls it back on the falling edge, where its back faces a copper bulb: it fires on landing and lights the bulb",
                "dual" => "an observer watching the input fires on each edge into a copper bulb and lights it",
                _ => "a sticky piston pushes an observer whose back then faces a copper bulb: it fires on landing and lights the bulb; pulled back on the falling edge, it fires into nothing",
            };
            let at = text.find('\n').map(|i| i + 1).unwrap_or(0);
            text.insert_str(at, &format!("# One-shot ({}, L = {l} gt): {how}; a comparator reads the bulb into a U of repeaters that comes back into the bulb and turns it off after the loop's delay (a bulb toggles only on rising edges of its power, so the loop's own falling edge does nothing). Generator: mctas circuit pulse family oneshot-mode --mode {} --kind {} --l-range {l}..{l}.\n", a.kind, a.mode, a.kind));
            std::fs::write(dir.join(format!("{name}.txt")), text)?;
        }
        return Ok(());
    }
    if a.name == "oneshot-for" {
        let (Some(r), Some(dir)) = (&a.l_range, &a.out_dir) else { bail!("oneshot-for needs --l-range a..b --out-dir D [--kind K]") };
        let (lo, hi) = r.split_once("..").ok_or_else(|| anyhow::anyhow!("--l-range a..b"))?;
        let (lo, hi): (u32, u32) = (lo.trim().parse()?, hi.trim().parse()?);
        std::fs::create_dir_all(dir)?;
        for l in (lo..=hi).filter(|l| l % 2 == 0 && *l >= 6) {
            let Some(mut text) = oneshot_for(l, &a.kind) else { continue };
            let at = text.find('\n').map(|i| i + 1).unwrap_or(0);
            text.insert_str(at, &format!("# One-shot ({}, L = {l} gt): a sticky piston pushes an observer whose back then faces a copper bulb; the observer fires on landing and lights the bulb; a comparator reads the bulb into a U of repeaters that comes back into the bulb and turns it off after the loop's delay (a bulb toggles only on rising edges of its power, so the loop's falling edge does nothing); the falling edge pulls the observer back where its pulse reaches nothing (generator: mctas circuit pulse family oneshot-for --l-range {l}..{l} --kind {}).\n", a.kind, a.kind));
            std::fs::write(dir.join(format!("pulse-oneshot-{}-L{l}.txt", a.kind)), text)?;
        }
        return Ok(());
    }
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
            "extend family: pulse-{name}\nextend axis: {l}\nextend blocks: {b}\nextend rule: {rule}\nextend n: {lo}..{hi}\nextend base: {lo}\nextend generator: mctas circuit pulse family {name} --n {{n}} --out {{out}}\n",
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
