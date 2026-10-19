//! `door fill`: region filling on redsim.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct FillArgs {
    design: PathBuf,
    /// Region cells, door coordinates `x,y;x,y` (ranges `x0..x1,y` allowed).
    #[arg(long, default_value = "")]
    cells: String,
    /// Cells always filled, `x,y=state|state;...` (shorthands as --palette); not counted in --k.
    #[arg(long, default_value = "")]
    fix: String,
    /// Per-cell palettes, `x,y=state|state;...` (shorthands as --palette).
    #[arg(long, default_value = "")]
    cell_palette: String,
    #[arg(long, default_value_t = 3)]
    k: usize,
    #[arg(long, default_value_t = 0)]
    kmin: usize,
    /// Comma list of states or shorthands: stone, glass, obsidian, wool, obs, obsh, obsv, rep,
    /// rep1..rep4, rep12, dust, note, lamp, target, torch, wtorch, cmp, bulb, trap, dropper,
    /// sculk, calib, redblock, sticky / piston (retracted, 4 facings; stickyh, stickyv, pistonh,
    /// pistonv: horizontal / vertical only).
    #[arg(long, default_value = "stone,obs,rep,dust,note,lamp,target,torch,wtorch")]
    palette: String,
    /// Fillings may be several pieces, each touching the build.
    #[arg(long)]
    gaps: bool,
    #[arg(long, default_value_t = 48)]
    threads: usize,
    #[arg(long, default_value_t = 2)]
    screen_cycles: u32,
    /// Extra judge positions of the final evaluation (28 = the record judge).
    #[arg(long, default_value_t = 28)]
    extra: u32,
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/shrink/finds")]
    out_dir: PathBuf,
    #[arg(long, default_value_t = 50)]
    max_finds: usize,
    /// Accept finds with at most this many blocks placed (0: any).
    #[arg(long, default_value_t = 0)]
    max_blocks: usize,
    /// Accept finds with at most this community volume (0: any).
    #[arg(long, default_value_t = 0)]
    max_volume: i32,
    /// Only count the fillings.
    #[arg(long)]
    count: bool,
    /// Keep only fillings that use a state containing one of these substrings (comma list,
    /// e.g. `sculk_sensor`: sculk variants of a solid door).
    #[arg(long, default_value = "")]
    require_any: String,
    #[arg(long, default_value_t = 30)]
    report: u64,
    /// Accept doors that pass the judge without FULL seamless / hallway / frame 12/12 too.
    #[arg(long)]
    any_seamless: bool,
    /// Region cells may hold (wiring) pistons; put piston states in the palette to re-grow them.
    #[arg(long)]
    allow_pistons: bool,
    /// Accept finds that are not rotation-safe (by default each find must also pass the judge
    /// turned 90/180/270 degrees, on redsim).
    #[arg(long)]
    any_rotation: bool,
    /// Only run the redsim rotation check on these designs and exit (calibration).
    #[arg(long)]
    rotation_only: bool,
    /// KEEP mode: region cells keep their blocks unless a filling changes them; --k counts the
    /// CHANGED cells (another palette state, or air where a block was): `ideal-rs --vary` with a
    /// `keep` alternative and at most k cells off it, on the real door.
    #[arg(long)]
    keep: bool,
}

pub fn cmd_fill(args: FillArgs) -> Result<()> {
    let FillArgs { design, cells, fix, cell_palette, k, kmin, palette, gaps, threads, screen_cycles, extra, out_dir, max_finds, max_blocks, max_volume, count, require_any, report, any_seamless, allow_pistons, any_rotation, rotation_only, keep } = args;
    if rotation_only {
        let (_, door) = crate::cosearch::load_door(&design)?;
        let opts = crate::cosearch::JudgeOpts::with_extra(extra);
        match crate::fill::rotation_check_rs(&door, &opts) {
            Ok(()) => println!("{}: rotation-safe on redsim (turned 90, 180, 270 degrees, {} positions each)", design.display(), opts.offsets.len()),
            Err(e) => println!("{}: NOT rotation-safe on redsim: {e}", design.display()),
        }
        return Ok(());
    }
    let o = crate::fill::FillOpts {
        design,
        cells: crate::fill::parse_cells(&cells)?,
        fixed: crate::fill::parse_cell_states(&fix)?,
        cell_palette: crate::fill::parse_cell_states(&cell_palette)?,
        k,
        kmin,
        palette: crate::fill::split_top(&palette, ','),
        gaps,
        threads,
        screen_cycles,
        extra,
        out_dir,
        max_finds,
        max_blocks,
        max_volume,
        count,
        report,
        any_seamless,
        allow_pistons,
        any_rotation,
        require_any: require_any.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        keep,
    };
    crate::fill::run(&o)
}
