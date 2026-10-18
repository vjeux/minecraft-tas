# Track fresh-c: a door family where open and close run the SAME symmetric control

## Results (all `door verify --extra-offsets 28`: 32 positions x 3 cycles, category solid)
| file | blocks | open / close ticks | what |
|---|---|---|---|
| `fc2.txt` = `BEST.txt` | **40** | 24 / 9 | fc1 with P_L a pull-type pulse piston WEST of the backbone (-3,2,0), fired by two observers off the lever (backbone 3 slimes) |
| `fc2f.txt` = `FAST.txt` | 41 | 22 / 9 | fc2 with the B window fed straight off the lever (observer (-2,5,0) above the lever + cross dust (-2,6,0) into J) instead of O_b watching O_a: window t10..18 (fastest open+close, 31) |
| `fc1.txt` | 42 | 24 / 14 | symmetric control: no hold, B blocked on close by the re-extended A |

fc2 block budget: door 9, left 3 slimes + P_L, right 6 slimes + P_R, A, B, P_M, Q, RB (25
mechanism); lever + its wall block 2, R_q + floor 2, O_a 1, X + floor 2, O_b + J 2, R_B + O 2,
P_L's two observers 2, BM observer + block 2 (15 control).

## The idea (sidesteps "Q's hold needs >= 10 gt")
rb6 (47) keeps the shuttle piston Q extended for 16 gt after the lever goes off (a 7-block silent
hold) because its close runs the whole B dance backwards before T may come back. fc1 closes
like observer-seq's close instead (A pushes C down one, T comes back, A pushes T+C home), and
gets every signal from ONE symmetric chain, the same delays on both lever edges:

* p0: an A pulse 2 gt after each flip (observer on the lever).
  * open: A is still extended (RB still powers it), the pulse is swallowed;
  * close: A (retracted, C under its face) pushes C from (1,3,0) to (1,2,0) and spits it.
* Q: the lever through ONE delay-2 repeater (4 gt), symmetric: on open RB leaves A (A lifts T) and
  lands next to P_R (P_R drags T out); on close RB comes back after p0's spit: P_R pulls T back
  into (1,3,0), then RB lands next to A, which pushes T and C home together and STAYS extended.
* B window (8 gt from a 2 gt pulse, R_B delay 4) at the same delay on both edges:
  * open: the normal UP dance (B pushes A down, A pulls C, B lifts A, A pulls C);
  * close: A is extended (immovable), so B's push fails and the observer's two A pulses are
    swallowed (A already powered). The window is harmless on close: no need for an
    open-only B.

Ideal-input sweep (`fc-sym.sched`, `door ideal`): works for every Q delay d in 2..7 with the A
pulse p <= d-2 (B window at 10). `fc.sched`: with an immediate Q on open, the close needs Q off
at 4..9 and p0 at 0..4.

