# Track tnt: handoff

State (2026-10-20): study 9 (TNT dupers) is complete: every variation in
[designs/circuits/tnt/VARIATIONS.md](../../tnt/VARIATIONS.md) has an archived design or a written
reason. Branch explore/tnt, merged with main. Notes: [NOTES.md](NOTES.md).

## Archived (designs/circuits/tnt/archive/, all robust in the game at 16 builds)

| design | variation | line |
|---|---|---|
| tnt-duper-12b | smallest self-running, FLAT (2 high: the minimum), TILEABLE (4 copies = 800 TNT/min) | 12 blocks, 200 TNT/min, period 6 |
| tnt-duper-triggered-13b | per pulse | 13 blocks, 1 TNT per pulse, latency 1 gt |
| tnt-lever-12b | lever on/off | runs while the input is off, stops at once, restarts 5 gt after |
| tnt-precise-28b | PRECISE, STATIONARY | every TNT at one point at fuse 1 (bit for bit), at rest 33+ gt |
| tnt-survive-28b | SURVIVABLE (`explode: on`) | 64 explosions in the judge run, design intact (they go off in water) |
| tnt-launch-15b | LAUNCHED | every TNT flung at exactly vz = -1.0, vy = 0.1568 blocks/gt |
| tnt-drop-16b | DROPPED straight down | flung, stopped by a wall, falls straight down 72+ blocks |
| tnt-honey-15b | SLIME-FREE / HONEY-ONLY | a honey frame; a wall fan hangs on a riding stone |

Written impossibilities / limits: MOVABLE (only a dust gives a TNT silent power inside a push, and
dust cannot ride a piston); 1-WIDE (the dust, its power source and the TNT form an L across both
horizontal axes); FLAT below 2 high (dust needs a support); one-push BURST (the 12-block push
limit; tiles give same-tick bursts); 2-wide and frame-free: not found, not proven impossible.

## The judge (crates/circuit/src/tnt.rs, docs/circuits.md; clockinfra reviewed every change)

* `tnt-duper` (self-running), `tnt-duper-triggered` (per pulse), `tnt-duper` with an input
  (switchable). The game runner follows each primed TNT to fuse 1 and removes it there (with
  `explode: on` it lets them explode and counts the explosions).
* Flags in the judge line, each its own catalog card: `precise`, `survives its own explosions`,
  `launched at`, `stationary`, `dropped`, `slime-free`.

## redsim (crates/redsim)

* TNT and dead coral fans (`tnt.rs`), PrimedTnt entities when `World::tnt_kick` is set
  (`entity/primed_tnt.rs`), slime bounce and `stepOn` in the entity core, clk-misc's piston
  entity pushing: the duper traces (12b, water aligner) are bit-identical with the game.
* Exact traces: designs/clocks/traces/exact/tnt-c1, tnt-phys-stone / -slime / -shaft.

## Tools (mctas circuit tnt ...)

`sim` (`--tile`), `run`, `search` (`--min-per`, `--forbid`), `shrink`, `improve`, `precise-sim`
(`--follow`), `align`, `precise-shrink`, `precision` (`--list`), `tile`.

## Ideas left

* A faster duper (> 200 TNT/min per engine): a 2-TNT push within the 12-block limit, or a faster
  engine than the sticky piston's 6 gt.
* A smaller precise / survivable duper (28 blocks today): `align` from the 15b launcher with a
  smaller aligner.
* 2-wide: an exhaustive census in a 2-wide box would settle it.
