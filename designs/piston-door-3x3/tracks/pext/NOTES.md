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
* m69T mm02 wiring (pockets3's satwire recipe, `--sat-loop` runs it in a loop: search a train,
  satwire it, double the search cost of the inputs next to the core pistons, repeat): the right
  stack and the bottom left left the cores once the trains respected pocket budgets (`--pocket
  cells=k`: top-left {(-2,3),(0,4,1)} <= 3 and bottom-left {(-1,-2),(-1,-1)} <= 3 ticks:
  `m69T-mm02-train-p33.ideal`); every train since has its core in the TOP-LEFT (close (-1,3) W,
  close (0,4), open (1,3)+(1,4), open (-1,3)+(1,5)), 10 trains, until the search runs out. The
  sealed 3-cell pocket (-1,4)(-1,5)(0,5) sees neither the lever nor another pocket.
  `--observe` (every observer filling of the pocket added to the design, W's and/or (0,4)'s
  inputs dropped, a train searched for the rest): 0 of 124 fillings, each way. c70b is searching
  mechmut variants with one more free cell in the top-left (lever-held right stack kept).
* The other m69T plans (mm01, 03, 04, 05, 06) have no train in 3M pairs (mm04, mm05 exhaust
  their whole pair space: proven none in this model).
* q7 family (w86 + clk-misc team-up: d67's 7x7 in2/mm01-oc-q7b and w86's q7c/q7d): no train in
  3M pairs whatever the moves. Decisive: even with one-edge pulses free (`--edge-cost 1
  --levels`), the opening can't be reproduced, because the search applies one input change at a
  time and lets the build go idle in between, and this family's opening (like 6x7-u1's) needs
  overlapping timing. A one-train for it would need a timed search (worlds not idle). With
  clk-misc's 15 free cells as the only pulse cells (`--cells`, the (-3,2) trigger `--clear`ed):
  10 cells off (b (0,-2) and c (1,-2) unfed). w86's read: the family is too action-heavy for the
  7x7's 15 free cells.
* Tool options since: `--cells` (pulse given feed cells instead of the plan's inputs), `--clear`
  (empty a cell in both rests), `--edge-cost k` (one-edge pulses, i.e. lever-state-gated, at a
  cost), `--levels`, `--fix`/`--drop`/`--observe`, real levers (`use` at t0) handled like
  lever-held inputs, plan variables, the open target from the plan's own timing, batched
  parallel expansion (3M pairs in ~45 s with `--threads 32`).
* Fixed 01:05: a pulse could overwrite a mechanism block standing in an input cell (the block then
  vanished when the pulse cleared): moves that set over or clear a mechanism block are refused
  now; earlier "1 cell off" near-misses were that artifact. Snapshots cover the whole simulated
  world (a block pushed past the watch box no longer vanishes on a rebuild).
* c70b's 7x7 u0 / u2 (every receiver touches a free feed, statics as stone): no train with zero
  one-edge pulses (2 cells off), but with `--edge-cost 16` a train of 21 shared pulses + ONE
  one-edge pulse: `u0-train-e16.ideal` (close-only (-1,-3), open 39 / close 39) and
  `u2-train-e16.ideal` (open-only (-3,1), open 42 / close 29), redsim 32x3 + game. satwire
  (pockets3's u0 recipe): cores u0 open (1,-2), u2 close (-1,-2). clk-misc: u0/u2's 16 free cells
  are 5 islands with no electrical path; each must start from an event it can see, and with the
  lever on top two islands never start (u0-l2 moves the lever). A search that tracks island starts
  (which islands have seen a change) would prune unwirable trains; not built.

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
