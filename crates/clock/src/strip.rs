//! `mctas clock strip` (vjeux 2026-10-22: "I counted 19 of these clocks that are actually just basic
//! observer clocks but with a bunch of unnecessary blocks around them. A clock archive shouldn't be
//! filled with unnecessary clock slop."): an archived clock must be IRREDUCIBLE.
//!
//! Blocks are removed greedily on redsim, one at a time to a fixed point, then face-adjacent pairs
//! (a torch and the block it hangs on, the two halves of a door); a removal is kept only when what
//! is left is still a clock that gives a builder everything the original gives ([`Props`]): the
//! same builder period (the one-edge rule) and pulse widths at the declared output, a usable output
//! when the original's is usable, no new open hopper top (item intrusion), a sculk clock no less
//! vibration-proof. A declared port's block and the kick's block are never removed, nor entities.
//! What is left is the irreducible core. Robustness over positions and rotations is the game's
//! (`clock verify` of the core): a block redsim calls unnecessary may be what makes the clock
//! robust, so a core counts only once the game confirms it (`clock archive` does that before it
//! refuses a design as slop).

use crate::sim::{SimOptions, SimReport, Verdict, simulate};
use crate::{Clock, Pos};

/// What the archive judges of a clock on redsim: the properties a stripped core must keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Props {
    /// The builder's period: the period with one rising edge per cycle, else the even spacing.
    pub period: u32,
    /// The output's pulse widths as a builder takes them (one width when all are equal).
    pub widths: Vec<u32>,
    /// The usable-output rule at the declared output (`clock::usable::judge`).
    pub usable: bool,
    /// The open hopper tops a stray item can reach (`clock::intrusion::intakes`).
    pub intakes: Vec<Pos>,
    /// A sculk clock's ambient property on redsim (`None`: no sensor).
    pub vibration_proof: Option<bool>,
}

fn widths_of(w: &[u32]) -> Vec<u32> {
    if !w.is_empty() && w.iter().all(|&x| x == w[0]) { vec![w[0]] } else { w.to_vec() }
}

/// The redsim run with the builder's period and widths at the DECLARED output: `None` when it is no
/// clock, declares no output, or its output pulses unevenly.
fn quick(c: &Clock) -> Option<(SimReport, u32, Vec<u32>)> {
    c.output?;
    let r = simulate(c, &SimOptions::default());
    if r.verdict != Verdict::Clock {
        return None;
    }
    let o = r.output.as_ref().filter(|o| !o.auto)?;
    let w = &o.wave;
    if w.edges == 0 || w.irregular().is_some() {
        return None;
    }
    let period = if w.edges <= 1 { r.period } else { *w.gaps.first()? };
    let widths = widths_of(&w.widths);
    Some((r, period, widths))
}

fn usable_at(c: &Clock, r: &SimReport) -> bool {
    let gap = r.output.as_ref().and_then(|o| o.wave.gaps.first().copied());
    crate::usable::judge(c, crate::usable::Rule::default(), gap).is_ok()
}

/// The clock's [`Props`] on redsim (`None`: no clock with an even declared output there).
pub fn props(c: &Clock) -> Option<Props> {
    let (r, period, widths) = quick(c)?;
    Some(Props { period, widths, usable: usable_at(c, &r), intakes: crate::intrusion::intakes(&c.design, &[]), vibration_proof: r.vibration.as_ref().map(|v| v.proof) })
}

/// Whether `c` gives a builder everything `base` describes (cheapest checks first).
pub fn keeps(base: &Props, c: &Clock) -> bool {
    let Some((r, period, widths)) = quick(c) else { return false };
    if period != base.period || widths != base.widths {
        return false;
    }
    if crate::intrusion::intakes(&c.design, &[]).iter().any(|p| !base.intakes.contains(p)) {
        return false;
    }
    // no sensor left: nothing for an ambient event to disturb
    let proof = r.vibration.as_ref().map(|v| v.proof).unwrap_or(true);
    if base.vibration_proof == Some(true) && !proof {
        return false;
    }
    !base.usable || usable_at(c, &r)
}

/// The cells strip never empties: the declared ports' and the kick's.
pub fn kept_cells(c: &Clock) -> Vec<Pos> {
    let mut v: Vec<Pos> = c.design.ports.values().copied().collect();
    if let Some(k) = &c.kick {
        v.push(k.pos());
    }
    v
}

