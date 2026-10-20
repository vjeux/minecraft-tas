//! `mctas site deploy`: the generated folder becomes ONE orphan commit on the gh-pages branch
//! (built with a temporary index, never touching the working tree or the current branch), pushed
//! with the token of `gh auth token` (the push recipe of AGENTS.md). GitHub Pages serves it at
//! https://vjeux.github.io/minecraft-tas/.
//!
//! Two guards (several agents deploy): the checkout must contain the commit the current site was
//! built from (else a deploy from an older tree would take features away: merge first), and
//! GitHub Pages builds a branch about 10 times an hour at most (later builds error out), so a
//! deploy waits when 9 builds started in the last hour. After the push it follows the Pages
//! build to its end and fails when it errors.

use crate::DeployArgs;
use anyhow::{Context, Result, bail};
use base64::Engine;
use std::path::Path;
use std::process::Command;

pub const PAGES_URL: &str = "https://vjeux.github.io/minecraft-tas/";
const REPO: &str = "vjeux/minecraft-tas";
/// GitHub Pages' soft limit is 10 builds an hour for a branch; keep one for the next deploy.
const BUILDS_PER_HOUR: usize = 9;

fn git(args: &[&str], env: &[(&str, &Path)], work_tree: Option<(&Path, &str)>) -> Result<String> {
    let mut c = Command::new("git");
    if let Some((w, git_dir)) = work_tree {
        // run inside the folder (so `.` is all of it) against the repository's git dir
        c.arg("-C").arg(w).arg("--git-dir").arg(git_dir).arg("--work-tree").arg(w);
    }
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().with_context(|| format!("git {}", args.join(" ")))?;
    if !o.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn gh_json(path: &str) -> Result<serde_json::Value> {
    let o = Command::new("gh").args(["api", path]).output().context("running gh")?;
    if !o.status.success() {
        bail!("gh api {path}: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(serde_json::from_slice(&o.stdout)?)
}

/// Seconds since 1970 of `2026-10-20T00:42:19Z`.
pub fn parse_utc(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once('T')?;
    let d: Vec<i64> = date.split('-').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let t: Vec<i64> = time.split(':').map(|x| x.split('.').next().unwrap_or(x).parse().ok()).collect::<Option<_>>()?;
    if d.len() != 3 || t.len() < 2 {
        return None;
    }
    // days from the civil date (Howard Hinnant)
    let (y, m, day) = (if d[1] <= 2 { d[0] - 1 } else { d[0] }, d[1], d[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146097 + doe - 719468) * 86400 + t[0] * 3600 + t[1] * 60 + t.get(2).copied().unwrap_or(0))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// The start times (seconds) of the Pages builds of the last hour.
fn builds_last_hour() -> Result<Vec<i64>> {
    let v = gh_json(&format!("repos/{REPO}/pages/builds?per_page=40"))?;
    let now = now_secs();
    Ok(v.as_array().map(|a| a.iter().filter_map(|b| b["created_at"].as_str().and_then(parse_utc)).filter(|t| now - t < 3600).collect()).unwrap_or_default())
}

/// The commit the current gh-pages was built from (its message: `... from <hash>`), if known.
fn deployed_source(remote: &str, branch: &str) -> Option<String> {
    git(&["fetch", "-q", remote, branch], &[], None).ok()?;
    let msg = git(&["log", "-1", "--format=%s", "FETCH_HEAD"], &[], None).ok()?;
    let hash = msg.rsplit(" from ").next()?.trim().to_string();
    (!hash.is_empty() && hash.chars().all(|c| c.is_ascii_hexdigit())).then_some(hash)
}

/// Guard 1: never deploy an older tree over a newer site: the checkout must contain the commit
/// the live site was built from.
fn guard_contains(a: &DeployArgs) -> Result<()> {
    if let Some(src) = deployed_source(&a.remote, &a.branch) {
        let known = git(&["cat-file", "-e", &format!("{src}^{{commit}}")], &[], None).is_ok();
        let contained = known && git(&["merge-base", "--is-ancestor", &src, "HEAD"], &[], None).is_ok();
        if !contained {
            bail!(
                "the live site was built from {src}, which this checkout does not contain{}: merge it first (git fetch && git merge {src}), rebuild, or --force",
                if known { "" } else { " (unknown here: fetch the branch it came from)" }
            );
        }
    }
    Ok(())
}

pub fn deploy(a: &DeployArgs) -> Result<()> {
    let dir = a.dir.canonicalize().with_context(|| format!("{}", a.dir.display()))?;
    if !dir.join("index.html").exists() || !dir.join("data/site.json").exists() {
        bail!("{} is not a generated site (no index.html / data/site.json): run `mctas site build` first", dir.display());
    }
    let git_dir = git(&["rev-parse", "--absolute-git-dir"], &[], None)?;
    let index = std::env::temp_dir().join(format!("mctas-site-index-{}", std::process::id()));
    let _ = std::fs::remove_file(&index);
    let env = [("GIT_INDEX_FILE", index.as_path())];
    git(&["add", "-A", "-f", "."], &env, Some((&dir, &git_dir)))?;
    let tree = git(&["write-tree"], &env, Some((&dir, &git_dir)))?;
    let _ = std::fs::remove_file(&index);
    let head = git(&["rev-parse", "--short=9", "HEAD"], &[], None).unwrap_or_default();
    let msg = format!("REDSTONEPEDIA: site built by `mctas site build` from {head}");
    let commit = git(&["commit-tree", &tree, "-m", &msg], &[], None)?;
    println!("gh-pages commit {commit} (tree {tree})");
    if a.dry_run {
        if !a.force {
            guard_contains(a)?;
        }
        println!("dry run: not pushed");
        return Ok(());
    }
    // guard 2: GitHub Pages' build rate
    if !a.force {
        loop {
            let mut recent = builds_last_hour().unwrap_or_default();
            if recent.len() < BUILDS_PER_HOUR {
                break;
            }
            recent.sort();
            let free_at = recent[recent.len() - BUILDS_PER_HOUR] + 3600;
            let wait = (free_at - now_secs()).max(10);
            let minutes = (wait + 59) / 60;
            if !a.wait {
                bail!("{} Pages builds in the last hour (GitHub builds about 10 an hour; later ones error out): try again in {minutes} min, or pass --wait", recent.len());
            }
            println!("{} Pages builds in the last hour: waiting {minutes} min", recent.len());
            std::thread::sleep(std::time::Duration::from_secs(wait.min(600) as u64));
        }
        // guard 1, checked right before the push (another deploy may have landed while waiting)
        guard_contains(a)?;
    }
    let token = Command::new("gh").args(["auth", "token"]).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if token.is_empty() {
        bail!("no GitHub token (`gh auth token`)");
    }
    let header = format!("http.extraHeader=Authorization: Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("vjeux:{token}")));
    let refspec = format!("+{commit}:refs/heads/{}", a.branch);
    // GitHub sometimes rejects a large push once ("remote rejected ... (failed)"): retry
    let mut last = String::new();
    let mut pushed = false;
    for attempt in 1..=3 {
        let o = Command::new("git").args(["-c", &header, "push", &a.remote, &refspec]).output()?;
        if o.status.success() {
            pushed = true;
            break;
        }
        last = String::from_utf8_lossy(&o.stderr).trim().to_string();
        eprintln!("push attempt {attempt} failed: {}", last.lines().last().unwrap_or(""));
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    if !pushed {
        bail!("push failed: {last}");
    }
    println!("pushed {} to {} {}", commit, a.remote, a.branch);
    if a.enable_pages {
        enable_pages(&a.branch)?;
    }
    follow_build(&commit)?;
    println!("{PAGES_URL}");
    Ok(())
}

/// Follow the Pages build of `commit` to its end (up to 6 minutes): fail when it errors.
fn follow_build(commit: &str) -> Result<()> {
    for _ in 0..72 {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let Ok(v) = gh_json(&format!("repos/{REPO}/pages/builds?per_page=10")) else { continue };
        let mine = v.as_array().and_then(|a| a.iter().find(|b| b["commit"].as_str() == Some(commit)).cloned());
        match mine.as_ref().and_then(|b| b["status"].as_str()) {
            Some("built") => {
                println!("Pages build of {}: built", &commit[..9.min(commit.len())]);
                return Ok(());
            }
            Some("errored") => {
                let why = mine.as_ref().and_then(|b| b["error"]["message"].as_str()).unwrap_or("");
                bail!("the Pages build of {} errored ({why}): a newer deploy or GitHub's build limit (about 10 an hour) may have superseded it", &commit[..9.min(commit.len())]);
            }
            _ => {}
        }
    }
    println!("Pages build of {}: still running after 6 min (check https://github.com/{REPO}/deployments)", &commit[..9.min(commit.len())]);
    Ok(())
}

/// Turn GitHub Pages on for `branch` (root folder) when it is off.
fn enable_pages(branch: &str) -> Result<()> {
    let probe = Command::new("gh").args(["api", &format!("repos/{REPO}/pages")]).output()?;
    if probe.status.success() {
        println!("GitHub Pages already on: {}", String::from_utf8_lossy(&probe.stdout).chars().take(300).collect::<String>());
        return Ok(());
    }
    let o = Command::new("gh")
        .args(["api", "-X", "POST", &format!("repos/{REPO}/pages"), "-f", &format!("source[branch]={branch}"), "-f", "source[path]=/"])
        .output()?;
    if !o.status.success() {
        bail!("enabling GitHub Pages failed: {} {}", String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim());
    }
    println!("GitHub Pages enabled on {branch}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_times() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2026-10-20T00:42:19Z"), Some(1792456939));
        assert_eq!(parse_utc("2026-10-20T00:42:19.123Z"), Some(1792456939));
        assert_eq!(parse_utc("nonsense"), None);
    }
}
