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

/// The other half of a two-block structure (a door's or a tall plant's upper/lower half, a bed's
/// head/foot): the game breaks one half without the other, so strip removes them together.
fn partner(p: Pos, s: &str) -> Option<Pos> {
    if s.contains("half=upper") {
        return Some([p[0], p[1] - 1, p[2]]);
    }
    if s.contains("half=lower") {
        return Some([p[0], p[1] + 1, p[2]]);
    }
    if s.contains("_bed[") {
        let o = facing_offset(s)?;
        return Some(if s.contains("part=head") { [p[0] - o[0], p[1] - o[1], p[2] - o[2]] } else { [p[0] + o[0], p[1] + o[1], p[2] + o[2]] });
    }
    None
}

/// The clock without the blocks at `cells` (an extended piston with its head, a door or bed with
/// its other half, a block with its block-entity data); `None` when nothing would be left or it
/// no longer parses.
pub fn without(c: &Clock, cells: &[Pos]) -> Option<Clock> {
    let mut d = c.design.clone();
    remove_cells(&mut d, cells);
    if d.blocks.is_empty() {
        return None;
    }
    Clock::from_design(d).ok()
}

/// [`without`] on a design, in place.
pub fn remove_cells(d: &mut mctas::design::Design, cells: &[Pos]) {
    for p in cells {
        if let Some(s) = d.blocks.remove(p) {
            d.block_entities.remove(p);
            if let Some(h) = head_of(*p, &s)
                && d.blocks.get(&h).is_some_and(|x| x.starts_with("minecraft:piston_head"))
            {
                d.blocks.remove(&h);
            }
            let id = s.split('[').next().unwrap_or("").to_string();
            if let Some(q) = partner(*p, &s)
                && d.blocks.get(&q).is_some_and(|x| x.split('[').next() == Some(id.as_str()))
            {
                d.blocks.remove(&q);
                d.block_entities.remove(&q);
            }
        }
    }
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
    if game_pulse(a).is_none() || game_pulse(a) != game_pulse(b) {
        return false;
    }
    if orig.robustness == "robust" && core.robustness != "robust" {
        return false;
    }
    let items_ok = |r: &crate::verify::RunOk| r.items.as_deref().is_none_or(|w| w == "ok");
    let proof = |r: &crate::verify::RunOk| r.vibration.as_ref().is_none_or(|v| v.proof);
    (!items_ok(a) || items_ok(b)) && (!proof(a) || proof(b))
}

/// The pulse a builder takes from a game run: its builder period (one rising edge per cycle, else
/// the even spacing) and its pulse widths; `None` without an even output.
pub fn game_pulse(r: &crate::verify::RunOk) -> Option<(u32, Vec<u32>)> {
    let w = &r.output.as_ref()?.wave;
    if w.edges == 0 || w.irregular().is_some() {
        return None;
    }
    Some((if w.edges <= 1 { r.period } else { *w.gaps.first()? }, widths_of(&w.widths)))
}

/// A design stripped in the game ([`strip_game`]).
pub struct GameStripped {
    pub stripped: Stripped,
    /// The clock judge on the original and on the core.
    pub orig: crate::verify::VerifyReport,
    pub core: crate::verify::VerifyReport,
    /// Candidate blocks left untried at the deadline (0: the core is irreducible in the game).
    pub untried: usize,
}