fn facing_offset(state: &str) -> Option<Pos> {
    let f = state.split("facing=").nth(1)?.split([',', ']']).next()?;
    Some(match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}

/// An extended piston's head cell (the head is part of the piston: they go together).
fn head_of(p: Pos, s: &str) -> Option<Pos> {
    let piston = s.starts_with("minecraft:piston[") || s.starts_with("minecraft:sticky_piston[");
    if !piston || !s.contains("extended=true") {
        return None;
    }
    let o = facing_offset(s)?;
    Some([p[0] + o[0], p[1] + o[1], p[2] + o[2]])
}

/// The clock without the blocks at `cells` (an extended piston with its head, a block with its
/// block-entity data); `None` when nothing would be left or it no longer parses.
pub fn without(c: &Clock, cells: &[Pos]) -> Option<Clock> {
    let mut d = c.design.clone();
    for p in cells {
        if let Some(s) = d.blocks.remove(p) {
            d.block_entities.remove(p);
            if let Some(h) = head_of(*p, &s)
                && d.blocks.get(&h).is_some_and(|x| x.starts_with("minecraft:piston_head"))
            {
                d.blocks.remove(&h);
            }
        }
    }
    if d.blocks.is_empty() {
        return None;
    }
    Clock::from_design(d).ok()
}

/// A stripped clock.
#[derive(Clone, Debug)]
pub struct Stripped {
    pub core: Clock,
    /// The blocks removed, in order (a piston's head goes with it, unlisted).
    pub removed: Vec<(Pos, String)>,
    /// Removals tried (each a redsim run or more).
    pub tries: usize,
}

impl Stripped {
    /// `x,y,z state; ...` (states shortened).
    pub fn removed_text(&self) -> String {
        self.removed.iter().map(|(p, s)| format!("{},{},{} {}", p[0], p[1], p[2], crate::short(s))).collect::<Vec<_>>().join("; ")
    }
}

fn candidates(c: &Clock, keep: &[Pos]) -> Vec<(Pos, String)> {
    c.design.blocks.iter().filter(|(p, s)| !keep.contains(p) && !s.starts_with("minecraft:piston_head")).map(|(p, s)| (*p, s.clone())).collect()
}

fn adjacent(a: Pos, b: Pos) -> bool {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs() == 1
}

/// Strip `c` to its irreducible core on redsim: single blocks to a fixed point, then (with `pairs`)
/// face-adjacent pairs, then singles again. `None` when the original is no clock with an even
/// declared output on redsim (nothing to compare a core with).
pub fn strip(c: &Clock, pairs: bool) -> Option<Stripped> {
    let base = props(c)?;
    let keep = kept_cells(c);
    let mut core = c.clone();
    let mut removed = Vec::new();
    let mut tries = 0;
    loop {
        let mut changed = false;
        for (p, s) in candidates(&core, &keep) {
            if !core.design.blocks.contains_key(&p) {
                continue;
            }
            let Some(next) = without(&core, &[p]) else { continue };
            tries += 1;
            if keeps(&base, &next) {
                removed.push((p, s));
                core = next;
                changed = true;
            }
        }
        if changed {
            continue;
        }
        if !pairs {
            break;
        }
        let cands = candidates(&core, &keep);
        let mut found = None;
        'pairs: for i in 0..cands.len() {
            for j in i + 1..cands.len() {
                if !adjacent(cands[i].0, cands[j].0) {
                    continue;
                }
                let Some(next) = without(&core, &[cands[i].0, cands[j].0]) else { continue };
                tries += 1;
                if keeps(&base, &next) {
                    found = Some((i, j, next));
                    break 'pairs;
                }
            }
        }
        match found {
            Some((i, j, next)) => {
                removed.push(cands[i].clone());
                removed.push(cands[j].clone());
                core = next;
            }
            None => break,
        }
    }
    Some(Stripped { core, removed, tries })
}

