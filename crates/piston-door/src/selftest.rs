//! `door selftest`: a fast known-answer suite every search and judge tool must pass before a push
//! (AGENTS.md's push gate). Each check runs real `mctas door ...` commands (this binary, as child
//! processes, from the repository root) on known answers and compares their output with what
//! must come out: archived anchors that WORK, regions the pocket search must re-find, model
//! checks with zero mismatches, a candidate the update-order model must reject, the SAT model
//! re-finding a known wiring, `door ideal-rs` agreeing with `door ideal`, generator membership
//! that must not change silently. One line per check, PASS or FAIL; `--quick` keeps the fast
//! subset (< 2 min). A check whose known answer changes on purpose (e.g. a generator that now
//! admits fast-clean) is updated here in the same commit, with the reason.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub struct Opts {
    pub quick: bool,
    pub only: Option<String>,
    pub list: bool,
    pub jobs: usize,
    pub jvm_jobs: usize,
    pub keep: bool,
}

type Pred = Box<dyn Fn(&[String]) -> std::result::Result<String, String> + Send + Sync>;

struct Check {
    name: String,
    what: &'static str,
    quick: bool,
    jvm: bool,
    /// commands run in order (args after `mctas`); `{tmp}` is this check's scratch folder
    steps: Vec<Vec<String>>,
    timeout: u64,
    pred: Pred,
}

const A: &str = "designs/piston-door-3x3/archive";
const C79: &str = "designs/piston-door-3x3/tracks/c79/c79-closed.txt";
const BULB: &str = "designs/piston-door-3x3/selftest/c79-bulb-periodic.txt";
/// A white-concrete door (the community 7x8's material): FULL + frame 12/12 follow the door's own
/// material in the redsim evaluator, as in `door properties`.
const CONCRETE: &str = "designs/piston-door-3x3/selftest/e78s-43-white-concrete.txt";
const ANCHORS: [&str; 5] = ["c79-community-63b-51b", "e88-t7b-43b", "t7b-nb2-43b", "s27-bl52-52b", "m79s-sculk-63b-50-50b"];
const SAT_PALETTE: &str = "block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,glass,leaves,upiston";

