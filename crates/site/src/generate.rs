//! `mctas site build`: everything into one folder.

use crate::pages::{self, Scripts};
use crate::{BuildArgs, circuits, clocks, doors, web};
use anyhow::{Context, Result, bail};
use site_model::{DATA_VERSION, Member, Site};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const STYLE: &str = include_str!("../assets/style.css");
pub const FAVICON: &str = include_str!("../assets/favicon.svg");

/// The repository commit (HEAD), or `unknown`.
pub fn commit(repo: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Empty `out`, refusing to delete a folder that is not a previous site build.
fn fresh_dir(out: &Path) -> Result<()> {
    if out.exists() {
        let empty = std::fs::read_dir(out)?.next().is_none();
        if !empty && !out.join("data/site.json").exists() {
            bail!("{} exists and is not a site build (no data/site.json): refusing to replace it", out.display());
        }
        std::fs::remove_dir_all(out).with_context(|| format!("removing {}", out.display()))?;
    }
    std::fs::create_dir_all(out)?;
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Copy every regular file of `from` (not recursive) into `to`.
pub fn copy_files(from: &Path, to: &Path, ext: Option<&str>) -> Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))?.flatten() {
        let p = e.path();
        if !p.is_file() || ext.is_some_and(|x| p.extension().is_none_or(|pe| pe != x)) {
            continue;
        }
        std::fs::copy(&p, to.join(e.file_name()))?;
        n += 1;
    }
    Ok(n)
}

/// Copy `<git ref>:web/viewer/*` into `to` (git ls-tree + git show, bytes as committed).
pub fn viewer_from_ref(repo: &Path, r: &str, to: &Path) -> Result<usize> {
    let git = |args: &[&str]| -> Result<Vec<u8>> {
        let o = std::process::Command::new("git").arg("-C").arg(repo).args(args).output()?;
        if !o.status.success() {
            bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
        }
        Ok(o.stdout)
    };
    let list = String::from_utf8(git(&["ls-tree", "-r", "--name-only", r, "web/viewer/"])?)?;
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for path in list.lines().filter(|l| !l.is_empty()) {
        let name = path.rsplit('/').next().unwrap_or(path);
        std::fs::write(to.join(name), git(&["show", &format!("{r}:{path}")])?)?;
        n += 1;
    }
    if !to.join("mcview.js").exists() {
        bail!("{r}: no web/viewer/mcview.js");
    }
    Ok(n)
}