/// The first removal (one block, else a face-adjacent pair) that keeps everything `c` gives a
/// builder on redsim: the cheap test of [`strip`] (`clock archive`'s guard). `None`: irreducible on
/// redsim, or no clock there.
pub fn first_removal(c: &Clock) -> Option<Stripped> {
    let base = props(c)?;
    let keep = kept_cells(c);
    let cands = candidates(c, &keep);
    let mut tries = 0;
    for (p, s) in &cands {
        let Some(next) = without(c, &[*p]) else { continue };
        tries += 1;
        if keeps(&base, &next) {
            return Some(Stripped { core: next, removed: vec![(*p, s.clone())], tries });
        }
    }
    for i in 0..cands.len() {
        for j in i + 1..cands.len() {
            if !adjacent(cands[i].0, cands[j].0) {
                continue;
            }
            let Some(next) = without(c, &[cands[i].0, cands[j].0]) else { continue };
            tries += 1;
            if keeps(&base, &next) {
                return Some(Stripped { core: next, removed: vec![cands[i].clone(), cands[j].clone()], tries });
            }
        }
    }
    None
}

/// Whether the game confirms a core (`core`, its `clock verify`) gives a builder what the original
/// (`orig`) gives: a clock with the same builder period and pulse widths at the declared output,
/// as robust over positions and rotations, no worse with dropped items, no less vibration-proof.
pub fn game_keeps(orig: &crate::verify::VerifyReport, core: &crate::verify::VerifyReport) -> bool {
    if core.verdict != Verdict::Clock {
        return false;
    }
    let (Some(a), Some(b)) = (orig.base(), core.base()) else { return false };
    let pulse = |r: &crate::verify::RunOk| -> Option<(u32, Vec<u32>)> {
        let w = &r.output.as_ref()?.wave;
        if w.edges == 0 || w.irregular().is_some() {
            return None;
        }
        Some((if w.edges <= 1 { r.period } else { *w.gaps.first()? }, widths_of(&w.widths)))
    };
    if pulse(a).is_none() || pulse(a) != pulse(b) {
        return false;
    }
    if orig.robustness == "robust" && core.robustness != "robust" {
        return false;
    }
    let items_ok = |r: &crate::verify::RunOk| r.items.as_deref().is_none_or(|w| w == "ok");
    let proof = |r: &crate::verify::RunOk| r.vibration.as_ref().is_none_or(|v| v.proof);
    (!items_ok(a) || items_ok(b)) && (!proof(a) || proof(b))
}

/// The track an archived design came from: its `# archived ... from designs/clocks/tracks/<t>/`
/// header, else its name's prefix (`clk-obs`, `census`, `hopclk`...).
pub fn track_of(text: &str, file: &str) -> String {
    if let Some(t) = text.lines().take(3).find_map(|l| l.split("from designs/clocks/tracks/").nth(1)).and_then(|r| r.split('/').next()) {
        return t.to_string();
    }
    let stem = file.trim_end_matches(".txt");
    let parts: Vec<&str> = stem.split('-').collect();
    match parts.as_slice() {
        ["clk", t, ..] => format!("clk-{t}"),
        [t, ..] => t.to_string(),
        [] => String::new(),
    }
}

/// Designs redsim cannot judge a removal in exactly: entities (boats, carts: their paths) and fluids
/// (water, lava, a waterlogged block: a removed wall lets them flow).
pub fn redsim_inexact(c: &Clock) -> Option<&'static str> {
    if !c.design.entities.is_empty() {
        return Some("it has entities (their paths are the game's)");
    }
    let fluid = |s: &str| s.starts_with("minecraft:water") || s.starts_with("minecraft:lava") || s.starts_with("minecraft:bubble_column") || s.contains("waterlogged=true");
    if c.design.blocks.values().any(|s| fluid(s)) {
        return Some("it holds fluids (a removed wall lets them flow)");
    }
    None
}

/// The name of a slop clock's core: the slop's track prefix (`clk-obs`, `clockinfra`...), the
/// builder period, the core's block kinds (the census abbreviations), its blocks and a short hash
/// of its content.
pub fn core_name(slop_file: &str, core: &Clock, period: u32) -> String {
    let stem = slop_file.trim_end_matches(".txt");
    let parts: Vec<&str> = stem.split('-').collect();
    let cut = parts.iter().position(|p| p.len() > 1 && p.starts_with('p') && p[1..].chars().all(|c| c.is_ascii_digit())).unwrap_or(parts.len().min(2)).max(1);
    let prefix = parts[..cut].join("-");
    let abbrev = |k: &str| -> String {
        match k {
            "observer" => "obs",
            "piston" => "pis",
            "sticky_piston" => "stk",
            "redstone_block" => "rb",
            "redstone_lamp" => "lamp",
            "note_block" => "note",
            "copper_bulb" => "bulb",
            other => other,
        }
        .to_string()
    };
    let kinds: Vec<String> = core.kinds().iter().map(|k| abbrev(k)).collect();
    let h = crate::catalog::content_hash(core);
    format!("{prefix}-p{period}-{}-{}b-{}", kinds.join("-"), core.blocks(), &h[..6.min(h.len())])
}

