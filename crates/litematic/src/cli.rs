//! `mctas litematic ...`: Litematica schematics of designs (docs/litematic.md).

use crate::{Meta, compare, design_comments, export, now_ms, read, region_design};
use anyhow::{Context, Result, bail};
use clap::Subcommand;
use mctas::design::Design;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum LitematicCmd {
    /// Write the .litematic of each design (Litematica's format for Minecraft 26.3): one region
    /// with every block, the `nbt:` block entities and the `entity:` entities.
    Export {
        designs: Vec<PathBuf>,
        /// Output: a `.litematic` file (one design) or a folder (default: next to each design).
        #[arg(long, short)]
        out: Option<PathBuf>,
        /// Description (default: the design's own comment lines).
        #[arg(long)]
        description: Option<String>,
        /// Check each file by reading it back and comparing it with the design.
        #[arg(long)]
        check: bool,
    },
    /// Read a .litematic: its metadata and regions (`--design`: each region as a design text;
    /// `--dump`: the whole NBT tree as SNBT).
    Read {
        file: PathBuf,
        #[arg(long)]
        design: bool,
        #[arg(long)]
        dump: bool,
    },
    /// Round trip: export each design in memory, read it back, compare block for block (states,
    /// block entities, entities, sizes, counts). Exits 1 on any difference.
    Check {
        designs: Vec<PathBuf>,
        /// Only the failures and the summary.
        #[arg(long, short)]
        quiet: bool,
    },
    /// In the REAL client: Fabric + MaLiLib + Litematica (from Modrinth) + the repository's probe
    /// mod paste each design's .litematic with Litematica's own code (placement + paste, as a
    /// creative operator on the bridge's server), then the world is compared with the design
    /// block for block, block entities and entities. Designs: files or folders. Uses an X display
    /// (Xvfb). Exits 1 on any difference.
    VerifyIngame {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = mctas::DEFAULT_VERSION)]
        version: String,
        /// Seconds to wait for one paste.
        #[arg(long, default_value_t = 180)]
        timeout: u64,
        /// Litematica's pasteNbtRestoreBehavior: none (its default; on a server block entities
        /// paste empty), place_data_modify, place_clone.
        #[arg(long, default_value = "place_data_modify")]
        nbt: String,
        /// Where Litematica pastes: server (the bridge's server: its command paste), single (a
        /// singleplayer world: its direct paste) or both.
        #[arg(long, default_value = "both")]
        world: String,
        /// Litematica's deduplicateSchematicEntities (Litematica's default is false: entities may
        /// be summoned twice on a server).
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        dedupe: bool,
    },
}

fn load(p: &Path) -> Result<(Design, String)> {
    let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
    let d = Design::parse(&text).with_context(|| format!("parsing {}", p.display()))?;
    Ok((d, text))
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "design".into())
}

pub fn run(cmd: LitematicCmd) -> Result<()> {
    match cmd {
        LitematicCmd::Export { designs, out, description, check } => {
            if designs.is_empty() {
                bail!("no designs given");
            }
            let one_file = out.as_ref().is_some_and(|o| o.extension().is_some_and(|e| e == "litematic"));
            if one_file && designs.len() > 1 {
                bail!("--out names one .litematic file but {} designs are given: pass a folder", designs.len());
            }
            for p in &designs {
                let (d, text) = load(p)?;
                let name = if d.name.is_empty() { stem(p) } else { d.name.clone() };
                let meta = Meta::new(&name, &description.clone().unwrap_or_else(|| design_comments(&text)), now_ms());
                let bytes = export(&d, &meta)?;
                if check {
                    compare(&d, &read(&bytes)?).with_context(|| format!("{}: read back", p.display()))?;
                }
                let dest = match &out {
                    Some(o) if one_file => o.clone(),
                    Some(dir) => {
                        std::fs::create_dir_all(dir)?;
                        dir.join(format!("{}.litematic", stem(p)))
                    }
                    None => p.with_extension("litematic"),
                };
                std::fs::write(&dest, &bytes).with_context(|| format!("writing {}", dest.display()))?;
                println!("{} ({} bytes{})", dest.display(), bytes.len(), if check { ", read back OK" } else { "" });
            }
        }
        LitematicCmd::Read { file, design, dump } => {
            let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let s = read(&bytes)?;
            if dump {
                println!("{}", s.root.to_snbt());
                return Ok(());
            }
            println!(
                "{}: \"{}\" by {}, Version {}{}, MinecraftDataVersion {}, enclosing {:?}, {:?} blocks in {:?} cells",
                file.display(),
                s.name,
                s.author,
                s.version,
                s.sub_version.map(|v| format!(".{v}")).unwrap_or_default(),
                s.data_version,
                s.enclosing,
                s.total_blocks,
                s.total_volume
            );
            if !s.description.is_empty() {
                println!("  description: {}", s.description);
            }
            for r in &s.regions {
                println!(
                    "  region {:?} at {:?} size {:?}: palette {} ({} bits), {} blocks, {} tile entities, {} entities; BlockStates re-packed: {}",
                    r.name,
                    r.position,
                    r.size,
                    r.palette.len(),
                    r.bits,
                    r.blocks.len(),
                    r.tiles.len(),
                    r.entities.len(),
                    if r.repacks_identically { "identical" } else { "DIFFERENT" }
                );
                if design {
                    print!("{}", region_design(r, &r.name).to_text());
                }
            }
        }
        LitematicCmd::VerifyIngame { designs, version, timeout, nbt, world, dedupe } => {
            use crate::ingame::World;
            let worlds = match world.as_str() {
                "server" => vec![World::Server],
                "single" | "singleplayer" => vec![World::Single],
                "both" => vec![World::Server, World::Single],
                other => anyhow::bail!("--world: server, single or both, not {other:?}"),
            };
            let paths = crate::ingame::collect(&designs)?;
            let o = crate::ingame::IngameOptions { version, timeout_s: timeout, nbt, worlds, dedupe, ..Default::default() };
            let verdicts = crate::ingame::verify_ingame(&paths, &o, &|s| eprintln!("{s}"))?;
            let mut bad = 0;
            for v in &verdicts {
                match &v.result {
                    Ok(s) => println!("OK   {} [{}]: {s}", v.name, v.world.name()),
                    Err(diffs) => {
                        bad += 1;
                        println!("DIFF {} [{}]: {} differences", v.name, v.world.name(), diffs.len());
                        for d in diffs.iter().take(12) {
                            println!("       {d}");
                        }
                    }
                }
            }
            println!("{} of {} pastes by Litematica in the real client are identical to their design", verdicts.len() - bad, verdicts.len());
            if bad > 0 {
                std::process::exit(1);
            }
        }
        LitematicCmd::Check { designs, quiet } => {
            let (mut ok, mut bad) = (0usize, 0usize);
            for p in &designs {
                let res = load(p).and_then(|(d, text)| {
                    let bytes = export(&d, &Meta::new(&d.name, &design_comments(&text), 0))?;
                    compare(&d, &read(&bytes)?)?;
                    Ok(bytes.len())
                });
                match res {
                    Ok(n) => {
                        ok += 1;
                        if !quiet {
                            println!("OK   {} ({n} bytes)", p.display());
                        }
                    }
                    Err(e) => {
                        bad += 1;
                        println!("FAIL {}: {e:#}", p.display());
                    }
                }
            }
            println!("litematic check: {ok} OK, {bad} failed");
            if bad > 0 {
                bail!("{bad} design(s) do not round-trip");
            }
        }
    }
    Ok(())
}
