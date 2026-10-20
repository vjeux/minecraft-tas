# Track tnt (study 9, TNT dupers): notes

Brief: vjeux 2026-10-20 "Can you do a study on tnt dupers" + "We want variations" (no internet:
everything from the decompiled 26.3 source). Variations and their judge measures:
[designs/circuits/tnt/VARIATIONS.md](../../tnt/VARIATIONS.md). Judge: `circuit: tnt-duper` /
`tnt-duper-triggered` (crates/circuit/src/tnt.rs, docs/circuits.md). Track tools:
`mctas circuit tnt sim | run | search | shrink | improve | precision | tile` (crates/circuit-tnt).

## Mechanism (read from the code, confirmed in the game)

* `PistonBaseBlock.moveBlocks` copies the pushed states first and blanks the sources last; a TNT
  that gets a neighbor update while powered in between primes where it stands and its moved copy
  lands: tnt-c1 (13 blocks, one push) was the first real-game dupe, its exact trace is
  tick-identical on redsim (designs/clocks/traces/exact/tnt-c1.trace).
* The in-window update: a dead coral fan riding a slime pops when the slime's source cell turns
  moving_piston (the flag-324 write updates shapes before the block entity exists).
* The in-window power: a TNT can never rest next to power (placement and landing prime it), so the
  power must appear inside the push. A stationary dust, connected at rest to a redstone block
  that moves with the structure, points at the TNT once that redstone block's source cell turns
  moving_piston (a block behind it moves in): connections are computed on the fly, no update.
* Self-running engine (12b): the dust strongly powers the obsidian it stands on, which powers the
  sticky piston beside it; the push takes the redstone block away (the piston loses power and
  pulls back), the pull brings it back: period 6, one dupe per push.

## What was tried

* redsim census around the 3-block P6 clock (sticky piston + slime + redstone block QC): 20M
  samples, 0 dupers (its geometry cannot put a block behind the redstone block in the push).
* seeded random search from c1's payload + an engine palette: 39 self-running dupers, smallest 15
  (after fixing the placement order to the judge's sorted one).
* random search from a 6-block core (TNT, fan on slime, dust on obsidian, redstone block): 200M
  samples, 0 dupers (the payload's arrangement is too specific to hit at random).
* local search (`improve`) from the 15b: 12b in seconds; 40M iterations, 3.5M distinct dupers,
  nothing smaller and nothing faster than 200 TNT/min.
* triggered: c1 with a sticky piston and an input = 13b; 20k judge-scored improve iterations found
  nothing smaller.

## Dead ends / limits

* Movable dupers need power inside the push without dust; the only other candidate in the code is
  an observer landing powered on a cell with a pending observer tick (it then powers its target
  silently until that tick runs). Not searched yet.
* Faster than 200 TNT/min per engine needs 2+ TNT per push (a burst) or a faster engine.
