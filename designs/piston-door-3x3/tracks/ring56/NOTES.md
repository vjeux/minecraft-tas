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

## 3. FULL FRAME (vjeux 03:36Z): layout J is 12/12 as it stands

Static plain stones in the three free frame cells L2, R2, F0 (nothing ever passes through them):
`mech/jf.txt` + `jf.ideal`: frame 12/12 at open (`door sheet`), WORKS with ideal inputs.

## 4. The BM gate: edge-dependent bottom with ONE pulse train

The bottom elevator needs open = A1 K B1 A2 B2 A3, close = A1 K B1 B2 (A2/A3 on close would undo).
BM parks in F2 only while the door is open (from K's pull on open until K's push on close), and A'
touches F2 at its high position and QC-reads it (through F1) at its low one. So an observer at (2,-2)
facing down that powers F2 is a GATE: its pulses reach A' only when BM is parked there. A2 = A' placed
high (by B''s spit) while the gate is on (onPlace extends it); A3 = A' landing low (B2) while the gate
is on. `mech/j7.ideal`, `j10.ideal`, `j11.ideal`: the SAME pulse train on both edges WORKS.

## 5. Single-U 7x8 (vjeux's hint 04:04Z): s1 (mechanism only, NOT wirable)

`mech/s1.txt` + `s1.ideal`: 7x8 (y -3..4), one sticky piston facing up in the bottom row, frame 12/12,
WORKS with ideal inputs. But its 2-row top is cut by D(0,4)+D(1,4)+C0/C1 into two halves that cannot
see each other, and the bottom cannot reach either: one lever wakes only one top half.

## 6. Wiring jf (7x9): top by hand, bottom by cosearch (assist mode)

* `mech/j6.txt`: real top (13 blocks): lever on an observer, observer chain along row 5 QC-fires the
  three top pullers, a trapdoor/observer column fires MR+BR.
* cosearch run 1 (j9 seed = jf + j6 top, `--assist j9.ideal`): wired A' (low and high) and B' for real,
  only K left assisted: `mech/p1.txt` (p2 = p1 minus a junk piston that left the box). `door ideal
  mech/p2.ideal`: WORKS at 32 positions with K as an ideal 2 gt pulse at tick 16..20.
  Left: watcher (-2,-1) on BL's puller -> a flag piston (-2,-2) facing down -> observer (-1,-2) ->
  oak log (0,-2) -> A' (three pulses: pull, push, pull = one net pull). Right: watcher (4,-1) on BR's
  puller -> lamp (4,-2) (lit at once, dark 4 gt late: two edges 10 apart) -> observer (4,-3) ->
  observer (3,-3) -> dropper (2,-3) (conductor) -> B' (B1, B2), and the gate observer (2,-2) watching
  the dropper -> F2 (A2, A3).
* K's only feeds are (4,-1) (the right watcher's cell) and (3,-2) (needs an upward emitter), and the
  right half wakes at 20 with j6's top: too late. `mech/j12.txt`: a DUST top (dust on the three top
  pullers, k2; MR+BR at k4) so the right half wakes at 6; its right half must now be re-timed (cosearch
  run 3).
