//! Videos rendered by the real Minecraft client.
//!
//! The client runs unmodified (plus a tiny launcher that hides the HUD) on a virtual X display
//! (Xvfb) with Mesa's software Vulkan/OpenGL, joins a simulator worker that listens on
//! localhost as a spectator, and ffmpeg records the display while the simulator steps the
//! contraption at a watchable tick rate. Linux only.

use crate::net::{self, Checksum};
use crate::paths;
use crate::setup::{self, ClientInstall};
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CLIENT_MAIN: &str = include_str!("../../../bridge/client/dev/vjeux/mctas/client/ClientMain.java");
const FFMPEG_URL: &str = "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";

/// Client install + our launcher class.
#[derive(Clone, Debug)]
pub struct ClientBuild {
    pub install: ClientInstall,
    pub classes: PathBuf,
}

impl ClientBuild {
    pub fn classpath(&self) -> String {
        let mut parts = vec![self.classes.display().to_string(), self.install.client_jar.display().to_string()];
        parts.extend(self.install.libs.iter().map(|p| p.display().to_string()));
        parts.join(":")
    }
}

/// Download the client and compile the launcher.
pub fn build_client(version: &str) -> Result<ClientBuild> {
    let install = setup::install_client(version)?;
    let hash = &net::sha256_hex(format!("{version}{CLIENT_MAIN}").as_bytes())[..12];
    let dir = paths::version_dir(version).join(format!("client-launcher-{hash}"));
    let classes = dir.join("classes");
    let build = ClientBuild { install, classes: classes.clone() };
    if dir.join(".built").exists() {
        return Ok(build);
    }
    let src = dir.join("src/dev/vjeux/mctas/client/ClientMain.java");
    std::fs::create_dir_all(src.parent().unwrap())?;
    std::fs::write(&src, CLIENT_MAIN)?;
    std::fs::create_dir_all(&classes)?;
    let mut cp = vec![build.install.client_jar.display().to_string()];
    cp.extend(build.install.libs.iter().map(|p| p.display().to_string()));
    let major = setup::required_java(version)?;
    let out = Command::new(setup::javac_for(&build.install.java))
        .args(["--release", &major.to_string(), "-nowarn", "-cp", &cp.join(":"), "-d"])
        .arg(&classes)
        .arg(&src)
        .output()?;
    if !out.status.success() {
        bail!("javac (client launcher) failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    std::fs::write(dir.join(".built"), b"ok")?;
    Ok(build)
}

/// A static ffmpeg build (downloaded once).
pub fn ffmpeg() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("MCTAS_FFMPEG") {
        return Ok(PathBuf::from(p));
    }
    let dir = paths::downloads();
    let find = || -> Option<PathBuf> {
        std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.path().join("ffmpeg"))
            .find(|p| p.exists() && p.parent().is_some_and(|d| d.to_string_lossy().contains("static")))
    };
    if let Some(p) = find() {
        return Ok(p);
    }
    let tarball = dir.join("ffmpeg-static.tar.xz");
    net::download(FFMPEG_URL, &tarball, Checksum::None)?;
    let status = Command::new("tar").arg("-xf").arg(&tarball).arg("-C").arg(&dir).status()?;
    if !status.success() {
        bail!("extracting ffmpeg failed");
    }
    find().context("ffmpeg not found after extraction")
}

/// A virtual X display.
pub struct Display {
    child: Child,
    pub number: u32,
}

