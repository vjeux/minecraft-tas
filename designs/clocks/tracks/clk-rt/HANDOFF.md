# clk-rt HANDOFF

Track: redstone-tick clocks (torches, repeaters, comparators, dust). Session
7c07d5cb-1b56-4d72-aecb-946261d96898, box devvm54942 (dedicated), branch explore/clk-rt, clone
~/agents/clk-rt/minecraft-tas. Theory, tables and mechanisms: [NOTES.md](NOTES.md). Board:
Space memory `minecraft-tas/clocks/families/rt.md`.

## What exists
- Module crates/clock-rt (`mctas clock rt <command>`, command table in NOTES.md): U-loop and
  rectangle-ring generators and sweeps, fading rings, comparator counters (single, nested, over any
  clock), divider chains, odd-period constructions (kicks at odd ticks), burnout sweep, shrink.
- Archive: clk-rt archives its own clocks (coordinator protocol of 10-19): `mctas clock archive
  <files> --workers 16 [--horizon N]`, commit designs/clocks/archive/, push, send the coordinator
  ONE line with the commit. About 330 clk-rt clocks archived (all robust except a few flagged
  rotation-dependent burnout clocks).

## Results (all `clock verify` robust, 32 builds)
- Every period from 4 to 100 has a clock: even periods with 8-31 blocks (2-torch U-rings, 1-torch
  inverter U-loops and 3x3/3x4 rings, repeater-only rectangle rings, P4 = subtract comparator 8b),
  odd periods 5-99 with 16-55 blocks (two pulses in a repeater ring of delay 2P, the second one
  started by a temporary torch at an odd tick).
- Long periods: comparator fading rings (P = 56k + 2 at 4k + 10 blocks), dividers (+18 blocks per
  x2), comparator COUNTERS (+26 blocks per x14): P1596 at 44 blocks, P22344 (18.6 min) at 70,
  P312816 (4 h 20 min) at 96; odd long periods with count-7 counters over an odd ring: P161 (44b),
  P1127 (72b).
- Burnout clocks: P200, P205 (odd kick), P210, P220, P410, P440 (odd kick), P872 (8-16 blocks).
- Families: torch+repeater, torch+dust+repeater, repeater only, dust+repeater, comparator only
  (kicked rings), comparator+dust (P4), comparator+repeater, torch+dust only (P12, 14 blocks),
  and the comparator/torch/dust/repeater mixes (U-loop per-family sweep, fading rings, counters).
- Theory (impossible.tsv rows, NOTES "Theory"): P1-3 impossible; odd periods need a second parity
  class (a kick at an odd tick); families with a torch cannot reach P4-8; torches alone or dust
  alone make no clock; inverter loops are 0 mod 4 and never odd; clean counter decrements need the
  pulse within L - 1 .. L + 1 gt of the lap.

## Next steps (ideas not done)
1. Smaller divider stages (18 blocks now) and k = 1 counters (lap 4 gt) that are robust: the
   1-comparator-pair fading ring P58 (14b) is position-dependent (P58/P60 by build).
2. A census-style lower bound for the smallest clocks (8 blocks for P10-20, 9 for P24-36): the
   clock census is far too slow at 8 blocks; a dedicated redstone-tick enumerator over rings would
   be needed.
3. Counter counts other than 14/7/5 (refill level, decrement size) for exact long periods; mixed
   counter + divider chains for every P in 1000..10000.
4. Rectangle rings bigger than 3x4 in an exhaustive sweep (only targeted constructions so far).

## Lessons
- Long sweeps must write results as they go: a 2-hour sweep killed at its bound lost everything.
- `clock verify` above P ~10000 needs `--horizon` = warmup + 3P (clockinfra lifted the old
  frame-memory limit; a 940000-tick verify used ~30 GB of RAM in mctas and took 14 minutes).
- Generated names carry the family (`uloop<n>t-`, `fade-`, `ring<w>x<h>-`, `rect<w>x<h>-`,
  `count-`, `odd-`, `div-`, `burn-`): two families once collided on `clk-rt-uloop-p88-17b`.

## Running processes
None after this session ends.
