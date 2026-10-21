//! `mctas site build`: everything into one folder.

use crate::pages::{self, Scripts};
use crate::{BuildArgs, circuits, clocks, doors, web};
use anyhow::{Context, Result, bail};
use site_model::{Build, DATA_VERSION, Member, Site};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const STYLE: &str = include_str!("../assets/style.css");
pub const FAVICON: &str = include_str!("../assets/favicon.svg");
/// Pages vjeux deleted (2026-10-20 13:35Z: "I asked to remove the things from the header, it's
/// not to put them back in the footer. Delete these pages altogether"): the door boards, the
/// extendable families' index, the list of everything and the about page. `mctas site smoke`
/// fails if one is written or linked.
pub const DELETED_PAGES: [&str; 4] = ["boards.html", "families/index.html", "all.html", "about.html"];

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
    let n = files_from_ref(repo, r, "web/viewer/", to)?;
    if !to.join("mcview.js").exists() {
        bail!("{r}: no web/viewer/mcview.js");
    }
    Ok(n)
}

/// Copy the files of `<git ref>:<folder>` (not recursive into subfolders' names: flattened) into `to`.
pub fn files_from_ref(repo: &Path, r: &str, folder: &str, to: &Path) -> Result<usize> {
    let git = |args: &[&str]| -> Result<Vec<u8>> {
        let o = std::process::Command::new("git").arg("-C").arg(repo).args(args).output()?;
        if !o.status.success() {
            bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
        }
        Ok(o.stdout)
    };
    let list = String::from_utf8(git(&["ls-tree", "-r", "--name-only", r, folder])?)?;
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for path in list.lines().filter(|l| !l.is_empty()) {
        let name = path.rsplit('/').next().unwrap_or(path);
        std::fs::write(to.join(name), git(&["show", &format!("{r}:{path}")])?)?;
        n += 1;
    }
    if n == 0 {
        bail!("{r}: nothing under {folder}");
    }
    Ok(n)
}

/// One card per mechanism: doors (the coordinator's decision (b): "near-identical variants become
/// one card") with the same category, box and open/close times, and clocks (critique C46:
/// identical pairs on the Clocks tab) with the same period, blocks, parts and output fold under
/// one twin (a door: its `host`, listed on the twin's page; a clock: the `twin` flag, hidden from
/// the default lists like the variants, listed among the twin's same-period clocks). A record
/// (a door leading a board, a family's base) is never folded. Returns (doors, clocks) folded.
fn fold_twins(builds: &mut [Build]) -> (usize, usize) {
    let mut doors: BTreeMap<(String, String, Option<u32>, Option<u32>, String, Vec<String>), Vec<usize>> = BTreeMap::new();
    let mut clocks: BTreeMap<(u32, u32, String, u32, u32), Vec<usize>> = BTreeMap::new();
    // what a builder picks a door by besides its numbers: a door with one of these never hides
    // under one without it
    const DOOR_PROPS: [&str; 7] = ["slime-free", "flat", "rotation-safe", "hallway", "eligible", "wall", "ground"];
    for (i, b) in builds.iter().enumerate() {
        if let Some(d) = &b.door {
            if d.host.is_none() {
                let props: Vec<String> = DOOR_PROPS.iter().filter(|p| b.flags.iter().any(|f| f == *p)).map(|p| p.to_string()).collect();
                doors.entry((b.group.clone(), b.dims.clone().unwrap_or_default(), d.open, d.close, d.seamless.clone(), props)).or_default().push(i);
            }
        }
        if let Some(c) = &b.clock {
            if c.host.is_none() && !c.instance && c.card.is_none() && !b.flags.iter().any(|f| f == "census" || f == "variant") {
                clocks.entry((c.period, b.blocks, b.group.clone(), c.on, c.edges)).or_default().push(i);
            }
        }
    }
    let mut folded = (0, 0);
    for mut same in doors.into_values().filter(|v| v.len() > 1) {
        same.sort_by_key(|&i| {
            let b = &builds[i];
            (b.door.as_ref().is_none_or(|d| d.leads.is_empty()), b.blocks, b.volume, b.name.clone())
        });
        let rep = same[0];
        let rep_id = builds[rep].id.clone();
        for &i in &same[1..] {
            if builds[i].door.as_ref().is_some_and(|d| !d.leads.is_empty()) {
                continue;
            }
            let id = builds[i].id.clone();
            if let Some(d) = builds[i].door.as_mut() {
                d.host = Some(rep_id.clone());
            }
            builds[i].flags.push("variant".into());
            if let Some(d) = builds[rep].door.as_mut() {
                d.variants.push(id);
            }
            folded.0 += 1;
        }
    }
    for mut same in clocks.into_values().filter(|v| v.len() > 1) {
        same.sort_by_key(|&i| (builds[i].name.starts_with("census-"), builds[i].volume, builds[i].name.clone()));
        for &i in &same[1..] {
            builds[i].flags.push("twin".into());
            folded.1 += 1;
        }
    }
    folded
}

