# Track ring56: a 7-wide flat FULL-seamless door designed by hand ("the door is the wire", mirror halves)

Brief (vjeux via coordinator, 2026-10-19 03:01Z): 7 wide x 8 tall (56) or x 9 (63), 1 wide, nothing in
front/behind, no slime/honey, door blocks parked in the ring when open (FULL seamless), hallway,
lever off at rest. Clean sheet, no catalogs/SAT.

Coordinates: door at x 0..2, y 0..2; ring S = L0..L2 (x=-1), R0..R2 (x=3), F0..F2 (y=-1), C0..C2 (y=3);
7 wide = x -2..4 (only ONE column outside each wall ring).

## 1. Paper analysis: who can feed whom in 7 wide

* A side stack of three pullers at x=-2 (y 0..2) can only be fed: top piston from (-2,3); middle piston
  ONLY by QC through the top piston's cell (from (-2,3) or L2); bottom piston only from (-2,-1) (or QC
  through L1, which never holds a source). So a classic side (TL, ML, BL all sideways) needs both a top
  and a bottom feed, and row y=1 is a wall from x=-2 to 4.
* **If TL leaves UP** (puller at (0,4)), (-2,2) is free and ONE component there emitting down (an
  observer facing up) powers ML's puller directly AND BL's puller by QC: the whole side stack is fed
  from above. Same on the right. This is the in-plane stack feed. It only works from ABOVE (QC looks
  up), so the mirror (bottom row down, feed from (-2,0)) fails for the top puller.
* The bottom region is then woken by an observer at (-2,-1) watching BL's puller (or (4,-1) on BR's):
  the one top -> bottom crossing. A pull-type side puller moves twice per pulse, so the watcher gives
  a DOUBLE pulse (t+2, t+6) per edge.
* Level-held side pullers are impossible from (-2,2): a torch there hangs on L2 (powered when open ->
  QC into ML's puller) and a redstone block there can only leave to cells next to (-2,2) (wire131's
  leak, re-derived).
* Door blocks may park BEYOND the ring (props: a ring cell a block passes through is a hole = minor
  dent; FULL still needs only no circuitry in T or S when open).

## 2. Layout J (7 x 9 = 63, x -2..4, y -3..5): mechanism WORKS with ideal inputs

`mech/j1.txt` + `mech/j1.ideal` (open/close schedules differ) and `mech/j1-sym.ideal` (THE SAME pulse
train on both edges): top row UP (pull-type pistons (0,4), (1,4), (2,4) facing down), ML+BL left,
MR+BR right (pull-type at x=-2/4, y 0..1), C DOWN 2 by the bottom elevator A' (1,-2) on B' (1,-3), BM
down into F1 then right into F2 by the corner pull-type K (3,-1). Free ring cells when open: L2, R2, F0.
10 pistons.

* A' is fed through F0 only (a ring stone strongly powered by an observer at (0,-2) facing down):
  any feed next to A' at (0,-2)/(2,-2) also QC-powers B' through A''s cell, and A''s own motion
  updates B'. At its high position A' touches F0 directly.
* B' is pulse-driven too: the first 2 gt pulse lifts A' (spit), the second lowers it.
* **Symmetric schedule** (j1-sym.ideal, 4 positions x 2 cycles WORKS): A' 0, K 6, B' 12, A' 16, B' 22,
  A' 28, then everything at the top (top row + sides) at t >= 28 on BOTH edges. On close the 2nd A'
  pulse pushes BM, C one cell too high (A' high pushes the line), and the late top-middle puller's
  close push brings TM, C, BM back down together: no edge-dependent logic needed.
  Windows (each alone): A1 0..3, K 3..9, B1 9..14, A2 14..19, B2 19..24, A3 26..32.

## Log
* 03:17Z j1 mechanism WORKS with ideal inputs (open/close schedules).
* 03:22Z j1-sym: one pulse train for both edges WORKS (top late).
