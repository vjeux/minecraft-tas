//! `mctas site deploy`: the generated folder becomes ONE orphan commit on the gh-pages branch
//! (built with a temporary index, never touching the working tree or the current branch), pushed
//! with the token of `gh auth token` (the push recipe of AGENTS.md). GitHub Pages serves it at
//! https://vjeux.github.io/minecraft-tas/.

use crate::DeployArgs;
use anyhow::{Context, Result, bail};
use base64::Engine;
use std::path::Path;
use std::process::Command;

pub const PAGES_URL: &str = "https://vjeux.github.io/minecraft-tas/";
const REPO: &str = "vjeux/minecraft-tas";

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
        println!("dry run: not pushed");
        return Ok(());
    }
    let token = Command::new("gh").args(["auth", "token"]).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if token.is_empty() {
        bail!("no GitHub token (`gh auth token`)");
    }
    let header = format!("http.extraHeader=Authorization: Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("vjeux:{token}")));
    let refspec = format!("+{commit}:refs/heads/{}", a.branch);
    let o = Command::new("git").args(["-c", &header, "push", &a.remote, &refspec]).output()?;
    if !o.status.success() {
        bail!("push failed: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    println!("pushed {} to {} {}", commit, a.remote, a.branch);
    if a.enable_pages {
        enable_pages(&a.branch)?;
    }
    println!("{PAGES_URL}");
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
