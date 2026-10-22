# Auto-brewers (study 20, track sorter)

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
frames), calibration/locked-bottom (4841, a real brew) and brewer-1's base run (6821, both stages,
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

## brewer-1 (49 blocks, 5x6x5, 1.9 potions/min, robust)

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

- Smaller: the dropper wiring is ~16 blocks; a comparator reading the top hopper could fire the
  sugar dropper when the wart arrives (no delay line), if the geometry fits.
- The tileable row: one clock for every stand, the clock's two lines along the row.
