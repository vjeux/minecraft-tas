//! A run's verdict, shared by the redsim runner and the real-game judge: what the machine did
//! (flies, shuttles, stops, breaks), its steady speed, latency, end state.

use crate::machine::{Machine, travel_name};
use crate::track::{self, Recorder, Steady};
use clock::Pos;

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// Nothing pending any more (no scheduled tick, block event or moving block): the machine
    /// came to rest at this tick.
    Idle(u32),
    /// A machine block reached the corridor's outer layer at this tick (it flew the whole
    /// corridor).
    Edge(u32),
    /// The horizon was reached with the machine still busy.
    Horizon(u32),
    /// A block was destroyed with a drop (an item entity) at this tick.
    Drop(u32, String),
    /// The model met a case it does not model (redsim) or the run failed.
    Error(u32, String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Moves itself steadily (same shape every P gt, d cells further) and ends intact (at rest
    /// against a stop, or still flying at the corridor's end / the horizon).
    Flies,
    /// Two-way: moves out and comes back, periodically (the whole state repeats in place, the
    /// translation takes 2+ values).
    Shuttles,
    /// Moved and came to rest intact, but never reached a steady flight (fewer than 2 periods).
    Moves,
    /// Never moved (or only moved in place, a clock).
    Still,
    /// Lost its shape: blocks left behind, missing, or a block destroyed.
    Broken,
    /// A case redsim does not model, or the run failed.
    Unmodeled,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Flies => "FLIES",
            Verdict::Shuttles => "SHUTTLES",
            Verdict::Moves => "MOVES",
            Verdict::Still => "STILL",
            Verdict::Broken => "BROKEN",
            Verdict::Unmodeled => "UNMODELED",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlyReport {
    pub name: String,
    pub blocks: usize,
    pub verdict: Verdict,
    /// Details: why it is broken, how it ended.
    pub why: String,
    pub steady: Option<Steady>,
    /// The first tick a machine cell changed after the kick (the first motion).
    pub first_change: Option<u32>,
    /// The first tick the machine stands one cell (or more) from where it started.
    pub first_arrival: Option<u32>,
    pub visits: Vec<(u32, Pos)>,
    pub end: End,
    /// Where it rests at the end (when its shape is intact).
    pub final_shift: Option<Pos>,
    /// End state vs the design moved by the final (or last known) shift.
    pub extra: Vec<(Pos, String)>,
    pub missing: Vec<(Pos, String)>,
    pub start: String,
    pub dims: [i32; 3],
}

impl FlyReport {
    /// The direction of the steady motion (a unit vector), when it moves along one axis.
    pub fn direction(&self) -> Option<Pos> {
        let s = self.steady.as_ref()?;
        let n = s.cells();
        if n == 0 || s.d.iter().filter(|c| **c != 0).count() != 1 {
            return None;
        }
        Some(s.d.map(|c| c / n))
    }

    /// Steady speed in gt per block (P / cells per period).
    pub fn speed(&self) -> Option<f64> {
        let s = self.steady.as_ref()?;
        let n = s.cells();
        if n == 0 { None } else { Some(s.period as f64 / n as f64) }
    }

    /// `12 gt/block` or `12.5 gt/block` (P/d exact as a fraction).
    pub fn speed_text(&self) -> String {
        match &self.steady {
            Some(s) if s.cells() > 0 => {
                let n = s.cells() as u32;
                if s.period % n == 0 { format!("{} gt/block", s.period / n) } else { format!("{}/{} gt/block", s.period, n) }
            }
            _ => "-".into(),
        }
    }

    /// The one-line summary.
    pub fn summary(&self) -> String {
        let [w, h, d] = self.dims;
        let end = match &self.end {
            End::Idle(t) => format!("at rest from tick {t}"),
            End::Edge(t) => format!("reached the corridor end at tick {t}"),
            End::Horizon(t) => format!("still running at tick {t}"),
            End::Drop(t, what) => format!("tick {t}: {what}"),
            End::Error(t, what) => format!("tick {t}: {what}"),
        };
        let shift = |s: Pos| format!("{},{},{}", s[0], s[1], s[2]);
        match self.verdict {
            Verdict::Flies | Verdict::Shuttles | Verdict::Moves => {
                let dir = self.direction().map(travel_name).unwrap_or("in place");
                let st = self.steady.as_ref().map(|s| format!("period {} gt, {} cells/period ({} periods seen), ", s.period, s.cells(), s.periods)).unwrap_or_default();
                let rest = self.final_shift.map(|s| format!(", rests intact at {}", shift(s))).unwrap_or_default();
                format!(
                    "{}: {} {}, {}, {st}first move t{} (one cell by t{}), {} blocks, {w}x{h}x{d}, {}{rest}, {}",
                    self.verdict.word(),
                    self.name,
                    self.speed_text(),
                    dir,
                    self.first_change.map(|t| t.to_string()).unwrap_or("-".into()),
                    self.first_arrival.map(|t| t.to_string()).unwrap_or("-".into()),
                    self.blocks,
                    end,
                    self.start
                )
            }
            v => format!("{}: {} {} ({end}){}", v.word(), self.name, self.why, if self.extra.is_empty() && self.missing.is_empty() { String::new() } else { format!("; extra {:?}; missing {:?}", short_cells(&self.extra), short_cells(&self.missing)) }),
        }
    }
}

fn short_cells(v: &[(Pos, String)]) -> Vec<String> {
    v.iter().take(6).map(|(p, s)| format!("{},{},{} {}", p[0], p[1], p[2], s)).collect()
}

/// Classify a recorded run.
pub fn classify(m: &Machine, r: &Recorder, end: End) -> FlyReport {
    let design = &m.design().blocks;
    let from = m.clock.free_from() as usize;
    let steady = track::steady(r, from);
    let visits = track::visits(r);
    let first_change = (1..r.ticks()).find(|t| r.canon[*t] != r.canon[0] || r.corner[*t] != r.corner[0]).map(|t| t as u32);
    let first_arrival = r.shifts.iter().enumerate().find(|(_, s)| s.is_some_and(|s| s != [0, 0, 0])).map(|(t, _)| t as u32);
    let final_shift = r.shifts.last().copied().flatten();
    let guess = final_shift.or_else(|| visits.last().map(|(_, s)| *s)).unwrap_or([0, 0, 0]);
    let (extra, missing) = if final_shift.is_some() { (Vec::new(), Vec::new()) } else { track::end_diff(r, design, guess) };
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in design.keys() {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let dims = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut rep = FlyReport {
        name: m.name().to_string(),
        blocks: m.blocks(),
        verdict: Verdict::Still,
        why: String::new(),
        steady: steady.clone(),
        first_change,
        first_arrival,
        visits: visits.clone(),
        end: end.clone(),
        final_shift,
        extra,
        missing,
        start: m.clock.start_label(),
        dims,
    };
    let moving_steady = steady.as_ref().is_some_and(|s| s.cells() > 0 && s.periods >= 2);
    let distinct: std::collections::BTreeSet<Pos> = visits.iter().map(|(_, s)| *s).collect();
    match &end {
        End::Drop(_, what) => {
            rep.verdict = Verdict::Broken;
            rep.why = what.clone();
        }
        End::Error(_, what) => {
            rep.verdict = Verdict::Unmodeled;
            rep.why = what.clone();
        }
        End::Edge(_) | End::Horizon(_) => {
            if moving_steady {
                rep.verdict = Verdict::Flies;
            } else if steady.as_ref().is_some_and(|s| s.cells() == 0 && s.periods >= 2) && distinct.len() >= 2 {
                rep.verdict = Verdict::Shuttles;
            } else if steady.as_ref().is_some_and(|s| s.cells() == 0 && s.periods >= 2) {
                rep.verdict = Verdict::Still;
                rep.why = format!("oscillates in place (period {} gt): a clock, not a machine", steady.as_ref().unwrap().period);
            } else {
                rep.verdict = Verdict::Broken;
                rep.why = "no steady motion".into();
                if rep.extra.is_empty() && rep.missing.is_empty() && final_shift.is_none() {
                    let (e, mi) = track::end_diff(r, design, guess);
                    rep.extra = e;
                    rep.missing = mi;
                }
            }
        }
        End::Idle(_) => match final_shift {
            Some(s) if s != [0, 0, 0] => {
                rep.verdict = if moving_steady { Verdict::Flies } else { Verdict::Moves };
            }
            Some(_) => {
                rep.verdict = Verdict::Still;
                rep.why = if distinct.len() >= 2 { "moved and came back once, then stopped".into() } else { "never moved".into() };
            }
            None => {
                rep.verdict = Verdict::Broken;
                rep.why = if r.last.is_empty() { "nothing left".into() } else { format!("at rest in a shape that is not the design ({} cells, design {})", r.last.len(), design.len()) };
            }
        },
    }
    rep
}