fn args(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

fn has(out: &str, pat: &str) -> std::result::Result<(), String> {
    if out.contains(pat) { Ok(()) } else { Err(format!("no `{pat}` in the output: {}", tail(out))) }
}

fn tail(out: &str) -> String {
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    let n = lines.len();
    lines[n.saturating_sub(2)..].join(" | ").chars().take(300).collect()
}

/// The verdict line of `door ideal` / `door ideal-rs` (WORKS ... / BROKEN: ...), normalized.
fn ideal_verdict(out: &str) -> Option<String> {
    out.lines().rev().find(|l| l.starts_with("WORKS") || l.starts_with("BROKEN")).map(|l| l.replace(" in redsim", ""))
}

fn checks() -> Vec<Check> {
    let mut v: Vec<Check> = Vec::new();
    // 1. the redsim judge on the anchors (and the periodic bulb fixture)
    let mut files: Vec<String> = ANCHORS.iter().map(|a| format!("{A}/{a}.txt")).collect();
    files.push(BULB.into());
    files.push(CONCRETE.into());
    let mut s = vec!["door".to_string(), "cosearch".into(), "eval".into()];
    s.extend(files.iter().cloned());
    let fl = files.clone();
    v.push(Check {
        name: "judge:redsim-anchors".into(),
        what: "the redsim judge (cosearch eval: 32 positions x 3 cycles, FULL, hallway, frame) on the archived anchors + the periodic bulb fixture + a white-concrete door: WORKS",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            let out = &o[0];
            for f in &fl {
                let line = out.lines().find(|l| l.starts_with(f.as_str())).ok_or(format!("{f}: no verdict line"))?;
                for need in ["WORKS", "FULL", "hallway ok", "frame 12/12"] {
                    if !line.contains(need) {
                        return Err(format!("{f}: no `{need}`: {}", line.chars().take(240).collect::<String>()));
                    }
                }
            }
            Ok(format!("{} designs WORK (FULL, hallway, frame 12/12)", fl.len()))
        }),
    });
    // 2. the real judge (JVM) on the anchors
    for (i, a) in ANCHORS.iter().enumerate() {
        v.push(Check {
            name: format!("judge:verify-{a}"),
            what: "`door verify` (the real game, 4 positions x 3 cycles, rotations) on an archived anchor: WORKS",
            quick: i == 0,
            jvm: true,
            steps: vec![args(&format!("door verify {A}/{a}.txt"))],
            timeout: 600,
            pred: Box::new(|o| {
                let l = o[0].lines().find(|l| l.starts_with("WORKS:")).ok_or(format!("not WORKS: {}", tail(&o[0])))?;
                Ok(l.chars().take(120).collect())
            }),
        });
    }
    v.push(Check {
        name: "judge:verify-periodic-bulb".into(),
        what: "`door verify` on c79 + a copper bulb that toggles once per cycle (closed state of period 2): the periodic judge accepts it",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("door verify {BULB} --no-rotations"))],
        timeout: 600,
        pred: Box::new(|o| has(&o[0], "WORKS:").map(|_| "WORKS (periodic rest)".into())),
    });
    // a NEGATIVE anchor: a judge that accepts everything fails here
    const NEG: &str = "designs/piston-door-3x3/selftest/c79-no-dust.txt";
    v.push(Check {
        name: "judge:redsim-negative".into(),
        what: "the redsim judge rejects c79 with its hidden dust (-2,4) removed (the door no longer opens)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("?door cosearch eval {NEG}"))],
        timeout: 120,
        pred: Box::new(|o| {
            let line = o[0].lines().find(|l| l.starts_with(NEG)).ok_or("no verdict line".to_string())?;
            if line.contains("WORKS") { Err(format!("accepted a broken door: {}", line.chars().take(200).collect::<String>())) } else { Ok("rejected".into()) }
        }),
    });
    v.push(Check {
        name: "judge:verify-negative".into(),
        what: "`door verify` rejects c79 with its hidden dust removed",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?door verify {NEG} --no-rotations"))],
        timeout: 600,
        pred: Box::new(|o| if o[0].lines().any(|l| l.starts_with("WORKS:")) { Err("accepted a broken door".into()) } else { has(&o[0], "BROKEN").map(|_| "rejected (BROKEN)".into()) }),
    });
    // 3. pockets re-find known regions
    let r = "-1,-3;-2,-3;-2,-2;-2,-1;-1,-2;0,-2";
    v.push(Check {
        name: "pockets:c79-bottom-left".into(),
        what: "c79's bottom-left wiring erased: plan-from --region + pockets k=5 (stone, dust, observers) re-find a filling that screens perfect",
        quick: true,
        jvm: false,
        steps: vec![
            args(&format!("door cosearch plan-from {C79} --region={r} --out {{tmp}}/q79-bl.ideal")),
            args(&format!("door cosearch pockets {C79} --box=-2,-3:4,5 --assist {{tmp}}/q79-bl.ideal --region-cells={r} --keep-lever --no-hallway --k 5 --fill-palette stone,dust,observer --threads 32 --out-dir {{tmp}}/bl")),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[1], "FIND ")?;
            let n = o[1].lines().filter(|l| l.starts_with("FIND ")).count();
            Ok(format!("{n} find line(s)"))
        }),
    });
    let c88 = "designs/piston-door-3x3/tracks/c88";
    v.push(Check {
        name: "pockets:s25-br-gaps".into(),
        what: "c88 s25 -> s26: the bottom-right chain (observer, AIR, normal piston, observer) is found with --gaps and not without",
        quick: true,
        jvm: false,
        steps: vec![
            args(&format!("door cosearch pockets {c88}/s25.txt --box=-3,-2:4,5 --assist {c88}/s25.ideal --keep-lever --gaps --k 3 --fill-palette observer,piston,stone,dust --threads 32 --out-dir {{tmp}}/br")),
            args(&format!("door cosearch pockets {c88}/s25.txt --box=-3,-2:4,5 --assist {c88}/s25.ideal --keep-lever --k 3 --fill-palette observer,piston,stone,dust --threads 32 --out-dir {{tmp}}/br0")),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "FIND ")?;
            if o[1].contains("FIND ") {
                return Err("found without --gaps too (the chain needs a gap)".into());
            }
            has(&o[1], " 0 finds")?;
            Ok("found with --gaps, not without".into())
        }),
    });
    // 4. satwire
    v.push(Check {
        name: "satwire:c79-from-wired-check".into(),
        what: "satwire's model on c79's own wiring (--from-wired --check): 0 piston-tick mismatches",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door satwire {C79} --from-wired --check --palette {SAT_PALETTE}"))],
        timeout: 300,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("check: ") && l.contains("mismatches")).ok_or(format!("no check line: {}", tail(&o[0])))?;
            if l.starts_with("check: 0 piston-tick mismatches") { Ok(l.to_string()) } else { Err(l.to_string()) }
        }),
    });
    let bud = |uo: bool| {
        args(&format!(
            "door satwire {A}/pockets-63b-50-0-50b.txt --from-wired --full-frame --forbid-ring --sound --lever-oop --log-doors --one-warp --dmax 0 --skip 0 --palette {SAT_PALETTE} --explain designs/piston-door-3x3/tracks/satwire/fixtures/bud48-pk3-s0.txt{}",
            if uo { " --update-order" } else { "" }
        ))
    };
    v.push(Check {
        name: "satwire:bud48-update-order".into(),
        what: "a saved 48-block candidate the judge rejects (a BUD: R pair under a ceiling lever): satwire --update-order rejects it",
        quick: true,
        jvm: false,
        steps: vec![bud(true)],
        timeout: 300,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.contains("REJECTS")).ok_or(format!("not rejected: {}", tail(&o[0])))?;
            if !l.contains("update-order") {
                return Err(format!("rejected, but not by the update-order clauses: {l}"));
            }
            Ok("rejected by the update-order clauses".into())
        }),
    });
    v.push(Check {
        name: "satwire:bud48-old-model".into(),
        what: "the same candidate WITHOUT --update-order is accepted by the model (the hole the update-order clauses close)",
        quick: false,
        jvm: false,
        steps: vec![bud(false)],
        timeout: 300,
        pred: Box::new(|o| if o[0].contains("REJECTS") { Err("rejected without --update-order (fixture or model changed)".into()) } else { has(&o[0], "explain").map(|_| "accepted (as expected)".into()) }),
    });
    // 5. boxproof (whole-box SAT model)
    v.push(Check {
        name: "boxproof:c79-fixed".into(),
        what: "boxproof's model admits the community 7x9 with every block fixed (--split-rounds), redsim screen Pass",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door boxproof bmc --w 7 --h 9 --dx 2 --dy 3 --t 17 --lever-on --frame12 --split-rounds --fix {C79} --fix-all --iters 2 --out {{tmp}}/fixed"))],
        timeout: 300,
        pred: Box::new(|o| {
            has(&o[0], "SAT in")?;
            has(&o[0], "screen Pass")?;
            Ok("SAT, screen Pass".into())
        }),
    });
    v.push(Check {
        name: "boxproof:c79-no-split".into(),
        what: "without --split-rounds the same instance is UNSAT (two block events of one round meet at (1,-1): the order matters)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door boxproof bmc --w 7 --h 9 --dx 2 --dy 3 --t 17 --lever-on --frame12 --fix {C79} --fix-all --iters 2 --out {{tmp}}/nosplit"))],
        timeout: 300,
        pred: Box::new(|o| has(&o[0], "UNSAT in").map(|_| "UNSAT".into())),
    });
    v.push(Check {
        name: "boxproof:c79-refind-cell".into(),
        what: "boxproof re-finds a freed c79 wiring cell (the hidden dust (-2,4)): SAT, screen Pass",
        quick: false,
        jvm: false,
        steps: vec![args(&format!("door boxproof bmc --w 7 --h 9 --dx 2 --dy 3 --t 17 --lever-on --frame12 --split-rounds --palette stone,dust,obs --fix {C79} --fix-all --free 0,7 --iters 5 --out {{tmp}}/refind"))],
        timeout: 600,
        pred: Box::new(|o| has(&o[0], "screen Pass").map(|_| "re-found, screen Pass".into())),
    });
    v.push(Check {
        name: "boxproof:fuzz".into(),
        what: "boxproof's model vs redsim on 3000 random 5x5 boxes (seed 11): no unflagged mismatch",
        quick: false,
        jvm: false,
        steps: vec![args("door boxproof fuzz --count 3000 --w 5 --h 5 --ticks 30 --threads 32 --seed 11")],
        timeout: 600,
        pred: Box::new(|o| {
            if let Some(l) = o[0].lines().find(|l| l.contains("MISMATCH (unflagged)")) {
                return Err(l.trim().to_string());
            }
            has(&o[0], "match (static)").map(|_| "0 unflagged mismatches".into())
        }),
    });
    // 6. door ideal-rs vs door ideal
    for (plan, quick) in [("designs/piston-door-3x3/tracks/bridge/s79/c78z.ideal", true), ("designs/piston-door-3x3/tracks/bridge/s79/c78z-u1.ideal", false), ("designs/piston-door-3x3/tracks/c88/s25.ideal", false)] {
        let stem = Path::new(plan).file_stem().unwrap().to_string_lossy().to_string();
        v.push(Check {
            name: format!("ideal:agree-{stem}"),
            what: "`door ideal-rs` (redsim) and `door ideal` (the game) give the same verdict and ticks on a plan, in the default tile phase and with --between-ticks (c78z: a 1 gt pulse)",
            quick,
            jvm: true,
            steps: vec![args(&format!("?door ideal-rs {plan}")), args(&format!("?door ideal {plan}")), args(&format!("?door ideal-rs {plan} --between-ticks")), args(&format!("?door ideal {plan} --between-ticks"))],
            timeout: 600,
            pred: Box::new(|o| {
                let mut out = Vec::new();
                for (k, mode) in [(0, "tile"), (2, "between")] {
                    let a = ideal_verdict(&o[k]).ok_or(format!("ideal-rs {mode}: no verdict: {}", tail(&o[k])))?;
                    let b = ideal_verdict(&o[k + 1]).ok_or(format!("ideal {mode}: no verdict: {}", tail(&o[k + 1])))?;
                    if a != b {
                        return Err(format!("{mode}: ideal-rs `{a}` vs ideal `{b}`"));
                    }
                    out.push(format!("{mode}: {}", a.chars().take(60).collect::<String>()));
                }
                Ok(out.join(" | "))
            }),
        });
    }
    // 6b. plan phase: an observer-driven input works at plan t exactly like the wired door at t
    const PH: &str = "designs/piston-door-3x3/selftest/s25-br-observer.ideal";
    v.push(Check {
        name: "ideal:plan-phase-observer".into(),
        what: "s25 + BR driven at the ticks s26's observer powers it: ideal-rs (default tile phase) opens/closes in 34/34 like the wired s26; the old --between-ticks rule is 1 gt late (35/35)",
        quick: true,
        jvm: false,
        steps: vec![
            args(&format!("?door ideal-rs {PH}")),
            args("?door cosearch eval designs/piston-door-3x3/tracks/c88/s26.txt"),
            args(&format!("?door ideal-rs {PH} --between-ticks")),
        ],
        timeout: 300,
        pred: Box::new(|o| {
            let plan = ideal_verdict(&o[0]).ok_or(format!("ideal-rs: no verdict: {}", tail(&o[0])))?;
            let wired = o[1].lines().find(|l| l.contains("s26.txt:")).ok_or(format!("s26: no verdict: {}", tail(&o[1])))?;
            if !wired.contains("WORKS open 34 close 34") {
                return Err(format!("the wired s26 changed: {}", wired.chars().take(120).collect::<String>()));
            }
            if !plan.contains("open after 34 ticks / closed after 34 ticks") {
                return Err(format!("tile-phase plan `{plan}` != the wired s26 (34 / 34)"));
            }
            let old = ideal_verdict(&o[2]).unwrap_or_default();
            if !old.contains("open after 35 ticks / closed after 35 ticks") {
                return Err(format!("--between-ticks gave `{old}` (expected 35 / 35)"));
            }
            Ok("plan at t = wired at t (34/34); between-ticks 35/35".into())
        }),
    });
    v.push(Check {
        name: "ideal:plan-phase-observer-jvm".into(),
        what: "the same plan in the game (`door ideal`, tile phase): 34/34 like the wired s26",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?door ideal {PH}"))],
        timeout: 600,
        pred: Box::new(|o| {
            let v = ideal_verdict(&o[0]).ok_or(format!("no verdict: {}", tail(&o[0])))?;
            if v.contains("open after 34 ticks / closed after 34 ticks") { Ok(v.chars().take(100).collect()) } else { Err(v) }
        }),
    });
    // 7. generator membership (docs/audit.md acceptance list)
    let expect: Vec<(&str, [&str; 6])> = vec![
        ("c79-closed", ["out", "out", "IN", "IN", "IN", "IN"]),
        ("fast-clean-a1c-107b", ["out", "out", "out", "out", "out", "out"]),
        ("eligible-s4ff-49b", ["out", "out", "out", "out", "out", "out"]),
    ];
    v.push(Check {
        name: "audit:generator-membership".into(),
        what: "docs/audit.md's acceptance list: c79 only with --ext-pullers (g3+), fast-clean and eligible-s4ff outside every generator (update here when a generator fix admits them)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("door audit --sections mech --only-flat {C79} {A}/fast-clean-a1c-107b.txt {A}/eligible-s4ff-49b.txt"))],
        timeout: 300,
        pred: Box::new(move |o| {
            for (name, cols) in &expect {
                let row = o[0].lines().find(|l| l.starts_with(&format!("| {name} |"))).ok_or(format!("{name}: no row"))?;
                let cells: Vec<&str> = row.split('|').map(str::trim).collect();
                // | design | mechanism | g1 .. g6 | ...
                let got: Vec<&str> = cells.iter().skip(3).take(6).cloned().collect();
                if got != cols.to_vec() {
                    return Err(format!("{name}: g1..g6 = {got:?}, expected {cols:?}"));
                }
            }
            Ok("membership as documented".into())
        }),
    });
    // 8. the clock judge (docs/clocks.md): known periods on redsim, the real game agreeing, a
    // non-clock rejected
    const CAL: &str = "designs/clocks/calibration";
    let cal: Vec<(&str, &str)> = vec![
        // (periods print with their real time since 2026-10-19, vjeux: "20 gt (1 s)")
        ("observer-pair", "cal-observer-pair period 6 gt (0.3 s), on 2/6, edges 1, warmup 0, family observer, 2 blocks, 2x1x1"),
        ("comparator-subtract", "cal-comparator-subtract period 4 gt (0.2 s), on 2/4, edges 1, warmup 2, family comparator+redstone_wire, 9 blocks, 3x2x2"),
        ("torch-repeater", "cal-torch-repeater period 20 gt (1 s), on 10/20, edges 1, warmup 0, family redstone_torch+redstone_wire+repeater, 14 blocks, 3x2x3"),
        ("bulb-observer", "cal-bulb-observer period 28 gt (1.4 s), on 14/28, edges 1, warmup 0, family copper_bulb+observer+redstone_wire+repeater, 13 blocks, 3x2x3"),
        ("observer-piston", "cal-observer-piston period 8 gt (0.4 s), on 4/8, edges 2, warmup 3, family observer+redstone_wire+sticky_piston, 8 blocks, 3x4x1"),
        ("sculk-piston", "cal-sculk-piston period 49 gt (2.45 s), on 30/49, edges 1, warmup 5, family repeater+sculk_sensor+sticky_piston, 6 blocks, 5x2x2"),
        ("hopper-pair", "cal-hopper-pair period 14 gt (0.7 s), on 7/14, edges 1, warmup 3, family comparator+hopper, 4 blocks, 3x2x1"),
        ("burnout-torch", "cal-burnout-torch period 220 gt (11 s), on 32/220, edges 8, warmup 0, family redstone_torch+redstone_wire+repeater, 10 blocks, 3x2x2"),
    ];
    let mut s = args("clock sim -q");
    s.extend(cal.iter().map(|(f, _)| format!("{CAL}/{f}.txt")));
    let n = cal.len();
    v.push(Check {
        name: "clock:sim-calibration".into(),
        what: "`clock sim` (exact redsim period) on the 8 calibration clocks: their known period, warmup, output waveform, family, blocks and box",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            for (f, want) in &cal {
                if !o[0].lines().any(|l| l.starts_with("CLOCK: ") && l.contains(want)) {
                    return Err(format!("{f}: expected `{want}`: {}", tail(&o[0])));
                }
            }
            Ok(format!("{n} calibration clocks: known periods"))
        }),
    });
    const NOKICK: &str = "designs/clocks/selftest/observer-pair-no-kick.txt";
    v.push(Check {
        name: "clock:sim-negative".into(),
        what: "`clock sim` says STOPS for the observer pair placed silently (no start kick: nothing ever fires)",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("clock sim -q {NOKICK}"))],
        timeout: 60,
        pred: Box::new(|o| has(&o[0], "STOPS: ").map(|_| "STOPS (not a clock)".into())),
    });
    v.push(Check {
        name: "clock:verify-calibration".into(),
        what: "`clock verify` (the real game, 2 positions x 4 rotations) on the observer pair and the subtract comparator: robust, P6 and P4, and redsim agrees",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("clock verify {CAL}/observer-pair.txt {CAL}/comparator-subtract.txt --positions 2 --horizon 1000 --workers 8"))],
        timeout: 600,
        pred: Box::new(|o| {
            for want in ["CLOCK: cal-observer-pair period 6 gt (0.3 s)", "CLOCK: cal-comparator-subtract period 4 gt (0.2 s)"] {
                let l = o[0].lines().find(|l| l.starts_with(want)).ok_or(format!("no `{want}`: {}", tail(&o[0])))?;
                if !l.ends_with("robust") {
                    return Err(format!("not robust: {l}"));
                }
            }
            if o[0].matches("redsim AGREES").count() != 2 {
                return Err(format!("redsim does not agree on both: {}", tail(&o[0])));
            }
            Ok("robust P6 + P4, redsim agrees".into())
        }),
    });
    v.push(Check {
        name: "clock:verify-negative".into(),
        what: "`clock verify` rejects the observer pair placed silently (no start kick)",
        quick: false,
        jvm: true,
        steps: vec![args(&format!("?clock verify {NOKICK} --positions 1 --no-rotations --no-sim --workers 1"))],
        timeout: 600,
        pred: Box::new(|o| if o[0].lines().any(|l| l.starts_with("CLOCK:")) { Err("accepted a non-clock".into()) } else { Ok("rejected".into()) }),
    });
    v
}

