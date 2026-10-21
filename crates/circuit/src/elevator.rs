//! Study 7's ELEVATOR (track fly, coordinator 2026-10-21): a vertical two-way flying machine
//! between two floors `distance` blocks apart (`circuit: elevator N`, `travel: 0 1 0`), a call
//! button at each floor (`port input.bottom`, `port input.top`: the design's buttons) and a
//! platform; the judge's armor stand rides it from `port rider` (the cell above the platform with
//! the machine at the bottom floor, as built).
//!
//! The rule (coordinator): a press at the floor where the platform stands sends it to the other
//! floor; a press at the other floor calls it; a press while it travels calls it to that floor
//! (latched: it reverses at the next stop when it was going the other way). So after any presses
//! it comes to rest at the floor implied by the LAST press: that button's floor, unless the
//! platform stood docked at that floor when it was pressed (then the other floor). A press within
//! [`DOCKED`] gt of its arrival there may go either way.
//!
//! Checked at the end of every phase: the machine (every non-station block) still for [`HOLD`]
//! gt, intact, at the expected floor; the rider (the only entity) standing on the platform there.
//! The rider must reach each floor's button from the platform (eye 1.62 above the platform, a
//! player's 4.5 block reach). Nothing may be left behind (it breaks the machine's shape).

use crate::check::Outcome;
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, InputHow, Kind, Pos};

/// The machine must stand still this long at the end of each phase.
pub const HOLD: usize = 60;
/// A press counts as made at a docked platform when the machine stood still at that floor for
/// this long before it; a press closer to the arrival may go either way.
pub const DOCKED: usize = 10;
/// A player's block interaction range (26.x), from the eyes (1.62 above the feet).
pub const REACH: f64 = 4.5;

fn height(c: &Circuit) -> i32 {
    match c.kind {
        Some(Kind::Elevator(n)) => n as i32,
        _ => c.distance.unwrap_or(0) as i32,
    }
}

/// One trip's generous estimate (10 gt per block, plus the start and the stop).
pub fn trip(c: &Circuit) -> u32 {
    12 * height(c) as u32 + 60
}

fn input(c: &Circuit, name: &str) -> Option<usize> {
    c.inputs.iter().position(|i| i.name == name)
}

