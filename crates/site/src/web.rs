//! The filtering UI: web/site (a separate cargo workspace, target wasm32-unknown-unknown),
//! compiled with cargo and bound with the wasm-bindgen CLI (`--target web`: an ES module the
//! page imports with one line; no other JavaScript).

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const UI_DIR: &str = "web/site";
pub const UI_NAME: &str = "redstonepedia";

/// A tool from PATH, else from ~/.cargo/bin.
fn tool(name: &str) -> PathBuf {
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(name);
            if p.is_file() {
                return p;
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cargo/bin").join(name)
}

/// The wasm-bindgen version web/site's Cargo.lock pins (the CLI must match it exactly).
fn locked_bindgen_version(ui: &Path) -> Option<String> {
    let lock = std::fs::read_to_string(ui.join("Cargo.lock")).ok()?;
    let mut lines = lock.lines();
    while let Some(l) = lines.next() {
        if l.trim() == "name = \"wasm-bindgen\"" {
            return lines.next()?.trim().strip_prefix("version = \"")?.strip_suffix('"').map(String::from);
        }
    }
    None
}

/// Build web/site in release mode for wasm32-unknown-unknown and bind it into `out`
/// (redstonepedia.js + redstonepedia_bg.wasm).
pub fn build_ui(repo: &Path, out: &Path) -> Result<()> {
    let ui = repo.join(UI_DIR);
    let cargo = std::env::var_os("CARGO").map(PathBuf::from).filter(|p| p.is_file()).unwrap_or_else(|| tool("cargo"));
    let st = Command::new(&cargo)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown", "--manifest-path"])
        .arg(ui.join("Cargo.toml"))
        .status()
        .with_context(|| format!("running {}", cargo.display()))?;
    if !st.success() {
        bail!("building {UI_DIR} for wasm32-unknown-unknown failed (needs `rustup target add wasm32-unknown-unknown`)");
    }
    let wasm = ui.join("target/wasm32-unknown-unknown/release").join(format!("{}.wasm", UI_NAME.replace('-', "_")));
    let bindgen = tool("wasm-bindgen");
    let want = locked_bindgen_version(&ui).unwrap_or_default();
    let have = Command::new(&bindgen).arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if !have.ends_with(&want) || want.is_empty() {
        bail!("wasm-bindgen CLI {have:?} does not match the crate ({want}): cargo install wasm-bindgen-cli --version {want} --locked");
    }
    std::fs::create_dir_all(out)?;
    let st = Command::new(&bindgen)
        .args(["--target", "web", "--no-typescript", "--out-name", UI_NAME, "--out-dir"])
        .arg(out)
        .arg(&wasm)
        .status()
        .with_context(|| format!("running {}", bindgen.display()))?;
    if !st.success() {
        bail!("wasm-bindgen failed on {}", wasm.display());
    }
    let size = std::fs::metadata(out.join(format!("{UI_NAME}_bg.wasm"))).map(|m| m.len()).unwrap_or(0);
    eprintln!("UI: {} ({} KB of WebAssembly)", out.display(), size / 1024);
    Ok(())
}
