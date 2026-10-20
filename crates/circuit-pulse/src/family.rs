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
        name: "oneshot-bulb-rise",
        kind: "rising-edge",
        l: "18 + 16n",
        blocks: "19 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "the input toggles a copper bulb on every rising edge (even 1 gt), a comparator reads it and an observer watching that comparator fires once per rising edge into a second bulb; a comparator reads the second bulb into a U of delay-4 repeaters back into it, which turns it off one loop later",
    },
    Family {
        name: "oneshot-bulb-ext",
        kind: "pulse-extender",
        l: "18 + 16n",
        blocks: "19 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "the bulb-triggered one-shot (oneshot-bulb-rise) judged as an extender",
    },
    Family {
        name: "rise-psub-u4",
        kind: "rising-edge",
        l: "13 + 16n",
        blocks: "25 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "the piston-subtract limiter (limiter-psub-u4) judged as a rising-edge detector",
    },
    Family {
        name: "fall-psub-u4",
        kind: "falling-edge",
        l: "15 + 16n",
        blocks: "22 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "a sticky piston the input powers pushes a redstone block out from behind a subtract comparator and pulls it back on the falling edge (an odd delay: the block lands in the block-entity phase); the comparator's side gets the input inverted by a redstone torch and delayed by a U of delay-4 repeaters (an even delay), so after the falling edge the output is on for an odd number of ticks",
    },
    Family {
        name: "limiter-psub-u4",
        kind: "pulse-limiter",
        l: "13 + 16n",
        blocks: "25 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "a sticky piston pushes a redstone block behind a subtract comparator (an odd delay: the piston moves in the tick after the input, the block lands in the block-entity phase); the input reaches the comparator's side through a U of delay-4 repeaters (an even delay), so the output is on for an odd number of ticks",
    },
    Family {
        name: "limiter-psub-u1",
        kind: "pulse-limiter",
        l: "?",
        blocks: "?",
        rule: "lengthen the repeater U by one row: one more delay-1 repeater (on stone) in each column",
        how: "limiter-psub-u4 with delay-1 repeaters",
    },
    Family {
        name: "dual-edge-reset",
        kind: "dual-edge",
        l: "30 + 16n",
        blocks: "32 + 4n",
        rule: "lengthen the repeater U by one row: one more delay-4 repeater (on stone) in each column",
        how: "an observer watching the input fires on both edges through a repeater into a copper bulb (locked by a comparator reading the bulb while it is lit); the comparator also feeds a U of delay-4 repeaters into a second bulb, read by a comparator that an observer watches: its back faces the first bulb, so each loop sends one 2 gt reset pulse that turns the bulb off",
    },
    Family {
        name: "dual-edge-reset1",
        kind: "dual-edge",
        l: "?",
        blocks: "?",
        rule: "lengthen the repeater U by one row: one more delay-1 repeater (on stone) in each column",
        how: "dual-edge-reset with delay-1 repeaters",
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

/// The bulb-triggered one-shot (min-pulse 1): the input toggles a first copper bulb on every rising
/// edge, a comparator reads it and an observer watching that comparator fires once per rising
/// edge into the one-shot's bulb; a second comparator reads that bulb into a U of repeaters back
/// into it, which turns it off one loop later.
pub fn oneshot_bulb_text(name: &str, kind: &str, delays: &[u8]) -> String {
    let k = delays.len() / 2;
    // x from -1 (input) to 5; z from 0 to k + 1
    let w = 7usize;
    let rows = k + 2;
    let mut l0 = vec![vec!['.'; w]; rows];
    let mut l1 = vec![vec!['.'; w]; rows];
    let at = |x: i32| (x + 1) as usize;
    l1[0][at(0)] = 'A';
    l1[0][at(1)] = 'K';
    l0[0][at(1)] = 'S';
    l1[0][at(2)] = 'V';
    l1[0][at(3)] = 'B';
    l1[0][at(4)] = 'C';
    l0[0][at(4)] = 'S';
    l1[0][at(5)] = 'W';
    l0[0][at(5)] = 'S';
    let mut palette = vec![
        "  S minecraft:stone".to_string(),
        "  A minecraft:waxed_copper_bulb[lit=false,powered=false]".into(),
        "  K minecraft:comparator[facing=west,mode=compare,powered=false]".into(),
        "  V minecraft:observer[facing=west,powered=false]".into(),
        "  B minecraft:waxed_copper_bulb[lit=false,powered=false]".into(),
        "  C minecraft:comparator[facing=west,mode=compare,powered=false]".into(),
        "  W minecraft:redstone_wire".into(),
    ];
    for j in 0..k {
        let z = 1 + j;
        let (cd, cu) = ((b'a' + j as u8) as char, (b'm' + j as u8) as char);
        palette.push(format!("  {cd} minecraft:repeater[delay={},facing=north,locked=false,powered=false]", delays[j]));
        palette.push(format!("  {cu} minecraft:repeater[delay={},facing=south,locked=false,powered=false]", delays[2 * k - 1 - j]));
        l1[z][at(5)] = cd;
        l0[z][at(5)] = 'S';
        l1[z][at(3)] = cu;
        l0[z][at(3)] = 'S';
    }
    for x in 3..=5 {
        l1[k + 1][at(x)] = 'W';
        l0[k + 1][at(x)] = 'S';
    }
    let rowtext = |l: &Vec<Vec<char>>| l.iter().map(|r| format!("  {}", r.iter().collect::<String>())).collect::<Vec<_>>().join("\n");
    format!(
        "name: {name}\ncircuit: {kind}\norigin: -1 0 0\nport input: -1 1 0\nport output: 4 1 0\npalette:\n{}\nlayer 0:\n{}\nlayer 1:\n{}\n",
        palette.join("\n"),
        rowtext(&l0),
        rowtext(&l1)
    )
}

/// The locked dual-edge one-shot with an EDGE reset (min-pulse about L): as `dual_lock_text`, but
/// the repeater U ends in a second copper bulb read by a comparator that an observer watches; the
/// observer's back faces the main bulb, so the bulb gets a 2 gt reset pulse once per loop (the
/// loop's rising edge toggles the second bulb, its falling edge does nothing) instead of staying
/// powered by the loop: the falling edge's trigger is answered as soon as the lock lets it go.
pub fn dual_lock2_text(name: &str, k: usize, d: u8) -> String {
    // x from 0 to 4; z from -(k+2) to 3 (rows listed from the smallest z)
    let zmin = -(k as i32 + 2);
    let mut l0: Vec<String> = Vec::new();
    let mut l1: Vec<String> = Vec::new();
    for z in zmin..=3 {
        let (r0, r1) = match z {
            3 => (".....", "....."),
            2 => (".....", "..V.."),
            1 => ("..SSS", "..TLW"),
            0 => ("S..SS", "KOBCW"),
            -1 => ("....S", "A...M"),
            z if z == zmin => ("SSSSS", "WWWWW"),
            _ => ("S...S", "N...M"),
        };
        l0.push(format!("  {r0}"));
        l1.push(format!("  {r1}"));
    }
    format!(
        "name: {name}\ncircuit: dual-edge\norigin: 0 0 {zmin}\nport input: 2 1 3\nport output: 3 1 0\npalette:\n  S minecraft:stone\n  V minecraft:observer[facing=south,powered=false]\n  T minecraft:repeater[delay=1,facing=south,locked=false,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\n  C minecraft:comparator[facing=west,mode=compare,powered=false]\n  L minecraft:repeater[delay=1,facing=east,locked=false,powered=false]\n  A minecraft:waxed_copper_bulb[lit=false,powered=false]\n  K minecraft:comparator[facing=north,mode=compare,powered=false]\n  O minecraft:observer[facing=west,powered=false]\n  N minecraft:repeater[delay={d},facing=north,locked=false,powered=false]\n  M minecraft:repeater[delay={d},facing=south,locked=false,powered=false]\n  W minecraft:redstone_wire\nlayer 0:\n{}\nlayer 1:\n{}\n",
        l0.join("\n"),
        l1.join("\n")
    )
}

/// Odd pulses by overlap: two detectors 1 gt apart, each through a delay-`d` repeater into one
/// dust, give 2d + 1 gt. `mode` rise: a bulb edge (+4) and a pushed observer (+5); fall: a pulled
/// observer (+5) and a torch-relit bulb edge (+6); 12 / 16 blocks.
pub fn odd_text(name: &str, kind: &str, mode: &str, d: u8) -> String {
    let l = 2 * d as u32 + 1;
    let how = match mode {
        "fall" => "two falling-edge detectors 1 gt apart: a sticky piston pulls back an observer that fires 2 gt after it lands (5 gt after the edge), and a redstone torch the input turns off (through dust into its block) lights again 2 gt after the edge and toggles a copper bulb, read by a comparator an observer watches (6 gt after the edge)",
        _ => "two rising-edge detectors 1 gt apart: a copper bulb the input toggles on every rising edge, read by a comparator an observer watches (fires 4 gt after the edge), and a sticky piston pushing an observer that fires 2 gt after it lands (5 gt after the edge; pulled back on the falling edge it fires into an empty cell)",
    };
    let head = format!(
        "name: {name}\ncircuit: {kind}\n# Odd pulses ({kind}, L = {l} gt): {how}; each feeds a delay-{d} repeater ({} gt pulses) into one dust, where the two pulses overlap into {} + 1 = {l} gt.\n",
        2 * d,
        2 * d
    );
    let pal = format!(
        "palette:\n  S minecraft:stone\n  W minecraft:redstone_wire\n  T minecraft:redstone_wall_torch[facing=south,lit=true]\n  A minecraft:waxed_copper_bulb[lit=false,powered=false]\n  K minecraft:comparator[facing=north,mode=compare,powered=false]\n  V minecraft:observer[facing=west,powered=false]\n  O minecraft:observer[facing=north,powered=false]\n  P minecraft:sticky_piston[extended=false,facing=east]\n  R minecraft:repeater[delay={d},facing=north,locked=false,powered=false]\n  Q minecraft:repeater[delay={d},facing=west,locked=false,powered=false]\n"
    );
    match mode {
        "fall" => format!("{head}origin: -2 0 -1\nport input: 0 1 -1\nport output: 1 1 2\n{pal}layer 0:\n  .S...\n  .....\n  ...S.\n  S.SS.\nlayer 1:\n  SW...\n  T.PO.\n  A..R.\n  KVQW.\n"),
        _ => format!("{head}origin: -1 0 0\nport input: 0 1 1\nport output: 2 1 2\n{pal}layer 0:\n  ....\n  ...S\n  S.SS\nlayer 1:\n  .PO.\n  A..R\n  KVQW\n"),
    }
}

/// The piston-subtract odd limiter: a subtract comparator's rear gets a redstone block a sticky
/// piston pushes behind it (an odd delay: the piston moves the tick after the input, the block
/// lands in the block-entity phase), its side the input through two repeaters (delays d1, d2):
/// L = 2 (d1 + d2) - 3.
pub fn limiter_piston_text(name: &str, kind: &str, d1: u8, d2: u8) -> String {
    let l = 2 * (d1 as i32 + d2 as i32) - 3;
    format!(
        "name: {name}\ncircuit: {kind}\n# Odd pulses ({kind}, L = {l} gt): a subtract comparator's rear gets a redstone block that a sticky piston pushes behind it (the piston moves in the tick after the input changes and the block lands in the block-entity phase: an odd delay), its side gets the input through two repeaters (delays {d1} and {d2}: an even delay); the output is on from the rear's arrival until the side's: L = 2 x ({d1} + {d2}) - 3 = {l} gt. An observer watching the input holds the piston through dust for 2 gt more, so a 1 gt input cannot make it leave its block behind the comparator (its retraction is cancelled until the block has landed).\norigin: -2 0 0\nport input: 0 1 0\nport output: 4 1 1\npalette:\n  S minecraft:stone\n  W minecraft:redstone_wire\n  O minecraft:observer[facing=east,powered=false]\n  A minecraft:sticky_piston[extended=false,facing=east]\n  B minecraft:redstone_block\n  C minecraft:comparator[facing=west,mode=subtract,powered=false]\n  Q minecraft:repeater[delay={d1},facing=west,locked=false,powered=false]\n  U minecraft:repeater[delay={d2},facing=west,locked=false,powered=false]\nlayer 0:\n  S..SSS.\n  SS...S.\nlayer 1:\n  WO.QUW.\n  WWAB.C.\n"
    )
}

/// The piston-subtract limiter with a repeater U on the side (an extendable odd family): the input
/// pushes a redstone block behind a subtract comparator with a sticky piston (odd delay) and
/// reaches the comparator's side through a U of k rows of delay-`d` repeaters (even delay).
pub fn limiter_psub_u_text(name: &str, kind: &str, k: usize, d: u8) -> String {
    // x from -2 to 4 (x = 4: the output; x = -2, -1: the observer hold); z from -1 to k + 1
    let mut l0: Vec<String> = Vec::new();
    let mut l1: Vec<String> = Vec::new();
    for z in -1..=(k as i32 + 1) {
        let (r0, r1) = match z {
            -1 => ("SS...S.", "WWAB.C."),
            0 => ("S....S.", "WO...W."),
            z if z == k as i32 + 1 => ("..SSSS.", "..WWWW."),
            _ => ("..S..S.", "..N..M."),
        };
        l0.push(format!("  {r0}"));
        l1.push(format!("  {r1}"));
    }
    format!(
        "name: {name}\ncircuit: {kind}\norigin: -2 0 -1\nport input: 0 1 0\nport output: 4 1 -1\npalette:\n  S minecraft:stone\n  W minecraft:redstone_wire\n  O minecraft:observer[facing=east,powered=false]\n  A minecraft:sticky_piston[extended=false,facing=east]\n  B minecraft:redstone_block\n  C minecraft:comparator[facing=west,mode=subtract,powered=false]\n  N minecraft:repeater[delay={d},facing=north,locked=false,powered=false]\n  M minecraft:repeater[delay={d},facing=south,locked=false,powered=false]\nlayer 0:\n{}\nlayer 1:\n{}\n",
        l0.join("\n"),
        l1.join("\n")
    )
}

/// The falling-edge piston-subtract: a sticky piston the input powers pushes a redstone block out
/// from behind a subtract comparator and pulls it back on the falling edge (it lands in the
/// block-entity phase: odd delay); the side gets the input inverted by a torch (through dust into
/// its block) and delayed by two repeaters: L = 2 (d1 + d2) - 1.
pub fn fall_psub_text(name: &str, d1: u8, d2: u8) -> String {
    let l = 2 * (d1 as i32 + d2 as i32) - 1;
    format!(
        "name: {name}\ncircuit: falling-edge\n# Odd falling-edge pulses (L = {l} gt): a sticky piston the input powers pushes a redstone block out from behind a subtract comparator and pulls it back on the falling edge (the piston moves the tick after the input, the block lands in the block-entity phase: an odd delay); the comparator's side gets the input inverted by a redstone torch (through dust into its block) and delayed by two repeaters ({d1} and {d2}: an even delay), so after the falling edge the output is on from the block's return until the torch's signal arrives: L = 2 x ({d1} + {d2}) - 1 = {l} gt.\norigin: -2 0 -1\nport input: 0 1 -1\nport output: 2 1 1\npalette:\n  S minecraft:stone\n  W minecraft:redstone_wire\n  T minecraft:redstone_wall_torch[facing=south,lit=true]\n  A minecraft:sticky_piston[extended=false,facing=east]\n  B minecraft:redstone_block\n  C minecraft:comparator[facing=north,mode=subtract,powered=false]\n  Q minecraft:repeater[delay={d1},facing=west,locked=false,powered=true]\n  U minecraft:repeater[delay={d2},facing=west,locked=false,powered=true]\nlayer 0:\n  .S...\n  .SSSS\nlayer 1:\n  SW.AB\n  TQUWC\n"
    )
}

/// The falling-edge piston-subtract with a repeater U after the torch (an extendable odd family).
pub fn fall_psub_u_text(name: &str, k: usize, d: u8) -> String {
    let mut l0: Vec<String> = Vec::new();
    let mut l1: Vec<String> = Vec::new();
    for z in -1..=(k as i32 + 1) {
        let (r0, r1) = match z {
            -1 => (".S...", "SW.AB"),
            0 => ("...SS", "T..WC"),
            z if z == k as i32 + 1 => ("SSSS.", "WWWW."),
            _ => ("S..S.", "N..M."),
        };
        l0.push(format!("  {r0}"));
        l1.push(format!("  {r1}"));
    }
    format!(
        "name: {name}\ncircuit: falling-edge\norigin: -2 0 -1\nport input: 0 1 -1\nport output: 2 1 1\npalette:\n  S minecraft:stone\n  W minecraft:redstone_wire\n  T minecraft:redstone_wall_torch[facing=south,lit=true]\n  A minecraft:sticky_piston[extended=false,facing=east]\n  B minecraft:redstone_block\n  C minecraft:comparator[facing=north,mode=subtract,powered=false]\n  N minecraft:repeater[delay={d},facing=north,locked=false,powered=true]\n  M minecraft:repeater[delay={d},facing=south,locked=false,powered=true]\nlayer 0:\n{}\nlayer 1:\n{}\n",
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
        "dual-edge-reset" => Some(dual_lock2_text(&format!("pulse-dual-edge-reset-n{n}"), k, 4)),
        "limiter-psub-u4" => Some(limiter_psub_u_text(&format!("pulse-limiter-psub-u4-n{n}"), "pulse-limiter", k, 4)),
        "rise-psub-u4" => Some(limiter_psub_u_text(&format!("pulse-rise-psub-u4-n{n}"), "rising-edge", k, 4)),
        "fall-psub-u4" => Some(fall_psub_u_text(&format!("pulse-fall-psub-u4-n{n}"), k, 4)),
        "limiter-psub-u1" => Some(limiter_psub_u_text(&format!("pulse-limiter-psub-u1-n{n}"), "pulse-limiter", k, 1)),
        "dual-edge-reset1" => Some(dual_lock2_text(&format!("pulse-dual-edge-reset1-n{n}"), k, 1)),
        "oneshot-bulb-rise" => Some(oneshot_bulb_text(&format!("pulse-oneshot-bulb-rise-n{n}"), "rising-edge", &vec![4; 2 * k])),
        "oneshot-bulb-ext" => Some(oneshot_bulb_text(&format!("pulse-oneshot-bulb-ext-n{n}"), "pulse-extender", &vec![4; 2 * k])),
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

/// The tunable group a generated design belongs to (stamped as its `group:` header).
pub fn group_of(family: &str, kind: &str) -> &'static str {
    match (family, kind) {
        ("limiter-u" | "limiter-u4" | "limiter-for", _) => "pulse-limiter-u",
        ("oneshot-ext" | "oneshot-for" | "oneshot-mode-rise", "pulse-extender") => "pulse-oneshot-ext",
        ("oneshot-rise" | "oneshot-u" | "oneshot-u1" | "oneshot-for" | "oneshot-mode-rise", _) => "pulse-oneshot-rise",
        ("oneshot-fall" | "oneshot-mode-fall", _) => "pulse-oneshot-fall",
        ("oneshot-mode-dual", _) => "pulse-oneshot-dual",
        ("dual-lock" | "dual-lock1", _) => "pulse-dual-lock",
        ("dual-edge-reset" | "dual-edge-reset1", _) => "pulse-dual-edge-reset",
        ("limiter-psub-u4" | "limiter-psub-u1", _) => "pulse-limiter-psub",
        ("rise-psub-u4", _) => "pulse-rise-psub",
        ("fall-psub-u4", _) => "pulse-fall-psub",
        ("oneshot-bulb-rise", _) => "pulse-oneshot-bulb-rise",
        ("oneshot-bulb-ext", _) => "pulse-oneshot-bulb-ext",
        _ => "pulse-other",
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
            let text = crate::stamp::stamp(&text, group_of(&format!("oneshot-mode-{}", a.mode), &a.kind), None, None);
            std::fs::write(dir.join(format!("{name}.txt")), text)?;
        }
        return Ok(());
    }
    if a.name == "odd" {
        // odd L by overlap, L = 3, 5, 7, 9 (repeater delay 1..4): --mode rise|fall --kind K
        let Some(dir) = &a.out_dir else { bail!("odd needs --mode rise|fall|piston --kind K --out-dir D") };
        std::fs::create_dir_all(dir)?;
        if a.mode == "piston" {
            // the piston-subtract limiter: L = 2 (d1 + d2) - 3 = 1, 3, ..., 13
            let short = if a.kind == "rising-edge" { "rise-psub" } else { "limiter-psub" };
            if a.kind == "falling-edge" {
                // the falling-edge version: L = 2 (d1 + d2) - 1 = 3, ..., 15
                for s in 2..=8u8 {
                    let (d1, d2) = (s / 2, s - s / 2);
                    let name = format!("pulse-fall-psub-L{}", 2 * s as i32 - 1);
                    let text = crate::stamp::stamp(&fall_psub_text(&name, d1, d2), "pulse-fall-psub", None, None);
                    std::fs::write(dir.join(format!("{name}.txt")), text)?;
                }
                return Ok(());
            }
            for s in 2..=8u8 {
                let (d1, d2) = (s / 2, s - s / 2);
                let name = format!("pulse-{short}-L{}", 2 * s as i32 - 3);
                let text = limiter_piston_text(&name, &a.kind, d1, d2);
                let text = crate::stamp::stamp(&text, &format!("pulse-{short}"), None, None);
                std::fs::write(dir.join(format!("{name}.txt")), text)?;
            }
            return Ok(());
        }
        let short = match (a.mode.as_str(), a.kind.as_str()) {
            ("fall", _) => "fall-odd2",
            (_, "pulse-limiter") => "limiter-odd2",
            (_, "pulse-extender") => "ext-odd2",
            _ => "rise-odd2",
        };
        for d in 1..=4u8 {
            let name = format!("pulse-{short}-L{}", 2 * d + 1);
            let text = odd_text(&name, &a.kind, &a.mode, d);
            let text = crate::stamp::stamp(&text, &format!("pulse-{short}"), None, None);
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
            let text = crate::stamp::stamp(&text, group_of("oneshot-for", &a.kind), None, None);
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
                let text = crate::stamp::stamp(&text, group_of("limiter-for", ""), None, None);
                std::fs::write(dir.join(format!("pulse-limiter-u-L{l}.txt")), text)?;
            }
            return Ok(());
        }
        let l = a.l.ok_or_else(|| anyhow::anyhow!("limiter-for needs --l"))?;
        let Some(mut text) = limiter_u_for(l) else { bail!("L must be even and >= 4") };
        let at = text.find('\n').map(|i| i + 1).unwrap_or(0);
        text.insert_str(at, &format!("# A subtract comparator's rear sees the input at once, its side gets it back through a U of repeaters (down one column, two dust across, up the other) whose delays add up to {l} gt; it outputs rear - side, so every input pulse is cut to {l} gt (generator: mctas circuit pulse family limiter-for --l {l}).\n"));
        let text = crate::stamp::stamp(&text, group_of("limiter-for", ""), None, None);
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
    let text = crate::stamp::stamp(&text, group_of(f.name, f.kind), None, None);
    match &a.out {
        Some(o) => std::fs::write(o, text)?,
        None => print!("{text}"),
    }
    Ok(())
}
