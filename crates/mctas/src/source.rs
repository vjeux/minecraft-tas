//! Reading the game's source code.
//!
//! Minecraft ships unobfuscated since 26.1, so decompiling the server jar with Vineflower gives
//! real class, method, field and parameter names. The decompiled tree lives in
//! `$MCTAS_HOME/versions/<v>/src` (never commit it). [`grep`] and [`find`] search it.

use crate::paths;
use crate::setup;
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Decompile `version` unless it already is (or `force`). Returns the source root.
pub fn decompile(version: &str, force: bool) -> Result<PathBuf> {
    decompile_jar(version, force, false)
}

/// The client jar's source (`$MCTAS_HOME/versions/<v>/src-client`: models, renderers, the
/// screen), from the client jar already downloaded (`versions/<v>/client.jar`: `mctas door
/// video` or `mctas web reference` fetch it), with the server's libraries for types.
pub fn decompile_client(version: &str, force: bool) -> Result<PathBuf> {
    decompile_jar(version, force, true)
}

fn decompile_jar(version: &str, force: bool, client: bool) -> Result<PathBuf> {
    let root = if client { paths::version_dir(version).join("src-client") } else { paths::source_dir(version) };
    let stamp = root.join(".mctas-decompiled");
    if stamp.exists() && !force {
        return Ok(root);
    }
    let install = setup::install_server(version)?;
    let jar = if client {
        let j = paths::version_dir(version).join("client.jar");
        if !j.exists() {
            bail!("no client jar at {} (mctas web reference downloads it)", j.display());
        }
        j
    } else {
        install.server_jar.clone()
    };
    let vineflower = setup::vineflower()?;
    let tmp = paths::version_dir(version).join(if client { "src-client.tmp" } else { "src.tmp" });
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let mut libs: Vec<String> = std::fs::read_dir(&install.libs_dir)?
        .flatten()
        .map(|e| e.path().display().to_string())
        .filter(|p| p.ends_with(".jar"))
        .collect();
    if client {
        // the server jar resolves the shared classes' types
        libs.push(install.server_jar.display().to_string());
    }
    libs.sort();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(64);
    eprintln!("decompiling Minecraft {version}{} with Vineflower ({threads} threads)", if client { " (client)" } else { "" });
    // Vineflower stops parsing value options after --silent: keep them first.
    let status = Command::new(&install.java)
        .arg("-Xmx16g")
        .arg("-jar")
        .arg(&vineflower)
        .arg("--indent-string=    ")
        .arg(format!("--thread-count={threads}"))
        .arg(format!("-e={}", libs.join(",")))
        .arg("--silent")
        .arg("--folder")
        .arg(&jar)
        .arg(&tmp)
        .status()
        .context("running Vineflower")?;
    if !status.success() {
        bail!("Vineflower failed ({status})");
    }
    let _ = std::fs::remove_dir_all(&root);
    std::fs::rename(&tmp, &root)?;
    std::fs::write(&stamp, version)?;
    Ok(root)
}

/// All `.java` files under the source root, sorted.
pub fn java_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "java") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// One matching line.
#[derive(Clone, Debug)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

/// Regex search over the decompiled sources, files filtered by a path substring. Parallel.
pub fn grep(root: &Path, pattern: &Regex, path_filter: Option<&str>, max_hits: usize) -> Result<Vec<Hit>> {
    let files: Vec<PathBuf> = java_files(root)?
        .into_iter()
        .filter(|p| path_filter.is_none_or(|f| p.to_string_lossy().contains(f)))
        .collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(32);
    let next = Mutex::new(0usize);
    let hits = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        let i = *n;
                        *n += 1;
                        i
                    };
                    let Some(path) = files.get(i) else { break };
                    let Ok(text) = std::fs::read_to_string(path) else { continue };
                    let mut local = Vec::new();
                    for (n, line) in text.lines().enumerate() {
                        if pattern.is_match(line) {
                            local.push(Hit { path: path.clone(), line: n + 1, text: line.to_string() });
                        }
                    }
                    if !local.is_empty() {
                        hits.lock().unwrap().extend(local);
                    }
                }
            });
        }
    });
    let mut hits = hits.into_inner().unwrap();
    hits.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    hits.truncate(max_hits);
    Ok(hits)
}

/// Source files for a class: `PistonBaseBlock`, `piston.PistonBaseBlock` or
/// `net.minecraft.world.level.block.piston.PistonBaseBlock` (inner classes live in their
/// outer class's file).
pub fn find(root: &Path, name: &str) -> Result<Vec<PathBuf>> {
    let name = name.trim_end_matches(".java");
    let outer = name.split('$').next().unwrap_or(name);
    let suffix = format!("{}.java", outer.replace('.', "/"));
    Ok(java_files(root)?
        .into_iter()
        .filter(|p| {
            let s = p.to_string_lossy();
            s.ends_with(&format!("/{suffix}")) || s == suffix
        })
        .collect())
}
