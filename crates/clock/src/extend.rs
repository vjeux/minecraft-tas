//! Extendable clocks (vjeux: "tons of variations where you just add 2 observers and make the tower
//! bigger: write it as <base gt> + n * <incremental gt> and describe how you extend it"). A family
//! is one archived base design carrying `extend` headers:
//!
//! ```text
//! extend family: clk-obs-note-ring
//! extend period: 10 + 4n          (or a geometric one: 40 * 2^n)
//! extend blocks: 6 + 2n
//! extend rule: insert 2 observers into the ring, one on each side
//! extend n: 0..12                 (verified in the game; this design is n = the low end, or `extend base:`)
//! extend generator: mctas clock obs note-ring --n {n} --out {out}
//! ```
//!
//! `clock extend-check` builds instances with the generator and judges them in the game (period and
//! blocks must follow the formulas); `clock archive` of a family checks n = low, low+1, low+2, the
//! middle and the high end first and records every instance in `archive/extend.tsv` (family, n,
//! period, blocks, type key), so the catalog shows one card per family and leaves its instances
//! out of the per-period gallery.

use crate::Clock;
use anyhow::{Context, Result, anyhow, bail};
use std::path::{Path, PathBuf};

/// A period or block-count formula in n: `a + b·n` or `a·k^n + c`.
#[derive(Clone, Debug, PartialEq)]
pub enum Formula {
    Linear { a: i64, b: i64 },
    Geometric { a: i64, k: i64, c: i64 },
}

impl Formula {
    /// Parse `10 + 4n`, `10+4*n`, `10 - 2n`, `7n`, `40 * 2^n`, `3*2^n + 1`, `12`.
    pub fn parse(s: &str) -> Result<Formula> {
        let t: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace('·', "*").to_lowercase();
        let num = |x: &str| -> Result<i64> { x.parse::<i64>().map_err(|_| anyhow!("bad number {x:?} in {s:?}")) };
        if let Some(i) = t.find("^n") {
            // a*k^n [+-c]
            let head = &t[..i];
            let tail = &t[i + 2..];
            let (a, k) = match head.rfind('*') {
                Some(j) => (num(&head[..j])?, num(&head[j + 1..])?),
                None => (1, num(head)?),
            };
            let c = if tail.is_empty() { 0 } else { num(tail.trim_start_matches('+'))? };
            return Ok(Formula::Geometric { a, k, c });
        }
        if !t.contains('n') {
            return Ok(Formula::Linear { a: num(&t)?, b: 0 });
        }
        // a [+-] b[*]n, or b[*]n [+-] a
        let t = t.replace("*n", "n");
        let split = t.char_indices().skip(1).filter(|(_, c)| *c == '+' || *c == '-').map(|(i, _)| i).next();
        let (x, y) = match split {
            Some(i) => (&t[..i], &t[i..]),
            None => (t.as_str(), ""),
        };
        let coef = |term: &str| -> Result<i64> {
            let body = term.trim_end_matches('n');
            match body {
                "" | "+" => Ok(1),
                "-" => Ok(-1),
                b => num(b.trim_start_matches('+')),
            }
        };
        let (a, b) = if x.ends_with('n') { (if y.is_empty() { 0 } else { num(y.trim_start_matches('+'))? }, coef(x)?) } else { (num(x)?, coef(y)?) };
        Ok(Formula::Linear { a, b })
    }

    pub fn eval(&self, n: i64) -> i64 {
        match *self {
            Formula::Linear { a, b } => a + b * n,
            Formula::Geometric { a, k, c } => a * k.pow(n.max(0) as u32) + c,
        }
    }

    /// `10 + 4n`, `40 · 2^n`.
    pub fn text(&self) -> String {
        match *self {
            Formula::Linear { a, b } if b == 0 => format!("{a}"),
            Formula::Linear { a, b } if b < 0 => format!("{a} − {}n", -b),
            Formula::Linear { a, b } => format!("{a} + {b}n"),
            Formula::Geometric { a, k, c } => {
                let head = if a == 1 { format!("{k}^n") } else { format!("{a} · {k}^n") };
                match c {
                    0 => head,
                    c if c < 0 => format!("{head} − {}", -c),
                    c => format!("{head} + {c}"),
                }
            }
        }
    }

