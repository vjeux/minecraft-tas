# Track bud (study 10, BUD switches): notes

Brief: designs/circuits/bud/VARIATIONS.md (what a BUD can vary, read in the 26.3 source) and the
judge kind `bud` (crates/circuit/src/bud.rs, docs/circuits.md#what-each-kind-is-judged-on).

## Archived (designs/circuits/bud/archive/, robust in the game, 16 builds)

| design | detects | latency | pulse | re-arm | footprint | blocks | notes |
|---|---|---|---|---|---|---|---|
| bud-observer | place remove state shape | 2 gt | 2 gt | 4 gt | 1 cell | 1 | silent; misses pure updates |
| bud-slime-qc | place remove state update | 3 gt | 1 gt | 6 gt | 5 cells | 3 | observer-free (QC sticky piston + slime + redstone block); jams on an update pair 2 gt apart |
| bud-fence-observer | place remove | 2 gt | 2 gt | 4 gt | 3 cells | 2 | presence only: the fence's connection follows a solid block, not its state |

## Census (`mctas circuit bud census`, redsim, classes only, every output cell)

Designs: connected, a block beside the target, an active component, one per structure (type key
with the target marked; cell sets reduced by the 8 symmetries around the target, the palettes hold
every facing).

| k | palette | structures | passing (design, output) | class sets found |
|---|---|---|---|---|
| 1 | all (50 states) | 79 | 3 | place remove state shape (an observer beside, above or below the target) |
| 2 | all (50 states) | 19,617 | 447 | place remove state shape only (observer + anything) |
| 3 | qc (16: pistons, slime, honey, redstone block, stone) | 201,608 | 6 | place remove state update only: sticky piston + slime or honey + redstone block (3 output cells each) |
| 3 | dropper (27: droppers, dispensers, observers, sticky pistons in every facing, redstone block, stone, slime) | 957,088 | 70,510 | place remove state shape (70,507: an observer + 2 blocks); place remove state update (3: the slime QC BUD) |

So, within redsim's blocks (no fences: redsim does not model them): no 2-block observer-free BUD
exists, and the slime/honey QC BUD is the only 3-block one; the honey version is the same
mechanism (not archived separately).

## Tried and why it failed

* Flat slime BUD (sticky piston facing east, slime in front, redstone block on the slime): a P6
  clock, not a BUD: the redstone block lands back before the slime block, whose landing updates
  the piston again.
* A QC piston that pushes its redstone block up beside the output and stays out: refused
  (designs/circuits/selftest/latching-bud.txt): the judge's placement updates every block, it
  fires there and never re-arms (output on at rest).

## Open

* An all-five detector needs two sensors (an observer for shape-only changes, a QC sensor for pure
  updates): an observer's back can never update a piston that is beside the target (the back cell
  is 2 cells from the target), so their outputs must be merged by wiring.
* Dropper / dispenser QC BUDs: `TRIGGERED` (a flags-2 change an observer sees, latency 2) stays set
  until the dropper is updated while unpowered, and droppers cannot be moved (block entity): a
  self-resetting one needs a moving power source that comes back without updating it; none in 3
  blocks (census above).
* A manual-reset kind (a `port input.reset`) is described in VARIATIONS.md but not judged yet: no
  latched design worth it so far.