pub fn build(a: &BuildArgs) -> Result<()> {
    let repo = PathBuf::from(".");
    if !repo.join(doors::ARCHIVE).is_dir() || !repo.join(clocks::ARCHIVE).is_dir() {
        bail!("run from the repository root (no {} or {} here)", doors::ARCHIVE, clocks::ARCHIVE);
    }
    let t0 = std::time::Instant::now();
    fresh_dir(&a.out)?;
    let out = &a.out;
    // the doors
    let release_assets = if a.release.is_empty() { Default::default() } else { piston_door::showcase::release_assets(&a.release) };
    if !a.release.is_empty() && release_assets.is_empty() {
        eprintln!("warning: no assets in release {} (gh): no video links", a.release);
    }
    let (door_builds, uncached) = doors::export(&repo, &doors::DoorOptions { release_assets, release_tag: a.release.clone() })?;
    if !uncached.is_empty() {
        eprintln!("{} archived doors have no cached verdict from main's judge yet (left out): {}", uncached.len(), uncached.join(", "));
    }
    copy_files(&repo.join(doors::ARCHIVE), &out.join("designs/doors"), Some("txt"))?;
    eprintln!("doors: {} builds ({:.1}s)", door_builds.len(), t0.elapsed().as_secs_f64());
    // the clocks
    let members_dir = (!a.no_members).then(|| out.join("designs/clocks/members"));
    let ce = clocks::export(&repo, &clocks::ClockOptions { members_dir, sim_horizon: a.sim_horizon, threads: a.threads })?;
    copy_files(&repo.join(clocks::ARCHIVE), &out.join("designs/clocks"), Some("txt"))?;
    eprintln!(
        "clocks: {} builds, {} extendable cards, {} members ({} judged, {} simulated, {} formula) ({:.1}s)",
        ce.builds.len(),
        ce.cards.len(),
        ce.members.len(),
        ce.members.iter().filter(|m| m.status == site_model::STATUS_JUDGED).count(),
        ce.members.iter().filter(|m| m.status == site_model::STATUS_REDSIM).count(),
        ce.members.iter().filter(|m| m.status == site_model::STATUS_FORMULA).count(),
        t0.elapsed().as_secs_f64()
    );
    for n in &ce.notes {
        eprintln!("  note: {n}");
    }
    if repo.join(clocks::TRACES).is_dir() {
        let n = copy_files(&repo.join(clocks::TRACES), &out.join("designs/clocks/traces"), Some("trace"))?;
        eprintln!("clock traces: {n} real-game recordings");
    }
    // the circuits
    let (circuit_builds, cnotes) = circuits::export(&repo)?;
    for b in &circuit_builds {
        let to = out.join(&b.design);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(repo.join(circuits::repo_path(b)), &to).with_context(|| format!("copying {}", circuits::repo_path(b)))?;
    }
    eprintln!("circuits: {} builds ({:.1}s)", circuit_builds.len(), t0.elapsed().as_secs_f64());
    for n in &cnotes {
        eprintln!("  note: {n}");
    }
    let mut builds = door_builds;
    builds.extend(ce.builds);
    builds.extend(circuit_builds);
    let site = Site { version: DATA_VERSION, generated: clock::catalog::now_utc(), commit: commit(&repo), builds, cards: ce.cards, members: ce.members };
    // data: the door properties tables live on the build pages only (the UI never reads them)
    let mut slim = site.clone();
    for b in &mut slim.builds {
        if let Some(d) = &mut b.door {
            d.props.clear();
        }
    }
    write(&out.join("data/site.json"), &serde_json::to_string(&slim)?)?;
    drop(slim);
    // the viewer (webrender's bundle: --viewer, --viewer-ref, else the checkout's web/viewer/)
    // and the UI
    let checkout_viewer = repo.join("web/viewer");
    let viewer_dir = a.viewer.clone().or_else(|| (a.viewer_ref.is_none() && !a.no_viewer && checkout_viewer.join("mcview.js").is_file()).then(|| checkout_viewer.clone()));
    let viewer = match (&viewer_dir, &a.viewer_ref) {
        _ if a.no_viewer => false,
        (Some(dir), _) => {
            let n = copy_files(dir, &out.join("viewer"), None)?;
            if !out.join("viewer/mcview.js").exists() {
                bail!("{}: no mcview.js in the viewer bundle", dir.display());
            }
            eprintln!("viewer: {n} files from {}", dir.display());
            true
        }
        (None, Some(r)) => {
            let n = viewer_from_ref(&repo, r, &out.join("viewer"))?;
            eprintln!("viewer: {n} files from {r}:web/viewer");
            true
        }
        (None, None) => false,
    };
    let app = if a.no_wasm {
        false
    } else {
        match &a.pkg {
            Some(dir) => {
                copy_files(dir, &out.join("pkg"), None)?;
                true
            }
            None => {
                web::build_ui(&repo, &out.join("pkg"))?;
                true
            }
        }
    };
    let scripts = Scripts { viewer, app: false };
    // pages
    write(&out.join("style.css"), STYLE)?;
    write(&out.join("favicon.svg"), FAVICON)?;
    // GitHub Pages: serve every file as is (no Jekyll)
    write(&out.join(".nojekyll"), "")?;
    write(&out.join("index.html"), &pages::index_page(&site, Scripts { viewer, app }))?;
    write(&out.join("all.html"), &pages::all_page(&site, scripts))?;
    write(&out.join("boards.html"), &pages::boards_page(&site, scripts))?;
    write(&out.join("about.html"), &pages::about_page(&site, scripts))?;
    write(&out.join("404.html"), &pages::not_found_page(&site))?;
    for b in &site.builds {
        let repo_path = match b.kind.as_str() {
            "door" => format!("{}/{}.txt", doors::ARCHIVE, b.name),
            "circuit" => circuits::repo_path(b),
            _ => format!("{}/{}", clocks::ARCHIVE, b.design.rsplit('/').next().unwrap_or("")),
        };
        let text = std::fs::read_to_string(repo.join(&repo_path)).unwrap_or_default();
        write(&out.join(&b.page), &pages::build_page(&site, b, &text, &repo_path, scripts))?;
    }
    // the obvious per-kind URLs resolve (/doors/, /clocks/, /circuits/)
    for (folder, title, kind) in [("doors", "Doors", "door"), ("clocks", "Clocks", "clock"), ("circuits", "Circuits", "circuit")] {
        write(&out.join(folder).join("index.html"), &pages::redirect_page(&site, "../", title, &format!("kind={kind}")))?;
    }
    let mut subs: Vec<(String, String)> = site.builds.iter().filter_map(|b| b.page.strip_prefix("circuits/").and_then(|r| r.split_once('/')).map(|(sub, _)| (sub.to_string(), b.group.clone()))).collect();
    subs.sort();
    subs.dedup_by(|a, b| a.0 == b.0);
    for (sub, _) in &subs {
        let frag = if sub == "calibration" { "kind=circuit&is=calibration".to_string() } else { format!("kind=circuit&q={sub}") };
        write(&out.join("circuits").join(sub).join("index.html"), &pages::redirect_page(&site, "../../", &format!("Circuits: {sub}"), &frag))?;
    }
    let mut by_card: BTreeMap<String, Vec<&Member>> = BTreeMap::new();
    for m in &site.members {
        by_card.entry(m.card.clone()).or_default().push(m);
    }
    for c in &site.cards {
        let ms = by_card.get(&c.id).cloned().unwrap_or_default();
        write(&out.join(&c.page), &pages::card_page(&site, c, &ms, scripts))?;
    }
    write(&out.join("families/index.html"), &pages::families_index(&site, &by_card, scripts))?;
    eprintln!(
        "wrote {}: {} builds, {} family pages, {} members, viewer: {}, UI: {} ({:.1}s)",
        out.display(),
        site.builds.len(),
        site.cards.len(),
        site.members.len(),
        if viewer { "yes" } else { "no (text cards)" },
        if app { "yes" } else { "no" },
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
