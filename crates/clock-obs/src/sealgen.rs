//! Sealed twins of item families (clk-obs, 2026-10-21; clk-misc's audit: a dropped item changes or
//! breaks 48 of 104 hopper clocks): `clock obs sealgen --family <base> --n N --out F` builds member
//! n of an archived family with the family's own generator and seals it (glass on every open
//! hopper top, `tap::sealed`; the clock must run unchanged on redsim): the generator of the twin
//! `<family>-sealed`. `clock obs seal-family <bases...> --out-dir D` writes each twin's base design:
//! the sealed base member with the family's period formula and n range, the blocks formula fitted
//! on three sealed members (refused when not linear), and that generator. `clock archive` then
//! judges the twin at its sample members in the game, and `clock extend-check --record` records
//! the members the original family recorded.

use anyhow::{Context, Result, anyhow, bail};
use clap::Args as ClapArgs;
use clock::Clock;
use clock::extend::Family;
use std::path::{Path, PathBuf};

#[derive(ClapArgs, Debug)]
pub struct SealgenArgs {
    /// The archived family base (its `extend` headers and generator).
    #[arg(long)]
    pub family: PathBuf,
    #[arg(long)]
    pub n: i64,
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(ClapArgs, Debug)]
pub struct SealFamilyArgs {
    /// Archived family bases with an open hopper top.
    pub bases: Vec<PathBuf>,
    #[arg(long, default_value = "/tmp/clock-seal-family")]
    pub out_dir: PathBuf,
}

/// Member n of the family at `base`, sealed: the design text (the generated file with its blocks
/// replaced, named `<generated name>-sealed`) and its block count.
fn sealed_member(base: &Path, n: i64) -> Result<(String, usize)> {
    let c = Clock::load(base)?;
    let f = Family::of(&c)?.ok_or_else(|| anyhow!("{}: no `extend family:` headers", base.display()))?;
    let dir = std::env::temp_dir().join(format!("clock-sealgen-{}-{}", std::process::id(), f.id));
    let p = f.build(base, n, &dir)?;
    let m = Clock::load(&p)?;
    // sealed with its declared output, else with the archived base's output (a generator can
    // predate a retap of the base), else with the output moved to its best usable tap (an output
    // cell can sit where the glass must go)
    let s = crate::tap::sealed(&m)
        .or_else(|| {
            let o = c.output.filter(|o| Some(*o) != m.output && !m.design.blocks.contains_key(o))?;
            crate::tap::sealed(&clock::usable::with_output(&m, o).ok()?)
        })
        .or_else(|| {
            let (u, _) = clock::usable::pick(&m, clock::usable::Rule::default(), None).ok()?;
            let t = if u.tap.kind == clock::usable::TapKind::Repeater { clock::usable::with_output(&m, u.tap.out).ok()? } else { clock::usable::joined(&m, u.tap).ok()? };
            crate::tap::sealed(&t)
        })
        .ok_or_else(|| anyhow!("{} n = {n}: sealing leaves an intake open or changes the clock on redsim", f.id))?;
    let text = std::fs::read_to_string(&p)?;
    let mut d = mctas::design::Design::parse(&text)?;
    d.blocks = s.design.blocks.clone();
    d.ports = s.design.ports.clone();
    d.meta.retain(|k, _| !k.starts_with("extend"));
    d.name = format!("{}-sealed", d.name);
    let head: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    let added = s.blocks() as i64 - m.blocks() as i64;
    let _ = std::fs::remove_dir_all(&dir);
    Ok((format!("{head}# sealed (clk-obs `clock obs sealgen`): {added} glass block(s) on the open hopper tops, so no dropped item can reach a hopper\n{}", d.to_text()), s.blocks()))
}

pub fn run_sealgen(a: SealgenArgs) -> Result<()> {
    let (text, _) = sealed_member(&a.family, a.n)?;
    std::fs::write(&a.out, text).with_context(|| format!("writing {}", a.out.display()))?;
    Ok(())
}

/// `a + bn` in the extend formula syntax.
fn formula(a: i64, b: i64) -> String {
    match (a, b) {
        (a, 0) => format!("{a}"),
        (0, b) => format!("{b}n"),
        (a, b) if a < 0 => format!("{b}n - {}", -a),
        (a, b) => format!("{a} + {b}n"),
    }
}