impl Display {
    pub fn start(width: u32, height: u32) -> Result<Display> {
        for number in 90..200u32 {
            let lock = PathBuf::from(format!("/tmp/.X{number}-lock"));
            let socket = PathBuf::from(format!("/tmp/.X11-unix/X{number}"));
            if lock.exists() || socket.exists() {
                continue;
            }
            let child = Command::new("Xvfb")
                .arg(format!(":{number}"))
                .args(["-screen", "0", &format!("{width}x{height}x24"), "-nolisten", "tcp", "+extension", "GLX"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("starting Xvfb (install xorg-x11-server-Xvfb)")?;
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                if socket.exists() {
                    return Ok(Display { child, number });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            bail!("Xvfb :{number} did not come up");
        }
        bail!("no free X display number")
    }

    pub fn name(&self) -> String {
        format!(":{}", self.number)
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The real game client, joining `localhost:port`.
pub struct Client {
    child: Child,
    pub log: PathBuf,
}

impl Client {
    pub fn launch(build: &ClientBuild, display: &Display, port: u16, width: u32, height: u32, game_dir: &Path) -> Result<Client> {
        std::fs::create_dir_all(game_dir)?;
        // First-launch screens and distractions off; small render distance (the scene is small).
        std::fs::write(
            game_dir.join("options.txt"),
            "onboardAccessibility:false\nskipMultiplayerWarning:true\njoinedFirstServer:true\ntutorialStep:none\n\
             pauseOnLostFocus:false\nrenderDistance:6\nsimulationDistance:5\nfov:0.0\nnarrator:0\nsoundCategory_master:0.0\n\
             guiScale:2\nfullscreen:false\nbobView:false\nrenderClouds:\"false\"\nparticles:2\nentityShadows:true\n",
        )?;
        let natives = game_dir.join("natives");
        std::fs::create_dir_all(&natives)?;
        let log = game_dir.join("client.log");
        let child = Command::new(&build.install.java)
            .arg("-Xmx3g")
            .arg("--enable-native-access=ALL-UNNAMED")
            .arg("--sun-misc-unsafe-memory-access=allow")
            .arg(format!("-Djava.library.path={}", natives.display()))
            .arg(format!("-Dorg.lwjgl.system.SharedLibraryExtractPath={}", natives.display()))
            .arg("-cp")
            .arg(build.classpath())
            .arg("dev.vjeux.mctas.client.ClientMain")
            .args(["--username", "mctas", "--version", &build.install.version])
            .arg("--gameDir")
            .arg(game_dir)
            .arg("--assetsDir")
            .arg(&build.install.assets_dir)
            .args(["--assetIndex", &build.install.asset_index])
            .args(["--uuid", "8d2b3e4f-0000-4000-8000-00000000c0de", "--accessToken", "0", "--versionType", "release"])
            .args(["--width", &width.to_string(), "--height", &height.to_string()])
            .args(["--quickPlayMultiplayer", &format!("127.0.0.1:{port}")])
            .env("DISPLAY", display.name())
            .current_dir(game_dir)
            .stdout(std::fs::File::create(&log)?)
            .stderr(Stdio::from(std::fs::File::options().append(true).open(&log)?))
            .spawn()
            .context("starting the Minecraft client")?;
        Ok(Client { child, log })
    }

    pub fn alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// ffmpeg grabbing the display into a lossless intermediate file.
pub struct Recorder {
    child: Child,
    pub raw: PathBuf,
}

impl Recorder {
    pub fn start(display: &Display, width: u32, height: u32, fps: u32, raw: &Path) -> Result<Recorder> {
        let child = Command::new(ffmpeg()?)
            .args(["-loglevel", "error", "-y", "-f", "x11grab", "-draw_mouse", "0"])
            .args(["-framerate", &fps.to_string(), "-video_size", &format!("{width}x{height}")])
            .args(["-i", &display.name()])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-qp", "0"])
            .arg(raw)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .context("starting ffmpeg")?;
        Ok(Recorder { child, raw: raw.to_path_buf() })
    }

    /// Ask ffmpeg to finish the file ('q' on stdin) and wait for it.
    pub fn stop(mut self) -> Result<PathBuf> {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q");
        }
        let _ = self.child.wait();
        Ok(self.raw.clone())
    }
}

/// Encode a browser-playable H.264 MP4 (optionally sped up by `speed`).
pub fn encode(raw: &Path, out: &Path, speed: f64) -> Result<()> {
    let mut cmd = Command::new(ffmpeg()?);
    cmd.args(["-loglevel", "error", "-y", "-i"]).arg(raw);
    if (speed - 1.0).abs() > 1e-6 {
        cmd.args(["-filter:v", &format!("setpts=PTS/{speed}")]);
    }
    let status = cmd
        .args(["-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(out)
        .status()?;
    if !status.success() {
        bail!("ffmpeg encode failed");
    }
    Ok(())
}

/// One frame of the display as PNG (for checks and thumbnails).
pub fn screenshot(display: &Display, width: u32, height: u32, out: &Path) -> Result<()> {
    let status = Command::new(ffmpeg()?)
        .args(["-loglevel", "error", "-y", "-f", "x11grab", "-video_size", &format!("{width}x{height}")])
        .args(["-i", &display.name(), "-frames:v", "1"])
        .arg(out)
        .status()?;
    if !status.success() {
        bail!("screenshot failed");
    }
    Ok(())
}

/// Yaw/pitch (Minecraft degrees) to look from `eye` at `target`.
pub fn look_at(eye: [f64; 3], target: [f64; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]);
    let yaw = (-dx.atan2(dz)).to_degrees();
    let pitch = (-dy.atan2((dx * dx + dz * dz).sqrt())).to_degrees();
    (yaw as f32, pitch as f32)
}

/// Upload (or replace) a file as an asset of a GitHub release, creating the release if needed.
/// Uses the `gh` CLI. Returns the asset's download URL.
pub fn upload_release_asset(repo: &str, tag: &str, file: &Path) -> Result<String> {
    let view = Command::new("gh").args(["release", "view", tag, "--repo", repo]).output()?;
    if !view.status.success() {
        let status = Command::new("gh")
            .args(["release", "create", tag, "--repo", repo, "--title", tag, "--notes", "Videos recorded with mctas (real Minecraft client)."])
            .status()?;
        if !status.success() {
            bail!("gh release create failed");
        }
    }
    let status = Command::new("gh").args(["release", "upload", tag, "--repo", repo, "--clobber"]).arg(file).status()?;
    if !status.success() {
        bail!("gh release upload failed");
    }
    let name = file.file_name().unwrap().to_string_lossy();
    Ok(format!("https://github.com/{repo}/releases/download/{tag}/{name}"))
}
