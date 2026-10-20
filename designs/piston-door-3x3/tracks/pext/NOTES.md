# pext (doors): one train for both edges, observers that watch the mechanism

pext joined the smaller-door campaign (2026-10-20) from the piston-extender study
(designs/circuits/pext/NOTES.md), whose wiring trick was a single static pulse schedule that does
the right thing on both lever edges because every pulse acts on whatever stands on its cell.

## `door interleave <plan.ideal>`: ONE pulse train for both lever edges
* A plan opens with one pulse sequence and closes with another; wiring that tells the two edges
  apart needs the lever's state everywhere. A train that works on both edges is delivered by one
  sequencer started by an observer watching the lever (it fires on either flip).
* Plain mode merges the plan's own steps (a step of one edge may fire during the other only where
  it changes nothing there; `--split` makes each input cell a step): on w1-k2 it gets stuck after
  3 + 2 steps (the two edges' sequences are too different).
* `--search N`: weighted A* (f = steps + `--weight` x cells off the two targets) over PAIRS of idle
  worlds (the closed door opening, the open door closing), every move = one input cell pulsed 2 gt
  on both at once, until (open, rest). Worlds are stored as their non-air cells (an idle world has
  no pending ticks or moving blocks, so a raw placement rebuilds it): 200k pairs in ~200 MB.
  The train found is judged timed (one pulse every 4..12 gt, the same events on both edges), then
  compacted on BOTH edges together (`ideal-rs --compact` moves each edge's groups on its own).
* **w1-k2 (c70b's 7x7 mm01-oc): a train exists.** Weight 2 finds 31 single pulses in 55k pairs
  (weight 1 and 4 do not within 200k); every 6 gt it works (open 183 / close 183), compacted it is
  27 distinct pulses, last at t32, open 37 / close 34 — redsim 32 positions x 3 cycles and the
  game (`door ideal`, 4 x 2). Plan: `w1k2-train.ideal` (design `w1k2-mm01-oc.txt`, c70b's).
  Per input (same ticks on both edges): TL+ML (-3,2) t0; MR (5,1) t0; (2,4)-down (3,4) t0 t6 t29;
  BR-down (2,-3) t0 t4 t26; (1,4)-down (1,4,1) t0 t4; corner (-1,4) t3; R (4,-2) t0 t3 t19 t23;
  L (-2,-2) t0 t10; BL-down (0,-3) t2; P (1,-1,1) t0 t3 t5; Q (0,-1,1) t13 t24 t32; BM-down (1,-3)
  t14 t21 t31.
* Open problem for a real door: P and BM-down are still boxed in (c77's obstruction); the train
  only removes the lever-state logic and fixes every tap's ticks. pockets3 (feed report on the
  train): no free cell for P, Q, BM-down; every other input has one; pockets k=4 stalls at 11 of 12.
* Batch over c70b's 53 smaller/ plans (`door interleave <plans...> --search 200000`, then 1M pairs
  at weights 1 and 3, and `--groups` = the plan's own tick groups as extra moves): only w1-k2 has a
  train. mm-w1 mm04 exhausts its whole pair space (417k) 2 cells off: PROVEN no single-pulse train.
  6x7-u1 mm01..05, 6x7-inside mm02/03 plateau at 2 cells (one block in the wrong place on one
  edge, `INTERLEAVE_DEBUG=1` prints it); 6x7-in2 at 4; m69T mm02 has a step-by-step train but no
  timing (its lever-held right side is level-driven, which the search's pulse moves mimic by
  pulsing a redstone block cell away).

## `door selffeed <plan.ideal>`: inputs replaced by observers watching the mechanism
* For each input, every observer in a free plane cell that powers the same piston (directly,
  through the cell above it, or through a solid block touching either) is judged with that input
  removed, optionally with the rest of the plan shifted (`--shifts`); greedy rounds keep what works
  at 32 positions. `--align S` prints each input's pulses next to the change ticks of every cell a
  candidate observer would watch (a watched piston body changes 2-3 times per action: an observer
  on it fires twice).
* Results: w1-k2: only ML's lever-time input (observer at (-2,0) watching L, or (-2,3) through
  TL's cell); 6x7-in2 mm01/06/07/08: none (each input fires 2-6 times; no single watched cell
  changes at those ticks).