    /// The same formula in seconds (20 gt = 1 s): `0.5 s + 0.2 s·n`, `2 s · 2^n`.
    pub fn seconds(&self) -> String {
        let s = |gt: i64| -> String {
            let v = gt as f64 / 20.0;
            let t = format!("{v:.2}");
            format!("{} s", t.trim_end_matches('0').trim_end_matches('.'))
        };
        match *self {
            Formula::Linear { a, b } if b == 0 => s(a),
            Formula::Linear { a, b } => format!("{} {} {}·n", s(a), if b < 0 { "−" } else { "+" }, s(b.abs())),
            Formula::Geometric { a, k, c } => {
                let head = format!("{} · {k}^n", s(a));
                match c {
                    0 => head,
                    c => format!("{head} {} {}", if c < 0 { "−" } else { "+" }, s(c.abs())),
                }
            }
        }
    }
}

/// An extendable family, read from a design's `extend` headers.
#[derive(Clone, Debug)]
pub struct Family {
    pub id: String,
    pub period: Formula,
    pub blocks: Formula,
    pub rule: String,
    /// The n range verified in the game.
    pub n_lo: i64,
    pub n_hi: i64,
    /// The n of the design carrying the headers.
    pub base: i64,
    pub generator: String,
    /// The card group (`extend group:`): families of one mechanism split into several cards share
    /// one catalog card (vjeux: "can we not have 300 variations of this one?"), with the group's
    /// optional `extend group title:`, `extend group period:` (the general formula, the card's
    /// heading) and `extend group note:` (taken from the first member carrying them).
    pub group: Option<String>,
    pub group_title: Option<String>,
    pub group_period: Option<String>,
    pub group_note: Option<String>,
}

/// The `extend group` header lines of a family base design, rewritten in place (`clock group`):
/// the old ones are dropped; `id` = None clears the group. The lines go after `extend family:`.
pub fn set_group(path: &Path, id: Option<&str>, title: Option<&str>, period: Option<&str>, note: Option<&str>) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut out: Vec<String> = Vec::new();
    let mut placed = false;
    for l in text.lines() {
        if l.starts_with("extend group") {
            continue;
        }
        out.push(l.to_string());
        if l.starts_with("extend family:") {
            placed = true;
            if let Some(id) = id {
                out.push(format!("extend group: {id}"));
                for (k, v) in [("title", title), ("period", period), ("note", note)] {
                    if let Some(v) = v {
                        out.push(format!("extend group {k}: {v}"));
                    }
                }
            }
        }
    }
    if !placed {
        bail!("{}: no `extend family:` line (not a family base design)", path.display());
    }
    let mut s = out.join("\n");
    s.push('\n');
    std::fs::write(path, s)?;
    let c = Clock::load(path)?;
    Family::of(&c)?.ok_or_else(|| anyhow!("{}: not a family after the edit", path.display()))?;
    Ok(())
}

impl Family {
    /// The family of a design (`None` without `extend family:`).
    pub fn of(c: &Clock) -> Result<Option<Family>> {
        let m = &c.design.meta;
        let Some(id) = m.get("extend family") else {
            return Ok(None);
        };
        let get = |k: &str| m.get(k).cloned().ok_or_else(|| anyhow!("extend family {id}: missing `{k}:`"));
        let range = get("extend n")?;
        let (lo, hi) = range.split_once("..").ok_or_else(|| anyhow!("extend n: {range:?}: want a..b"))?;
        let n_lo: i64 = lo.trim().parse().with_context(|| format!("extend n: {range:?}"))?;
        let n_hi: i64 = hi.trim().trim_start_matches('=').parse().with_context(|| format!("extend n: {range:?}"))?;
        let base = match m.get("extend base") {
            Some(b) => b.trim().parse().with_context(|| format!("extend base: {b:?}"))?,
            None => n_lo,
        };
        Ok(Some(Family {
            id: id.trim().to_string(),
            period: Formula::parse(&get("extend period")?)?,
            blocks: Formula::parse(&get("extend blocks")?)?,
            rule: get("extend rule")?.trim().to_string(),
            n_lo,
            n_hi,
            base,
            generator: m.get("extend generator").cloned().unwrap_or_default().trim().to_string(),
            group: m.get("extend group").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            group_title: m.get("extend group title").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            group_period: m.get("extend group period").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            group_note: m.get("extend group note").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        }))
    }

