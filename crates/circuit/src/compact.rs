//! `mctas circuit compact` (track clk-rt, coordinator 2026-10-21: vjeux's standing rule, the
//! MINIMUM VOLUME per world-best): `anneal`'s box-bounded search on ANY circuit kind, judged by the
//! kind's own redsim judge (`verify::sim`). A candidate passes when the judge passes with the seed's
//! kind and the seed's axis value (`archive::axis`: the study's column, `L = 4 gt`, `xor`, `counter
//! 3`, `detects ...`); passing designs rank by the judge's box (every non-air cell over the run,
//! `verify::dims`, what the archive's volume is), then by blocks. Tiers: 0 the judge fails, 1 it
//! passes with another axis value, 2 it passes with the seed's. The palette defaults to the seed's
//! own block states plus stone (`turn` moves give every facing). Block-entity and entity cells stay
//! frozen. `--game N` judges the N smallest designs in the real game (`verify`: robust, the same
//! axis). `--all`: per (kind, axis) of every study's archive, the current minimum-volume robust
//! design is the seed of one run; DIR/sweep.tsv lists what got smaller.

use crate::anneal::{self, Chains, Found, Space, St};
use crate::{Circuit, Kind, archive, verify};
use anyhow::{Context, Result, anyhow};
use clock::Pos;
use mctas::design::Design;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub struct Options {
    /// The cells blocks may occupy (default: the seed's bounding box: designs only shrink).
    pub region: Option<(Pos, Pos)>,
    /// Palette block states (empty: the seed's own states plus stone).
    pub palette: Vec<String>,
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    pub stale: usize,
    pub repair: usize,
    /// Game-judge this many of the smallest passing designs (0: none).
    pub game: usize,
    pub version: String,
}

/// What a passing candidate must keep: the seed's kind and axis value.
struct Want {
    kind: Kind,
    axis: String,
}

/// The kind's judge on one state: (score, verdict).
fn evaluate(seed: &Design, st: &St, want: &Want, region: (Pos, Pos), vo: &verify::Options) -> (anneal::Score, String) {
    let outside = -st.outside(region);
    let mk = |tier: u8| anneal::Score { tier, partial: 0, outside, neg_volume: 0, neg_blocks: 0 };
    let c = match Circuit::from_design(st.design(seed)) {
        Ok(c) => c,
        Err(e) => return (mk(0), format!("refused: {e:#}")),
    };
    let rep = match verify::sim(&c, vo) {
        Ok(r) => r,
        Err(e) => return (mk(0), format!("judge error: {e:#}")),
    };
    let (Ok(t), Some(out)) = (&rep.trace, &rep.outcome) else { return (mk(0), "no verdict".into()) };
    if !out.pass {
        return (mk(0), out.why.chars().take(160).collect());
    }
    let axis = archive::axis(&want.kind, out, &verify::family(&c, t));
    if axis != want.axis {
        return (mk(1), format!("passes as {axis}"));
    }
    let d = verify::dims(&c, t);
    let vol = d[0] as i64 * d[1] as i64 * d[2] as i64;
    let metrics: String = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
    (anneal::Score { tier: 2, partial: 0, outside, neg_volume: -vol, neg_blocks: -(st.b.len() as i64) }, format!("judged box {}x{}x{} = {vol}; {axis}; {metrics}", d[0], d[1], d[2]))
}

/// One run's result.
pub struct RunResult {
    pub kind: String,
    pub axis: String,
    pub seed_volume: i64,
    pub seed_blocks: usize,
    /// The smallest passing designs (volume, blocks, file) and each one's game verdict
    /// (`GAME-OK`, `game-fail: ...`, or empty when not game-judged).
    pub best: Vec<(i64, usize, PathBuf, String)>,
}

/// The real game on a design: GAME-OK when it passes robust with the same axis.
fn game_check(pool: &mctas::sim::Pool, d: &Design, want: &Want, gvo: &verify::Options) -> Result<String> {
    let c = Circuit::from_design(d.clone())?;
    let rep = verify::verify_in_pool(pool, &c, gvo)?;
    let line = rep.summary(&c);
    let same = rep.base().is_some_and(|b| archive::axis(&want.kind, &b.outcome, &b.family) == want.axis);
    Ok(if rep.pass && rep.verdict == "robust" && same { format!("GAME-OK {line}") } else { format!("game-fail: {line}") })
}