/// Strip in the GAME (designs redsim cannot judge a removal in: entities, fluids): the same greedy
/// removal as [`strip`], each removal judged by the clock judge, first on the base build only (a
/// screen: the same builder period and pulse widths), then the full judge over positions and
/// rotations ([`game_keeps`]: as robust, no worse with items, no less vibration-proof). `seed`
/// (redsim's removals) is tried first, all at once, then block by block before the others. Stops
/// at `deadline` (the blocks left untried are counted). `None`: the original is no clock with an
/// even output in the game.
pub fn strip_game(pool: &mctas::sim::Pool, c: &Clock, full: &crate::verify::VerifyOptions, seed: &[Pos], deadline: std::time::Instant) -> anyhow::Result<Option<GameStripped>> {
    use crate::verify::{VerifyOptions, verify_in_pool};
    let orig = verify_in_pool(pool, c, full)?;
    let Some(want) = orig.base().filter(|_| orig.verdict == Verdict::Clock).and_then(game_pulse) else { return Ok(None) };
    let screen = VerifyOptions { positions: 1, rotations: false, reload: false, reload_at: None, ..full.clone() };
    let screen_ok = |x: &Clock| -> anyhow::Result<bool> {
        let r = verify_in_pool(pool, x, &screen)?;
        Ok(r.verdict == Verdict::Clock && r.base().and_then(game_pulse).as_ref() == Some(&want))
    };
    let full_ok = |x: &Clock| -> anyhow::Result<Option<crate::verify::VerifyReport>> {
        let r = verify_in_pool(pool, x, full)?;
        Ok(game_keeps(&orig, &r).then_some(r))
    };
    let keep = kept_cells(c);
    let mut core = c.clone();
    let mut core_rep = orig.clone();
    let mut removed: Vec<(Pos, String)> = Vec::new();
    let mut tries = 0;
    let seed: Vec<Pos> = seed.iter().copied().filter(|p| !keep.contains(p) && c.design.blocks.contains_key(p)).collect();
    if !seed.is_empty()
        && let Some(next) = without(&core, &seed)
    {
        tries += 1;
        if screen_ok(&next)?
            && let Some(r) = full_ok(&next)?
        {
            removed.extend(seed.iter().map(|p| (*p, c.design.blocks[p].clone())));
            core = next;
            core_rep = r;
        }
    }
    let mut untried = 0;
    'passes: loop {
        let mut changed = false;
        // redsim's removals first, then the others
        let mut cands = candidates(&core, &keep);
        cands.sort_by_key(|(p, _)| !seed.contains(p));
        for (i, (p, s)) in cands.iter().enumerate() {
            if std::time::Instant::now() > deadline {
                untried = cands.len() - i;
                break 'passes;
            }
            let Some(next) = without(&core, &[*p]) else { continue };
            tries += 1;
            if screen_ok(&next)?
                && let Some(r) = full_ok(&next)?
            {
                removed.push((*p, s.clone()));
                core = next;
                core_rep = r;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(Some(GameStripped { stripped: Stripped { core, removed, tries }, orig, core: core_rep, untried }))
}

/// A generated family member as the site publishes it, at the family's period for its n (`want`:
/// one rising edge per cycle): its declared output when it gives that, else (a generator older
/// than the output rule) an exterior cell where the judge's lamp lights once per `want` ticks
/// (redsim's picks, as `site/simulate/members.rs` moves it); with its [`Props`].
fn member_props(m: &Clock, want: u32) -> Option<(Clock, Props)> {
    if let Some(p) = props(m).filter(|p| p.period == want) {
        return Some((m.clone(), p));
    }
    let mut d = m.design.clone();
    d.ports.remove("output");
    let free = Clock::from_design(d).ok()?;
    for (to, _) in crate::sim::lamp_outputs(&free, 40) {
        let mut d = free.design.clone();
        d.ports.insert("output".into(), to);
        let Ok(moved) = Clock::from_design(d) else { continue };
        if let Some(p) = props(&moved).filter(|p| p.period == want) {
            return Some((moved, p));
        }
    }
    None
}

/// An extendable family stripped ([`strip_family`]).
pub struct FamilyStripped {
    pub family: crate::extend::Family,
    /// The cells every member drops (with the base design's states).
    pub removed: Vec<(Pos, String)>,
    /// Blocks each member loses.
    pub fewer: usize,
    /// Members checked (redsim clocks with an even output before the removal).
    pub members: usize,
    pub log: Vec<String>,
}

/// Strip an extendable family (the coordinator, 2026-10-23: a slop base is slop at every
/// extension): the base design's redsim removals ([`strip`]), kept only for the cells whose removal
/// keeps EVERY member n_lo..=n_hi working: each member generated by the family's generator, the
/// same cells removed, the same builder period and pulse widths at its output, usable when it was,
/// no new open hopper top, and the same number of blocks fewer than every other member. Whole set
/// first, then cell by cell. `None`: the base is irreducible on redsim (or no family).
pub fn strip_family(base_path: &std::path::Path, threads: usize) -> anyhow::Result<Option<FamilyStripped>> {
    let c = Clock::load(base_path)?;
    let Some(f) = crate::extend::Family::of(&c)? else { return Ok(None) };
    let Some(st) = strip(&c, true) else { return Ok(None) };
    let mut log = Vec::new();
    if let Some(p) = props(&c)
        && p.period as i64 != f.period.eval(f.base)
    {
        log.push(format!("the base's declared output pulses every {} gt, not once per {} gt (the family's period at n = {}): its output is not the family's", p.period, f.period.eval(f.base), f.base));
    }
    if st.removed.is_empty() {
        return Ok(None);
    }
    // every member, generated once, with its own properties on redsim
    let tmp = mctas::paths::run_dir().join(format!("strip-family-{}-{}", f.id, std::process::id()));
    let ns: Vec<i64> = (f.n_lo..=f.n_hi).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let members: std::sync::Mutex<Vec<(i64, Clock, Props)>> = Default::default();
    let skipped: std::sync::Mutex<Vec<String>> = Default::default();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&n) = ns.get(i) else { break };
                    let m = f.build(base_path, n, &tmp).and_then(|p| {
                        let m = Clock::load(&p);
                        if p != base_path {
                            let _ = std::fs::remove_file(&p);
                        }
                        m
                    });
                    match m {
                        Ok(m) => match member_props(&m, f.period.eval(n).max(0) as u32) {
                            Some((m, pr)) => members.lock().unwrap().push((n, m, pr)),
                            None => skipped.lock().unwrap().push(format!("n = {n}: no output gives one pulse per {} gt on redsim (not a period-{} member)", f.period.eval(n), f.period.eval(n))),
                        },
                        Err(e) => skipped.lock().unwrap().push(format!("n = {n}: {e:#}")),
                    }
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&tmp);
    let mut members = members.into_inner().unwrap();
    members.sort_by_key(|(n, _, _)| *n);
    log.extend(skipped.into_inner().unwrap());
    // no member to check: nothing says the removal keeps the family
    if members.is_empty() {
        log.push("no member is a clock with an even output on redsim: nothing to check a removal against".into());
        return Ok(Some(FamilyStripped { family: f, removed: Vec::new(), fewer: 0, members: 0, log }));
    }
    // whether removing `cells` keeps every member (the same count fewer in each)
    let test = |cells: &[Pos]| -> Result<usize, String> {
        let k = c.blocks() - without(&c, cells).map(|x| x.blocks()).unwrap_or(c.blocks());
        let next = std::sync::atomic::AtomicUsize::new(0);
        let fail: std::sync::Mutex<Option<String>> = Default::default();
        std::thread::scope(|s| {
            for _ in 0..threads.max(1) {
                s.spawn(|| {
                    loop {
                        if fail.lock().unwrap().is_some() {
                            break;
                        }
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((n, m, pr)) = members.get(i) else { break };
                        let why = match without(m, cells) {
                            None => Some("nothing left".to_string()),
                            Some(x) if m.blocks() - x.blocks() != k => Some(format!("{} blocks fewer, not {k}", m.blocks() - x.blocks())),
                            Some(x) if !keeps(pr, &x) => Some("another clock without them".to_string()),
                            Some(_) => None,
                        };
                        if let Some(w) = why {
                            *fail.lock().unwrap() = Some(format!("n = {n}: {w}"));
                        }
                    }
                });
            }
        });
        match fail.into_inner().unwrap() {
            Some(w) => Err(w),
            None => Ok(k),
        }
    };
    let all: Vec<Pos> = st.removed.iter().map(|(p, _)| *p).collect();
    let mut kept: Vec<Pos> = Vec::new();
    let mut fewer = 0;
    match test(&all) {
        Ok(k) => {
            kept = all.clone();
            fewer = k;
        }
        Err(why) => {
            log.push(format!("all {} cells: {why}; cell by cell", all.len()));
            for p in &all {
                let mut t = kept.clone();
                t.push(*p);
                match test(&t) {
                    Ok(k) => {
                        kept = t;
                        fewer = k;
                    }
                    Err(why) => log.push(format!("{},{},{} stays: {why}", p[0], p[1], p[2])),
                }
            }
        }
    }
    let removed: Vec<(Pos, String)> = st.removed.into_iter().filter(|(p, _)| kept.contains(p)).collect();
    Ok(Some(FamilyStripped { family: f, removed, fewer, members: members.len(), log }))
}

