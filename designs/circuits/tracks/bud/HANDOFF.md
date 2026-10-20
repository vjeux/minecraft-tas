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

* An all-five detector (observer + QC sensor, outputs merged by wiring).
* A faster QC BUD (latency 1: the piston's block event at t+1 moving a block that cuts a dust
  step?), a place-only / remove-only detector (presence level + edge detector).
* A manual-reset variant (`port input.reset`), described in VARIATIONS.md, not judged.
* `circuit::archive::beats_on_variation` (from explore/shulker, when it lands on main): add bud's
  metrics (lower re-arm, footprint, latency, no jams) so a bigger but better BUD keeps its card.