fn one_family(base: &Path, out_dir: &Path) -> Result<String> {
    let c = Clock::load(base)?;
    let f = Family::of(&c)?.ok_or_else(|| anyhow!("no `extend family:` headers"))?;
    if clock::intrusion::intakes(&c.design, &[]).is_empty() {
        bail!("{}: no open hopper top", f.id);
    }
    // three members fit the blocks formula (glass grows with the hoppers a member adds)
    let ns: Vec<i64> = [f.base, f.base + 1, (f.n_lo + f.n_hi) / 2].into_iter().filter(|n| (f.n_lo..=f.n_hi).contains(n)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let mut pts = Vec::new();
    for &n in &ns {
        pts.push((n, sealed_member(base, n)?.1 as i64));
    }
    let (n0, b0) = pts[0];
    let slope = if pts.len() > 1 { (pts[1].1 - b0) / (pts[1].0 - n0) } else { 0 };
    let a0 = b0 - slope * n0;
    for &(n, b) in &pts {
        if a0 + slope * n != b {
            bail!("{}: sealed blocks not linear in n ({:?})", f.id, pts);
        }
    }
    let (text, _) = sealed_member(base, f.base)?;
    let mut d = mctas::design::Design::parse(&text)?;
    let id = format!("{}-sealed", f.id);
    d.name = id.clone();
    let generator = format!("mctas clock obs sealgen --family {} --n {{n}} --out {{out}}", base.display());
    for (k, v) in [
        ("extend family", id.clone()),
        ("extend period", c.design.meta.get("extend period").cloned().unwrap_or_else(|| f.period.text()).trim().to_string()),
        ("extend blocks", formula(a0, slope)),
        ("extend rule", format!("{}; sealed: glass on the open hopper tops (no dropped item reaches a hopper)", f.rule)),
        ("extend n", format!("{}..{}", f.n_lo, f.n_hi)),
        ("extend base", f.base.to_string()),
        ("extend generator", generator),
    ] {
        d.meta.insert(k.to_string(), v);
    }
    // the twins of one card's families share their own card: `<group>-sealed`
    if let Some(g) = &f.group {
        d.meta.insert("extend group".into(), format!("{g}-sealed"));
        if let Some(t) = &f.group_title {
            d.meta.insert("extend group title".into(), format!("{t}, sealed"));
        }
        if let Some(p) = &f.group_period {
            d.meta.insert("extend group period".into(), p.clone());
        }
        let note = f.group_note.clone().map(|n| format!("{n}; glass on the open hopper tops")).unwrap_or_else(|| "glass on the open hopper tops: no dropped item reaches a hopper".into());
        d.meta.insert("extend group note".into(), note);
    }
    let head: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    let out = out_dir.join(format!("{id}.txt"));
    std::fs::write(&out, format!("{head}# The sealed twin of {} (clk-obs `clock obs seal-family`): each member is the original's, sealed.\n{}", f.id, d.to_text()))?;
    Ok(format!("TWIN\t{id}\tperiod {}\tblocks {}\t{}", d.meta.get("extend period").cloned().unwrap_or_default(), formula(a0, slope), out.display()))
}

pub fn run_seal_family(a: SealFamilyArgs) -> Result<()> {
    std::fs::create_dir_all(&a.out_dir)?;
    for b in &a.bases {
        match one_family(b, &a.out_dir) {
            Ok(l) => println!("{l}"),
            Err(e) => println!("NONE\t{}\t{e:#}", b.display()),
        }
    }
    Ok(())
}

#[derive(ClapArgs, Debug)]
pub struct SealArchiveArgs {
    /// Twin base designs (`seal-family`'s output).
    pub twins: Vec<PathBuf>,
    #[arg(long, default_value = "designs/clocks/archive")]
    pub archive: PathBuf,
}

/// `clock obs seal-archive <twins...>`: each twin archived (`clock archive`, its sample members
/// judged in the game) with a caption naming its original, then the members its original family
/// recorded (extend.tsv) judged and recorded on the twin (`clock extend-check --record`).
pub fn run_seal_archive(a: SealArchiveArgs) -> Result<()> {
    let me = std::env::current_exe()?;
    let recorded = clock::extend::read_instances(&a.archive);
    for t in &a.twins {
        let c = Clock::load(t)?;
        let f = Family::of(&c)?.ok_or_else(|| anyhow!("{}: not a family base", t.display()))?;
        let orig = f.id.trim_end_matches("-sealed").to_string();
        let caption = format!("The sealed twin of {orig}: each member is {orig}'s with glass on its open hopper tops, so no dropped item can change its timing (clk-misc's audit: a dropped item changes or breaks the open family); period {}, {} blocks.", c.design.meta.get("extend period").cloned().unwrap_or_default(), c.design.meta.get("extend blocks").cloned().unwrap_or_default());
        let o = std::process::Command::new(&me).args(["clock", "archive"]).arg(t).args(["--caption", &caption]).output()?;
        let out = String::from_utf8_lossy(&o.stdout);
        let last = out.lines().filter(|l| l.starts_with("ARCHIVED") || l.starts_with("NOT ARCHIVED") || l.starts_with("REFUSED") || l.starts_with("SKIP")).last().unwrap_or("(no verdict line)").to_string();
        println!("{}: {}", f.id, last.chars().take(200).collect::<String>());
        if !last.starts_with("ARCHIVED") {
            continue;
        }
        // the archived file, as `clock archive` names it
        let archived = last.trim_start_matches("ARCHIVED ").split(": ").next().map(PathBuf::from).unwrap_or_else(|| a.archive.join(format!("{}.txt", f.id)));
        let have: std::collections::BTreeSet<i64> = recorded.iter().filter(|i| i.family == f.id).map(|i| i.n).collect();
        let ns: Vec<String> = recorded.iter().filter(|i| i.family == orig && !have.contains(&i.n) && (f.n_lo..=f.n_hi).contains(&i.n)).map(|i| i.n.to_string()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        if ns.is_empty() || !archived.exists() {
            continue;
        }
        let o = std::process::Command::new(&me).args(["clock", "extend-check"]).arg(&archived).args(["--n", &ns.join(","), "--record"]).output()?;
        let out = String::from_utf8_lossy(&o.stdout);
        let ok = out.lines().filter(|l| l.starts_with("OK")).count();
        let fail: Vec<&str> = out.lines().filter(|l| l.starts_with("FAIL")).collect();
        println!("  members n = {}: {ok} OK{}", ns.join(","), if fail.is_empty() { String::new() } else { format!(", FAIL: {}", fail.join(" | ").chars().take(300).collect::<String>()) });
    }
    Ok(())
}
