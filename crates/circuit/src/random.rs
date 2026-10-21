//! Study 16, RANDOMIZERS (coordinator 2026-10-21): one press lights exactly one of N outputs,
//! uniformly at random. The randomness is the game's level random (a dropper's or dispenser's
//! random slot: `DispenserBlockEntity.getRandomSlot`, uniform over the filled slots): every
//! build's sandbox is seeded from its watch box and a salt ([`seed`]), and redsim mirrors that
//! random draw for draw (`World::mirror_level_random`), so the screen and the game see the same
//! outcomes and a run can be replayed exactly.
//!
//! * Ports: one input and N outputs. The input is pressed for real: a button of the design is
//!   pressed by the judge and holds for its own time (wood 30 gt, stone 20 gt); any other input is
//!   held on like a wooden then a stone button, alternately (`tnt::PRESSES`). Every output is a
//!   TAP a builder can take: the design's repeater or comparator in the output cell, its front on
//!   a cell outside the design's box ([`check_ports`]).
//! * Stimulus: [`presses`] presses (24 per output, at least 48), each window the press, the
//!   circuit's recovery gap and a varying wait ([`PACES`]: the presses come at several paces).
//! * Each press's window: exactly one output turns on, once, and no other output comes on. Style
//!   `pulse`: every output off again by the window's end; style `hold`: the chosen output stays on
//!   until the next press changes it (exactly one output on at every window's end). Nothing is on
//!   before the first press, and every output wins at least once. Metrics: the style, the latency
//!   (a press to its output turning on, the slowest), the pulse (the shortest, pulse style), the
//!   presses per run.
//! * Uniformity ([`pool`]): the counts of every build pooled (the judge's 16 builds, each its own
//!   seed; the redsim screen's [`SCREEN_SALTS`] salts) and tested against N equal shares with a
//!   chi-square test at p = 0.001 ([`critical`]); the result rides in the verdict's flag
//!   (`uniform: chi2 ... over ... presses`), not in the metrics the builds are compared on.

use crate::check::Outcome;
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, InputHow, Pos};
use anyhow::{Result, bail};

/// The extra waits (gt) added to the windows in turn: the presses come at several paces.
pub const PACES: [u32; 8] = [0, 7, 3, 13, 1, 22, 5, 9];

/// The redsim screen's salts (salt 0, the base run, and these more), pooled for its uniformity
/// test.
pub const SCREEN_SALTS: i64 = 8;

/// Presses per run: 24 per output, at least 48 (every output then wins at least once in a run
/// with near certainty: N (1 - 1/N)^(24 N) < 1e-9).
pub fn presses(n: u32) -> usize {
    (24 * n as usize).max(48)
}

/// The level random's seed of a build whose watch box is `from..to` (the shulker judge's
/// `build_seed`): the game sandbox is created with it, redsim mirrors it.
pub fn seed(from: Pos, to: Pos, salt: i64) -> i64 {
    redsim::redstone::shulker::build_seed(from, to, salt)
}

/// The forced chunks of a watch box (the game sandbox's, mirrored by redsim).
pub fn chunks(from: Pos, to: Pos) -> [i32; 4] {
    redsim::redstone::shulker::chunk_range(from, to)
}

/// The ports a randomizer needs (`Circuit::check_ports`).
pub fn check_ports(c: &Circuit, n: u32) -> Result<()> {
    if c.inputs.len() != 1 {
        bail!("randomizer {n}: needs exactly one input (pressed like a button)");
    }
    if c.outputs.len() != n as usize {
        bail!("randomizer {n}: needs {n} outputs, has {}", c.outputs.len());
    }
    let Some((lo, hi)) = c.clock.design.bounds() else { bail!("randomizer {n}: no blocks") };
    for o in &c.outputs {
        let st = c.clock.design.blocks.get(&o.pos).map(|s| s.as_str()).unwrap_or("");
        let id = st.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
        if id != "repeater" && id != "comparator" {
            bail!("randomizer {n}: output {} at {:?} must be the design's repeater or comparator (the tap a builder takes the signal from), not {}", o.name, o.pos, if id.is_empty() { "air" } else { id });
        }
        // `facing` is the input side: the output points the other way
        let facing = st.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
        let v = match facing {
            "north" => [0, 0, 1],
            "south" => [0, 0, -1],
            "west" => [1, 0, 0],
            _ => [-1, 0, 0],
        };
        let front = [o.pos[0] + v[0], o.pos[1], o.pos[2] + v[2]];
        let outside = (0..3).any(|i| front[i] < lo[i] || front[i] > hi[i]);
        if !outside {
            bail!("randomizer {n}: output {} at {:?} must face out of the design's box (its front {front:?} is inside it: a builder could not continue the line)", o.name, o.pos);
        }
    }
    Ok(())
}

