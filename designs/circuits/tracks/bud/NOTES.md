# Track bud (study 10, BUD switches): notes

Brief: designs/circuits/bud/VARIATIONS.md (what a BUD can vary, read in the 26.3 source) and the
judge kind `bud` (crates/circuit/src/bud.rs, docs/circuits.md#what-each-kind-is-judged-on).

## Archived (designs/circuits/bud/archive/, robust in the game, 16 builds)

| design | detects | latency | pulse | re-arm | footprint | blocks | notes |
|---|---|---|---|---|---|---|---|
| bud-observer | place remove state shape | 2 gt | 2 gt | 4 gt | 1 cell | 1 | silent; misses pure updates |
| bud-slime-qc | place remove state update | 3 gt | 1 gt | 6 gt | 5 cells | 3 | observer-free (QC sticky piston + slime + redstone block); jams on an update pair 2 gt apart |
| bud-slime-qc-fast | place remove state update | 1 gt | 5 gt | 6 gt | 4 cells | 9 | the same piston also drags a stone off a dust step in the tick after the update: the fastest observer-free BUD found |
| bud-fence-observer | place remove | 2 gt | 2 gt | 4 gt | 3 cells | 2 | presence only: the fence's connection follows a solid block, not its state |
| bud-torch-dropper | place remove | 2 gt | 2 gt | 4 gt | 1 cell | 3 | presence by conduction: a torch under the target powers a solid block there, which powers a dropper an observer watches (group torch-presence); found by the k=3 census; the empty dropper clicks |
| bud-torch-lamp | place remove | 2 gt place / 6 gt remove | 2 gt | 2 gt | 1 cell | 3 | the same with a lamp: silent, removal seen 4 gt late, re-arm 2 (group torch-presence) |

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
| 3 | all (50 states) | 5,994,737 | 97,252 | place remove state shape (97,229); place remove state update (6: the slime/honey QC BUD); place remove (17: the torch presence detector, a torch under the target + a power follower beside it + an observer) |
| 4 | sticky pistons in every facing, slime, honey, redstone block, stone (10) | 3,285,503 | 958 | place remove state update only; on the full judge (redsim): 852 pass, 166 of them `fragile` (a probe block carried away or left behind), the other 686 all jam on an update pair 2 gt apart, re-arm 6 gt, latency 3, pulse 1, footprint 4 or 5: none beats the archived ones on a variation metric |

All census runs: `mctas circuit bud census --k K --palette P --threads 6` (k=3 all: 50 min on 6
threads).

So, within redsim's blocks (no fences: redsim does not model them): no 2-block observer-free BUD
exists, and the slime/honey QC BUD is the only 3-block one (the honey version is the same
mechanism, not archived separately); no 3-block design detects all five classes, a pure update
alone, or anything but these three class sets; the only 3-block presence detectors are the torch
ones (its dropper, dispenser and lamp followers in their facings: the dropper and lamp versions
archived as one group).

## Tried and why it failed

* The jam (both QC designs, an update pair 2 gt apart): the second update reaches the piston
  while its push is under way and its QC source is moving (no power), so it queues a retraction
  that runs in the next block-event phase, before the moving blocks land: the sticky piston
  retracts without them (it spits the slime block), the redstone block stays up beside the output.
  A fix needs power on the piston from t+1 to t+3 that goes away once the blocks land; an
  observer watching what moves fires at t+3 but no observer position both sees a moved cell and
  powers the piston (every cell that powers it through quasi-connectivity is beside the slime
  block and would be dragged), and a block landing cannot cut a dust step in a cell that was
  occupied. Not solved.
* 11-block first version of the fast BUD (an extra dust and support so the low dust did not point
  into the piston): the step turned 90 degrees makes the low dust point at the output instead.
* 8-block fast BUD (the stone powered by the high dust as the only QC source, no redstone block):
  a P6 clock, the stone lands back with the slime block and re-arms the piston as it updates it.
* Flat slime BUD (sticky piston facing east, slime in front, redstone block on the slime): a P6
  clock, not a BUD: the redstone block lands back before the slime block, whose landing updates
  the piston again.
* A QC piston that pushes its redstone block up beside the output and stays out: refused
  (designs/circuits/selftest/latching-bud.txt): the judge's placement updates every block, it
  fires there and never re-arms (output on at rest).

## Open

* An all-five detector needs two sensors (an observer for shape-only changes, a QC sensor for pure
  updates): an observer's back can never update a piston that is beside the target (the back cell
  is 2 cells from the target), so their outputs must be merged by wiring. Feeding the observer into
  the piston through dust fails either way (tried on the slime BUD): the observer's 2 gt pulse
  updates the piston again 2 gt into its push (the jam: it spits the slime block), and delayed
  past the push it fires the piston a second time after a placement. So the outputs must be OR'd
  outside the piston: about 12-16 blocks of dust and supports (the slime BUD's output is 3 high,
  the fast one's is beside the target), a union of two archived designs: not built.
* Dropper / dispenser QC BUDs: `TRIGGERED` (a flags-2 change an observer sees, latency 2) stays set
  until the dropper is updated while unpowered, and droppers cannot be moved (block entity): a
  self-resetting one needs a moving power source that comes back without updating it; none in 3
  blocks (census above).
* A manual-reset kind (a `port input.reset`) is described in VARIATIONS.md but not judged yet: no
  latched design worth it so far.