/// The repository path of a build's design file.
pub fn repo_design_path(b: &Build) -> String {
    match b.kind.as_str() {
        "door" => format!("{}/{}.txt", doors::ARCHIVE, b.name),
        "circuit" => circuits::repo_path(b),
        _ => format!("{}/{}", clocks::ARCHIVE, b.design.rsplit('/').next().unwrap_or("")),
    }
}

/// The site path of the `.litematic` next to a design file (`designs/doors/x.txt` ->
/// `designs/doors/x.litematic`).
pub fn litematic_path(design: &str) -> String {
    match design.strip_suffix(".txt") {
        Some(stem) => format!("{stem}.litematic"),
        None => format!("{design}.litematic"),
    }
}

/// How to use a build once pasted, for the schematic's description.
fn usage_note(b: &Build) -> String {
    match (&b.door, &b.clock, &b.circuit) {
        (Some(_), _, _) => "Flip the lever to open the door, again to close it.".into(),
        (_, Some(c), _) => match c.start.strip_prefix("kick: last ") {
            Some(cell) => format!("To start it after pasting, break and re-place the block at {cell} (from the design's origin)."),
            None if c.start.starts_with("self") => "It starts by itself once pasted.".into(),
            None => format!("Start: {}.", c.start),
        },
        // what it does and its connections, in words (critique C31: "flying-machine: inputs ,
        // outputs ." printed empty lists)
        (_, _, Some(c)) => {
            let mut s = crate::plain::one_liner(c);
            let names = |v: &[String]| v.iter().map(|n| n.to_uppercase()).collect::<Vec<_>>().join(", ");
            match (c.inputs.is_empty(), c.outputs.is_empty()) {
                (false, false) => s.push_str(&format!(" Input {}, output {}.", names(&c.inputs), names(&c.outputs))),
                (false, true) => s.push_str(&format!(" Input {}.", names(&c.inputs))),
                (true, false) => s.push_str(&format!(" Output {}.", names(&c.outputs))),
                (true, true) => {}
            }
            s
        }
        _ => String::new(),
    }
}

