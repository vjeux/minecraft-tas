# Track buildtime: the edit-build loop

vjeux: "it shouldn't take 2 min to build". Goal: a one-line change rebuilds in seconds with the same
command (`cargo build --release`), the same binary (`target/release/mctas`) and every search as fast
as before. How the build is set up now and how to add a command: [internals.md#build](../../../../docs/internals.md#build).
Tools used: `cargo build --release --timings` (frontend / codegen per crate), `nm --size-sort` on
the binary, the incremental session directories (which units were rebuilt), `perf stat -e
instructions:u,cycles:u` for the runtime checks; stable toolchain only (no `-Z`).

## Where the time went
* Before: any change in piston-door (cli.rs or a leaf like sheet.rs) rebuilt the whole crate: 144-158 s
  on an idle 72-core devserver (piston-door 145 s = frontend 31 s + codegen 113 s; the binary 3 s),
  250-370 s on a loaded box (load 77: frontend 50 s, codegen 196 s).
* The long pole: `nm --size-sort -S target/release/mctas` shows `<DoorCmd as clap::Subcommand>::augment_subcommands`
  at **785 KB of machine code in one function**, `cli::run` 236 KB, `DoorCmd::from_arg_matches_mut`
  115 KB (the next biggest function in the binary: 52 KB). rustc places every function of a module in the
  module's codegen unit and never splits a function, so LLVM optimized ~1.1 MB of clap and dispatch
  code in one unit, serially, on every build.
* Incremental alone does not help: the unit holding those functions is re-optimized whenever cli.rs
  changes (one-line cli.rs edit: 368 s under load, 358 s of it codegen), and a cold incremental build
  was slower (codegen 403 s vs 196 s).
* Not the problem: linking (rust-lld is already the toolchain's default linker on x86_64 Linux,
  `-fuse-ld=lld` through its gcc-ld; the binary's crate + link takes 1-3 s; mold is not installed on
  these hosts); debuginfo (`debug = 0` changed nothing measurable on the door crates).

## What was done
1. **The door CLI split into 7 crates by area** (crates/door-judge, -search, -mech, -sat, -redsim,
   -vanilla, -video), each with its own `DoorCmd` slice and `run`, flattened back into `mctas door` by
   crates/mctas-cli/src/door.rs (`#[command(flatten)]`). Moved verbatim by a one-off Rust tool
   (variants, arms and helpers: same lines, same order; it checks every line lands once); every
   `--help` page identical except the order of the `mctas door --help` list. The biggest clap
   function is now 222 KB (door-mech) and each crate builds in parallel.
2. **The door crates at opt-level 0**: optimizing clap's derive output is what costs, not generating
   it: door-mech cold 28 s at O3, 24 s at O1, **3 s at O0** (debug = 0: 27 s). They only parse
   arguments and dispatch: every arm calls piston-door / mctas / redsim functions (optimized); the
   closures they pass to `Pool::map` / `thread::scope` only call library functions.
3. **incremental = true, codegen-units = 16, redsim non-incremental** (see the runtime checks).

Result, idle box, main 96a62d6d vs this branch (one-line string change, then its undo, each built;
the one-off tool makes the change and times `cargo build --release`): a door command 157 s -> 2.9 s,
piston-door/src/sheet.rs 144 s -> 25 s, redsim/src/world.rs 153 s -> 8 s, mctas/src/design.rs
158 s -> 7 s, clean build (deps too) 178 s -> 82 s. Under load (77 on 72 cores) the old build took
250-370 s; expect ~2x the idle numbers there.

## Runtime checks (perf stat user-space counters, 3 rounds alternating)
Exploration, base = main 176c0f7 (instructions are deterministic; wall and cycles move +-2-3% run to
run on this box):
| benchmark | base | incremental, 256 units | redsim whole | 16 units, redsim whole |
|---|---|---|---|---|
| `cosearch eval` 352 archive designs, 1 thread: instructions:u | 154.99 G | 156.20 G (+0.8%) | 154.84 G | 155.24 G (+0.2%) |
| same, cycles:u median | 60.6 G | 60.1 G | 59.6 G | 60.4 G |
| `boxproof bmc --w 7 --h 9 --t 20` SAT encoding (single thread), median | 27.0 s | | 28.3 s (+4.5%) | 27.3 s |
| `boxproof fuzz --count 40000` (all cores): instructions:u | 1386 G | | 1395 G (+0.7%) | 1395 G (+0.7%) |
| same, cycles:u median | 563 G | | 569 G | 568 G (min 561 vs 562) |

The pushed branch vs main 96a62d6d:
| benchmark | main | branch |
|---|---|---|
| `cosearch eval`, 1 thread: instructions:u | 155.25 G | 155.50 G (+0.2%) |
| same, wall median (cycles:u median) | 20.0 s (59.9 G) | 20.2 s (61.1 G) |
| `boxproof bmc 7x9 t20` encoding, median | 27.5 s | 27.2 s |
| `boxproof fuzz 40000`: instructions:u | 1390.6 G | 1395.2 G (+0.3%) |
| same, wall median (cycles:u median) | 12.7 s (560 G) | 13.0 s (567 G) |
The split alone (piston-door non-incremental, 16 units) already moved fuzz by +0.4% instructions:
the 16 units are merged from a different set of modules once cli.rs is gone, the normal codegen noise
of any edit. `door verify` (the JVM judge) prints the same verdicts and takes the same time.

## Rejected
* incremental with its default 256 codegen units: boxproof's SAT encoding +4.5%, cosearch eval
  +0.8% instructions (redsim's tick loop split across units). It did make a piston-door edit faster
  (20 s vs 27 s).
* opt-level 1 for the door crates: only ~15% faster than O3.
* `lto = "off"`: profile-wide (not per crate), would lose cross-unit inlining everywhere.
* mold / another linker: linking is 1-3 s already (rust-lld).

## Next ideas
* A piston-door module edit is now ~27 s on an idle box: after any change, the ThinLTO step re-optimizes
  the 16 units (6 of 22 rebuilt from MIR, 16 re-optimized after a sheet.rs edit). Moving piston-door's
  leaf modules (only the CLI uses them: boxproof, satwire + satsweep, mechspace, fullmech, mechwire,
  obschain, connect, pistfuzz, vanilla_build/gadget, world, sheet, harvest, relever, rowsfire, shapes,
  tickvideo, the designs_* catalog) into their own optimized crates would make most edits rebuild
  only a small crate, at the price of a layering rule (library code could not call into them).