/// `circuit compact <seed>`: one annealing run.
pub fn compact_one(seed_path: &Path, o: &Options, out: &Path, verbose: bool) -> Result<RunResult> {
    let c0 = Circuit::load(seed_path)?;
    let kind = c0.kind.clone().ok_or_else(|| anyhow!("{}: no `circuit: <kind>` line", seed_path.display()))?;
    let seed = c0.clock.design.clone();
    let vo = verify::Options { version: o.version.clone(), ..Default::default() };
    let rep0 = verify::sim(&c0, &vo)?;
    let (Ok(t0), Some(out0)) = (&rep0.trace, &rep0.outcome) else { anyhow::bail!("{}: the judge gives no verdict on the seed", seed_path.display()) };
    anyhow::ensure!(out0.pass, "{}: the seed fails its judge: {}", seed_path.display(), out0.why);
    let want = Want { axis: archive::axis(&kind, out0, &verify::family(&c0, t0)), kind: kind.clone() };
    // a variant is no member of the seed's extendable family (its rule and generator are the seed's)
    let mut seed = seed;
    seed.meta.retain(|k, _| !k.starts_with("extend"));
    let (slo, shi) = seed.bounds().ok_or_else(|| anyhow!("empty seed"))?;
    let region = o.region.unwrap_or((slo, shi));
    let mut pal: Vec<String> = o.palette.clone();
    if pal.is_empty() {
        let states: BTreeSet<String> = seed.blocks.values().cloned().collect();
        pal = states.into_iter().collect();
        if !pal.iter().any(|s| s == "minecraft:stone") {
            pal.push("minecraft:stone".into());
        }
    }
    // block entities and entities stay where they are
    let mut frozen: BTreeSet<Pos> = seed.block_entities.keys().copied().collect();
    for e in &seed.entities {
        frozen.insert([e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]);
    }
    let st0 = St::of(&seed);
    let (s0, v0) = evaluate(&seed, &st0, &want, region, &vo);
    anyhow::ensure!(s0.tier == 2, "{}: the seed does not pass its own axis on the search's judge: {v0}", seed_path.display());
    if verbose {
        eprintln!("{}: {} {}, judged volume {}, {} blocks", seed_path.display(), kind.name(), want.axis, -s0.neg_volume, st0.b.len());
    }
    let keep = |_: &BTreeMap<Pos, String>, _: Pos| false;
    let chains = Chains {
        seed: &seed,
        space: Space { region, frozen: &frozen },
        palette: &pal,
        iterations: o.iterations,
        workers: o.workers,
        rng: o.rng,
        stale: o.stale,
        repair: o.repair,
        pass_tier: 2,
        keep: &keep,
        out,
        verbose,
    };
    let (seed_r, want_r, vo_r) = (&seed, &want, &vo);
    let make_eval = || -> anneal::Eval<'_> {
        let mut cache: HashMap<St, (anneal::Score, String)> = HashMap::new();
        Box::new(move |cand: &St, _cur: anneal::Score, _walk: bool| {
            if let Some(v) = cache.get(cand) {
                return v.clone();
            }
            let v = evaluate(seed_r, cand, want_r, region, vo_r);
            if cache.len() > 50_000 {
                cache.clear();
            }
            cache.insert(cand.clone(), v.clone());
            v
        })
    };
    let (found, _best) = anneal::run(&chains, &st0, s0, &v0, make_eval);
    let front = anneal::front(&found, 2);
    anneal::write_front(&seed, &front, out)?;
    let mut res = RunResult { kind: kind.name(), axis: want.axis.clone(), seed_volume: -s0.neg_volume, seed_blocks: st0.b.len(), best: Vec::new() };
    // the smallest designs that beat the seed (volume, or the same volume with fewer blocks)
    let better: Vec<(usize, &&Found)> = front.iter().enumerate().filter(|(_, f)| (-f.score.neg_volume, f.st.b.len()) < (res.seed_volume, res.seed_blocks)).collect();
    let pool = if o.game > 0 && !better.is_empty() { Some(verify::spawn_pool(&verify::Options { workers: 8, version: o.version.clone(), ..Default::default() })?) } else { None };
    let gvo = verify::Options { workers: 8, version: o.version.clone(), ..Default::default() };
    let mut judged = 0;
    for (i, f) in better {
        let name = anneal::front_name(&seed, f, i);
        let file = out.join("front").join(format!("{name}.txt"));
        let game = match &pool {
            Some(p) if judged < o.game => {
                judged += 1;
                let mut d = f.st.design(&seed);
                d.name = name.clone();
                game_check(p, &d, &want, &gvo).unwrap_or_else(|e| format!("game-fail: {e:#}"))
            }
            _ => String::new(),
        };
        if verbose && !game.is_empty() {
            eprintln!("  game: {name} {}", game.chars().take(200).collect::<String>());
        }
        res.best.push((-f.score.neg_volume, f.st.b.len(), file, game));
    }
    if !res.best.is_empty() {
        let mut gt = String::from("volume\tblocks\tfile\tgame\n");
        for (v, b, f, g) in &res.best {
            gt += &format!("{v}\t{b}\t{}\t{g}\n", f.display());
        }
        std::fs::write(out.join("game.tsv"), gt)?;
    }
    Ok(res)
}