/// Write `<design>.litematic` next to every build's and every member's design in the site.
fn write_litematics(site: &Site, repo: &Path, out: &Path) -> Result<usize> {
    let time = litematic::now_ms();
    let mut n = 0;
    for b in &site.builds {
        let path = repo_design_path(b);
        let text = std::fs::read_to_string(repo.join(&path)).with_context(|| format!("reading {path}"))?;
        let d = mctas::design::Design::parse(&text).with_context(|| format!("parsing {path}"))?;
        let desc = [crate::explain::caption_text(&b.caption).as_str(), &usage_note(b), &format!("REDSTONEPEDIA: https://vjeux.github.io/minecraft-tas/{}", b.page)].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        let bytes = litematic::export(&d, &litematic::Meta::new(&b.name, &desc, time)).with_context(|| format!("{path}: litematic"))?;
        write_bytes(&out.join(litematic_path(&b.design)), &bytes)?;
        n += 1;
    }
    // members: the generated ones (a member that is its family's archived base design shares that
    // build's file, already written)
    let build_designs: std::collections::BTreeSet<&str> = site.builds.iter().map(|b| b.design.as_str()).collect();
    for m in site.members.iter().filter(|m| !m.design.is_empty() && !build_designs.contains(m.design.as_str())) {
        let text = std::fs::read_to_string(out.join(&m.design)).with_context(|| format!("reading {}", m.design))?;
        let d = mctas::design::Design::parse(&text).with_context(|| format!("parsing {}", m.design))?;
        let fam = site.cards.iter().find(|c| c.id == m.card).and_then(|c| c.families.iter().find(|f| f.id == m.family));
        let desc = format!(
            "{} n = {}: period {} gt ({}), {} blocks ({}). {}REDSTONEPEDIA: https://vjeux.github.io/minecraft-tas/families/{}.html",
            m.family,
            m.n,
            m.period,
            site_model::real_time(m.period),
            m.blocks,
            site_model::status_label(&m.status),
            fam.map(|f| format!("Extend: {}. ", f.rule)).unwrap_or_default(),
            m.card
        );
        let name = format!("{}-n{}", m.family, m.n);
        let bytes = litematic::export(&d, &litematic::Meta::new(&name, &desc, time)).with_context(|| format!("{}: litematic", m.design))?;
        write_bytes(&out.join(litematic_path(&m.design)), &bytes)?;
        n += 1;
    }
    Ok(n)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
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
    let (door_builds, uncached, not_ours) = doors::export(&repo, &doors::DoorOptions { release_assets, release_tag: a.release.clone() })?;
    if !uncached.is_empty() {
        eprintln!("{} archived doors have no cached verdict from main's judge yet (left out): {}", uncached.len(), uncached.join(", "));
    }
    if !not_ours.is_empty() {
        let by_prov = repo.join(doors::ARCHIVE).join(doors::PROVENANCE).exists();
        let conflicts: Vec<&String> = not_ours.iter().filter(|(_, why)| why.starts_with("CONFLICT")).map(|(s, _)| s).collect();
        let unlisted: Vec<&String> = not_ours.iter().filter(|(_, why)| why.starts_with("not listed")).map(|(s, _)| s).collect();
        eprintln!(
            "{} archived doors left out as community work ({}; only our own designs are published)",
            not_ours.len(),
            if by_prov { "by provenance.tsv + the seed rule" } else { "no provenance.tsv yet: by the seed rule (community*, c79*, closed-b-fill*, satwire-c79*, satwire-63b*, c88*, fills of community designs)" }
        );
        if !conflicts.is_empty() {
            eprintln!(
                "  WARNING: {} doors marked ours in provenance.tsv match the seed rule and stay out (e.g. {}): fix provenance.tsv or the seed rule (site/src/doors.rs COMMUNITY_PREFIXES)",
                conflicts.len(),
                conflicts.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if !unlisted.is_empty() {
            eprintln!("  WARNING: {} archived doors are not in provenance.tsv and stay out until listed: {}", unlisted.len(), unlisted.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
    }
    // (the design files are copied after the one-per-kind selection: only published builds')
    eprintln!("doors: {} builds ({:.1}s)", door_builds.len(), t0.elapsed().as_secs_f64());
    // the clocks (no extendable families, no generated members: vjeux 00:33Z, "All these
    // variations are useless, remove them")
    let ce = clocks::export(&repo, &clocks::ClockOptions { members_dir: None, sim_horizon: a.sim_horizon, threads: a.threads })?;
    eprintln!("clocks: {} builds ({:.1}s)", ce.builds.len(), t0.elapsed().as_secs_f64());
    for n in &ce.notes {
        eprintln!("  note: {n}");
    }
    if repo.join(clocks::TRACES).is_dir() {
        let n = copy_files(&repo.join(clocks::TRACES), &out.join("designs/clocks/traces"), Some("trace"))?;
        eprintln!("clock traces: {n} real-game recordings");
    }
    // the circuits
    let (circuit_builds, cnotes) = circuits::export(&repo)?;
    eprintln!("circuits: {} builds ({:.1}s)", circuit_builds.len(), t0.elapsed().as_secs_f64());
    for n in &cnotes {
        eprintln!("  note: {n}");
    }
    let mut builds = door_builds;
    builds.extend(ce.builds);
    builds.extend(circuit_builds);
    // one card per mechanism (the coordinator's decision (b), critique C46): doors and clocks
    // whose cards would read the same fold under one
    let (fd, fc) = fold_twins(&mut builds);
    eprintln!("  note: one card per mechanism: {fd} doors and {fc} clocks folded under a twin with the same numbers");
    // no extendable families: no family pages, no generated members (vjeux 00:33Z)
    let mut cards: Vec<site_model::Card> = Vec::new();
    let mut members: Vec<Member> = Vec::new();
    // (and nothing says "comes in every size" any more)
    for b in &mut builds {
        b.flags.retain(|f| f != "extendable");
    }
    // one world-best design per kind (vjeux 21:47Z): one door card with every board's record,
    // one clock card with the best clock of every period
    for n in crate::kinds::select_doors(&mut builds).into_iter().chain(crate::kinds::select_clocks(&mut builds, &mut cards, &mut members)) {
        eprintln!("  note: {n}");
    }
    // no simulator attribution in the captions (vjeux 02:01Z)
    for b in &mut builds {
        b.caption = crate::explain::scrub_attribution(&b.caption);
    }
    // only the published builds' design files (the others stay in the repository): doors,
    // clocks and circuits alike
    for b in &builds {
        let to = out.join(&b.design);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        let from = repo.join(repo_design_path(b));
        std::fs::copy(&from, &to).with_context(|| format!("copying {}", from.display()))?;
    }
    let site = Site { version: DATA_VERSION, generated: clock::catalog::now_utc(), commit: commit(&repo), builds, cards, members };
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
        match (&a.pkg, &a.pkg_ref) {
            (Some(dir), _) => {
                copy_files(dir, &out.join("pkg"), None)?;
                web::check_copied_ui(&repo, &out.join("pkg"), &dir.display().to_string())?;
                true
            }
            (None, Some(r)) => {
                let n = files_from_ref(&repo, r, "pkg/", &out.join("pkg"))?;
                web::check_copied_ui(&repo, &out.join("pkg"), &format!("{r}:pkg"))?;
                eprintln!("UI: {n} files from {r}:pkg (not rebuilt)");
                true
            }
            (None, None) => {
                web::build_ui(&repo, &out.join("pkg"))?;
                true
            }
        }
    };
    let scripts = Scripts { viewer, app: false, ui: app };
    // pages
    write(&out.join("style.css"), STYLE)?;
    write(&out.join("favicon.svg"), FAVICON)?;
    // cache-busting: the pages load every bundle, the CSS and the data by a content hash
    pages::set_versions(pages::Versions {
        css: pages::bytes_hash(STYLE.as_bytes()),
        viewer_js: pages::file_hash(&out.join("viewer/mcview.js")),
        viewer_wasm: pages::file_hash(&out.join("viewer/mcview_bg.wasm")),
        ui_js: pages::file_hash(&out.join(format!("pkg/{}.js", web::UI_NAME))),
        ui_wasm: pages::file_hash(&out.join(format!("pkg/{}_bg.wasm", web::UI_NAME))),
        data: pages::file_hash(&out.join("data/site.json")),
    });
    // GitHub Pages: serve every file as is (no Jekyll)
    write(&out.join(".nojekyll"), "")?;
    write(&out.join("index.html"), &pages::index_page(&site, Scripts { viewer, app, ui: app }))?;
    write(&out.join("404.html"), &pages::not_found_page(&site))?;
    // pages vjeux deleted (13:35Z: "Delete these pages altogether"): never written, and gone from
    // an output folder an earlier build left them in
    for gone in DELETED_PAGES {
        let _ = std::fs::remove_file(out.join(gone));
    }
    // Litematica schematics of every build and every generated family member (a design that
    // cannot be exported fails the build: nothing is dropped silently)
    let n = write_litematics(&site, &repo, out)?;
    eprintln!("litematic: {n} schematics ({:.1}s)", t0.elapsed().as_secs_f64());
    for b in &site.builds {
        let repo_path = repo_design_path(b);
        let text = std::fs::read_to_string(repo.join(&repo_path)).unwrap_or_default();
        write(&out.join(&b.page), &pages::build_page(&site, b, &text, &repo_path, scripts))?;
    }
    // the obvious per-kind URLs resolve (/doors/, /clocks/); /circuits/ is the studies' overview
    for (folder, title, kind) in [("doors", "Doors", "door"), ("clocks", "Clocks", "clock")] {
        write(&out.join(folder).join("index.html"), &pages::redirect_page(&site, "../", title, &format!("kind={kind}")))?;
    }
    write(&out.join("circuits/index.html"), &pages::circuits_index(&site, scripts))?;
    let mut subs: Vec<String> = site.builds.iter().filter_map(|b| b.page.strip_prefix("circuits/").and_then(|r| r.split_once('/')).map(|(sub, _)| sub.to_string())).collect();
    subs.sort();
    subs.dedup();
    for sub in &subs {
        write(&out.join("circuits").join(sub).join("index.html"), &pages::study_page(&site, sub, scripts))?;
    }
    let mut by_card: BTreeMap<String, Vec<&Member>> = BTreeMap::new();
    for m in &site.members {
        by_card.entry(m.card.clone()).or_default().push(m);
    }
    for c in &site.cards {
        let ms = by_card.get(&c.id).cloned().unwrap_or_default();
        write(&out.join(&c.page), &pages::card_page(&site, c, &ms, scripts))?;
    }
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