/// The kind's stimulus (`check::stimulus`): [`presses`] real presses from `start`, each window the
/// press, `gap` and the next of [`PACES`].
pub fn stimulus(c: &Circuit, n: u32, start: u32, gap: u32) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let mut t = start;
    let button = c.inputs.first().is_some_and(|i| i.how == InputHow::Button);
    let own = c.inputs.first().map(|i| crate::tnt::press_ticks(c.clock.design.blocks.get(&i.pos).map(|s| s.as_str()).unwrap_or(""))).unwrap_or(30);
    for k in 0..presses(n) {
        let len = if button { own } else { crate::tnt::PRESSES[k % crate::tnt::PRESSES.len()] };
        s.pulse(0, t, if button { 1 } else { len });
        let w = len + gap + PACES[k % PACES.len()];
        s.phase(t, t + w, format!("press {}", k + 1));
        t += w;
    }
    s.ticks = t;
    s
}

/// The counts line of a run's table (`counts: c0 c1 ...`, output order), which [`pool`] reads.
const COUNTS: &str = "counts:";

/// Judge one run (`check::check`): see the module doc.
pub fn check(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let k = n as usize;
    if c.outputs.len() != k {
        return Outcome::fail(format!("randomizer {n}: needs {n} outputs"), Vec::new());
    }
    let frames = t.frames();
    let outs: Vec<Vec<bool>> = (0..k).map(|i| t.out_seq(i)).collect();
    let name = |i: usize| c.outputs[i].name.clone();
    let mut table = Vec::new();
    // nothing before the first press (from the first frame the circuit may be driven)
    let rest_from = (c.clock.free_from() + crate::stimulus::SETTLE) as usize;
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(frames).min(frames);
    for i in 0..k {
        if let Some(f) = (rest_from..first).find(|&f| outs[i][f]) {
            return Outcome::fail(format!("output {} is on at frame {f}, before the first press", name(i)), table);
        }
    }
    let mut counts = vec![0u32; k];
    let mut style: Option<&'static str> = None;
    let (mut latency, mut pulse) = (0usize, usize::MAX);
    for (from, to, label) in &s.phases {
        let (a, b) = (*from as usize, (*to as usize).min(frames));
        if a + 1 >= b {
            return Outcome::fail(format!("{label}: the run ended before it"), table);
        }
        let rises: Vec<(usize, usize)> = (0..k).flat_map(|i| (a.max(1)..b).filter(|&f| outs[i][f] && !outs[i][f - 1]).map(move |f| (i, f)).collect::<Vec<_>>()).collect();
        let on_end: Vec<usize> = (0..k).filter(|&i| outs[i][b - 1]).collect();
        let win = |i: usize| -> String { format!("{label}: {}", name(i)) };
        // which style this window shows, and the output it chose
        let (st, chosen) = match on_end.as_slice() {
            [] => {
                // pulse: exactly one output pulsed, once
                match rises.as_slice() {
                    [(i, _)] => ("pulse", *i),
                    [] => return Outcome::fail(format!("{label}: no output turned on"), table),
                    many => return Outcome::fail(format!("{label}: {} outputs turned on ({})", many.len(), many.iter().map(|(i, f)| format!("{} at +{} gt", name(*i), f - a)).collect::<Vec<_>>().join(", ")), table),
                }
            }
            [x] => {
                // hold: the chosen output is the one on at the end; nothing else turned on, and it
                // turned on at most once (none: chosen again, it stayed on)
                if let Some((i, f)) = rises.iter().find(|(i, _)| i != x) {
                    return Outcome::fail(format!("{label}: {} turned on at +{} gt but {} is the one left on", name(*i), f - a, name(*x)), table);
                }
                if rises.len() > 1 {
                    return Outcome::fail(format!("{label}: {} turned on {} times", name(*x), rises.len()), table);
                }
                ("hold", *x)
            }
            many => return Outcome::fail(format!("{label}: {} outputs on at the window's end ({})", many.len(), many.iter().map(|i| name(*i)).collect::<Vec<_>>().join(", ")), table),
        };
        match style {
            None => style = Some(st),
            Some(prev) if prev != st => return Outcome::fail(format!("{label}: a {st} where the earlier presses gave a {prev}"), table),
            _ => {}
        }
        // a hold must also clear the other outputs: none may be on at any frame of the window
        // after the chosen one turned on
        if let Some(&(_, f)) = rises.first() {
            latency = latency.max(f - a);
            if st == "pulse" {
                let len = (f..b).take_while(|&g| outs[chosen][g]).count();
                pulse = pulse.min(len);
            }
            for i in (0..k).filter(|&i| i != chosen) {
                if let Some(g) = (f..b).find(|&g| outs[i][g]) {
                    return Outcome::fail(format!("{label}: {} still on at +{} gt after {} turned on", name(i), g - a, name(chosen)), table);
                }
            }
        }
        counts[chosen] += 1;
        table.push(win(chosen));
    }
    if let Some(i) = (0..k).find(|&i| counts[i] == 0) {
        return Outcome::fail(format!("output {} never turned on in {} presses", name(i), s.phases.len()), table);
    }
    table.push(format!("{COUNTS} {}", counts.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")));
    let style = style.unwrap_or("pulse");
    let mut metrics = vec![("style".to_string(), style.to_string()), ("latency".to_string(), format!("{latency} gt"))];
    if style == "pulse" && pulse != usize::MAX {
        metrics.push(("pulse".to_string(), format!("{pulse} gt")));
    }
    metrics.push(("presses".to_string(), s.phases.len().to_string()));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The counts a run's table recorded ([`check`]), if any.
pub fn counts_of(o: &Outcome) -> Option<Vec<u32>> {
    let line = o.table.iter().rev().find(|l| l.starts_with(COUNTS))?;
    line[COUNTS.len()..].split_whitespace().map(|v| v.parse().ok()).collect()
}

/// The chi-square statistic of `counts` against equal shares.
pub fn chi2(counts: &[u32]) -> f64 {
    let total: u32 = counts.iter().sum();
    if counts.is_empty() || total == 0 {
        return 0.0;
    }
    let e = total as f64 / counts.len() as f64;
    counts.iter().map(|&o| (o as f64 - e).powi(2) / e).sum()
}

/// The chi-square value that a uniform randomizer exceeds with probability 0.001, for `df`
/// degrees of freedom (the table up to 15, the Wilson-Hilferty approximation beyond).
pub fn critical(df: usize) -> f64 {
    const T: [f64; 15] = [10.828, 13.816, 16.266, 18.467, 20.515, 22.458, 24.322, 26.124, 27.877, 29.588, 31.264, 32.909, 34.528, 36.123, 37.697];
    if (1..=15).contains(&df) {
        return T[df - 1];
    }
    let d = df.max(1) as f64;
    let z = 3.090_232;
    d * (1.0 - 2.0 / (9.0 * d) + z * (2.0 / (9.0 * d)).sqrt()).powi(3)
}

/// The pooled uniformity test of several runs' outcomes: `Ok(flag)` (`uniform: chi2 X (limit
/// Y) over P presses in R runs, counts ...`) or `Err(why)`.
pub fn pool<'a>(outcomes: impl IntoIterator<Item = &'a Outcome>, n: u32) -> std::result::Result<String, String> {
    let mut total = vec![0u32; n as usize];
    let mut runs = 0;
    for o in outcomes {
        let Some(cs) = counts_of(o) else { continue };
        if cs.len() != total.len() {
            continue;
        }
        runs += 1;
        for (t, c) in total.iter_mut().zip(cs) {
            *t += c;
        }
    }
    if runs == 0 {
        return Err("no run recorded its counts".into());
    }
    let x = chi2(&total);
    let lim = critical(n as usize - 1);
    let presses: u32 = total.iter().sum();
    let counts = total.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("/");
    if x > lim {
        Err(format!("not uniform: chi2 {x:.1} over {presses} presses in {runs} runs exceeds {lim:.1} (p = 0.001, {} degrees of freedom); counts {counts}", n - 1))
    } else {
        Ok(format!("uniform: chi2 {x:.1} (limit {lim:.1}) over {presses} presses in {runs} runs, counts {counts}"))
    }
}

/// The judge's builds pooled (`verify::verify_in_pool`): the uniformity test over every passing
/// build; a failure fails the report, a pass rides in the base build's flag.
pub fn pool_report(rep: &mut crate::verify::VerifyReport, n: u32) {
    let res = pool(rep.builds.iter().filter_map(|b| b.result.as_ref().ok()).filter(|b| b.outcome.pass).map(|b| &b.outcome), n);
    match res {
        Err(why) => {
            rep.pass = false;
            rep.verdict = why;
        }
        Ok(flag) => {
            let base = rep.builds.iter().position(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass)).or_else(|| rep.builds.iter().position(|b| b.result.as_ref().is_ok_and(|k| k.outcome.pass)));
            if let Some(i) = base
                && let Ok(k) = &mut rep.builds[i].result
            {
                k.outcome.why = if k.outcome.why.is_empty() { flag } else { format!("{}; {flag}", k.outcome.why) };
            }
        }
    }
}

/// The redsim screen's pooled test (`verify::sim`): the base run (salt 0) and salts 1 ..
/// [`SCREEN_SALTS`] - 1, each judged, their counts pooled. `run` records a run of the circuit with
/// a salt.
pub fn screen<F>(c: &Circuit, n: u32, s: &Schedule, base: Outcome, mut run: F) -> Outcome
where
    F: FnMut(&Circuit) -> std::result::Result<Trace, String>,
{
    let mut outs = vec![base.clone()];
    for salt in 1..SCREEN_SALTS {
        let mut cs = c.clone();
        cs.salt = salt;
        let o = match run(&cs) {
            Ok(t) => check(&cs, n, s, &t),
            Err(why) => Outcome::fail(why, Vec::new()),
        };
        if !o.pass {
            return Outcome::fail(format!("seed salt {salt}: {}", o.why), o.table);
        }
        if o.metrics != base.metrics {
            return Outcome::fail(format!("seed salt {salt} measures {} (salt 0: {})", o.metrics_text(), base.metrics_text()), o.table);
        }
        outs.push(o);
    }
    match pool(outs.iter(), n) {
        Ok(flag) => {
            let mut b = base;
            b.why = if b.why.is_empty() { flag } else { format!("{}; {flag}", b.why) };
            b
        }
        Err(why) => Outcome::fail(why, base.table),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chi2_and_limits() {
        assert!((chi2(&[50, 50]) - 0.0).abs() < 1e-9);
        assert!((chi2(&[60, 40]) - 4.0).abs() < 1e-9);
        assert!((critical(1) - 10.828).abs() < 1e-9);
        // the approximation is close to the table where both exist
        let d = 15.0f64;
        let approx = d * (1.0 - 2.0 / (9.0 * d) + 3.090_232 * (2.0 / (9.0 * d)).sqrt()).powi(3);
        assert!((approx - 37.697).abs() < 0.3, "{approx}");
        assert!(critical(31) > critical(15));
    }
}