/// `circuit compact <seed> <seed>...`: one run per seed (DIR/<study>/<seed stem>/), DIR/sweep.tsv
/// in `compact_all`'s format (the record is the seed itself).
pub fn compact_list(seeds: &[PathBuf], o: &Options, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut tsv = String::from("study\tkind\taxis\tseed\tseed volume\tseed blocks\tbest volume\tbest blocks\tgame\tfile\n");
    for seed in seeds {
        let stem = seed.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let file = seed.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let study = Circuit::load(seed).ok().and_then(|c| c.kind.map(|k| k.study().to_string())).unwrap_or_else(|| "unknown".into());
        let run_dir = out.join(&study).join(&stem);
        let line = match compact_one(seed, o, &run_dir, false) {
            Ok(res) => {
                let (bv, bb, f, game) = res.best.first().map(|(v, b, f, g)| (v.to_string(), b.to_string(), f.display().to_string(), g.split_whitespace().next().unwrap_or("").to_string())).unwrap_or_default();
                println!("{study}: {} {}: {file} {}, {} blocks -> {}", res.kind, res.axis, res.seed_volume, res.seed_blocks, if bv.is_empty() { "no smaller design".to_string() } else { format!("{bv} volume, {bb} blocks {game} {f}") });
                format!("{study}\t{}\t{}\t{file}\t{}\t{}\t{bv}\t{bb}\t{game}\t{f}\n", res.kind, res.axis, res.seed_volume, res.seed_blocks)
            }
            Err(e) => {
                println!("{study}: {file} skipped: {}", format!("{e:#}").chars().take(160).collect::<String>());
                format!("{study}\t\t\t{file}\t\t\t\t\tskipped\t\n")
            }
        };
        tsv += &line;
        std::fs::write(out.join("sweep.tsv"), &tsv)?;
    }
    Ok(())
}

