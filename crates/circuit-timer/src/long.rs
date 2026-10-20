//! `mctas circuit timer long`: two-stage comparator-ring timers for long L.
//!
//! A ring whose round is longer than the shortest charge pulse (4 gt) is refilled unevenly by it
//! and its output flickers as the stale slots die, so long rings cannot take the input directly.
//! Stage 1 is the 13-block two-comparator ring (round 4 gt: a 4 gt pulse fills it) whose output
//! then lasts at least 28 gt; a dust beside its comparator's stone A1 points into stage 2's stone
//! I2, and stage 2 is a ring of 2k + 1 comparators (round 4k + 2 gt) that loses 1 per round
//! (its last comparator outputs into a dust, the next dust points into I2), refilled evenly by
//! stage 1's long output. Once stage 1 dies, stage 2 counts its 14 levels down: L grows by about
//! 56 gt per k. What charges stage 1 (`--front`): the input beside a dust of its return line (an
//! off-delay) or the copper-bulb two-observer edge front (a retriggerable monostable).

use crate::rings::Front;
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

type P3 = (i32, i32, i32);

/// The design text of the two-stage timer with `k` comparators per stage-2 row.
pub fn design(name: &str, k: i32, front: Front) -> String {
    let mut cells: BTreeMap<P3, char> = BTreeMap::new();
    // place a block on the ring plane (y = 1), with a stone under comparators and dust
    let mut put = |x: i32, z: i32, c: char| {
        cells.insert((x, 1, z), c);
        if matches!(c, 'E' | 'W' | 'n' | 's' | 'd' | 'L' | 'M') {
            cells.insert((x, 0, z), 'S');
        }
    };
    // stage 1: C1 reads I1 and powers A1, C2 reads A1 and powers T1, three dust back into I1
    put(0, 0, 'S'); // A1
    put(1, 0, 'E'); // C1
    put(2, 0, 'S'); // I1
    put(0, 1, 'n'); // C2
    put(0, 2, 'S'); // T1
    put(1, 2, 'd');
    put(2, 2, 'd');
    put(2, 1, 'd');
    let output = if front == Front::NonRetrig {
        // stage 2 north of stage 1, its stone A2 beside the gate dust (0, -1) so the gate reads
        // stage 1 OR stage 2: every edge is ignored until the output itself has ended
        put(0, -2, 'S'); // A2
        for i in 0..k {
            put(-k + i, -2, 'W'); // the bottom row, flowing east into A2
            put(-1 - i, -4, 'E'); // the top row, flowing west from B2
        }
        put(0, -3, 's'); // c2, reads A2, powers B2
        put(0, -4, 'S'); // B2
        put(-k - 1, -2, 'S'); // I2
        put(-k - 1, -4, 'd'); // T2: the top row's last comparator outputs into this dust
        put(-k - 1, -3, 'd'); // and this one points into I2: a loss of 1 per round
        // the link: a repeater reading A1 (15 whatever stage 1's level), dust west along z = 0,
        // then a repeater north into I2
        put(-1, 0, 'L');
        for x in -k - 1..=-2 {
            put(x, 0, 'd');
        }
        put(-k - 1, -1, 'M');
        (1, 1, -2)
    } else {
        // the link: a repeater reading A1 into I2, so stage 2 is refilled at 15 whatever stage
        // 1's level (a dust would refill it at stage 1's decaying level, unevenly)
        put(-1, 0, 'L');
        // stage 2
        put(-2, 0, 'S'); // I2
        for i in 0..k {
            put(-3 - i, 0, 'E'); // the bottom row, flowing west
            put(-2 - k + i, -2, 'W'); // the top row, flowing east
        }
        put(-3 - k, 0, 'S'); // A2
        put(-3 - k, -1, 's'); // c2, reads A2, powers B2
        put(-3 - k, -2, 'S'); // B2
        put(-2, -2, 'd'); // T2: the top row's last comparator outputs into this dust
        put(-2, -1, 'd'); // and this one points into I2: a loss of 1 per round
        (-4 - k, 1, 0)
    };
    let (input, how) = match front {
        Front::Input => ((1, 1, 3), "the input beside the corner dust of stage 1's return line holds stage 1 up".to_string()),
        Front::Retrig => ((6, 3, 0), "each rising edge charges stage 1 with 4 gt of 15 (copper bulb, comparator K, observer O1 down into a stone and a cross dust, observer O2 watching O1 into the dust on top of I1)".to_string()),
        Front::NonRetrig => ((2, 1, -6), "a rising edge charges stage 1 through a gate (copper bulb, comparator K, observer O1, a delay-3 repeater, a subtract comparator into I1 whose side a repeater feeds from the dust between stage 1's stone A1 and stage 2's stone A2: every edge is ignored until the output itself has ended); stage 2 sits north of stage 1, fed through a repeater, a dust line and a second repeater".to_string()),
    };
    for (p, c) in front.blocks() {
        cells.insert(p, c);
    }
    let all: Vec<P3> = cells.keys().copied().chain([input, output]).collect();
    let (x0, x1) = (all.iter().map(|p| p.0).min().unwrap(), all.iter().map(|p| p.0).max().unwrap());
    let y1 = all.iter().map(|p| p.1).max().unwrap();
    let (z0, z1) = (all.iter().map(|p| p.2).min().unwrap(), all.iter().map(|p| p.2).max().unwrap());
    let used: std::collections::BTreeSet<char> = cells.values().copied().collect();
    let palette: String = [
        ('S', "minecraft:stone"),
        ('d', "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]"),
        ('E', "minecraft:comparator[facing=east,mode=compare,powered=false]"),
        ('W', "minecraft:comparator[facing=west,mode=compare,powered=false]"),
        ('n', "minecraft:comparator[facing=north,mode=compare,powered=false]"),
        ('s', "minecraft:comparator[facing=south,mode=compare,powered=false]"),
        ('L', "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]"),
        ('M', "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]"),
        ('N', "minecraft:comparator[facing=north,mode=compare,powered=false]"),
        ('G', "minecraft:comparator[facing=north,mode=subtract,powered=false]"),
        ('o', "minecraft:observer[facing=north,powered=false]"),
        ('R', "minecraft:repeater[delay=3,facing=north,locked=false,powered=false]"),
        ('r', "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
        ('O', "minecraft:observer[facing=east,powered=false]"),
        ('U', "minecraft:observer[facing=up,powered=false]"),
        ('B', "minecraft:waxed_copper_bulb[lit=false,powered=false]"),
    ]
    .iter()
    .filter(|(c, _)| used.contains(c))
    .map(|(c, s)| format!("  {c} {s}\n"))
    .collect();
    let mut layers = String::new();
    for y in 0..=y1 {
        layers.push_str(&format!("layer {y}:\n"));
        for z in z0..=z1 {
            layers.push_str("  ");
            for x in x0..=x1 {
                layers.push(cells.get(&(x, y, z)).copied().unwrap_or('.'));
            }
            layers.push('\n');
        }
    }
    let (kind, group, title, axis, blocks, nmax, front_arg) = match front {
        Front::Input => ("off-delay", "off-delay-2stage", "two-stage comparator-ring off-delay", "52 + 56n", "24 + 4n", 7, ""),
        Front::Retrig => ("monostable retriggerable", "mono-retrig-2stage", "two-stage comparator-ring retriggerable monostable", "60 + 56n", "32 + 4n", 8, " --front retrig"),
        Front::NonRetrig => ("monostable non-retriggerable", "mono-nonretrig-2stage", "two-stage comparator-ring non-retriggerable monostable", "62 + 56n", "38 + 6n", 8, " --front nonretrig"),
    };
    let rule = match front {
        Front::NonRetrig => "one more comparator (on its stone) in each row of stage 2's ring and one more dust (on its stone) in the link",
        _ => "one more comparator (on its stone) in each row of stage 2's ring",
    };
    let extend = format!(
        "extend family: {group}\nextend axis: {axis}\nextend blocks: {blocks}\nextend rule: {rule}\nextend n: 1..{nmax}\nextend base: {k}\nextend generator: mctas circuit timer long --k {{n}} --file {{out}}{front_arg}\n"
    );
    format!(
        "name: {name}\ncircuit: {kind}\ngroup: {group}\ngroup title: {title}\ngroup note: stage 2 has 2k + 1 comparators (k per row): L grows by 56 gt per k, up to k = {nmax} (a longer stage-2 round than stage 1's shortest output is refilled unevenly); `mctas circuit timer long --k K`\n{extend}# Two comparator rings (crates/circuit-timer/src/long.rs). Stage 1 (two comparators, round 4 gt): {how}; once its charge stops it decays 2 per round, so its output lasts at least 28 gt. A repeater reading its stone A1 powers stage 2's stone I2: stage 2 is a ring of {} comparators (round {} gt) that loses 1 per round, refilled evenly at 15 by stage 1's long output; once stage 1 dies it counts its levels down, and the output lamp beside its stone A2 goes dark at 0.\norigin: {x0} 0 {z0}\nport input: {} {} {}\nport output: {} {} {}\npalette:\n{palette}{layers}",
        2 * k + 1,
        4 * k + 2,
        input.0,
        input.1,
        input.2,
        output.0,
        output.1,
        output.2
    )
}

pub fn run(out: Option<&Path>, file: Option<&Path>, ks: &[i32], front: Front) -> Result<()> {
    let prefix = format!("{}-2stage", front.prefix());
    if let Some(file) = file {
        // one design to one file (the `extend generator:` form)
        let [k] = ks else { anyhow::bail!("--file takes exactly one --k") };
        let name = format!("{prefix}-k{k}");
        if let Some(p) = file.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(file, design(&name, *k, front))?;
        return Ok(());
    }
    let out = out.ok_or_else(|| anyhow::anyhow!("--out DIR or --file FILE"))?;
    std::fs::create_dir_all(out)?;
    for &k in ks {
        let name = format!("{prefix}-k{k}");
        let path = out.join(format!("{name}.txt"));
        std::fs::write(&path, design(&name, k, front))?;
        println!("{}", path.display());
    }
    Ok(())
}
