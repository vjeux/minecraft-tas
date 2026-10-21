//! A clock member generated and simulated: the members' cache domain (build.rs hashes this file
//! with the clock crates and the generator's own crate).

use clock::Clock;
use clock::extend::Family as ExtFamily;
use clock::sim::{SimOptions, Verdict, simulate};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A generated clock member: its design text (its output retapped when the generator's broke the
/// output rule) and what the judge's simulation says.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemberGen {
    pub text: String,
    pub dims: String,
    pub volume: u32,
    /// The period redsim measured, when it found the design to be a clock within the horizon.
    pub sim: Option<u32>,
    pub blocks: usize,
    /// Its own numbers from that simulation (None: redsim found no clock).
    pub run: Option<site_model::MemberRun>,
}

/// Width x height x depth and volume of a design's box.
pub fn dims_of(c: &Clock) -> (String, u32) {
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    (format!("{}x{}x{}", d[0], d[1], d[2]), (d[0] * d[1] * d[2]).max(0) as u32)
}

/// Generate member `n` of family `f` (its archived base design at `base_path`) into `tmp` and
/// simulate it, once (at the default horizon): the output check is the period's run, and a
/// retapped output comes with its own run (the retap search simulates its pick).
pub fn generate_member(f: &ExtFamily, base_path: &Path, n: i64, horizon: u32, tmp: &Path) -> Result<MemberGen, String> {
    let path = f.build(base_path, n, tmp).map_err(|e| format!("{e:#}"))?;
    let result = (|| {
        let c = Clock::load(&path).map_err(|e| format!("{e:#}"))?;
        // a generator older than the output rule (its output a cell a block enters during the
        // cycle, or not an exterior air cell): the member's output moves to the exterior air cell
        // where the judge's lamp lights evenly, redsim's pick (as `clock retap` moved the archived
        // clocks'), with the run of the design with that output
        let retap = |c: &Clock| -> Result<Option<(Clock, clock::sim::SimReport)>, String> {
            let mut d = c.design.clone();
            d.ports.remove("output");
            let free = Clock::from_design(d).map_err(|e| format!("{e:#}"))?;
            match clock::sim::lamp_outputs(&free, 1).into_iter().next() {
                Some((to, rep)) => {
                    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    std::fs::write(&path, clock::gaps::with_output(&text, to, c.output, None).map_err(|e| format!("{e:#}"))?).map_err(|e| e.to_string())?;
                    Ok(Some((Clock::load(&path).map_err(|e| format!("{e:#}"))?, rep)))
                }
                None => Ok(None),
            }
        };
        // an output cell holding a block of the design breaks the rule whatever the run: straight
        // to the retap (its search runs the design itself; when it finds no cell, the design is no
        // clock without its output either, and the run below says what it is), one long run saved
        // (dropring n = 426: 50 s)
        let held = c.output.is_some_and(|o| c.design.blocks.contains_key(&o));
        let early = if held { retap(&c)? } else { None };
        let (c, report) = match early {
            Some((moved, rep)) => (moved, rep),
            None => {
                let first = simulate(&c, &SimOptions::default());
                // (a held cell's retap already found nothing: not again)
                if !held && first.verdict == Verdict::Invalid && first.why.contains("output cell") {
                    match retap(&c)? {
                        Some((moved, rep)) => (moved, rep),
                        None => (c, first),
                    }
                } else {
                    (c, first)
                }
            }
        };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        // the period at another horizon than the default one: its own run
        let default = SimOptions::default();
        let report = if horizon > 0 && horizon != default.horizon { simulate(&c, &SimOptions { horizon, margin: default.margin }) } else { report };
        let clock = (report.verdict == Verdict::Clock).then_some(&report);
        let sim = clock.filter(|_| horizon > 0 && f.period.eval(n) <= horizon as i64).map(|r| r.period);
        // its own numbers (the base design's are not its: critique, an extended hopper clock of
        // 148 ticks said "on for 18 ticks", its base's 18 of 20)
        let run = clock.and_then(|r| r.output.as_ref()).map(|o| site_model::MemberRun {
            warmup: report.warmup,
            on: o.wave.on,
            edges: o.wave.edges,
            pulses: format!("{:?}", o.wave.widths),
            pulse_every: (!o.wave.gaps.is_empty() && !clock::catalog::uneven(&o.wave.gaps)).then(|| o.wave.gaps[0]).filter(|g| *g > 0 && *g < report.period),
        });
        let (dims, volume) = dims_of(&c);
        Ok(MemberGen { text, dims, volume, sim, blocks: c.blocks(), run })
    })();
    let _ = std::fs::remove_file(&path);
    result
}