fn items_ok(w: &str) -> bool {
    w.is_empty() || w == "ok"
}

fn flat(dims: &str) -> bool {
    dims.split('x').nth(1).and_then(|h| h.parse::<i32>().ok()) == Some(1)
}

/// `clock strip --apply`: the slop clocks of a strip run (`results`: its strip.tsv and cores), each
/// removed when an archived clock that is not slop gives a builder at least as much (the builder
/// period and pulse widths, robust, usable, no worse with items, flat, vibration-proof) with no
/// more blocks than the core, else its core archived (judged in the game by `clock archive`: kept
/// only when the game confirms everything the slop clock had) and the slop clock removed; a core
/// the game does not confirm keeps the slop clock (the blocks were needed). Every removal is
/// recorded in aliases.tsv under the clock that stays. Returns one line per slop clock.
pub fn apply(pool: Option<&mctas::sim::Pool>, dir: &std::path::Path, img_dir: &std::path::Path, results: &std::path::Path, o: &crate::verify::VerifyOptions) -> anyhow::Result<Vec<String>> {
    use crate::catalog::{Row, builder_period, pulse_widths, read_rows};
    let tsv = std::fs::read_to_string(results.join("strip.tsv"))?;
    let entries: Vec<(String, String)> = tsv.lines().skip(1).filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((f.first()?.to_string(), f.get(4).unwrap_or(&"").to_string()))
    }).collect();
    let slop: std::collections::BTreeSet<String> = entries.iter().map(|(f, _)| f.clone()).collect();
    let staged = results.join("staged");
    std::fs::create_dir_all(&staged)?;
    let mut gone: std::collections::BTreeSet<String> = Default::default();
    let mut alias_rows: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    let vib_ok = |k: &Row, s_proof: bool| -> bool {
        // a clock with a sensor stands in for one only if it is vibration-proof too
        if !s_proof {
            return true;
        }
        match Clock::load(&dir.join(&k.file)) {
            Ok(c) if crate::vibration::has_sculk(&c.design) => props(&c).and_then(|p| p.vibration_proof).unwrap_or(false),
            Ok(_) => true,
            Err(_) => false,
        }
    };
    for (file, removed) in &entries {
        let rows = read_rows(dir)?;
        let Some(s) = rows.iter().find(|r| &r.file == file).cloned() else {
            lines.push(format!("SKIP {file}: no verdict row"));
            continue;
        };
        let stem = file.trim_end_matches(".txt");
        if let Ok(orig) = Clock::load(&dir.join(file))
            && let Some(why) = redsim_inexact(&orig)
        {
            lines.push(format!("KEPT {file}: {why}: redsim's strip is no evidence"));
            continue;
        }
        let core = Clock::load(&results.join(format!("{stem}-core.txt")))?;
        let Some(cp) = props(&core) else {
            lines.push(format!("KEPT {file}: its core is no clock on redsim any more"));
            continue;
        };
        let s_proof = Clock::load(&dir.join(file)).ok().and_then(|c| props(&c)).and_then(|p| p.vibration_proof).unwrap_or(true);
        let (lo, hi) = core.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
        let core_flat = hi[1] == lo[1];
        let k = rows
            .iter()
            .filter(|r| r.shown() && r.file != *file && !slop.contains(&r.file) && !gone.contains(&r.file))
            .filter(|r| builder_period(r) == Some(cp.period) && pulse_widths(r) == cp.widths && r.blocks <= core.blocks())
            .filter(|r| s.robustness != "robust" || r.robustness == "robust")
            .filter(|r| !s.usable.starts_with("yes") || r.usable.starts_with("yes"))
            .filter(|r| !items_ok(&s.items) || items_ok(&r.items))
            .filter(|r| !core_flat || flat(&r.dims))
            .filter(|r| vib_ok(r, s_proof))
            .min_by_key(|r| (r.blocks, !r.usable.starts_with("yes"), r.volume, r.file.clone()))
            .cloned();
        if let Some(k) = k {
            gone.insert(file.clone());
            alias_rows.push(format!("{}\t{file}\t{stem} (slop: {} with {removed}, which changed nothing)\n", k.file, k.file.trim_end_matches(".txt")));
            lines.push(format!("REMOVED {file} ({} blocks; core {}): {} ({} blocks) gives a builder as much", s.blocks, core.blocks(), k.file, k.blocks));
            continue;
        }
        // its core, judged in the game and archived under its own name
        let name = core_name(file, &core, cp.period);
        let Some(pool) = pool else {
            lines.push(format!("ARCHIVE {file} ({} blocks): its core {name} ({} blocks) needs the game", s.blocks, core.blocks()));
            continue;
        };
        let mut d = core.design.clone();
        d.name = name.clone();
        let path = staged.join(format!("{name}.txt"));
        std::fs::write(&path, format!("# the irreducible core of {file} (mctas clock strip, 2026-10-22): removed {removed}\n{}", d.to_text()))?;
        let caption = format!("The irreducible core of {stem} (`mctas clock strip`, 2026-10-22): the same clock without {removed}, which changed nothing a builder gets.");
        match crate::catalog::archive(pool, &path, o, dir, Some(&caption)) {
            Ok(a) if a.new => {
                let new_file = a.file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                let rows = read_rows(dir)?;
                let n = rows.iter().find(|r| r.file == new_file).cloned();
                let why = match &n {
                    None => Some("no verdict row".to_string()),
                    Some(n) if builder_period(n) != builder_period(&s) || pulse_widths(n) != pulse_widths(&s) => Some(format!("another pulse in the game ({} / {})", n.gaps, n.pulses)),
                    Some(n) if s.robustness == "robust" && n.robustness != "robust" => Some(format!("{} in the game", n.robustness)),
                    Some(n) if s.usable.starts_with("yes") && !n.usable.starts_with("yes") => Some(format!("output {}", n.usable)),
                    Some(n) if items_ok(&s.items) && !items_ok(&n.items) => Some(n.items.clone()),
                    _ => None,
                };
                match why {
                    None => {
                        gone.insert(file.clone());
                        alias_rows.push(format!("{new_file}\t{file}\t{stem} (slop: the core with {removed}, which changed nothing)\n"));
                        lines.push(format!("REPLACED {file} ({} blocks) by its core {new_file} ({} blocks, judged in the game: {})", s.blocks, core.blocks(), a.summary));
                    }
                    Some(why) => {
                        // the blocks were needed after all: the core leaves, the clock stays
                        crate::catalog::write_tables(dir, &[new_file.clone()].into_iter().collect())?;
                        let _ = std::fs::remove_file(dir.join(&new_file));
                        lines.push(format!("KEPT {file}: its core fails in the game: {why}"));
                    }
                }
            }
            Ok(a) => {
                let x = a.file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                match read_rows(dir)?.iter().find(|r| r.file == x) {
                    Some(r) if r.shown() && r.file != *file => {
                        gone.insert(file.clone());
                        alias_rows.push(format!("{x}\t{file}\t{stem} (slop: {x} with {removed}, which changed nothing)\n"));
                        lines.push(format!("REMOVED {file}: its core is already archived as {x} ({})", a.summary));
                    }
                    _ => lines.push(format!("KEPT {file}: its core is archived as {x}, not shown ({})", a.summary)),
                }
            }
            Err(e) => lines.push(format!("KEPT {file}: its core is not archived: {e:#}")),
        }
    }
    // a dry run (no game) writes nothing
    if pool.is_none() {
        return Ok(lines);
    }
    crate::catalog::write_tables(dir, &gone)?;
    for f in &gone {
        let stem = f.trim_end_matches(".txt");
        for p in [dir.join(f), img_dir.join(format!("{stem}.jpg")), img_dir.join(format!("{stem}.strip.jpg"))] {
            let _ = std::fs::remove_file(p);
        }
    }
    if !alias_rows.is_empty() {
        let ap = dir.join(crate::catalog::ALIASES);
        let mut text = std::fs::read_to_string(&ap).unwrap_or_else(|_| "kept\talias_file\talias_name\n".into());
        for r in &alias_rows {
            text.push_str(r);
        }
        std::fs::write(&ap, text)?;
    }
    Ok(lines)
}
