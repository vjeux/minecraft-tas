//! `mctas bench`: how many simulations fit on this machine and how fast they tick.

use anyhow::{Result, bail};
use clap::Args;
use mctas::sim::{PlaceMode, Pool, SandboxOptions, WorkerConfig};
use std::time::Instant;

#[derive(Args)]
pub struct BenchArgs {
    /// Worker JVMs (default: half the CPUs).
    #[arg(long)]
    workers: Option<usize>,
    /// Sandboxes per worker.
    #[arg(long, default_value_t = 8)]
    sandboxes: usize,
    /// Ticks per sandbox.
    #[arg(long, default_value_t = 1000)]
    ticks: u32,
    /// Max heap per worker, MiB.
    #[arg(long, default_value_t = 768)]
    heap: u32,
    /// What runs in each sandbox: `clock` (observer clock driving a sticky piston) or `empty`.
    #[arg(long, default_value = "clock")]
    workload: String,
}

/// An observer clock pulsing a sticky piston that pushes a block back and forth: exercises
/// scheduled ticks, block events, moving-piston block entities and neighbor updates.
fn clock_blocks() -> Vec<([i32; 3], &'static str)> {
    vec![
        ([-2, 0, 0], "minecraft:stone"),
        ([-1, 0, 0], "minecraft:sticky_piston[facing=west]"),
        ([0, 0, 0], "minecraft:observer[facing=east]"),
        ([1, 0, 0], "minecraft:observer[facing=west]"),
    ]
}

pub fn run(version: &str, args: BenchArgs) -> Result<()> {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let workers = args.workers.unwrap_or((cpus / 2).max(1));
    let cfg = WorkerConfig { version: version.to_string(), heap_mb: args.heap, cpus: 2, ..Default::default() };
    eprintln!("starting {workers} workers ({} MiB heap each) on {cpus} CPUs", args.heap);
    let t0 = Instant::now();
    let pool = Pool::spawn(workers, &cfg)?;
    let startup = t0.elapsed();

    let t1 = Instant::now();
    let per_worker = args.sandboxes;
    let workload = args.workload.clone();
    let created = pool.map((0..workers).collect(), |w, _, _| {
        let mut ids = Vec::new();
        for _ in 0..per_worker {
            let sb = w.create(&SandboxOptions::default())?;
            if workload == "clock" {
                w.set_blocks(sb, &clock_blocks(), PlaceMode::Update)?;
            }
            ids.push(sb);
        }
        Ok(ids)
    });
    let created: Vec<Vec<u32>> = created.into_iter().collect::<Result<_>>()?;
    let creation = t1.elapsed();

    // Warm up the JIT, then measure.
    let ticks = args.ticks;
    let warm: Vec<Result<()>> = pool.map(created.clone(), |w, _, ids| {
        w.step_many(&ids, 200, false)?;
        Ok(())
    });
    warm.into_iter().collect::<Result<Vec<_>>>()?;
    let t2 = Instant::now();
    let timed: Vec<Result<f64>> = pool.map(created.clone(), |w, _, ids| {
        let t = Instant::now();
        w.step_many(&ids, ticks, false)?;
        Ok(t.elapsed().as_secs_f64())
    });
    let wall = t2.elapsed().as_secs_f64();
    let per_worker_secs: Vec<f64> = timed.into_iter().collect::<Result<_>>()?;

    // The clock must still be running: a dead clock would make the numbers meaningless.
    if workload_is_clock(&args.workload) {
        let mut w = pool.worker(0);
        let st = w.status(created[0][0])?;
        if st.idle {
            bail!("the bench clock stopped: {st:?}");
        }
    }

    let mut rss_total = 0u64;
    for i in 0..pool.len() {
        rss_total += pool.worker(i).rss_kib().unwrap_or(0);
    }
    let sandboxes = workers * per_worker;
    let sandbox_ticks = sandboxes as f64 * ticks as f64;
    let mean_worker = per_worker_secs.iter().sum::<f64>() / per_worker_secs.len() as f64;
    println!("workers               {workers}");
    println!("sandboxes             {sandboxes} ({per_worker} per worker, workload: {})", args.workload);
    println!("worker startup        {:.1} s for all (parallel)", startup.as_secs_f64());
    println!("sandbox creation      {:.1} ms each", creation.as_secs_f64() * 1000.0 / per_worker as f64);
    println!("ticks per sandbox     {ticks}");
    println!("wall time             {wall:.2} s");
    println!("throughput            {:.0} sandbox-ticks/s total", sandbox_ticks / wall);
    println!("per sandbox           {:.0} ticks/s ({:.1}x real time)", ticks as f64 / wall, ticks as f64 / wall / 20.0);
    println!("cost per tick         {:.1} us (per sandbox, single core)", mean_worker * 1e6 / (per_worker as f64 * ticks as f64));
    println!("memory (RSS)          {:.1} GiB total, {:.0} MiB per worker, {:.1} MiB per sandbox (incl. JVM share)",
        rss_total as f64 / 1048576.0,
        rss_total as f64 / 1024.0 / workers as f64,
        rss_total as f64 / 1024.0 / sandboxes as f64);
    Ok(())
}

fn workload_is_clock(w: &str) -> bool {
    w == "clock"
}
