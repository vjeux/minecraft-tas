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
* **m69T mm02 (c70b's 6x9 = 54): a train exists** once lever-held inputs are handled (cells whose
  block differs between the open and closed rest and is a redstone block: the lever switches them at
  t0 of each edge, the train never touches them; and the plan's level-off clears are kept): its
  lever-held right side (3 extended pistons whose redstone blocks the lever replaces) plus ONE
  26-pulse train for the left half, identical on both edges: open 31 / close 31, redsim 32 x 3 and
  the game. Plan `m69T-mm02-train.ideal`. This removes w86's blocker ("its left/right halves can't
  share one lever"): the lever holds the right side and anything that fires on both flips starts
  the left train. Left-half ticks: (-2,3) t8 t18 t22; (-1,-2) t3 t23; (-1,-1) t6 t12 t25; (0,-4)
  t0 t4 t19; (0,4,1) t0 t13 t28; (1,3,1) t7 t9; (1,4,1) t0 t16; (2,-2) t0 t8; (2,5) t0 t5 t12;
  (3,-2) t3; (3,4) t0 t7.
* Batch over c70b's 53 smaller/ plans (`door interleave <plans...> --search 300000`): trains for
  w1-k2 and m69T mm02 only; the rest stall 2-12 cells off (`INTERLEAVE_DEBUG=1` prints the cells;
  6x7-u1 is one block in the wrong place on one edge). `--groups` (the plan's tick groups as moves)
  and `--pairs` (two inputs at once) widen the moves.

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