    /// The n values `clock archive` checks: low, low+1, low+2, the middle, high.
    pub fn sample(&self) -> Vec<i64> {
        let mut v = vec![self.n_lo, self.n_lo + 1, self.n_lo + 2, (self.n_lo + self.n_hi) / 2, self.n_hi];
        v.retain(|n| (self.n_lo..=self.n_hi).contains(n));
        v.sort();
        v.dedup();
        v
    }

    /// The n values a circuit family is judged at: every n of its range when it has at most
    /// `EVERY_MAX` members (no unjudged step between judged ones), else [`Family::sample`].
    pub fn judged_ns(&self) -> Vec<i64> {
        if self.n_hi - self.n_lo < EVERY_MAX { (self.n_lo..=self.n_hi).collect() } else { self.sample() }
    }

    /// Build instance n with the generator (argv split on spaces; `{n}` and `{out}` replaced;
    /// `mctas` is this executable). The base n with no generator: the design itself.
    pub fn build(&self, base_path: &Path, n: i64, dir: &Path) -> Result<PathBuf> {
        if self.generator.is_empty() {
            if n == self.base {
                return Ok(base_path.to_path_buf());
            }
            bail!("extend family {}: no `extend generator:` to build n = {n}", self.id);
        }
        std::fs::create_dir_all(dir)?;
        let out = dir.join(format!("{}-n{n}.txt", self.id));
        let argv: Vec<String> = self.generator.split_whitespace().map(|a| a.replace("{n}", &n.to_string()).replace("{out}", &out.display().to_string())).collect();
        let (prog, args) = argv.split_first().ok_or_else(|| anyhow!("empty generator"))?;
        let exe = if prog == "mctas" {
            let me = std::env::current_exe()?;
            // a binary rebuilt while this run goes on: /proc/self/exe says "<path> (deleted)"
            if me.to_string_lossy().ends_with(" (deleted)") {
                bail!("tool error: this mctas binary was replaced while running ({}); run long jobs from a copy of the binary", me.display());
            }
            me
        } else {
            PathBuf::from(prog)
        };
        let o = std::process::Command::new(&exe).args(args).output().with_context(|| format!("tool error: running {}", self.generator))?;
        if !o.status.success() {
            bail!("generator for n = {n} failed: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""));
        }
        if !out.exists() {
            bail!("generator for n = {n} wrote no {}", out.display());
        }
        Ok(out)
    }
}

/// One verified instance (a row of `archive/extend.tsv`).
#[derive(Clone, Debug)]
pub struct Instance {
    pub family: String,
    pub n: i64,
    pub period: u32,
    pub blocks: usize,
    pub type_key: String,
    pub verdict: String,
}

pub const EXTEND_TSV: &str = "extend.tsv";

/// A circuit family with at most this many members is judged at every n ([`Family::judged_ns`]).
pub const EVERY_MAX: i64 = 16;

pub fn read_instances(archive: &Path) -> Vec<Instance> {
    std::fs::read_to_string(archive.join(EXTEND_TSV))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 6).then(|| Instance { family: f[0].into(), n: f[1].parse().unwrap_or(0), period: f[2].parse().unwrap_or(0), blocks: f[3].parse().unwrap_or(0), type_key: f[4].into(), verdict: f[5].into() })
        })
        .collect()
}

