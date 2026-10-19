//! `mctas site ...`: REDSTONEPEDIA, the website of every build (docs/site.md).
//!
//! - `mctas site build --out site/` exports every build of the repository (the archived 3x3
//!   piston doors, the clocks and their extendable families, the circuit studies) with all its
//!   properties to `site/data/site.json`, writes one HTML page per build and per extendable
//!   family, copies the design files, builds the filtering UI (web/site: Rust compiled to
//!   WebAssembly with wasm-bindgen) into `site/pkg/` and copies webrender's 3D viewer bundle
//!   into `site/viewer/` when given one.
//! - `mctas site deploy --dir site/` publishes the folder as the `gh-pages` branch (GitHub
//!   Pages: https://vjeux.github.io/minecraft-tas/).

pub mod clocks;
pub mod deploy;
pub mod doors;
pub mod generate;
pub mod pages;
pub mod serve;
pub mod web;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum SiteCmd {
    /// Generate the whole site into a folder: data/site.json, one page per build and per
    /// extendable family, the design files, the filtering UI (WebAssembly) and the 3D viewer.
    Build(BuildArgs),
    /// Publish a generated folder as the gh-pages branch (GitHub Pages), one orphan commit.
    Deploy(DeployArgs),
    /// Serve a generated folder on 127.0.0.1 for a local preview.
    Serve {
        #[arg(long, default_value = "site")]
        dir: PathBuf,
        #[arg(long, default_value_t = 8000)]
        port: u16,
    },
    /// Check a generated site in a headless Chrome (`--dump-dom`): the UI loads the data, draws
    /// the cards, the time search expands families, facet links filter, build and family pages
    /// render. Exits non-zero on a failed check.
    Smoke {
        #[arg(long, default_value = "site")]
        dir: PathBuf,
        /// The headless Chrome (default: $CHROME, else ~/tools/chrome-headless-shell-linux64).
        #[arg(long)]
        chrome: Option<PathBuf>,
        /// Virtual time each page gets to run its scripts (ms).
        #[arg(long, default_value_t = 20_000)]
        budget: u32,
    },
}

#[derive(Args)]
pub struct BuildArgs {
    /// Output folder (created; an existing one must be a previous site build: it is replaced).
    #[arg(long, default_value = "site")]
    pub out: PathBuf,
    /// webrender's viewer bundle (a folder with mcview.js + mcview_bg.wasm), copied to viewer/.
    /// Without it the pages show text cards where the 3D views go.
    #[arg(long)]
    pub viewer: Option<PathBuf>,
    /// A prebuilt filtering UI (wasm-bindgen output of web/site), copied to pkg/ instead of
    /// building it.
    #[arg(long)]
    pub pkg: Option<PathBuf>,
    /// Skip the filtering UI (no WebAssembly build; the index then only links to all.html).
    #[arg(long)]
    pub no_wasm: bool,
    /// GitHub release whose `<stem>.mp4` assets are the doors' videos (`gh`); empty: no links.
    #[arg(long, default_value = "videos")]
    pub release: String,
    /// List extendable family members from their formulas only: nothing generated or simulated.
    #[arg(long)]
    pub no_members: bool,
    /// redsim horizon (ticks) for checking each generated member's period; 0: no check.
    #[arg(long, default_value_t = 50_000)]
    pub sim_horizon: u32,
    /// Threads for generating and simulating members.
    #[arg(long, default_value_t = 32)]
    pub threads: usize,
}

#[derive(Args)]
pub struct DeployArgs {
    /// The generated site folder.
    #[arg(long, default_value = "site")]
    pub dir: PathBuf,
    #[arg(long, default_value = "gh-pages")]
    pub branch: String,
    #[arg(long, default_value = "origin")]
    pub remote: String,
    /// Build the commit but do not push.
    #[arg(long)]
    pub dry_run: bool,
    /// Also turn GitHub Pages on for the branch if it is off (`gh api`).
    #[arg(long)]
    pub enable_pages: bool,
}

pub fn run(cmd: SiteCmd) -> Result<()> {
    match cmd {
        SiteCmd::Build(a) => generate::build(&a),
        SiteCmd::Deploy(a) => deploy::deploy(&a),
        SiteCmd::Serve { dir, port } => serve::serve(&dir, port),
        SiteCmd::Smoke { dir, chrome, budget } => {
            let fails = serve::smoke(&dir, chrome.as_deref(), budget)?;
            if fails > 0 {
                bail!("{fails} smoke check(s) failed");
            }
            println!("all smoke checks passed");
            Ok(())
        }
    }
}
