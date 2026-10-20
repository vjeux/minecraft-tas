//! `door audit`, `door selftest`, `door reuse-check`: known answers and audits of the tools
//! themselves.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct AuditArgs {
    /// Design files or folders (default: tracks/c79/c79-closed.txt).
    designs: Vec<PathBuf>,
    /// Comma list of checks, mech, wiring, rules (or all).
    #[arg(long, default_value = "all")]
    sections: String,
    /// Also run the JVM checks (the real judge, props, props-dyn, eligible) in section 1.
    #[arg(long)]
    jvm: bool,
    /// Write the markdown here instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Write every extracted flat mechanism with a macro solution as a families catalog TSV.
    #[arg(long)]
    tsv: Option<PathBuf>,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// State cap of the macro solve.
    #[arg(long, default_value_t = 200_000)]
    cap: usize,
    /// Sources scanned by the rules section.
    #[arg(long, default_value = "crates/piston-door/src")]
    src: PathBuf,
    /// Section 2: skip designs with no flat mechanism.
    #[arg(long)]
    only_flat: bool,
    /// Print one design's extracted mechanism (paths, moves, providers, membership) instead.
    #[arg(long)]
    explain: bool,
    /// Generator membership of piece lists instead (ideal-input designs): lines
    /// `W H x0 y0 piece piece ...` (box in door coordinates).
    #[arg(long)]
    member: Option<PathBuf>,
}

pub fn cmd_audit(version: &str, args: AuditArgs) -> Result<()> {
    let AuditArgs { designs, sections, jvm, out, tsv, threads, cap, src, only_flat, explain, member } = args;
    if let Some(m) = member {
        return crate::audit::member_file(&m);
    }
    let designs = if designs.is_empty() { vec![PathBuf::from("designs/piston-door-3x3/tracks/c79/c79-closed.txt")] } else { designs };
    if explain {
        for d in &designs {
            crate::audit::explain(d)?;
        }
    } else {
        crate::audit::run(&crate::audit::Opts { designs, sections, jvm, out, tsv, threads, cap, version: version.to_string(), src, only_flat })?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct SelftestArgs {
    /// The fast subset only.
    #[arg(long)]
    quick: bool,
    /// Only checks whose name contains one of these (comma list).
    #[arg(long)]
    only: Option<String>,
    /// List the checks and exit.
    #[arg(long)]
    list: bool,
    /// Checks run at once.
    #[arg(long, default_value_t = 6)]
    jobs: usize,
    /// JVM checks (door verify / door ideal) at once.
    #[arg(long, default_value_t = 2)]
    jvm_jobs: usize,
    /// Keep the scratch folder even when everything passes.
    #[arg(long)]
    keep: bool,
}

pub fn cmd_selftest(args: SelftestArgs) -> Result<()> {
    let SelftestArgs { quick, only, list, jobs, jvm_jobs, keep } = args;
    crate::selftest::run(&crate::selftest::Opts { quick, only, list, jobs, jvm_jobs, keep })?;
    Ok(())
}

#[derive(clap::Args)]
pub struct ReuseCheckArgs {
    design: PathBuf,
    #[arg(long, default_value_t = 30)]
    times: u32,
    #[arg(long, default_value_t = 1)]
    cycles: u32,
    /// Recreate the sandbox before every run instead of reusing it.
    #[arg(long)]
    fresh: bool,
}

pub fn cmd_reuse_check(version: &str, args: ReuseCheckArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let ReuseCheckArgs { design, times, cycles, fresh } = args;
    let d = load_design(&design)?;
    let mut w = Worker::spawn(&cfg)?;
    let opts = VerifyOptions { cycles, offsets: vec![[0, 0, 0]], ..Default::default() };
    let mut sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut line = String::new();
    let mut passes = 0;
    for _ in 0..times {
        if fresh {
            w.drop_sandbox(sb)?;
            sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        }
        let ok = crate::verify_in(&mut w, sb, &d, &opts).is_ok();
        passes += ok as u32;
        line.push(if ok { '+' } else { '.' });
    }
    println!("{line}\n{passes}/{times} passed in {} sandbox(es)", if fresh { "fresh" } else { "one reused" });
    Ok(())
}