/// The phases: each a press sequence (offset from the phase start, input name), then a settle.
pub fn scenarios(c: &Circuit) -> Vec<(String, Vec<(u32, &'static str)>)> {
    let tr = trip(c);
    let mut v: Vec<(String, Vec<(u32, &'static str)>)> = vec![
        ("call up".into(), vec![(0, "top")]),
        ("send down".into(), vec![(0, "top")]),
        ("send up".into(), vec![(0, "bottom")]),
        ("call down".into(), vec![(0, "bottom")]),
        ("call up, called back mid-trip".into(), vec![(0, "top"), (tr / 2, "bottom")]),
        ("send up, called up mid-trip".into(), vec![(0, "bottom"), (tr / 3, "top")]),
        ("send down, called back mid-trip".into(), vec![(0, "top"), (tr / 2, "top")]),
        ("call down, called down again".into(), vec![(0, "bottom"), (tr / 4, "bottom")]),
    ];
    // seeded random presses: 1..4 of them, any button, gaps of 5 gt to 1.5 trips
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut rnd = |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    for k in 0..6 {
        let n = 1 + rnd(4) as usize;
        let mut at = 0u32;
        let mut p = Vec::new();
        for j in 0..n {
            if j > 0 {
                at += 5 + rnd((tr * 3 / 2) as u64) as u32;
            }
            p.push((at, if rnd(2) == 0 { "bottom" } else { "top" }));
        }
        v.push((format!("random {}", k + 1), p));
    }
    // the sweep: a first press at either floor, then a second press at either floor at 16
    // offsets across the trip (2 gt to one trip estimate)
    for first in ["bottom", "top"] {
        for second in ["bottom", "top"] {
            for j in 0..16 {
                let k = 2 + j * tr / 16;
                v.push((format!("{first} then {second} after {k} gt"), vec![(0, first), (k, second)]));
            }
        }
    }
    v
}

/// The elevator's stimulus: rest at the bottom floor, then each scenario and its settle.
pub fn stimulus(c: &Circuit) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let settle = 3 * trip(c) + 120;
    let mut t = c.clock.free_from() + 2 * HOLD as u32;
    s.phase(0, t, "rest");
    for (label, presses) in scenarios(c) {
        let last = presses.iter().map(|p| p.0).max().unwrap_or(0);
        for (off, name) in &presses {
            if let Some(i) = input(c, name) {
                s.pulse(i, t + off, 1);
            }
        }
        s.phase(t, t + last + settle, label);
        t += last + settle;
    }
    s.ticks = t;
    s
}

/// The static checks: travel up, two buttons, a rider cell, each floor's button in reach.
pub fn check_ports(c: &Circuit) -> anyhow::Result<()> {
    if c.travel != Some([0, 1, 0]) {
        anyhow::bail!("elevator: needs `travel: 0 1 0` (it starts at the bottom floor)");
    }
    if c.inputs.iter().any(|i| i.how != InputHow::Button) {
        anyhow::bail!("elevator: its call inputs are the design's buttons (`port input.bottom: x y z button`, `port input.top: ...`)");
    }
    let Some(r) = c.clock.design.ports.get("rider").copied() else {
        anyhow::bail!("elevator: needs `port rider: x y z` (the cell above the platform, the machine at the bottom floor)");
    };
    let h = height(c);
    for (name, dy) in [("bottom", 0), ("top", h)] {
        let Some(b) = c.inputs.iter().find(|i| i.name == name) else { continue };
        let eye = [r[0] as f64 + 0.5, (r[1] + dy) as f64 + 1.62, r[2] as f64 + 0.5];
        let d = (0..3).map(|k| (b.pos[k] as f64 + 0.5 - eye[k]).powi(2)).sum::<f64>().sqrt();
        if d > REACH {
            anyhow::bail!("elevator: the {name} button at {:?} is {d:.2} blocks from the rider's eyes at that floor (a player reaches {REACH})", b.pos);
        }
    }
    Ok(())
}

/// The judge (see the module doc).
pub fn check(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    if let Some(e) = &t.ended {
        return Outcome::fail(format!("the run ended early: {e}"), vec![e.clone()]);
    }
    let h = height(c);
    let Some(rider) = c.clock.design.ports.get("rider").copied() else { return Outcome::fail("no rider port", table) };
    let mut r = crate::fly::Recorder::machine(&c.clock.design.blocks, None, &c.stations);
    for f in 0..t.frames() {
        r.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    let n = r.canon.len();
    let same = |a: usize, b: usize| r.canon[a] == r.canon[b] && r.corner[a] == r.corner[b];
    let floor_of = |name: &str| if name == "top" { h } else { 0 };
    let shift_y = |f: usize| r.shifts[f].filter(|p| p[0] == 0 && p[2] == 0).map(|p| p[1]);
    // the press frames: rising edges of each button's powered state
    let mut presses: Vec<(usize, &str)> = Vec::new();
    for (i, inp) in c.inputs.iter().enumerate() {
        let lv = &t.inputs[i];
        for f in 1..lv.len() {
            if lv[f] && !lv[f - 1] {
                presses.push((f, if inp.name == "top" { "top" } else { "bottom" }));
            }
        }
    }
    presses.sort();
    let (mut trip_up, mut trip_down) = (None::<usize>, None::<usize>);
    for (k, (from, to, label)) in s.phases.iter().enumerate() {
        let (from, to) = (*from as usize, (*to as usize).min(n));
        if to <= from + HOLD {
            return Outcome::fail(format!("{label}: too short to judge"), table);
        }
        let last = to - 1;
        // the expected floor: from the last press of the phase
        let mut expect: Vec<i32> = vec![0];
        let mut last_press = None;
        if let Some(&(f, name)) = presses.iter().filter(|(f, _)| *f >= from && *f < to).last() {
            let here = floor_of(name);
            let still = (f.saturating_sub(DOCKED)..f).all(|g| g >= 1 && same(g, f - 1));
            let at = shift_y(f - 1) == Some(here);
            expect = if at && still {
                vec![h - here]
            } else if at {
                vec![here, h - here]
            } else {
                vec![here]
            };
            last_press = Some((f, name));
        } else if k > 0 {
            // no press: it stays where the previous phase left it
            expect = vec![shift_y(from.saturating_sub(1)).unwrap_or(-1)];
        }
        if !(last - HOLD..=last).all(|f| same(f, last)) {
            return Outcome::fail(format!("{label}: still moving {HOLD} gt before the phase ends"), table);
        }
        let Some(y) = shift_y(last) else {
            return Outcome::fail(format!("{label}: stopped, but the machine is not intact (or not straight above its start)"), table);
        };
        if !expect.contains(&y) {
            let want = expect.iter().map(|e| if *e == 0 { "the bottom floor" } else { "the top floor" }).collect::<Vec<_>>().join(" or ");
            let pressed = last_press.map(|(f, nm)| format!(" (last press: {nm} at frame {f})")).unwrap_or_default();
            return Outcome::fail(format!("{label}: rests {y} blocks up, want {want}{pressed}"), table);
        }
        // the rider: the one entity, on the platform at that floor
        let snap = t.entities.iter().rev().find(|(f, _)| (*f as usize) <= last && (*f as usize) > last - HOLD);
        let Some((sf, ents)) = snap else {
            return Outcome::fail(format!("{label}: no entity record at the end of the phase"), table);
        };
        if ents.len() != 1 || !ents[0].0.ends_with("armor_stand") {
            return Outcome::fail(format!("{label}: {} entities at frame {sf} ({}), want the rider alone", ents.len(), ents.iter().map(|e| e.0.as_str()).collect::<Vec<_>>().join(", ")), table);
        }
        let p = ents[0].1;
        let (wx, wy, wz) = (rider[0] as f64 + 0.5, (rider[1] + y) as f64, rider[2] as f64 + 0.5);
        if (p[0] - wx).abs() > 0.3 || (p[2] - wz).abs() > 0.3 || (p[1] - wy).abs() > 0.1 {
            return Outcome::fail(format!("{label}: the rider is at ({:.2}, {:.2}, {:.2}), not on the platform ({wx:.2}, {wy:.2}, {wz:.2})", p[0], p[1], p[2]), table);
        }
        // the trip time: the press to the first frame of the final rest
        let rest = (from..=last).rev().take_while(|&f| same(f, last)).last().unwrap_or(from);
        if let Some((f, _)) = last_press {
            let d = rest.saturating_sub(f);
            match label.as_str() {
                "call up" => trip_up = Some(d),
                "send down" => trip_down = Some(d),
                _ => {}
            }
        }
        table.push(format!("{label}: rests {} at frame {rest}, rider at ({:.2}, {:.2}, {:.2})", if y == 0 { "at the bottom" } else { "at the top" }, p[0], p[1], p[2]));
    }
    let mut metrics = vec![("height".into(), format!("{h} blocks"))];
    if let (Some(u), Some(d)) = (trip_up, trip_down) {
        metrics.push(("trip".into(), format!("up {u} gt, down {d} gt")));
    }
    metrics.push(("rider".into(), "carries a rider, calls and sends from each floor".into()));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// Unused offsets guard (a phase's press at `Pos`): kept for the mid-trip sweep.
pub fn _pos(_: Pos) {}
