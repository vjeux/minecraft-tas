//! Installing what a simulation needs: a Java runtime, the Minecraft server jar and its
//! libraries, and the Vineflower decompiler.

use crate::net::{self, Checksum};
use crate::paths;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const VINEFLOWER_VERSION: &str = "1.12.0";
const VINEFLOWER_SHA256: &str = "1dfcfe974395734fa467ce620661c7623d05ba83670de0529b1fbd63ff548b9d";

/// The pieces of an installed server, ready to put on a classpath.
#[derive(Clone, Debug)]
pub struct ServerInstall {
    pub version: String,
    pub java: PathBuf,
    pub server_jar: PathBuf,
    pub libs_dir: PathBuf,
}

/// The version's metadata JSON from Mojang (cached next to the jars).
pub fn version_json(version: &str) -> Result<Value> {
    let cached = paths::version_dir(version).join("version.json");
    if let Ok(bytes) = std::fs::read(&cached) {
        if let Ok(v) = serde_json::from_slice(&bytes) {
            return Ok(v);
        }
    }
    let manifest = net::get_json(VERSION_MANIFEST)?;
    let url = manifest["versions"]
        .as_array()
        .and_then(|vs| vs.iter().find(|v| v["id"] == version))
        .and_then(|v| v["url"].as_str())
        .ok_or_else(|| anyhow!("Minecraft version {version} is not in Mojang's manifest"))?
        .to_string();
    let bytes = net::get(&url)?;
    std::fs::create_dir_all(paths::version_dir(version))?;
    std::fs::write(&cached, &bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// The latest release id according to Mojang.
pub fn latest_release() -> Result<String> {
    let manifest = net::get_json(VERSION_MANIFEST)?;
    manifest["latest"]["release"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no latest release in the manifest"))
}

/// Major Java version a Minecraft version asks for.
pub fn required_java(version: &str) -> Result<u32> {
    let v = version_json(version)?;
    Ok(v["javaVersion"]["majorVersion"].as_u64().unwrap_or(21) as u32)
}

/// Download the server and unpack the "bundler" jar into `server.jar` + `libs/`.
pub fn install_server(version: &str) -> Result<ServerInstall> {
    let java = java(required_java(version)?)?;
    let server_jar = paths::server_jar(version);
    let libs_dir = paths::libs_dir(version);
    let stamp = paths::version_dir(version).join(".server-installed");
    if !stamp.exists() || !server_jar.exists() {
        let v = version_json(version)?;
        let server = &v["downloads"]["server"];
        let url = server["url"].as_str().ok_or_else(|| anyhow!("{version} has no server download"))?;
        let sha1 = server["sha1"].as_str().unwrap_or_default();
        let bundle = paths::downloads().join(format!("server-bundle-{version}.jar"));
        eprintln!("downloading Minecraft {version} server");
        net::download(url, &bundle, Checksum::Sha1(sha1))?;
        unbundle(&bundle, &server_jar, &libs_dir)?;
        std::fs::write(&stamp, b"ok")?;
    }
    Ok(ServerInstall { version: version.to_string(), java, server_jar, libs_dir })
}

/// Since 1.18 the server download is a launcher that carries the real jar and the libraries
/// inside META-INF. Extract them (checking the SHA-256 the bundle lists for each).
fn unbundle(bundle: &Path, server_jar: &Path, libs_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(bundle)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let read_entry = |zip: &mut zip::ZipArchive<std::fs::File>, name: &str| -> Result<Vec<u8>> {
        let mut entry = zip.by_name(name).with_context(|| format!("{name} missing from the server bundle"))?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        Ok(buf)
    };
    std::fs::create_dir_all(libs_dir)?;
    let versions = String::from_utf8(read_entry(&mut zip, "META-INF/versions.list")?)?;
    let libraries = String::from_utf8(read_entry(&mut zip, "META-INF/libraries.list")?)?;
    for (list, prefix, is_server) in [(&versions, "META-INF/versions/", true), (&libraries, "META-INF/libraries/", false)] {
        for line in list.lines().filter(|l| !l.trim().is_empty()) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                bail!("bad bundle list line: {line}");
            }
            let (sha256, path) = (cols[0], cols[2]);
            let bytes = read_entry(&mut zip, &format!("{prefix}{path}"))?;
            if net::sha256_hex(&bytes) != sha256 {
                bail!("checksum mismatch for {path} in the server bundle");
            }
            let dest = if is_server {
                server_jar.to_path_buf()
            } else {
                libs_dir.join(Path::new(path).file_name().unwrap())
            };
            std::fs::write(dest, bytes)?;
        }
    }
    Ok(())
}

/// A `java` binary of at least `major`: `$MCTAS_JAVA`, a JDK we installed earlier, the one on
/// `PATH`, or else a fresh Eclipse Temurin download.
pub fn java(major: u32) -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("MCTAS_JAVA") {
        return Ok(PathBuf::from(p));
    }
    if let Ok(entries) = std::fs::read_dir(paths::jdks()) {
        let mut candidates: Vec<PathBuf> = entries.flatten().map(|e| e.path().join("bin").join("java")).collect();
        candidates.sort();
        for candidate in candidates.into_iter().rev() {
            if java_major(&candidate).is_some_and(|m| m >= major) {
                return Ok(candidate);
            }
        }
    }
    if java_major(Path::new("java")).is_some_and(|m| m >= major) {
        return Ok(PathBuf::from("java"));
    }
    install_temurin(major)
}

