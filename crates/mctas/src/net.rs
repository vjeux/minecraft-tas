//! Downloads. HTTP goes through the system `curl` so corporate proxies and custom CA bundles
//! keep working without configuring a TLS stack here.

use anyhow::{Context, Result, bail};
use sha1::Digest as _;
use std::path::Path;
use std::process::Command;

/// Fetch a URL into memory.
pub fn get(url: &str) -> Result<Vec<u8>> {
    let out = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--max-time", "600", url])
        .output()
        .context("running curl (is it installed?)")?;
    if !out.status.success() {
        bail!("curl {url} failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(out.stdout)
}

/// Fetch a URL and parse it as JSON.
pub fn get_json(url: &str) -> Result<serde_json::Value> {
    let bytes = get(url)?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing JSON from {url}"))
}

/// Expected checksum of a download.
#[derive(Clone, Copy, Debug)]
pub enum Checksum<'a> {
    Sha1(&'a str),
    Sha256(&'a str),
    None,
}

/// Download `url` to `dest` (atomically, via a `.part` file) unless `dest` already exists with
/// the right checksum.
pub fn download(url: &str, dest: &Path, checksum: Checksum) -> Result<()> {
    if dest.exists() && verify(dest, checksum).unwrap_or(false) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("part");
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "--max-time", "1800", "-o"])
        .arg(&part)
        .arg(url)
        .status()
        .context("running curl (is it installed?)")?;
    if !status.success() {
        bail!("download of {url} failed ({status})");
    }
    if !verify(&part, checksum)? {
        bail!("checksum mismatch for {url}");
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// Check a file against a checksum (`Checksum::None` always passes).
pub fn verify(path: &Path, checksum: Checksum) -> Result<bool> {
    let data = std::fs::read(path)?;
    Ok(match checksum {
        Checksum::Sha1(expected) => hex(&sha1::Sha1::digest(&data)) == expected.to_ascii_lowercase(),
        Checksum::Sha256(expected) => hex(&sha2::Sha256::digest(&data)) == expected.to_ascii_lowercase(),
        Checksum::None => true,
    })
}

/// Lowercase hex of a digest.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 of a byte slice, hex encoded.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sha2::Sha256::digest(bytes))
}
