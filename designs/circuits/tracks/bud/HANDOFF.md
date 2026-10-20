# Track bud (study 10, BUD switches): handoff

Owner so far: clk-obs (branch explore/clk-obs). Coordinator f1a42990.

## Where things are

* What a BUD can vary and why, from the 26.3 source: designs/circuits/bud/VARIATIONS.md.
* The judge kind `bud`: crates/circuit/src/bud.rs (stimulus, `check`, `judge` for screens),
  documented in docs/circuits.md (the `bud` bullet). Judge actions added for it: clock::Action
  `ShapeSet` (flags 2) and `Update` (pure `updateNeighborsAt`), on redsim (clock/src/sim.rs,
  circuit-fly, redsim-fuzz) and in the game (clock/src/verify.rs; the bridge's `set` mode `shape`).
  Footprint probes: `Circuit::probe_cells`; target cells are marked in both type keys.
* Calibrations (selftest circuit:sim/verify-calibration): designs/circuits/calibration/
  observer-bud.txt, slime-bud.txt; refusals designs/circuits/selftest/observer-pair-bud.txt,
  latching-bud.txt, observer-bud-claims-update.txt.
* Archive: designs/circuits/bud/archive/ (3 designs, one per class set); track files
  designs/circuits/tracks/bud/ (NOTES.md: results, census table, failures, open items).
* Census: `mctas circuit bud census --k K --palette all|qc|dropper|"<state>;<state>"`
  (crates/circuit-bud): k ≤ 2 over 50 states and k = 3 over the piston and dropper palettes are
  done (NOTES.md).

## How to continue

1. Merge main before every report (four studies edit the same lists: Kind, STUDIES, check,
   selftest calibrations, docs/circuits.md).
2. New design: write it in designs/circuits/tracks/bud/, `mctas circuit sim <f> --table` (redsim),
   `mctas circuit verify <f> --positions 4` (the game), then `mctas circuit archive <f>`; the
   caption is the design's first comment block.
3. A judge change: full `mctas door selftest` before pushing (calibration lines in
   crates/piston-door/src/selftest.rs).

## Open (NOTES.md has the details)

* The jam fix: a self-resetting QC BUD that survives two updates 2 gt apart (every one found so far
  jams; the latched ones do not). NOTES.md lists what rules out the obvious holds and the route
  being tried (a latched QC BUD with an automatic reset).
* Done since: all five (bud-all-five), latency 1 (bud-slime-qc-fast), place only
  (bud-place-only), manual reset (judge latched mode, bud-latched-qc / -fast), the catalog's
  beats_on_variation knows bud's metrics (latency, re-arm, footprint, jams; fragile never wins).