/// Major version of a java binary (`java -version` prints e.g. `"25.0.4.1"` or `"1.8.0_504"`).
pub fn java_major(java: &Path) -> Option<u32> {
    let out = Command::new(java).arg("-version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let quoted = text.split('"').nth(1)?;
    let mut parts = quoted.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

fn install_temurin(major: u32) -> Result<PathBuf> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        other => bail!("no Temurin build known for architecture {other}"),
    };
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "mac",
        other => bail!("no Temurin build known for OS {other}"),
    };
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
    );
    let assets = net::get_json(&api)?;
    let package = &assets[0]["binary"]["package"];
    let link = package["link"].as_str().ok_or_else(|| anyhow!("no Temurin {major} for {os}/{arch}"))?;
    let sha256 = package["checksum"].as_str().unwrap_or_default();
    let tarball = paths::downloads().join(format!("temurin-{major}-{os}-{arch}.tar.gz"));
    eprintln!("downloading Java {major} (Eclipse Temurin)");
    net::download(link, &tarball, Checksum::Sha256(sha256))?;
    std::fs::create_dir_all(paths::jdks())?;
    let status = Command::new("tar").arg("-xzf").arg(&tarball).arg("-C").arg(paths::jdks()).status()?;
    if !status.success() {
        bail!("extracting {} failed", tarball.display());
    }
    let release = assets[0]["release_name"].as_str().unwrap_or_default();
    let home = paths::jdks().join(release);
    let java = if os == "mac" { home.join("Contents/Home/bin/java") } else { home.join("bin/java") };
    if java_major(&java).is_none_or(|m| m < major) {
        bail!("installed JDK at {} does not run", home.display());
    }
    Ok(java)
}

/// `javac` next to a `java` binary.
pub fn javac_for(java: &Path) -> PathBuf {
    match java.parent() {
        Some(bin) if !bin.as_os_str().is_empty() => bin.join("javac"),
        _ => PathBuf::from("javac"),
    }
}

/// The Vineflower decompiler jar (pinned version and checksum).
pub fn vineflower() -> Result<PathBuf> {
    let dest = paths::downloads().join(format!("vineflower-{VINEFLOWER_VERSION}.jar"));
    let url = format!(
        "https://github.com/Vineflower/vineflower/releases/download/{VINEFLOWER_VERSION}/vineflower-{VINEFLOWER_VERSION}.jar"
    );
    net::download(&url, &dest, Checksum::Sha256(VINEFLOWER_SHA256))?;
    Ok(dest)
}