/// Add (or replace) instances in `archive/extend.tsv`.
pub fn record(archive: &Path, rows: &[Instance]) -> Result<()> {
    let mut all = read_instances(archive);
    all.retain(|r| !rows.iter().any(|x| x.family == r.family && x.n == r.n));
    all.extend(rows.iter().cloned());
    all.sort_by(|a, b| (&a.family, a.n).cmp(&(&b.family, b.n)));
    let mut s = String::from("family\tn\tperiod\tblocks\ttype\tverdict\n");
    for r in &all {
        s.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\n", r.family, r.n, r.period, r.blocks, r.type_key, r.verdict));
    }
    std::fs::write(archive.join(EXTEND_TSV), s)?;
    Ok(())
}

/// Judge instances of a family in the game: build each n with the generator, `clock verify` it
/// (the same judge as `clock archive`), compare its period and blocks with the formulas. Returns
/// one (instance, ok, line) per n.
pub fn check(pool: &mctas::sim::Pool, base_path: &Path, f: &Family, ns: &[i64], o: &crate::verify::VerifyOptions) -> Result<Vec<(Instance, bool, String)>> {
    let dir = mctas::paths::run_dir().join(format!("extend-{}-{}", f.id, std::process::id()));
    let mut out = Vec::new();
    for &n in ns {
        let path = f.build(base_path, n, &dir)?;
        let c = Clock::load(&path)?;
        let rep = crate::verify::verify_in_pool(pool, &c, o)?;
        let (want_p, want_b) = (f.period.eval(n), f.blocks.eval(n));
        let type_key = crate::ident::type_key(&c);
        let (period, verdict) = match (rep.verdict, rep.base()) {
            (crate::sim::Verdict::Clock, Some(b)) => (b.period, rep.robustness.clone()),
            _ => (0, rep.summary()),
        };
        let blocks = c.blocks();
        let ok = period as i64 == want_p && blocks as i64 == want_b;
        let line = format!(
            "{} n = {n}: period {} (formula {want_p}), {blocks} blocks (formula {want_b}), {verdict}",
            if ok { "OK  " } else { "FAIL" },
            if period > 0 { crate::gt_time(period) } else { "-".into() }
        );
        out.push((Instance { family: f.id.clone(), n, period, blocks, type_key, verdict }, ok, line));
    }
    Ok(out)
}

/// Parse `--n`: `a..b` (every n), `a,b,c`, or one number.
pub fn parse_ns(s: &str) -> Result<Vec<i64>> {
    if let Some((a, b)) = s.split_once("..") {
        let (a, b): (i64, i64) = (a.trim().parse()?, b.trim().trim_start_matches('=').parse()?);
        return Ok((a..=b).collect());
    }
    s.split(',').map(|x| x.trim().parse::<i64>().map_err(|e| anyhow!("--n {s:?}: {e}"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas() {
        assert_eq!(Formula::parse("10 + 4n").unwrap(), Formula::Linear { a: 10, b: 4 });
        assert_eq!(Formula::parse("10+4*n").unwrap(), Formula::Linear { a: 10, b: 4 });
        assert_eq!(Formula::parse("7n").unwrap(), Formula::Linear { a: 0, b: 7 });
        assert_eq!(Formula::parse("7n - 3").unwrap(), Formula::Linear { a: -3, b: 7 });
        assert_eq!(Formula::parse("12").unwrap(), Formula::Linear { a: 12, b: 0 });
        assert_eq!(Formula::parse("40 * 2^n").unwrap(), Formula::Geometric { a: 40, k: 2, c: 0 });
        assert_eq!(Formula::parse("3*2^n + 1").unwrap(), Formula::Geometric { a: 3, k: 2, c: 1 });
        assert_eq!(Formula::parse("10 + 4n").unwrap().eval(3), 22);
        assert_eq!(Formula::parse("40 * 2^n").unwrap().eval(3), 320);
        assert_eq!(Formula::parse("10 + 4n").unwrap().text(), "10 + 4n");
        assert_eq!(Formula::parse("10 + 4n").unwrap().seconds(), "0.5 s + 0.2 s·n");
        assert_eq!(Formula::parse("40 * 2^n").unwrap().seconds(), "2 s · 2^n");
    }
}