## fc1 layout (42 = 27 mechanism + 15 control)
* Mechanism (rb6's, rearranged): door 9, left backbone 3 slimes + 1 under the walkway
  (-1,-1..2,0), P_L (0,-1,0) under the walkway (pulse-driven: push+spit / pull), right structure
  6 slimes + P_R (2,4,0), A (1,4,0) extended + B (1,5,0), P_M (1,-2,0), Q (0,4,1) + RB (1,4,1)
  (the shuttle moved to the z=+1 side).
* Lever (-2,4,0) on the wall of (-2,4,1). R_q (-1,4,1) delay 2 reads that strongly powered block
  and points into Q's back (floor (-1,3,1)).
* O_a (-1,4,0) watches the lever -> X dust (0,4,0) (a line E-W pointing into A) on (0,3,0)
  (the floor reaches A in its lower position). X sits WEST of A, where rb6 had P_L.
* O_b (-1,5,0) watches O_a (+2 gt) -> block (-1,6,0) -> R_B (0,6,0) delay 4 facing west -> B's
  quasi cell (1,6,0): window t12..20. O (0,5,0) watches R_B -> X (A's two dance pulses).
* P_M: rb6's observer on the bottom-right door block (2,-1,0) + block (2,-2,0).
* P_L: observer chain off that observer: (2,-1,-1) facing south, (1,-1,-1) facing east, block
  (0,-1,-1) next to P_L (any timing works for the side pistons).

## Searches that found nothing below 40 (fc2 is a local minimum)
* `door minimize --moves all` (also `--plateau-pairs --walk 100`), `door regrow --size 2` (5.4k
  candidates) and `--size 3` (1.6M), `door grow --k 1` after cutting P_L's two observers or the
  window's O_b + J.
* Open speed: open = b + 12 with b >= d + 6 (T must be aside before B) and d >= p0 + 2 >= 4, so
  22 is this family's floor (fc2f).

## What did not work / why
* Pulse-driven Q (push+spit RB): the spit places RB in the block-event phase, so P_R fires in the
  same phase, before T (lifted 2 ticks earlier) has landed: T is not dragged.
* p0 as a short B pulse (B pushes A+C, pulls A back): the window's observer pulses A while it is
  down and A is still moving when B retracts, so A stays down; any extra window costs more than
  O_a.
* A solid block instead of the X dust: it would power Q (next to (0,4,0)) and gives A no update
  in its lower position.
* P_L push-type under the walkway (fc1): 5 blocks (pusher, extra slime, 2 chain observers, block)
  vs 3 for the pull-type west of the backbone.

## Next ideas
* The 15 control blocks are: lever 2, Q delay 2, p0 3 (O_a + X + floor, X shared with the dance),
  window 4 (O_b, J, R_B, O), P_L 2, P_M 2. Sharing one more pulse source between P_L / P_M /
  the window would reach 39: e.g. a door layout where J (once per edge) is next to a side piston.
* The same symmetric trick should apply to the mirrored family (C down, M dragged aside by a side
  structure at y=-1, T pulled by a pulse piston): same count unless the bottom offers sharing.

## Round 2 (after the record): searching for 39
Nothing below 40 found. Searched (all on fc2, judged as usual):
* `door regrow --any` (cut ANY 2 control blocks, add 1 within margin 1 / 2 of either): 102k /
  416k candidates, none pass the screen.
* cut 3, add 2 (`door grow --k 2`): {both P_L observers + O_b} in two boxes (136k, 1.57M),
  {O_b, J, obs1} (296k), {R_q, its floor, obs1} (192k), {R_q, its floor, O_b} (274k): none.
* Layout analysis (why the control is pinned):
  * X (A's pulse dust) must sit at (0,4,0) on (0,3,0) or at (1,4,+-1) on (1,3,+-1) to reach A in
    both positions; the observer feeding X then has exactly one orientation per cell, so the
    lever sits in O_a's face cell.
  * Q must be at (0,4,+-1) (RB has to go from next to A to next to P_R in one push), so R_q's
    input is (-2,4,+-1) (or a QC variant), which no cell next to O_a's lever can be: the
    lever needs its own attachment block and R_q its floor.
  * P_L only takes 2 gt pulses (its close must spit) or a steady push from the east, and the
    only east-side push cells ((0,3,0), (0,-1,0)) cost a slime and collide with X / the lever.
  * Odd pulse counts toggle a pulse piston correctly: X pulses 3 times per edge, so P_L or P_M
    COULD hang off X's floor if they were next to it; neither can be (geometry).
* Mechanism variants considered and rejected: RB riding the left structure instead of Q (needs
  P_R off <= t_L+2 on close but on >= t_L+3 on open with a pull-type P_L; with a push-type P_L
  the window is [t_L+3, t_L+4] = lever + 8 gt, buildable, but P_R then needs its own repeater
  next to it, far from the lever: 42-43); T dragged by the left structure (X loses its floor);
  pull-type P_R (needs RB next to x >= 4).

## Why the layout looks like this (constraints found)
* Pulse-driven Q (push+spit RB) breaks the T drag: the spit places RB in the block-event phase,
  so P_R fires in the same phase, before T has landed (T is not dragged). Q must be steady.
* RB and A's pulse dust can't share a side of A; the delay repeater for Q must sit next to the
  lever (or its attachment), and the observer for p0 too. X west of A (P_L moved under the
  walkway) is what lets one lever feed both.

## Tool
* `door place <base> --groups <file> --out <file>`: every combination of alternative patch
  groups, screened at 1 position, judged at the default positions (for wiring puzzles).
