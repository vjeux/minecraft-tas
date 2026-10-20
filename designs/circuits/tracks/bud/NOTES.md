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

## Built since (coordinator 09:15Z)

* All five: bud-all-five (13 blocks). The observer is fed into the fast BUD's low dust, not into
  the piston: after a placement that dust is already lit by the open step when the observer fires,
  so the piston hears nothing new mid-push; after a shape-only change the dust lighting up is what
  updates the piston.
* Place only: bud-place-only (4 blocks: the torch presence level into the 2-block rising-edge
  detector of the pulse study).
* Manual reset: the judge's latched mode (`port input.reset`) and bud-latched-qc (2 blocks + the
  reset block) / bud-latched-qc-fast (7 blocks, latency 1). Latched QC BUDs never jam: the piston
  stays powered while it is out.

* The jam fix: bud-qc-nojam (5 blocks, robust, no jams): two NON-sticky pistons pass a package (a
  slime block, the arming redstone block on top, an observer under it) back and forth. A non-sticky
  piston that retracts during its own push leaves exactly what a late retraction leaves (it never
  pulls), so a second update cannot strand anything; its head vanishes at once when it retracts,
  so the moved observer (firing 2 gt after it lands) updates the second piston, armed by the
  pushed redstone block, just after the first piston's head has gone. Found by `bud census --base`
  over the two pistons + 1 block (18 QC structures; 2 blocks: 3,217 jam-free intact ones out of
  6,922, all latency 3, re-arm 8). What ruled out the other routes is kept below.

* Remove only: bud-remove-only (4 blocks), place-only's mirror: the observer is pulled back into
  the output cell on a removal; facing down it watches only its own piston (footprint 1). The k=3
  census over 50 states has no place-only or remove-only design: 4 blocks is the minimum.

## Tileable (coordinator 10:40Z)

Judge: a `tile:` claim builds 3 copies along the step and runs every class at every copy's target;
each copy must answer its own target exactly as the single design and stay silent for the
others' (docs/circuits.md). `mctas circuit bud tile` finds a design's densest pitch per axis,
`bud census --tile N` every candidate's. Every archived BUD carries its densest `tile:`.

| class set | densest | smallest design at that pitch | why not denser / smaller |
|---|---|---|---|
| place remove state shape | every 1 | bud-observer-stone (2) | the 1-block observer tiles every 2: its lamp, strongly powered, lights the next (census k=1 --tile 3) |
| place remove state update | every 2 | bud-slime-qc (3) | every 1 is impossible with armed pistons: a piston's move updates its neighbors, so the next copy's armed piston fires (census k=3 qc --tile 2: densest 2; k=4 over sticky pistons, slime, honey, redstone block, stone --tile 1: none of the 958 QC structures tiles every 1) |
| place remove | every 1 (stacked) | bud-fence-stone (3) | 2-block presence designs (observer + fence, wall, pane or bars) tile every 2 (census k=2 --tile 2); the torch version needs 4 blocks to tile every 1 in a row |
| place | every 1 | bud-place-only (4) | no 3-block place-only design |
| remove | every 1 | bud-remove-only (4) | no 3-block remove-only design |
| all five | every 4 | bud-all-five (13) | the only one |
| latched (manual reset) | every 2 (stacked) | bud-latched-qc (2) | armed piston |

The rest: bud-observer, the torch group, bud-fence-observer every 2; bud-qc-nojam every 3;
bud-slime-qc-fast, bud-latched-qc-fast every 4.

## Dead ends of the jam fix (kept)

* The piston must still be powered at the block-event phase of t+3 (the retraction queued by the
  second update runs there, before the moving blocks land); every QC-power cell is beside the
  slime block (dragged) or in its column (pushed); no observer cell both watches a moved cell and
  powers the piston; every gate made of moving blocks opens again on the way back, re-firing the
  piston as it finishes retracting. `bud census --base` over the latched BUD + 2 blocks (267,258
  structures) finds no automatic reset. The non-sticky loop (bud-qc-nojam) avoids all of this.

## Open

* A faster jam-free BUD (bud-qc-nojam: latency 3, re-arm 8): the loop censuses found none faster.
* An all-five BUD that tiles denser than every 4.
