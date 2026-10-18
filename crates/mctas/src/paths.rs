//! Where things live on disk.
//!
//! Everything downloaded or generated goes under `$MCTAS_HOME` (default `~/.cache/mctas`), never
//! into the repository: Minecraft jars and decompiled sources must not be committed.

use std::path::PathBuf;

/// Root of the cache: `$MCTAS_HOME`, else `$XDG_CACHE_HOME/mctas`, else `~/.cache/mctas`.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("MCTAS_HOME") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("mctas");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".cache").join("mctas")
}

/// Downloaded archives and single files (JDK tarball, Vineflower, raw server jar).
pub fn downloads() -> PathBuf {
    home().join("dl")
}

/// Extracted Java runtimes.
pub fn jdks() -> PathBuf {
    home().join("jdk")
}

/// Per Minecraft version: server.jar, libs/, src/, bridge builds.
pub fn version_dir(version: &str) -> PathBuf {
    home().join("versions").join(version)
}

/// The unbundled server jar (game classes + data) of a version.
pub fn server_jar(version: &str) -> PathBuf {
    version_dir(version).join("server.jar")
}

/// The libraries the server needs, one jar each.
pub fn libs_dir(version: &str) -> PathBuf {
    version_dir(version).join("libs")
}

/// The decompiled sources of a version.
pub fn source_dir(version: &str) -> PathBuf {
    version_dir(version).join("src")
}

/// Scratch directories for running workers (each worker gets a fresh "universe").
pub fn run_dir() -> PathBuf {
    home().join("run")
}
