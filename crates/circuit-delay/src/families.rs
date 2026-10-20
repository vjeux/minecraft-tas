//! Delay-line families (generators): every chain runs east from the input cell (0, 1, 0) at
//! y = 1 on floors at y = 0; the output is the last element's own output or the marker cell after
//! it.

use crate::build::B;

/// A line of repeaters with the given delays (redstone ticks 1-4): N = 2 x sum, 2 blocks each.
pub fn repeaters(delays: &[u8], name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    for (i, d) in delays.iter().enumerate() {
        b.rep([1 + i as i32, 1, 0], "west", *d, false);
    }
    b.port("output", [delays.len() as i32, 1, 0]);
    b
}

/// A line of k compare-mode comparators: N = 2k, 2 blocks each.
pub fn comparators(k: usize, name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    for i in 0..k {
        b.cmp([1 + i as i32, 1, 0], "west", false, false);
    }
    b.port("output", [k as i32, 1, 0]);
    b
}

/// k observers in a line (the first watches the input cell) carry each input edge as a 2 gt
/// pulse; a waxed copper bulb toggles on every pulse (= the input level again) and a comparator
/// reads it: N = 2k + 2, k + 3 blocks.
pub fn observers(k: usize, name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    for i in 0..k {
        b.put([1 + i as i32, 1, 0], "observer[facing=west,powered=false]");
    }
    let k = k as i32;
    b.put([k + 1, 1, 0], "waxed_copper_bulb[lit=false,powered=false]");
    b.cmp([k + 2, 1, 0], "west", false, false);
    b.port("output", [k + 2, 1, 0]);
    b
}

/// The odd delay line: a sticky piston pushes a redstone block one cell (it arrives 3 gt after the
/// input rises and leaves 1 gt after it falls); the output cell touches the block's new cell, and a
/// repeater behind that cell feeds the output again 2 gt later through a stone and two dusts. The
/// OR of the 3/1 and 5/3 copies is a 3/3 delay. The piston must see the input change between ticks
/// (a repeater in front would trigger it inside the tick: 4/2 and an even N), so longer odd delays
/// put the OR on a dust and run repeaters (delays `tail`) after it: N = 3 + 2 x sum(tail),
/// 9 blocks, or 11 + 2 x len(tail) with a tail.
pub fn piston_or(tail: &[u8], name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, -3]);
    b.put([0, 1, -2], "sticky_piston[extended=false,facing=south]");
    b.put([0, 1, -1], "redstone_block");
    b.rep([1, 1, 0], "west", 1, false);
    b.stone([2, 1, 0]);
    b.dust([2, 1, 1]);
    b.dust([1, 1, 1]);
    if tail.is_empty() {
        b.port("output", [0, 1, 1]);
        return b;
    }
    b.dust([0, 1, 1]);
    for (i, d) in tail.iter().enumerate() {
        b.rep([-1 - i as i32, 1, 1], "east", *d, false);
    }
    b.port("output", [-(tail.len() as i32), 1, 1]);
    b
}

/// The fewest repeaters whose delays (1-4 redstone ticks) sum to `t`: 4s, then the rest.
pub fn split_ticks(t: u32) -> Vec<u8> {
    let mut v = vec![4u8; (t / 4) as usize];
    if t % 4 != 0 {
        v.insert(0, (t % 4) as u8);
    }
    v
}

/// An extendable family's card (docs/clocks.md "Extendable clocks", read by `circuit archive`).
pub struct Card {
    pub id: String,
    pub axis: String,
    pub blocks: String,
    pub rule: String,
    pub lo: u32,
    pub hi: u32,
    pub group: &'static str,
    pub gtitle: &'static str,
    pub gaxis: &'static str,
    pub gnote: &'static str,
}

/// The track's extendable families: rep2 rep4 rep6 rep8, cmp, or5 or7 or9 or11, torch.
pub const FAMILIES: [&str; 10] = ["rep2", "rep4", "rep6", "rep8", "cmp", "or5", "or7", "or9", "or11", "torch"];

fn residue(name: &str, prefix: &str, allowed: &[u32]) -> Option<u32> {
    let r: u32 = name.strip_prefix(prefix)?.parse().ok()?;
    allowed.contains(&r).then_some(r)
}

