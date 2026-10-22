# Auto-brewers (study 21, track sorter)

Coordinator brief (2026-10-21): brewing stands fed by hoppers; water bottles, blaze powder and a set
ingredient sequence (nether wart, then sugar: swiftness) from supply chests, potions into an output
chest, no manual step. Judge: N complete batches, nothing wasted or brewed in the wrong order, fuel
accounted, intrusion-safe, robust, in the game and in redsim (the brewing stand modelled exactly).
Metrics: potions/min, blocks. Minimum volume for one stand first, then a tileable multi-stand row.
The judge, its runs and metrics: docs/circuits.md "brewer N" (crate circuit::brewer).

## The model

redsim's brewing stand is a port of `BrewingStandBlockEntity.serverTick` (read with
`mctas src show BrewingStandBlockEntity`): 400-gt brews, blaze powder worth 20 brews, the
ingredient and the bottles checked every tick (a brew stops if either changes), the
`has_bottle_N` block state, the hopper faces (top: the ingredient slot 3; sides: bottles 0-2 and
fuel 4; bottom: every slot, slot 3 only for a glass bottle). The recipes come from the game's
`data/minecraft/recipe/brewing/*.json`; potions are items `potion[KIND]` (stack of 1).
Tick-identical with the game (`brewer-run --compare`) on calibration/unlocked-bottom (4961
frames), calibration/locked-bottom (4841, a real brew) and brewer-1's and brewer-small's base runs (6821 each, both stages,
4 batches).

## What makes it hard

- The ingredient slot STACKS: a hopper on top pushes every wart (and then the sugar would wait
  forever). Each stage needs exactly one item put in at the right time.
- A stage's end is invisible to a hopper chain except as "the hopper above can push again": an
  event-driven design (the next wart pushed in when the slot empties) cannot take the last batch
  out without a timer. So brewer-1 is clock-driven.
- The bottom hopper takes ANY potion: a water bottle that enters while it is unlocked leaves
  unbrewed. Bottles must only enter while the bottom hopper is locked, and finished potions only
  leave while the bottles hopper is locked: break-before-make on both edges of the clock.

## brewer-small (42 blocks, 3x6x5, 1.9 potions/min, robust; archived, replaces brewer-1)

The same clock, bottles hopper, bottom hopper and slow release as brewer-1, but the droppers are
fired by the latch clock's OTHER torch: the torch under the block beside the bulb lights for ~6 gt
each time the hopper it watches runs empty, i.e. exactly once a period, when the brewing half
starts (it is what flips the bulb). Both droppers touch that block. The wart dropper (under the
hopper column U -> V -> T over the stand) lifts its wart straight in; the sugar dropper sends its
sugar through two more hoppers, so it reaches the column 16 gt after the wart (both droppers fire
in the same tick, and which one ticks first follows the update order, which rotates with the
build: a longer item path is the rotation-proof way to order them). The 16-block repeater line of
brewer-1 is gone; the box loses its two wiring columns. `circuit compact` (now judging brewers by
`brewer::sim_score`) found nothing smaller from either seed.

## brewer-1 (49 blocks, 5x6x5, 1.9 potions/min, robust; first record, kept in the track)

A latch clock (two hoppers passing 106 stones, a waxed copper bulb remembering the direction;
848 gt a half). The bulb's comparator line (dust A) points into the bottles hopper (locked in the
"extraction" half); the clock's wall torch r (its inverse) locks the hopper under the stand (the
"brewing" half). When the brewing half starts, a repeater line from r strongly powers a block
under the wart dropper and, through a delay-4 repeater, a block beside the sugar dropper: the wart
reaches the hopper on top of the stand 8 gt before the sugar, which waits there until the wart is
used up (stage 1 done), then drops in for stage 2. The droppers are powered through blocks beside
them, never by a repeater pointing into them: a dropper pointed into is a powered conductor and
would lock the hopper next to it.

Pitfalls met on the way (calibration/):
- A dropper directly above r (brew1 in /tmp, not kept): droppers are conductors, r strongly powers
  the block above it, and that block touched dust A: a 4-gt oscillation that fired the droppers
  every cycle (all the wart and sugar dumped at once).
- No slow release (calibration/no-slow-release): the torch lags dust A by 2 gt, so when the
  brewing half starts the bottles hopper is unlocked 2 gt before the hopper under the stand locks,
  and the second batch's first bottle is taken out unbrewed. Fix: a delay-2 repeater reading the
  clock's powered stone (under dust A) feeds a dust cross beside the bottles hopper, keeping it
  locked 4 gt after dust A falls.
- Branching dust A sideways (to feed the slow release) turns A into an L: it stops pointing into
  the bottles hopper. The slow release reads the stone under A instead.

## Next

- The tileable row (judged at 2, 4 and 8 stands since 2026-10-21; the fuel chest holds at least
  a stack, as the smelter's coal). Tiling brewer-small as it is (row-naive, `tile: 3 0 0`) fails
  at 2 stands: its supply hoppers point into the first slice's droppers, so the first stand takes
  every wart and sugar (6 brews, 0 for the second) and no potion leaves (its output hopper feeds
  the next slice's chest cell).
  Each slice must get exactly one wart and one sugar per batch (in that order)
  from shared chests, and at least one blaze powder. Bottles meter themselves (a hopper with two
  filler slots holds three: a stack of one), but wart, sugar and blaze powder stack to 64, so any
  hopper or dropper buffer hoards: the first slices take everything and the last ones get water
  bottles with no wart (the judge sees water potions). A comparator reading a buffer cannot lock
  the hopper that feeds it directly (its output lands two cells from the buffer, never next to a
  hopper that touches the buffer), so a metered dropper costs about four more blocks (comparator,
  the block it powers, a dust cross on it, a block next to the feeder), twice per slice; locking
  the bottom hopper early needs the clock's pulse ORed into its line (the comparator line and the
  torch line keep their 2-gt race at every slice). Estimate: ~35 blocks a stand, 2 wide.
- What a working slice needs, worked out 2026-10-21 (not built yet: three metered paths and two
  clock lines did not fit a short time box): per stand, a dropper for the wart over the top hopper
  and one for the sugar beside it
  (fired on the rising edge of the "brewing half" line, the sugar one 2 gt later through a
  repeater), each fed by a hopper from its line and METERED: a comparator reads the dropper and,
  through three dust (comparator front, a turn, a dust pointing into the feeder; on glass, since a
  dust locks the hopper it stands on), locks the feeder while the dropper holds an item; the
  feeder's push and pull in one tick leave it one spare, so a slice keeps exactly 2 of each: with
  dead-end lines and the judge's 2 batches a stand, every slice fills to exactly its 2 batches.
  Bottles: a reservoir hopper (3 fillers, always open, so the line does not run past while the
  bottles hopper is locked) into the bottles hopper (4 fillers, locked in the extraction half):
  stand 3 + 2 + 1 = 6 bottles a slice. Fuel: a hopper locked by a comparator reading the stand
  (bounded, a few a stand; the stack covers 8 stands). The clock in the start piece, its two lines
  (bottles lock with the slow release folded in; brewing half for the bottom hoppers and the
  droppers) along the row with a repeater per slice. About 60 blocks a stand.
- Smaller one-stand designs: the slow release (4 blocks) and the four glass covers.
