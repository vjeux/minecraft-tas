# Track fresh-c: a door family where open and close run the SAME symmetric control

## Results (all `door verify --extra-offsets 28`: 32 positions x 3 cycles, category solid)
| file | blocks | open / close ticks | what |
|---|---|---|---|
| `fc1.txt` = `BEST.txt` | **42** | 24 / 14 | symmetric control: no hold, B blocked on close by the re-extended A |

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

## Why the layout looks like this (constraints found)
* Pulse-driven Q (push+spit RB) breaks the T drag: the spit places RB in the block-event phase,
  so P_R fires in the same phase, before T has landed (T is not dragged). Q must be steady.
* RB and A's pulse dust can't share a side of A; the delay repeater for Q must sit next to the
  lever (or its attachment), and the observer for p0 too. X west of A (P_L moved under the
  walkway) is what lets one lever feed both.

## Tool
* `door place <base> --groups <file> --out <file>`: every combination of alternative patch
  groups, screened at 1 position, judged at the default positions (for wiring puzzles).