/// What the real game client needs (for recording videos with Minecraft's own renderer).
#[derive(Clone, Debug)]
pub struct ClientInstall {
    pub version: String,
    pub java: PathBuf,
    pub client_jar: PathBuf,
    pub libs: Vec<PathBuf>,
    pub assets_dir: PathBuf,
    pub asset_index: String,
}

fn rules_allow(lib: &Value) -> bool {
    let Some(rules) = lib["rules"].as_array() else { return true };
    let os = match std::env::consts::OS {
        "macos" => "osx",
        other => other,
    };
    let mut allowed = false;
    for r in rules {
        let matches = r["os"]["name"].as_str().is_none_or(|n| n == os);
        if matches {
            allowed = r["action"] == "allow";
        }
    }
    allowed
}

/// Run `jobs` downloads on `threads` threads.
fn download_all(jobs: Vec<(String, PathBuf, String)>, threads: usize) -> Result<()> {
    let queue = std::sync::Mutex::new(jobs);
    let errors = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let job = queue.lock().unwrap().pop();
                    let Some((url, dest, sha1)) = job else { break };
                    if let Err(e) = net::download(&url, &dest, Checksum::Sha1(&sha1)) {
                        errors.lock().unwrap().push(format!("{url}: {e:#}"));
                    }
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    if !errors.is_empty() {
        bail!("{} downloads failed, first: {}", errors.len(), errors[0]);
    }
    Ok(())
}

/// Download the client jar, its libraries (natives for this OS) and the assets.
pub fn install_client(version: &str) -> Result<ClientInstall> {
    let v = version_json(version)?;
    let java = java(required_java(version)?)?;
    let dir = paths::version_dir(version);
    let client_jar = dir.join("client.jar");
    let client = &v["downloads"]["client"];
    net::download(
        client["url"].as_str().ok_or_else(|| anyhow!("no client download"))?,
        &client_jar,
        Checksum::Sha1(client["sha1"].as_str().unwrap_or_default()),
    )?;
    let mut jobs = Vec::new();
    let mut libs = Vec::new();
    for lib in v["libraries"].as_array().into_iter().flatten() {
        if !rules_allow(lib) {
            continue;
        }
        let art = &lib["downloads"]["artifact"];
        let (Some(url), Some(path)) = (art["url"].as_str(), art["path"].as_str()) else { continue };
        let dest = dir.join("client-libs").join(path);
        jobs.push((url.to_string(), dest.clone(), art["sha1"].as_str().unwrap_or_default().to_string()));
        libs.push(dest);
    }
    let assets_dir = paths::home().join("assets");
    let index = &v["assetIndex"];
    let asset_index = index["id"].as_str().unwrap_or("legacy").to_string();
    let index_path = assets_dir.join("indexes").join(format!("{asset_index}.json"));
    net::download(
        index["url"].as_str().ok_or_else(|| anyhow!("no asset index"))?,
        &index_path,
        Checksum::Sha1(index["sha1"].as_str().unwrap_or_default()),
    )?;
    let objects: Value = serde_json::from_slice(&std::fs::read(&index_path)?)?;
    for obj in objects["objects"].as_object().into_iter().flat_map(|o| o.values()) {
        let Some(hash) = obj["hash"].as_str() else { continue };
        let dest = assets_dir.join("objects").join(&hash[..2]).join(hash);
        if dest.exists() {
            continue;
        }
        let url = format!("https://resources.download.minecraft.net/{}/{hash}", &hash[..2]);
        jobs.push((url, dest, hash.to_string()));
    }
    if !jobs.is_empty() {
        eprintln!("downloading {} client files", jobs.len());
    }
    download_all(jobs, 32)?;
    Ok(ClientInstall { version: version.to_string(), java, client_jar, libs, assets_dir, asset_index })
}