/// A file-system-safe slug of an axis value.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// `circuit compact --all`: one run per (kind, axis) of each study, seeded with its current
/// minimum-volume robust design. Writes DIR/<study>/<kind>-<axis>/ per run and DIR/sweep.tsv.
pub fn compact_all(root: &Path, studies: &[String], o: &Options, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut tsv = String::from("study\tkind\taxis\tseed\tseed volume\tseed blocks\tbest volume\tbest blocks\tgame\tfile\n");
    for study in studies {
        let dir = archive::archive_dir(root, study);
        let rows = archive::read_rows(&dir).with_context(|| format!("reading {}", dir.display()))?;
        let mut groups: BTreeMap<(String, String), Vec<&archive::Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| r.robustness == "robust") {
            groups.entry((r.kind.clone(), r.axis.clone())).or_default().push(r);
        }
        for ((kind, axis), rs) in groups {
            let Some(rec) = rs.iter().min_by_key(|r| (r.volume, r.blocks, r.file.clone())) else { continue };
            let seed = dir.join(&rec.file);
            let run_dir = out.join(study).join(slug(&format!("{kind} {axis}")));
            let line = match compact_one(&seed, o, &run_dir, false) {
                Ok(res) => {
                    let (bv, bb, file, game) = res.best.first().map(|(v, b, f, g)| (v.to_string(), b.to_string(), f.display().to_string(), g.split_whitespace().next().unwrap_or("").to_string())).unwrap_or_default();
                    // the archive's volume is the game judge's; the search's is redsim's
                    let note = if res.seed_volume != rec.volume { format!(" (archive volume {})", rec.volume) } else { String::new() };
                    println!("{study}: {kind} {axis}: record {} {}{note}, {} blocks -> {}", rec.file, res.seed_volume, res.seed_blocks, if bv.is_empty() { "no smaller design".to_string() } else { format!("{bv} volume, {bb} blocks {game} {file}") });
                    format!("{study}\t{kind}\t{axis}\t{}\t{}\t{}\t{bv}\t{bb}\t{game}\t{file}\n", rec.file, res.seed_volume, res.seed_blocks)
                }
                Err(e) => {
                    println!("{study}: {kind} {axis}: record {} skipped: {}", rec.file, format!("{e:#}").chars().take(160).collect::<String>());
                    format!("{study}\t{kind}\t{axis}\t{}\t{}\t{}\t\t\tskipped\t\n", rec.file, rec.volume, rec.blocks)
                }
            };
            tsv += &line;
            std::fs::write(out.join("sweep.tsv"), &tsv)?;
        }
    }
    Ok(())
}

/// The function part of an archived caption (before its first `: `), without parentheticals and
/// block counts (a rearranged variant has its own).
fn caption_head(cap: &str) -> String {
    let head = cap.split(": ").next().unwrap_or(cap);
    let mut out = String::new();
    let mut depth = 0;
    for ch in head.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    let parts: Vec<&str> = out.split(',').map(|p| p.trim()).filter(|p| !(p.ends_with(" blocks") && p.split_whitespace().next().is_some_and(|n| n.parse::<u32>().is_ok()))).collect();
    parts.join(", ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `circuit compact --collect SWEEP --out DIR`: each game-OK design of a sweep (`sweep.tsv`) written
/// to DIR/<study>/<seed>-v<volume>-<blocks>b.txt with a caption built from its seed's archived
/// caption (the archive takes a design's first comment block as its caption). Returns the files
/// written (existing ones are kept).
pub fn collect(root: &Path, sweep: &Path, into: &Path) -> Result<Vec<PathBuf>> {
    let text = std::fs::read_to_string(sweep.join("sweep.tsv")).with_context(|| format!("reading {}", sweep.join("sweep.tsv").display()))?;
    let mut written = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 10 || f[8] != "GAME-OK" {
            continue;
        }
        let (study, seed_file, sv, sb, bv, bb, file) = (f[0], f[3], f[4], f[5], f[6], f[7], f[9]);
        let caps = archive::read_captions(&archive::archive_dir(root, study));
        let seed_stem = seed_file.trim_end_matches(".txt");
        let mut d = Design::load(Path::new(file))?;
        d.meta.retain(|k, _| !k.starts_with("extend"));
        let name = format!("{seed_stem}-v{bv}-{bb}b");
        d.name = name.clone();
        let (_, size) = anneal::volume(&d.blocks);
        let head = caps.get(seed_file).map(|c| caption_head(c)).unwrap_or_else(|| seed_stem.to_string());
        let cap = format!("{head}: {seed_stem}'s mechanism rearranged by clk-rt's circuit compact (2026-10-21), judged box {bv} (was {sv}), {bb} blocks (was {sb}); its blocks sit in {}x{}x{}.", size[0], size[1], size[2]);
        let dir = into.join(study);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{name}.txt"));
        if path.exists() {
            continue;
        }
        std::fs::write(&path, format!("# {cap}\n{}", d.to_text()))?;
        written.push(path);
    }
    Ok(written)
}