/// Run one command with a timeout; stdout + stderr. A first argument starting with `?` marks a
/// command whose exit status is part of its answer (e.g. `door ideal` on a BROKEN plan exits 1).
fn run_step(exe: &Path, argv: &[String], tmp: &Path, k: usize, timeout: Duration) -> std::result::Result<String, String> {
    let any_exit = argv.first().is_some_and(|a| a.starts_with('?'));
    let mut argv: Vec<String> = argv.to_vec();
    if any_exit {
        argv[0] = argv[0][1..].to_string();
    }
    let out = tmp.join(format!("step{k}.log"));
    let f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe).args(&argv).stdout(Stdio::from(f)).stderr(Stdio::from(f2)).spawn().map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if t0.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timeout after {} s: mctas {}", timeout.as_secs(), argv.join(" ")));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    if !status.success() && !any_exit {
        return Err(format!("exit {:?}: mctas {}: {}", status.code(), argv.join(" ").chars().take(120).collect::<String>(), tail(&text)));
    }
    Ok(text)
}

pub fn run(o: &Opts) -> Result<()> {
    if !Path::new("designs/piston-door-3x3").is_dir() {
        bail!("run `door selftest` from the repository root (designs/piston-door-3x3 not found)");
    }
    let all = checks();
    let sel: Vec<&Check> = all.iter().filter(|c| (!o.quick || c.quick) && o.only.as_ref().is_none_or(|s| s.split(',').any(|p| c.name.contains(p)))).collect();
    if o.list {
        for c in &all {
            println!("{:<34} {} {}", c.name, if c.quick { "quick" } else { "full " }, c.what);
        }
        return Ok(());
    }
    let exe = std::env::current_exe().context("current exe")?;
    let root: PathBuf = std::env::temp_dir().join(format!("mctas-selftest-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    println!("door selftest{}: {} checks, {} at a time ({} JVM), scratch {}", if o.quick { " --quick" } else { "" }, sel.len(), o.jobs, o.jvm_jobs, root.display());
    let t0 = Instant::now();
    let next = AtomicUsize::new(0);
    let jvm_slots = Arc::new((Mutex::new(o.jvm_jobs.max(1)), Condvar::new()));
    let results: Mutex<Vec<(usize, bool, String)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs.max(1) {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(c) = sel.get(k) else { break };
                if c.jvm {
                    let (m, cv) = &*jvm_slots;
                    let mut n = m.lock().unwrap();
                    while *n == 0 {
                        n = cv.wait(n).unwrap();
                    }
                    *n -= 1;
                }
                let tmp = root.join(c.name.replace(':', "_"));
                let _ = std::fs::create_dir_all(&tmp);
                let tc = Instant::now();
                let mut outs = Vec::new();
                let mut err = None;
                for (i, st) in c.steps.iter().enumerate() {
                    let argv: Vec<String> = st.iter().map(|a| a.replace("{tmp}", &tmp.display().to_string())).collect();
                    match run_step(&exe, &argv, &tmp, i, Duration::from_secs(c.timeout)) {
                        Ok(t) => outs.push(t),
                        Err(e) => {
                            err = Some(e);
                            break;
                        }
                    }
                }
                if c.jvm {
                    let (m, cv) = &*jvm_slots;
                    *m.lock().unwrap() += 1;
                    cv.notify_one();
                }
                let (ok, msg) = match err {
                    Some(e) => (false, e),
                    None => match (c.pred)(&outs) {
                        Ok(m) => (true, m),
                        Err(m) => (false, m),
                    },
                };
                println!("{} {:<34} {:>6.1} s  {}", if ok { "PASS" } else { "FAIL" }, c.name, tc.elapsed().as_secs_f64(), msg);
                results.lock().unwrap().push((k, ok, msg));
            });
        }
    });
    let res = results.into_inner().unwrap();
    let failed: Vec<&str> = res.iter().filter(|r| !r.1).map(|r| sel[r.0].name.as_str()).collect();
    println!("selftest: {} passed, {} failed in {:.0} s", res.len() - failed.len(), failed.len(), t0.elapsed().as_secs_f64());
    if !o.keep && failed.is_empty() {
        let _ = std::fs::remove_dir_all(&root);
    }
    if !failed.is_empty() {
        bail!("selftest FAILED: {} (outputs in {})", failed.join(", "), root.display());
    }
    Ok(())
}