/// A family base design without the stripped cells: the blocks removed, `extend remove:` set (every
/// generated member drops them) and `extend blocks:` lowered by the blocks each member loses.
pub fn family_base_text(text: &str, removed: &[Pos], fewer: usize, f: &crate::extend::Family) -> anyhow::Result<String> {
    use crate::extend::Formula;
    let mut d = mctas::design::Design::parse(text)?;
    remove_cells(&mut d, removed);
    let mut cells: Vec<Pos> = f.remove.clone();
    cells.extend(removed.iter().copied());
    d.meta.insert("extend remove".into(), cells.iter().map(|p| format!("{},{},{}", p[0], p[1], p[2])).collect::<Vec<_>>().join("; "));
    let k = fewer as i64;
    let blocks = match f.blocks {
        Formula::Linear { a, b } if b == 0 => format!("{}", a - k),
        Formula::Linear { a, b } => format!("{} {} {}n", a - k, if b < 0 { "-" } else { "+" }, b.abs()),
        Formula::Geometric { a, k: base, c } => format!("{a} * {base}^n {} {}", if c - k < 0 { "-" } else { "+" }, (c - k).abs()),
    };
    d.meta.insert("extend blocks".into(), blocks);
    Ok(d.to_text())
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
    // file, removed, and for a game strip (columns 7-9: `game`, the core's builder period and pulse
    // widths, judged in the game) the core's pulse
    let entries: Vec<(String, String, Option<(u32, Vec<u32>)>)> = tsv
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let game = (f.get(6) == Some(&"game")).then(|| {
                let p: u32 = f.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
                let w: Vec<u32> = f.get(8).map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
                (p, w)
            });
            Some((f.first()?.to_string(), f.get(4).unwrap_or(&"").to_string(), game))
        })
        .collect();
    let slop: std::collections::BTreeSet<String> = entries.iter().map(|(f, _, _)| f.clone()).collect();
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
    for (file, removed, game) in &entries {
        let rows = read_rows(dir)?;
        let Some(s) = rows.iter().find(|r| &r.file == file).cloned() else {
            lines.push(format!("SKIP {file}: no verdict row"));
            continue;
        };
        let stem = file.trim_end_matches(".txt");
        if game.is_none()
            && let Ok(orig) = Clock::load(&dir.join(file))
            && let Some(why) = redsim_inexact(&orig)
        {
            lines.push(format!("KEPT {file}: {why}: redsim's strip is no evidence"));
            continue;
        }
        let core = Clock::load(&results.join(format!("{stem}-core.txt")))?;
        // the core's pulse: judged in the game (a game strip), else on redsim
        let cp = match game {
            Some((period, widths)) => Props { period: *period, widths: widths.clone(), usable: false, intakes: Vec::new(), vibration_proof: None },
            None => match props(&core) {
                Some(cp) => cp,
                None => {
                    lines.push(format!("KEPT {file}: its core is no clock on redsim any more"));
                    continue;
                }
            },
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