/// The card of family `name`.
pub fn card(name: &str) -> anyhow::Result<Card> {
    let rep = ("delay-repeater-line", "repeater line", "N = 2 x the sum of the delays", "2 blocks (repeater + stone) per started 8 gt");
    let or = ("delay-piston-or", "piston OR line (odd N)", "N = 3 + 2 x the tail's delays", "a sticky piston's 3/1 copy OR'ed with itself 2 gt later, then a repeater tail");
    if let Some(r) = residue(name, "rep", &[2, 4, 6, 8]) {
        return Ok(Card {
            id: format!("delay-rep{r}"),
            axis: format!("{r} + 8n"),
            blocks: "2 + 2n".into(),
            rule: "add one delay-4 repeater (on its stone) at the end".into(),
            lo: 0,
            hi: 10,
            group: rep.0,
            gtitle: rep.1,
            gaxis: rep.2,
            gnote: rep.3,
        });
    }
    if let Some(b) = residue(name, "or", &[5, 7, 9, 11]) {
        return Ok(Card {
            id: format!("delay-piston-or{b}"),
            axis: format!("{b} + 8n"),
            blocks: "13 + 2n".into(),
            rule: "add one delay-4 repeater (on its stone) at the end of the tail".into(),
            lo: 0,
            hi: 6,
            group: or.0,
            gtitle: or.1,
            gaxis: or.2,
            gnote: or.3,
        });
    }
    match name {
        "cmp" => Ok(Card {
            id: "delay-cmp".into(),
            axis: "2 + 2n".into(),
            blocks: "2 + 2n".into(),
            rule: "add one compare-mode comparator (on its stone) at the end".into(),
            lo: 0,
            hi: 10,
            group: "",
            gtitle: "",
            gaxis: "",
            gnote: "",
        }),
        "torch" => Ok(Card {
            id: "delay-torch".into(),
            axis: "6 + 4n".into(),
            blocks: "6 + 4n".into(),
            rule: "add two torches and two blocks to the tower".into(),
            lo: 0,
            hi: 4,
            group: "",
            gtitle: "",
            gaxis: "",
            gnote: "",
        }),
        _ => anyhow::bail!("unknown family {name:?} (one of {FAMILIES:?})"),
    }
}

/// Member n of family `name` and its caption.
pub fn member(name: &str, n: u32) -> anyhow::Result<(B, String)> {
    let c = card(name)?;
    let nm = format!("{}-n{n}", c.id);
    if let Some(r) = residue(name, "rep", &[2, 4, 6, 8]) {
        let mut d = vec![(r / 2) as u8];
        d.extend(std::iter::repeat_n(4u8, n as usize));
        let total = r + 8 * n;
        // plain first sentence (critique: the caption opened with family bookkeeping)
        let what = if n == 0 {
            format!("One repeater set to {}: the signal comes out {total} ticks later", r / 2)
        } else {
            format!("A line of {} repeaters, the first set to {} and the rest to 4: the signal comes out {total} ticks later", n + 1, r / 2)
        };
        return Ok((repeaters(&d, &nm), format!("{what}. Each repeater adds 2 ticks per notch of its setting, so any even delay is a line of them, 2 blocks (repeater + stone) per started 8 ticks (track delay, family {}: N = {} gt, n = {n})", c.id, c.axis)));
    }
    if let Some(b) = residue(name, "or", &[5, 7, 9, 11]) {
        let mut t = vec![((b - 3) / 2) as u8];
        t.extend(std::iter::repeat_n(4u8, n as usize));
        let total = b + 8 * n;
        return Ok((piston_or(&t, &nm), format!("Delay line N = {total} gt (track delay, family {}: N = {} gt, n = {n}): a sticky piston pushes a redstone block one cell (it arrives 3 gt after the input rises, leaves 1 gt after it falls); a dust next to its new cell is also fed 2 gt later by a repeater behind that cell (through a stone and two dusts): the OR of the 3/1 and 5/3 copies is a 3/3 delay; repeaters after the dust add the rest (delays {t:?})", c.id, c.axis)));
    }
    match name {
        "cmp" => Ok((comparators(n as usize + 1, &nm), format!("Comparator line N = {} gt (track delay, family delay-cmp: N = 2 + 2n gt, n = {n}): {} compare-mode comparator(s) in a row on stones, 2 gt each; keeps pulses down to 2-3 gt", 2 + 2 * n, n + 1))),
        "torch" => {
            let k = 2 + 2 * n as usize;
            Ok((torches(k, &nm), format!("Torch tower N = {} gt (track delay, family delay-torch: N = 6 + 4n gt, n = {n}): a repeater powers a block, {k} torches climb from it (each torch powers the block above it, the next torch stands on that block): N = 2 + 2 x {k} gt, non-inverting with an even count", 6 + 4 * n)))
        }
        _ => anyhow::bail!("unknown family {name:?}"),
    }
}

/// A repeater reading the input powers block B0; a tower of k torches climbs from it (each torch
/// strongly powers the block above it, the next torch stands on that block): N = 2 + 2k,
/// non-inverting for even k.
pub fn torches(k: usize, name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    b.rep([1, 1, 0], "west", 1, false);
    b.stone([2, 1, 0]);
    let mut y = 2;
    for i in 0..k {
        // the torch at y, the block above it at y + 1 (not after the last torch)
        b.torch([2, y, 0], i % 2 == 0);
        if i + 1 < k {
            b.stone([2, y + 1, 0]);
        }
        y += 2;
    }
    b.port("output", [2, y - 2, 0]);
    b
}
