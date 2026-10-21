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
    // 6a. the first sub-56 mechanism (7x7 = 49b, c77 w1): ideal inputs must keep working
    const W1: &str = "designs/piston-door-3x3/tracks/c77/seven/w1.ideal";
    // 6a'. door shuttle re-finds mm01z's shuttled observer (clk-rt): P (boxed, no free neighbour)
    v.push(Check {
        name: "shuttle:mm01z".into(),
        what: "`door shuttle` on c70b's 7x7 mm01 with P's inputs as targets re-finds mm01z's shuttle (S (-2,-2) east carrying an observer at (-1,-2) facing down into (0,-2)) and only it, WORKS at 32 positions",
        quick: true,
        jvm: false,
        steps: vec![args("door shuttle designs/piston-door-3x3/selftest/shuttle-mm01.ideal --target 0,-1,1;1,-1,1")],
        timeout: 300,
        pred: Box::new(|o| {
            let line = o[0].lines().find(|l| l.contains("shuttles WORK")).ok_or(format!("no summary: {}", tail(&o[0])))?;
            if !line.starts_with("1 shuttles WORK") {
                return Err(format!("expected exactly 1 shuttle: {line}"));
            }
            has(&o[0], "S [-2, -2, 0] facing east, O [-1, -2, 0] facing down").map(|_| "1 shuttle: S (-2,-2) east + observer (-1,-2) down".into())
        }),
    });
    v.push(Check {
        name: "ideal:7x7-w1".into(),
        what: "c77's 7x7 (49b) mechanism w1 (13 sticky pistons: the center emptied by a pull after a column wiggle, ring pistons escaping to the corners) WORKS with ideal inputs (--between-ticks) in 55 / 43",
        quick: true,
        jvm: false,
        steps: vec![args(&format!("?door ideal-rs {W1} --between-ticks"))],
        timeout: 300,
        pred: Box::new(|o| {
            let v = ideal_verdict(&o[0]).ok_or(format!("ideal-rs: no verdict: {}", tail(&o[0])))?;
            if !v.starts_with("WORKS") || !v.contains("open after 55 ticks / closed after 43 ticks") {
                return Err(format!("w1 changed: `{v}` (expected WORKS 55 / 43)"));
            }
            Ok("WORKS 55 / 43".into())
        }),
    });
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
    // 6c. the first sub-56 mechanisms in 6-tall boxes (w86): ring pistons resting in frame cells, a
    // carried ring puller, wiggles, QC-coupled stacks avoided; redsim must keep opening and closing them
    for (plan, label) in [("designs/piston-door-3x3/selftest/w86-a3s-7x6.ideal", "a3s-7x6"), ("designs/piston-door-3x3/selftest/w86-a3r-8x6.ideal", "a3r-8x6")] {
        v.push(Check {
            name: format!("ideal:w86-{label}"),
            what: "w86's 6-tall mechanisms (7x6 = 42b a3s, 8x6 = 48b a3r: ideal inputs, FULL frame 12/12, the game agrees): `door ideal-rs` WORKS at 4 positions x 2 cycles",
            quick: true,
            jvm: false,
            steps: vec![args(&format!("?door ideal-rs {plan}"))],
            timeout: 120,
            pred: Box::new(|o| {
                let v = ideal_verdict(&o[0]).ok_or(format!("no verdict: {}", tail(&o[0])))?;
                if v.starts_with("WORKS") { Ok(v.chars().take(100).collect()) } else { Err(v) }
            }),
        });
    }
    // 7. generator membership (docs/audit.md acceptance list)
    let expect: Vec<(&str, [&str; 6])> = vec![
        ("c79-closed", ["out", "IN", "IN", "IN", "IN", "IN"]),
        ("fast-clean-a1c-107b", ["out", "out", "out", "out", "out", "out"]),
        ("eligible-s4ff-49b", ["out", "out", "out", "out", "out", "out"]),
    ];
    v.push(Check {
        name: "audit:generator-membership".into(),
        what: "docs/audit.md's acceptance list: c79 IN every default generator (g2+: extended-at-rest pullers are the default since 2026-10-19), out only of g1 = --no-ext-pullers; fast-clean and eligible-s4ff outside every generator (update here when a generator fix admits them)",
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
    // 7b. ideal-input mechanisms of the smaller-door campaign (piece lists): c77's 7x7 w1 is in
    // the every-flag generators (g6, g7 = + --ring-escape); w86's 7x6 a3s is outside every one
    // (BM leaves through the center column: a 3-move path). Update when a generator admits it.
    v.push(Check {
        name: "audit:member-ideal".into(),
        what: "door audit --member smaller/ideal-mechs.txt: 7x7 w1 IN g6 and g7 only, 7x6 a3s out of g1..g7",
        quick: true,
        jvm: false,
        steps: vec![args("door audit --member designs/piston-door-3x3/tracks/c70b/smaller/ideal-mechs.txt")],
        timeout: 300,
        pred: Box::new(|o| {
            let row = |tag: &str| -> std::result::Result<Vec<String>, String> {
                let l = o[0].lines().find(|l| l.starts_with(&format!("| {tag} "))).ok_or(format!("no {tag} row"))?;
                Ok(l.split('|').map(|s| s.trim().to_string()).skip(2).take(7).collect())
            };
            let w1 = row("7x7 (-2,-2)")?;
            let a3s = row("7x6 (-2,-1)")?;
            let want_w1 = ["out", "out", "out", "out", "out", "IN", "IN"];
            if w1 != want_w1 {
                return Err(format!("w1 g1..g7 = {w1:?}, expected {want_w1:?}"));
            }
            if a3s.iter().any(|c| c != "out") {
                return Err(format!("a3s g1..g7 = {a3s:?}, expected all out"));
            }
            Ok("w1 IN g6/g7, a3s outside every generator".into())
        }),
    });
    // 8. the clock judge (docs/clocks.md): known periods on redsim, the real game agreeing, a
    // non-clock rejected
    const CAL: &str = "designs/clocks/calibration";
    let cal: Vec<(&str, &str)> = vec![
        // (periods print with their real time since 2026-10-19, vjeux: "20 gt (1 s)"; outputs are
        // exterior air cells read by the power the judge's lamp there receives since 2026-10-20:
        // bulb-observer and burnout-torch have no such cell that pulses evenly and left the list)
        ("observer-pair", "cal-observer-pair period 6 gt (0.3 s), on 2/6, edges 1, warmup 2, family observer, 2 blocks, 2x1x1"),
        ("comparator-subtract", "cal-comparator-subtract period 4 gt (0.2 s), on 2/4, edges 1, warmup 2, family comparator+redstone_wire, 9 blocks, 3x2x2"),
        ("torch-repeater", "cal-torch-repeater period 20 gt (1 s), on 10/20, edges 1, warmup 0, family redstone_torch+redstone_wire+repeater, 14 blocks, 3x2x3"),
        ("observer-piston", "cal-observer-piston period 8 gt (0.4 s), on 4/8, edges 2, warmup 3, family observer+redstone_wire+sticky_piston, 8 blocks, 3x4x1"),
        ("sculk-piston", "cal-sculk-piston period 49 gt (2.45 s), on 30/49, edges 1, warmup 5, family repeater+sculk_sensor+sticky_piston, 6 blocks, 5x2x2"),
        ("hopper-pair", "cal-hopper-pair period 14 gt (0.7 s), on 7/14, edges 1, warmup 7, family comparator+hopper, 4 blocks, 3x2x1"),
    ];
    let mut s = args("clock sim -q");
    s.extend(cal.iter().map(|(f, _)| format!("{CAL}/{f}.txt")));
    let n = cal.len();
    v.push(Check {
        name: "clock:sim-calibration".into(),
        what: "`clock sim` (exact redsim period) on 6 calibration clocks (exterior air outputs): their known period, warmup, output waveform, family, blocks and box",
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
        name: "clock:item-despawn".into(),
        what: "`clock sim` says NO-REPEAT for a leaf clock fed by an item a player could drop: the item despawns at age 6000 and the judge runs item designs 12000 gt (a clock must run forever)",
        quick: true,
        jvm: false,
        steps: vec![args("clock sim -q designs/clocks/selftest/item-despawn.txt")],
        timeout: 120,
        pred: Box::new(|o| has(&o[0], "NO-REPEAT: ").map(|_| "NO-REPEAT (the item despawns)".into())),
    });
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
    v.push(Check {
        name: "clock:verify-reload".into(),
        what: "`clock verify --reload` (the chunks saved, unloaded and loaded again mid-run): the torch-repeater clock survives it; the observer pair does not in every rotation (it comes back at P4)",
        quick: false,
        jvm: true,
        steps: vec![
            args(&format!("clock verify {CAL}/torch-repeater.txt --positions 1 --workers 4 --reload --no-sim")),
            args(&format!("?clock verify {CAL}/observer-pair.txt --positions 2 --horizon 1000 --workers 8 --reload --no-sim")),
        ],
        timeout: 900,
        pred: Box::new(|o| {
            let l = o[0].lines().find(|l| l.starts_with("CLOCK: cal-torch-repeater")).ok_or(format!("torch-repeater: {}", tail(&o[0])))?;
            if !l.ends_with("robust") {
                return Err(format!("torch-repeater not robust across a reload: {l}"));
            }
            if o[1].lines().any(|l| l.starts_with("CLOCK: cal-observer-pair") && l.ends_with("robust")) {
                return Err(format!("the observer pair passed the reload in every rotation: {}", tail(&o[1])));
            }
            Ok("torch-repeater survives a reload, the observer pair does not".into())
        }),
    });
    v.push(Check {
        name: "clock:verify-entities".into(),
        what: "`clock verify` with design entities: the observer pair beside a minecart on a rail is a robust clock (the cart summoned, turned and still there at the end); with the cart in the air (it falls away) it is rejected",
        quick: false,
        jvm: true,
        steps: vec![
            args("?clock verify designs/clocks/selftest/observer-pair-cart.txt --positions 1 --no-sim --workers 4"),
            args("?clock verify designs/clocks/selftest/observer-pair-falling-cart.txt --positions 1 --no-rotations --no-sim --workers 1"),
        ],
        timeout: 600,
        pred: Box::new(|o| {
            if !o[0].lines().any(|l| l.starts_with("CLOCK: entity-test-decor period 6 gt") && l.ends_with("robust")) {
                return Err(format!("the cart design is not a robust P6 clock: {}", o[0].lines().next().unwrap_or("")));
            }
            if o[1].lines().any(|l| l.starts_with("CLOCK:")) || !o[1].contains("entities at the end") {
                return Err(format!("the falling cart was not rejected for its entity: {}", o[1].lines().next().unwrap_or("")));
            }
            Ok("cart kept: robust P6; cart lost: rejected".into())
        }),
    });
    // 9. the circuit judge (docs/circuits.md): known metrics on redsim and in the real game, a
    // false claim refused. R7 (real input devices, 2026-10-20): a plain input is the judge's stone
    // (or the design's own receiving block) powered by the judge's repeater from outside the
    // build, so no pulse or gap is shorter than 2 gt ("2 gt (the shortest a real input makes)"),
    // and piston and TNT events come 1 gt earlier (the input arrives during a tick, not between
    // ticks): sticky-extender 3 -> 2 gt, tnt-duper-triggered latency 1 -> 0 gt
    const CIRC: &str = "designs/circuits/calibration";
    let circ: Vec<(&str, &str)> = vec![
        ("subtract-rising-edge", "cal-subtract-rising-edge rising-edge L 4 gt, delay 2 gt, min-pulse 4 gt, 12 blocks, 2x2x3"),
        ("bulb-tff", "cal-bulb-tff tff latency 2 gt, edge rising, min-pulse 2 gt (the shortest a real input makes), min-period 4 gt (the shortest a real input makes), 3 blocks, 2x2x1"),
        ("repeater-delay", "cal-repeater-delay delay N 4 gt, min-pulse 4 gt, min-gap 4 gt, 2 blocks, 1x2x1"),
        ("torch-not", "cal-torch-not gate not function not, latency 2 gt, 4 blocks, 3x2x1"),
        ("dust-or", "cal-dust-or gate or function or, latency 0 gt, class instant, 2 blocks, 1x2x1"),
        ("repeater-dlatch", "cal-repeater-dlatch d-latch latency 4 gt, enable low, 4 blocks, 1x2x2"),
        // 2026-10-21 (clk-rt, site QA): the counter judge adds `counts`; this toggling design gives
        // one pulse per 4 inputs (a square wave), so its axis is no longer plain `counter 2`
        ("bulb-divider", "cal-bulb-divider counter 2 N 2, output toggles every 2 pulses, counts one pulse per 4 inputs (toggles every 2: a square wave, a divider by 4), 6 blocks, 4x2x1"),
        ("sticky-extender", "cal-sticky-extender piston-extender 1 N 1, extend 2 gt, retract 2 gt, 2 blocks, 3x1x1"),
        // 2026-10-21 (track fly, clk-misc's real-use audit): a horizontal flyer's ground clearance
        // (circuit::robust::fly_clearance): fly-d1 has slime in its bottom layer (it would drag
        // the grass), the two-way machine an observer watching straight down (grass under it turns
        // to dirt on a random tick): both need a block of air under them
        ("flying-machine", "cal-flying-machine flying-machine speed 10 gt/block, period 10 gt, 1 cell(s), latency 2 gt, ground needs 1 block of air above the ground, 7 blocks, 4x2x1"),
        ("two-way-machine", "cal-two-way-machine flying-machine two-way period 98 gt over 6 positions, latency 2 gt, ground needs 1 block of air above the ground, 14 blocks, 2x2x9"),
        // 2026-10-21 (track fly, real use): a one-way flyer started by a real lever, judged over
        // the audit's flip gaps (check::START_FLIPS: off 4..20 gt after the launch, on again ...)
        ("fly-start", "cal-fly-start flying-machine speed 10 gt/block, period 10 gt, 1 cell(s), latency 2 gt, start flip the lever, ground flies along the ground, 7 blocks, 3x4x1"),
        ("repeater-line", "cal-repeater-line-n0 delay N 2 gt, min-pulse 2 gt (the shortest a real input makes), min-gap 2 gt (the shortest a real input makes), 2 blocks, 1x2x1"),
        ("dropper-tff-reset", "cal-dropper-tff-reset tff-reset latency 13 gt, reset 6 gt, min-pulse 2 gt (the shortest a real input makes), 8 blocks, 3x3x2"),
        ("observer-bud", "cal-observer-bud bud detects place remove state shape, latency 2 gt, pulse 2 gt, re-arm 4 gt, footprint 1 cell, 1 blocks, 1x1x1"),
        ("slime-bud", "cal-slime-bud bud detects place remove state update, latency 3 gt, pulse 1 gt, re-arm 6 gt, jams update pair 2 gt apart, footprint 5 cells, 3 blocks, 1x4x1"),
        ("latched-bud", "cal-latched-bud bud detects place remove state update, latency 3 gt, reset manual, off 1 gt after it, re-arm 2 gt, footprint 5 cells, 2 blocks, 3x1x1"),
        ("observer-stone-bud", "cal-observer-stone-bud bud detects place remove state shape, latency 2 gt, pulse 2 gt, re-arm 4 gt, footprint 1 cell, tileable every 1 (0 0 1), 2 blocks, 2x1x1"),
        ("sculk-filter-rise", "cal-sculk-filter-rise rising-edge L 10 gt, delay 1 gt, min-pulse 2 gt (the shortest a real input makes), vibration-proof yes, 5 blocks, 2x2x4"),
        ("bulb-counter2", "cal-bulb-counter2 binary-counter 2 latency 6 gt, edge rising, per bit 4.0 blocks, 8 blocks, 5x2x2"),
        ("bulb-counter-slice", "cal-bulb-counter-slice binary-counter 1 latency 2 gt, edge falling, per bit 5.0 blocks, slice 5 blocks per bit, latency 2 + 4 (N-1) gt, 5 blocks, 3x2x2"),
        ("hex-gt", "cal-hex-gt comparator 4 hex latency 4 gt (b goes 0 -> 8 from a=8 b=0), per bit 1.0 blocks, 4 blocks, 1x2x2"),
        ("tnt-duper", "cal-tnt-duper tnt-duper rate 200 TNT/min, period 6 gt, per period 1, latency 1 gt, 15 blocks, 3x3x5"),
        ("tnt-duper-triggered", "cal-tnt-duper-triggered tnt-duper-triggered per pulse 1, latency 0 gt, gap 20 gt, 13 blocks, 3x2x4"),
        ("bulb-burst2", "cal-bulb-burst2 burst 2 N 2, pulse 2 gt, period 6 gt, latency 12 gt, span 20 gt, min-pulse 2 gt (the shortest a real input makes), retrigger mixed, repeat 12 gt, 7 blocks, 5x2x1"),
        ("bulb-divider4", "cal-bulb-divider4 divider 4 N 4, duty 50%, output square, latency 4 gt, min-period 4 gt (the shortest a real input makes), 6 blocks, 4x2x1"),
        ("bulb-monostable", "cal-bulb-monostable monostable L 8 gt, delay 12 gt, retrigger non-retriggerable, re-arm 16 gt, min-pulse 2 gt (the shortest a real input makes), 6 blocks, 4x2x1"),
        ("tap-off-delay", "cal-tap-off-delay off-delay L 4 gt, latency 2 gt, min-pulse 2 gt (the shortest a real input makes), 26 blocks, 5x2x3"),
        ("subtract-on-delay", "cal-subtract-on-delay on-delay T 8 gt, shortening 4 gt, latency 6 gt, 16 blocks, 3x3x3"),
        ("torch-pulse-train", "cal-torch-pulse-train pulse-train P 12 gt, pulse 6 gt, latency 6 gt, stop 4 gt, 15 blocks, 2x3x6"),
        // R7: adjacent inputs driven from opposite sides, one through the design's own repeater
        ("d-latch-adjacent", "cal-d-latch-adjacent d-latch latency 6 gt, enable high, tileable every 2 (0 2 0), 7 blocks, 2x2x3"),
        // study 11's stimulus scaled with a long timer: inputs up to 3x the sizing gap, honest
        // min-pulse (an item timer reloads one item per 8 gt), 4 gt trains not judged past it
        ("item-timer-off-delay", "cal-item-timer-off-delay off-delay L 310 gt, latency 4 gt, min-pulse 320 gt, 6 blocks, 2x2x3"),
    ];
    // R7 refusals: an observer watching the input cell (a real input changes no block there), the
    // window flip-flops whose clock feeds a comparator's side straight from the input cell (a
    // diode's side reads only a signal source: dust, a diode, a redstone block, not the judge's
    // powered stone), and the
    // dust encoder whose d3 sits between its two output lines (every way in puts the judge's
    // switch against a line or an output cell)
    let r7: [&str; 5] = ["observer-dual-edge", "window-dff", "window-dff-enable", "window-shift1", "dust-encoder2"];
    let mut s = args("circuit sim");
    s.extend(circ.iter().map(|(f, _)| format!("{CIRC}/{f}.txt")));
    s.extend(r7.iter().map(|f| format!("{CIRC}/{f}.txt")));
    s.push("designs/circuits/selftest/dust-or-claimed-and.txt".into());
    s.push("designs/circuits/selftest/floating-not.txt".into());
    s.push("designs/circuits/selftest/shortening-delay-line.txt".into());
    s.push("designs/circuits/selftest/plus4-stretcher.txt".into());
    // a circuit must drive something (the judge's lamp in each air output cell): a redirection-only
    // output refused, a comparator AND kept with its metrics
    s.push("designs/circuits/selftest/update-less-nor.txt".into());
    // a design file left with merge markers is not a design: parsing its HEAD half silently once
    // put a broken file on main (clk-item 2026-10-20)
    s.push("designs/circuits/selftest/merge-markers.txt".into());
    s.push("designs/circuits/selftest/comparator-and.txt".into());
    s.push("designs/circuits/selftest/stone-output-nimply.txt".into());
    // the sequential memory kinds: a claim each refused (a reset that drives nothing, a level
    // latch as an edge-triggered register, an enable that drives nothing, one cell as two)
    for f in ["bulb-tff-claimed-reset", "dlatch-claimed-dff", "dff-claimed-enable", "dff-claimed-shift2"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the BUD kind: a clock, a BUD that never re-arms, a false claim, a pulse BUD given a reset
    // input (not latched) and an observer BUD claiming to tile side by side (cross-talk) refused
    for f in ["observer-pair-bud", "latching-bud", "observer-bud-claims-update", "observer-bud-claims-latched", "observer-bud-tile1"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the vibration test: a sculk design that hears its own lever, a false `vibration: proof`
    for f in ["sculk-hears-lever", "sculk-claims-proof"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // study 12: a plain encoder claiming to be a priority encoder refused (two inputs at once)
    s.push("designs/circuits/selftest/dust-encoder2-claims-priority.txt".into());
    // study 11: a monostable whose second edge cuts its pulse, an OR off-delay that does not
    // restart, a false retriggerable claim refused; a tiled off-delay kept (`tile 5 blocks x3`)
    // and its crosstalking 4-block tiling refused
    for f in ["oneshot-retriggered", "repeater-or-off-delay", "bulb-claims-retriggerable", "tile-off-delay-ring", "tile-off-delay-crosstalk"] {
        s.push(format!("designs/circuits/selftest/{f}.txt"));
    }
    // the TNT judge's usefulness rule: a duper whose TNT explodes in its own water refused
    s.push("designs/circuits/selftest/tnt-water-duper.txt".into());
    // the TNT judge's PRECISE check over kicks: an air burst precise for 99.5% of kick angles loses
    // the flag (a TNT in a sliver of angles stops short at its x-stop and goes off elsewhere)
    s.push("designs/circuits/selftest/tnt-airburst-sliver.txt".into());
    // the TNT judge's turned builds: a fast duper that primes nothing turned once is rotation-dependent
    s.push("designs/circuits/selftest/tnt-fast-rotation.txt".into());
    // the start judge: a lever watched by an observer that stays on the parked part (a flip 4 gt
    // after the launch fires the machine again mid-cycle) refused
    s.push("designs/circuits/selftest/fly-start-watcher-stays.txt".into());
    // the docking judge: a machine that makes one trip per switch-on and parks with the switch on
    s.push("designs/circuits/selftest/fly-docking-parks.txt".into());
    let circ2 = circ.clone();
    let n = circ.len();
    v.push(Check {
        name: "circuit:sim-calibration".into(),
        what: "`circuit sim` (redsim) on the 31 calibration circuits (edge detectors, T flip-flop, delay lines, NOT, OR, D latch, divider, piston extender, flying machine, a lever-started flying machine, two-way flying machine with return stations, T flip-flop with reset, an observer BUD and the slime-block QC BUD, a latched QC BUD, a tileable observer BUD, self-running and triggered TNT dupers, study 11's burst, /4 divider, monostable, off-delay, on-delay and gated pulse train, a vibration-proof sculk rising edge, study 12's 2-bit bulb ripple counter and its bit slice stacked to 4 bits, the 2-comparator hex a > b on chest strengths), inputs driven by real devices (R7): their known metrics; the OR gate claiming AND, a floating dust, a shortening delay line claiming pulse-limiter, a +4 gt stretcher claiming pulse-extender, an update-less (redirection-only) NOR, a stone output, four false memory claims, five false BUDs (a clock, a BUD that never re-arms, an observer claiming pure updates, an observer BUD with a reset input, an observer BUD claiming to tile side by side), three false study-11 timers (a one-shot cut by a second edge, an OR off-delay that does not restart, a false retriggerable claim), two sculk designs failing the vibration test (one hears its own lever, one falsely says vibration-proof) and a plain encoder claiming priority refused; five R7 refusals (an observer watching its input cell, three window flip-flops feeding a comparator's side from the input cell, study 12's dust encoder whose d3 input has no way in); a comparator AND kept (its b input through the design's own repeater, R7 model B); study 11's `tile:` judged copy by copy: an off-delay ring tiled 5 blocks apart kept (`tileable every 5`), 4 apart refused (copy 0's lamp reads copy 1's ring)",
        quick: true,
        jvm: false,
        steps: vec![s],
        timeout: 120,
        pred: Box::new(move |o| {
            for (f, want) in &circ {
                if !o[0].lines().any(|l| l.starts_with("CIRCUIT: ") && l.contains(want) && l.ends_with("redsim")) {
                    return Err(format!("{f}: expected `{want}`: {}", tail(&o[0])));
                }
            }
            has(&o[0], "FAIL: cal-dust-or-claimed-and gate and: claims and but behaves as or")?;
            has(&o[0], "FAIL: cal-floating-not gate not: redstone_wire")?;
            has(&o[0], "FAIL: cal-shortening-delay-line pulse-limiter: limiting not witnessed")?;
            has(&o[0], "REFUSED: designs/circuits/selftest/merge-markers.txt: parsing designs/circuits/selftest/merge-markers.txt: line 6: \"<<<<<<< HEAD\": unresolved merge markers, not a design")?;
            has(&o[0], "FAIL: cal-plus4-stretcher pulse-extender: never extends")?;
            // R7: the update-less NOR's dust turned toward a redstone block written into an input
            // cell; the judge's stone there is no source to turn toward, so it never switches
            has(&o[0], "FAIL: cal-update-less-nor gate nor: not a gate: the output is true (truth table 1111)")?;
            has(&o[0], "FAIL: cal-stone-output-nimply gate nimply: output out at [2, 1, 1] holds stone, which drives nothing by itself")?;
            has(&o[0], "CIRCUIT: cal-comparator-and gate and function and, latency 6 gt, glitches 2, 7 blocks, 3x2x2, redsim")?;
            has(&o[0], "FAIL: cal-observer-dual-edge dual-edge: the longest input pulse (40 gt) gives 0 output pulse(s)")?;
            has(&o[0], "FAIL: cal-window-dff dff: rising edge: M 1 clk rises: q = 0, want 1")?;
            has(&o[0], "FAIL: cal-window-dff-enable dff-enable: rising edge: M 1 clk rises: q = 0, want 1")?;
            has(&o[0], "FAIL: cal-window-shift1 shift 1: rising edge: M 1 clk rises: q = 0, want 1")?;
            has(&o[0], "FAIL: cal-bulb-tff-claimed-reset tff-reset: A r 4: an r pulse must clear q")?;
            has(&o[0], "FAIL: cal-dlatch-claimed-dff dff: rising edge: M 1 d=1 clk=0: q = 1, want 0")?;
            has(&o[0], "FAIL: cal-dff-claimed-enable dff-enable: rising edge:")?;
            has(&o[0], "FAIL: cal-dff-claimed-shift2 shift 2: rising edge:")?;
            has(&o[0], "FAIL: cal-observer-pair-bud bud: the output is on at rest")?;
            has(&o[0], "FAIL: cal-latching-bud bud: the output is on at rest")?;
            has(&o[0], "FAIL: cal-observer-bud-claims-update bud: claims update but does not detect it")?;
            has(&o[0], "FAIL: cal-observer-bud-claims-latched bud: not latched: after a place the output goes off")?;
            has(&o[0], "FAIL: cal-observer-bud-tile1 bud: not tileable (every 1 (0 0 1)): copy 0: cross-talk")?;
            has(&o[0], "FAIL: cal-sculk-hears-lever rising-edge: hears its input device")?;
            has(&o[0], "FAIL: cal-sculk-claims-proof falling-edge: not vibration-proof (it says `vibration: proof`)")?;
            has(&o[0], "FAIL: cal-dust-encoder2-claims-priority encoder 2 priority: input d3 at [2, 1, 2]: no side of the input cell leads out of the build")?;
            has(&o[0], "FAIL: cal-dust-encoder2 encoder 2: input d3 at [2, 1, 2]: no side of the input cell leads out of the build")?;
            has(&o[0], "FAIL: cal-oneshot-retriggered monostable: re-trigger behaviour changes with the delay")?;
            has(&o[0], "FAIL: cal-repeater-or-off-delay off-delay: min-pulse 8 gt")?;
            has(&o[0], "FAIL: cal-bulb-claims-retriggerable monostable retriggerable: claims retriggerable but behaves as non-retriggerable")?;
            has(&o[0], "CIRCUIT: cal-tile-off-delay-ring off-delay L 24 gt, latency 0 gt, min-pulse 4 gt, tileable every 5 (5 0 0), 13 blocks, 3x2x3, redsim")?;
            has(&o[0], "FAIL: cal-tile-off-delay-crosstalk off-delay: tile 4 0 0 (copies driven 7 gt apart): copy 0 of 3 measures L 33 gt")?;
            has(&o[0], "FAIL: cal-tnt-water-duper tnt-duper: useless TNT: a TNT explodes in a fluid")?;
            has(&o[0], "CIRCUIT: cal-tnt-airburst-sliver tnt-duper rate 22.2 TNT/min")?;
            if o[0].lines().any(|l| l.starts_with("CIRCUIT: cal-tnt-airburst-sliver ") && l.contains("precise:")) {
                return Err("cal-tnt-airburst-sliver: still PRECISE (the check over kicks, circuit::robust::precise_over_kicks, must drop the flag)".into());
            }
            if !o[0].lines().any(|l| l.starts_with("CIRCUIT: cal-tnt-fast-rotation ") && l.contains("rotation-dependent: turned 1 time(s): no primed TNT")) {
                return Err(format!("cal-tnt-fast-rotation: not flagged rotation-dependent (verify::sim checks a TNT duper's turned builds): {}", tail(&o[0])));
            }
            has(&o[0], "FAIL: cal-fly-start-watcher-stays flying-machine: no steady flight after the launch")?;
            has(&o[0], "FAIL: cal-fly-docking-parks flying-machine: run 1: no round trip repeats while the switch is on")?;
            Ok(format!("{n} calibration circuits: known metrics; a false claim, a floating dust, a delay line, a stretcher, an update-less NOR, a stone output, 4 false memory claims, 5 false BUDs, 3 false study-11 timers, 2 sculk designs failing the vibration test, a false priority encoder and 5 R7 input refusals; the comparator AND kept; a tiled off-delay kept, its crosstalking tiling refused; a TNT duper exploding in water refused; an air burst imprecise in slivers of kick angles not flagged precise; a fast duper that primes nothing turned flagged rotation-dependent"))
        }),
    });
    v.push(Check {
        name: "circuit:verify-calibration".into(),
        what: "`circuit verify` (the real game, 2 positions x 4 rotations) on the 31 calibration circuits, inputs driven by real devices (R7) (BUDs included: the judge's shape-only and pure-update actions in the game; TNT dupers included: every primed TNT recorded then removed; study 11's timers on their long input trains; the sculk rising edge with its lever's and ambient game events; study 12's encoder, counter, bit slice and hex comparator): robust, the same metrics as redsim; the update-less NOR refused, the comparator AND robust; study 11's tiled off-delay (3 copies 5 apart, each against its own input) robust",
        quick: false,
        jvm: true,
        steps: vec![{
            let mut s = args("?circuit verify --positions 2 --workers 8");
            s.extend(circ2.iter().map(|(f, _)| format!("{CIRC}/{f}.txt")));
            s.push("designs/circuits/selftest/update-less-nor.txt".into());
            s.push("designs/circuits/selftest/comparator-and.txt".into());
            s.push("designs/circuits/selftest/dlatch-claimed-dff.txt".into());
            // a copper-bulb toggle given a reset input is no latch (redsim does not model bulbs)
            s.push("designs/circuits/selftest/bulb-toggle-bud.txt".into());
            // study 11's `tile:`: three copies 5 apart, each judged against its own input
            s.push("designs/circuits/selftest/tile-off-delay-ring.txt".into());
            // the TNT judge's usefulness rule in the game: TNT exploding in the duper's own water
            s.push("designs/circuits/selftest/tnt-water-duper.txt".into());
            // the start judge in the game: the watcher left on the parked part refused
            s.push("designs/circuits/selftest/fly-start-watcher-stays.txt".into());
            s
        }],
        timeout: 600,
        pred: Box::new(move |o| {
            for (f, want) in &circ2 {
                if !o[0].lines().any(|l| l.starts_with("CIRCUIT: ") && l.contains(want) && l.ends_with("robust")) {
                    return Err(format!("{f}: expected `{want}`, robust: {}", tail(&o[0])));
                }
            }
            has(&o[0], "FAIL: cal-update-less-nor gate nor: not a gate: the output is true (truth table 1111)")?;
            has(&o[0], "CIRCUIT: cal-comparator-and gate and function and, latency 6 gt, glitches 2, 7 blocks, 3x2x2, robust")?;
            has(&o[0], "FAIL: cal-dlatch-claimed-dff dff: rising edge: M 1 d=1 clk=0: q = 1, want 0")?;
            has(&o[0], "FAIL: cal-bulb-toggle-bud bud: latches")?;
            has(&o[0], "FAIL: cal-tnt-water-duper tnt-duper: useless TNT: a TNT explodes in a fluid")?;
            has(&o[0], "FAIL: cal-fly-start-watcher-stays flying-machine: no steady flight after the launch")?;
            has(&o[0], "CIRCUIT: cal-tile-off-delay-ring off-delay L 24 gt, latency 0 gt, min-pulse 4 gt, tileable every 5 (5 0 0), 13 blocks, 3x2x3, robust")?;
            Ok(format!("{} calibration circuits robust, redsim's metrics; the update-less NOR, the latch claiming dff, a bulb toggle claiming a latched BUD and a start lever whose watcher stays on the parked part refused, the comparator AND robust, a tiled off-delay robust copy by copy", circ2.len()))
        }),
    });
    // 10. study 8: shulker loaders and unloaders, judged in the game by their own runner (judge
    // chests, every item and box counted): track shulker's always-on loader and unloader with
    // unstackables (27 items per box: a fast run) and their badges, and a loader that breaks its
    // boxes early refused
    v.push(Check {
        name: "circuit:verify-shulker".into(),
        what: "`circuit verify` (the real game, 1 position x 4 rotations) on track shulker's always-on loader and unloader with unstackables: 3 boxes, robust, hopper pace (cycle 224 / 220 gt), every badge they claim (always-on, colours, restart, box-starved, drop-free); a loader that breaks its boxes at the first item refused; `circuit sim` says redsim does not model them yet",
        quick: false,
        jvm: true,
        steps: vec![
            args("?circuit verify --positions 1 --workers 4 designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt designs/circuits/selftest/shulker-early-loader.txt"),
            args("circuit sim designs/circuits/shulker/tracks/shulker/auto-loader-1.txt"),
        ],
        timeout: 900,
        pred: Box::new(|o| {
            // the drop's random offset can move a median time by a gt: match around the times
            let line = |name: &str| o[0].lines().find(|l| l.starts_with(&format!("CIRCUIT: {name} "))).unwrap_or("").to_string();
            let l = line("shulker-auto-loader-1");
            if !(l.contains("shulker-loader 1 boxes/min ") && l.contains("items/s 2.") && l.contains("cycle 22") && l.contains("hoppers awake 3, badges always-on+colours+restart+top-up+box-starved+drop-free, bill 16 iron, 4 quartz, 28 redstone, 1 slime, 31 blocks, 5x5x5, robust")) {
                return Err(format!("auto loader: {l:?}: {}", tail(&o[0])));
            }
            let l = line("shulker-auto-unloader-1");
            if !(l.contains("shulker-unloader 1 boxes/min ") && l.contains("items/s 2.4") && l.contains("cycle 22") && l.contains("badges always-on+colours+box-starved+drop-free, bill 16 iron, 5 quartz, 35 redstone, 1 slime, 49 blocks, 5x6x8, robust")) {
                return Err(format!("auto unloader: {l:?}: {}", tail(&o[0])));
            }
            has(&o[0], "FAIL: cal-shulker-early-loader shulker-loader 1: box 1 arrived at t=")?;
            has(&o[1], "UNMODELED: shulker-auto-loader-1: redsim does not judge shulker loaders")?;
            Ok("always-on loader and unloader robust at hopper pace with all their badges; the early breaker refused".into())
        }),
    });
    // the TNT judge's on/off LEVER check (circuit::tnt::check_lever, vjeux 2026-10-21: "Can we have
    // an on off switch for all the tnt"): the clockless engine with its lever passes (off as built,
    // 4 on/off cycles at other moments of the period, back to rest each time); the same engine
    // without its inverter torch runs with the lever off and is refused. Full selftest only: the
    // 200 TNT/min engine's explosions take ~40 s of redsim each
    v.push(Check {
        name: "circuit:tnt-lever".into(),
        what: "`circuit sim` (redsim) on the TNT lever check: tnt-engine-lever-17b passes (200 TNT/min, stops within 2 gt, restarts within 5 gt), the engine without its inverter torch is refused (`lever off at rest: N TNT primed`)",
        quick: false,
        jvm: false,
        steps: vec![args("circuit sim designs/circuits/tracks/clk-misc/tnt-engine-lever-17b.txt designs/circuits/selftest/tnt-lever-nostop.txt")],
        timeout: 300,
        pred: Box::new(|o| {
            if !o[0].lines().any(|l| l.starts_with("CIRCUIT: tnt-engine-lever-17b tnt-duper rate 200 TNT/min, period 6 gt, per period 1, runs lever on, switch lever off: stops within 2 gt; on: restarts within 5 gt")) {
                return Err(format!("tnt-engine-lever-17b: not the known lever verdict: {}", tail(&o[0])));
            }
            has(&o[0], "FAIL: cal-tnt-lever-nostop tnt-duper: lever off at rest:")?;
            Ok("the lever engine kept (stops within 2 gt, restarts within 5 gt); the engine without its inverter refused at rest".into())
        }),
    });
    // redsim's shulker port: the real-game traces of the reference loader and unloader replayed
    // tick for tick (box placement, hoppers, comparators, the piston-broken drop as the bridge
    // seeds it, its pickup)
    v.push(Check {
        name: "circuit:shulker-compare".into(),
        what: "`circuit shulker compare` replays the exact real-game traces of the reference loader and unloader (3 boxes of unstackables each) in redsim: every cell and entity identical every tick",
        quick: true,
        jvm: false,
        steps: vec![args("circuit shulker compare designs/circuits/shulker/traces/exact/shulker-ref-loader-1.trace designs/circuits/shulker/traces/exact/shulker-ref-unloader-1.trace")],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "OK shulker-ref-loader-1: 687 ticks identical")?;
            has(&o[0], "OK shulker-ref-unloader-1: 678 ticks identical")?;
            Ok("reference loader and unloader traces tick-identical in redsim".into())
        }),
    });
    // the judge's full-chest check (a chest full of boxes, as a player fills it): track pext's
    // 28-block loader jams there (its first dispenser fire comes before a box arrives and the
    // chest never empties); track shulker's always-on loader and unloader pass. The endless-supply
    // check (that chest kept full, refilled before every gt): track pext's first 1x6x9 loader jams
    // there (it counts the chest's level changes); the always-on loader and unloader pass
    v.push(Check {
        name: "circuit:shulker-full-chest".into(),
        what: "`circuit shulker rs --only full-chest`, `--only endless` and `--only one-box` (redsim): a chest full of boxes (27), that chest kept full, then boxes coming one at a time; a loader whose box-arrival line counts the chest's on and off jams with 0 of 26 boxes in the first, one that counts its level changes jams in the second, one that reads the chest at all jams in the third, track shulker's always-on loader and unloader deliver all in every one",
        quick: true,
        jvm: false,
        steps: vec![
            args("circuit shulker rs --only full-chest designs/circuits/selftest/shulker-full-chest-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt"),
            args("circuit shulker rs --only endless designs/circuits/selftest/shulker-endless-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt"),
            args("circuit shulker rs --only one-box designs/circuits/selftest/shulker-one-box-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt designs/circuits/shulker/tracks/shulker/auto-unloader-1.txt"),
            args("circuit shulker rs --only one-box-burst designs/circuits/selftest/shulker-one-box-burst-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt"),
            args("circuit shulker rs --only one-box-burst,one-box-burst-23 designs/circuits/selftest/shulker-one-box-pace-jam.txt designs/circuits/shulker/tracks/shulker/auto-loader-1.txt"),
        ],
        timeout: 120,
        pred: Box::new(|o| {
            has(&o[0], "FAIL cal-shulker-full-chest-jam full-chest")?;
            has(&o[0], "0 of 26 boxes")?;
            has(&o[0], "OK shulker-auto-loader-1 full-chest")?;
            has(&o[0], "OK shulker-auto-unloader-1 full-chest")?;
            has(&o[1], "FAIL cal-shulker-endless-jam endless")?;
            has(&o[1], "0 of 26 boxes")?;
            has(&o[1], "OK shulker-auto-loader-1 endless")?;
            has(&o[1], "OK shulker-auto-unloader-1 endless")?;
            has(&o[2], "FAIL cal-shulker-one-box-jam one-box")?;
            has(&o[2], "0 of 3 boxes")?;
            has(&o[2], "OK shulker-auto-loader-1 one-box")?;
            has(&o[2], "OK shulker-auto-unloader-1 one-box")?;
            has(&o[3], "FAIL cal-shulker-one-box-burst-jam one-box-burst")?;
            has(&o[3], "OK shulker-auto-loader-1 one-box-burst")?;
            has(&o[4], "OK cal-shulker-one-box-pace-jam one-box-burst ")?;
            has(&o[4], "FAIL cal-shulker-one-box-pace-jam one-box-burst-23")?;
            has(&o[4], "2 of 3 boxes")?;
            has(&o[4], "OK shulker-auto-loader-1 one-box-burst-23")?;
            Ok("full chest, endless supply, one box at a time, nearly full boxes one at a time (at three paces): the weak loaders jam, the always-on loader and unloader deliver every box".into())
        }),
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
