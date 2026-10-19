//! Shared pieces of the `mctas door ...` command line: the search knobs of the search commands
//! ([`SearchArgs`]) and the design loader. The commands themselves live in the door CLI crates
//! (crates/door-*, one per area), flattened into `mctas door` by crates/mctas-cli/src/door.rs.

use crate::VerifyOptions;
use anyhow::Result;
use mctas::design::Design;
use mctas::sim::WorkerConfig;
use std::path::PathBuf;

/// Local-search knobs shared by `minimize` and `harvest` (see search.rs).
#[derive(clap::Args, Clone)]
pub struct SearchArgs {
    /// Worker JVMs (capped by MCTAS_MAX_WORKERS).
    #[arg(long, default_value_t = 32)]
    pub workers: usize,
    /// Move families: a comma list of singles,pairs,triples,delays,moves,subs,reroute,renet, or all.
    #[arg(long, default_value = "singles")]
    pub moves: String,
    /// Acceptance: the judge's four positions plus this many (as `door verify --extra-offsets`).
    #[arg(long, default_value_t = 28)]
    pub extra_offsets: u32,
    /// Screen candidates at the first N judge positions before judging them.
    #[arg(long, default_value_t = 2)]
    pub screen_offsets: usize,
    /// Open/close cycles per screening position.
    #[arg(long, default_value_t = 1)]
    pub screen_cycles: u32,
    /// Settle cap (ticks) while screening.
    #[arg(long, default_value_t = 200)]
    pub screen_ticks: u32,
    /// Neighborhood radius of compounds (neutral edit + one removal) and triples.
    #[arg(long, default_value_t = 2)]
    pub radius: i32,
    /// When nothing improves, try every verified neutral edit followed by removals and re-routes.
    #[arg(long)]
    pub plateau: bool,
    /// With --plateau, also try pair removals after each verified neutral edit.
    #[arg(long)]
    pub plateau_pairs: bool,
    /// Plateau depth (2 implies --plateau): also a second neutral edit after each first.
    #[arg(long, default_value_t = 1)]
    pub depth: usize,
    /// Random-walk steps over verified neutral designs without improving (implies --plateau).
    #[arg(long, default_value_t = 0)]
    pub walk: usize,
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// What to minimize: blocks, or worst-cycle ticks then blocks: open, close, total (open+close).
    #[arg(long, default_value = "blocks")]
    pub objective: String,
    /// Speed objectives: allow up to this many blocks more than the start design.
    #[arg(long, default_value_t = 0)]
    pub slack: usize,
    /// Screen with the judge's own code (per-tick frames) instead of the faster frame-less copy.
    #[arg(long)]
    pub screen_frames: bool,
    /// Speed boards: only accept designs that open within this many ticks (every cycle).
    #[arg(long)]
    pub max_open: Option<u32>,
    /// Speed boards: only accept designs that close within this many ticks (every cycle).
    #[arg(long)]
    pub max_close: Option<u32>,
    /// Also accept candidates that add a sculk sensor to a solid design (category sculk): put
    /// `minecraft:sculk_sensor` (or `calibrated_sculk_sensor[facing=..]`) in the states.
    #[arg(long)]
    pub to_sculk: bool,
    /// Shape boards: only accept designs within these shape limits relative to the door: a
    /// preset (wall, wall2, flat, ground, no-above, narrow1, narrow2, wall-ground) or a comma list
    /// of key<=n (front behind thick below above left right side volume).
    #[arg(long)]
    pub shape: Option<String>,
    /// Judge every candidate as a tile module (tile.rs): wide:N, period:P:N, stack:N,
    /// vperiod:Q:N or step:dx,dy,dz:N (copies of the design, one lever, every opening checked).
    #[arg(long)]
    pub tile: Option<String>,
    /// Accept only rotation-safe designs (the judge also passes turned 90, 180 and 270 degrees;
    /// the boards rank rotation-safe designs). The start design must be rotation-safe too.
    #[arg(long)]
    pub rotation_safe: bool,
    /// Accept only designs at least this seamless (quart, semi, full, supercov, super, optionally
    /// `+hall`: the coordinator's hallway must build too; `door properties`' definition, one
    /// recorded cycle at the design's own position).
    #[arg(long)]
    pub seamless: Option<String>,
    /// Also accept candidates that add entities to a solid design (category entity, the "Entities
    /// allowed" board): put `cart`, `chestcart` or `stand` in `door grow --states`.
    #[arg(long)]
    pub to_entity: bool,
    /// Accept candidates that are not rotation-safe. By default a rotation-safe start design
    /// only accepts rotation-safe candidates (the boards rank rotation-safe doors).
    #[arg(long)]
    pub any_rotation: bool,
    /// With --tile: a shared end piece (see `door verify --tile-extra`).
    #[arg(long)]
    pub tile_extra: Option<PathBuf>,
    /// Record eligibility (Squid regulation 3.1.3): accept only designs that also work with
    /// repeater input (the lever's support block powered by a delay-1 repeater, both edges; the
    /// props-dyn check) at every judge position. The start design must pass it too.
    #[arg(long)]
    pub repeater_input: bool,
    /// --repeater-input plus props-dyn's hallway check, footsteps (sculk designs) and QUART
    /// SEAMLESS or better: every record-eligibility check a search can keep.
    #[arg(long)]
    pub eligible: bool,
}

impl SearchArgs {
    pub fn options(&self) -> Result<crate::search::SearchOptions> {
        let offsets = crate::search::judge_offsets(self.extra_offsets);
        let mut tile = self.tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
        if let (Some(t), Some(p)) = (tile.as_mut(), &self.tile_extra) {
            t.extra = Some(load_design(p)?);
        }
        Ok(crate::search::SearchOptions {
            families: crate::search::parse_families(&self.moves)?,
            screen: VerifyOptions {
                cycles: self.screen_cycles,
                max_ticks: self.screen_ticks,
                offsets: offsets.iter().take(self.screen_offsets.max(1)).copied().collect(),
                keep_frames: false,
                empty_walkway: true,
                tile: tile.clone(),
            },
            judge: VerifyOptions { offsets, tile, ..Default::default() },
            radius: self.radius,
            plateau: self.plateau || self.plateau_pairs || self.walk > 0 || self.depth > 1,
            plateau_pairs: self.plateau_pairs,
            depth: self.depth,
            walk: self.walk,
            deadline: None,
            seed: self.seed,
            objective: crate::search::parse_objective(&self.objective)?,
            slack: self.slack,
            screen_frames: self.screen_frames,
            max_open: self.max_open,
            max_close: self.max_close,
            to_sculk: self.to_sculk,
            shape: self.shape.as_deref().map(crate::ShapeLimits::parse).transpose()?,
            rotation_safe: self.rotation_safe,
            seamless: self.seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?,
            to_entity: self.to_entity,
            keep_rotations: !self.any_rotation,
            repeater_input: self.repeater_input || self.eligible,
            eligible: self.eligible,
        })
    }

    pub fn pool(&self, version: &str) -> Result<mctas::sim::Pool> {
        mctas::sim::Pool::spawn(self.workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })
    }
}

/// Load a design path, or a built-in design name prefixed with `@` (e.g. `@v1`).
pub fn load_design(path: &std::path::Path) -> Result<Design> {
    let s = path.to_string_lossy();
    if let Some(name) = s.strip_prefix('@') {
        return crate::designs::build(name).ok_or_else(|| anyhow::anyhow!("no built-in design {name}"));
    }
    Design::load(path)
}
