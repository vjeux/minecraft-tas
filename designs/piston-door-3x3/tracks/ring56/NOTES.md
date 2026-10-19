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

## 7. Lever polarity is free (vjeux 04:50Z): levers hung on a side puller

* A lever hung on the WEST face of a side puller level-holds that puller directly and the one BELOW it by
  QC (the lever touches the lower puller's QC cell), with zero wiring, ON at rest = closed. Both move
  once per edge, so an observer watching each gives ONE clean pulse per edge (no doubles, no mergers).
* `mech/t8.txt` (+ `t8.ideal`): wire64b's 7x8-014 with BR pulled right, so the ONLY piston below the
  door is U(1,-2). Mechanism WORKS with ideal inputs. Its top has a causality problem for the lever trick:
  A's high feed (2,4) is in the top-right while a lever on the left side wakes the top-left first, and
  the two halves only meet through the elevator (B would move before A).
* `mech/t10.txt` (+ `t10.ideal`, `t10w.ideal` windows): 7x8, the lever on TL's puller (TL + ML held),
  BL DOWN (U (0,-2)), BM down (U (1,-2)), TM up then LEFT into C0 (corner E(-1,3)), C up 2 (A/B), TR UP,
  MR/BR right. The top-left owns the whole elevator (A's feed (0,4) is free because TL goes left) and gets
  the lever's clean pulse from an observer (-2,3) on TL's puller; the bottom-left gets one from (-2,0) on
  ML's puller. Static L0, R2, F2. WORKS with ideal inputs; windows: TR, MR/BR, BL free (0..60), BM >= 30,
  the elevator may start 0..12 after the flip. `mech/t10s.txt` = that seed with the real lever and the
  four observers; cosearch run 5 wires it (assist mode).
* Tool fix: `cosearch search`'s screen required an unpowered lever at rest (the judge already accepted
  either polarity): fixed in its own commit.

## 8. Where it stands (end of the night) and what to try next

* No wired 7x8 or 7x9 yet. Mechanisms that WORK with ideal inputs (frame 12/12): jf / j1-j13 (7x9, lever
  off), s1 (7x8, not wirable: split top), t8 (7x8, one piston below the door), t10 (7x8, lever-ON family).
* jf: everything wired by cosearch (p2) except the corner piston K, whose only feeds are (3,-2) (needs an
  upward emitter) and (4,-1) (the right stack's watcher); with j6's top the right half wakes at 20, with
  j12's dust top at 6, and neither run found K plus the B'/gate pulses (best 77/79 without assists).
  Single source of trouble: watching a pull-type piston gives a DOUBLE pulse, and the bottom halves are too
  small for mergers.
* t10 (lever on TL's puller, ON at rest): both crossings are clean single pulses. Remaining walls: (1) the
  top-left must sequence the elevator (A x3, E, B x2 over ~30 ticks) in 4 free cells; (2) BM's puller must
  move >= 30 ticks after the flip with the symmetric train (an earlier BM makes the close's low A-push
  shove the whole column down). cosearch run 5 (t10s seed, all other pistons assisted): best 77/79 with
  2-4 assists left (always the elevator).
* Ideas not tried yet:
  - an ASYMMETRIC (edge-aware) top for t10 so BM can move early (the close's low A-push is the only reason
    for BM >= 30); gate the open-only pulses through door blocks parked in the frame (TM in C0, TR in C2)
    the way BM in F2 gates A2/A3 in jf;
  - the B double trick: a pull-type B fed a 4 gt double lifts A and lowers it again; A2 fits between if
    A-low's power window is [B1+5, B1+7) (one odd tick: from a landing-powered piston);
  - TR pulled RIGHT in t10 (C2 static): the top-right then reaches A at both heights ((2,4) direct + QC,
    and the static C2 as c79's relay into A-low) and has 8 free cells for the sequencer.
* `mech/t8L.txt` (+ `t8L.ideal`, windows `t8Lw.ideal`): t8 with the lever ON at rest on the EAST face of
  TR's puller (TR + MR held), observer (4,3) facing down = one clean pulse into the top-right, which owns
  every elevator feed (A at both heights from (2,4): direct high, QC + front update low; B from (2,5);
  the corner W(3,3)). WORKS with ideal inputs at 4 positions. Windows: TL up, the left stack and BR are
  FREE (0..60); BM >= 36; the elevator may start 0..12 after the flip. Still one piston below the door.
  The open problem is the same: a 30-tick, 6-tap elevator sequencer from one pulse in ~4 free cells.

## 9. 8x8 and the parity wall (06:00Z)

* `mech/t8x.txt` (+ `t8x.ideal`): 8x8 = t8's 7x8 mechanism + a wiring column x=5. Lever (ON at rest)
  outside on the east face of the stone (5,1): the stone and ONE cross dust (5,2) level-hold all three
  right pullers (MR directly, BR by QC, TR through the dust). Observers (4,3) facing down and (4,-1)
  facing up = one clean pulse per edge into the top-right and the bottom-right. Ideal WORKS, frame
  12/12 (static frame cells are stone = the door block, as the judge now requires).
* cosearch on t8x (assist mode, 29 focus cells) wired everything but the left stack within 20 min
  (best 77/79 with that single assist: `mech/q7.txt`; C still not lifted there); on t8L (7x8) it
  reached 75/79 with the left stack + BR's puller assisted.
* THE PARITY WALL: the side without the lever hears the rest of the door only through the elevator
  (an observer on B at (0,5), or on A at (0,4) when TL is not pulled up). B moves twice per edge (B1
  lowers A, B2 lifts it) and high A twice (A1, A3); low A moves once but no cell outside the door
  touches it. So that side gets an EVEN number of pulses per edge and a pull-type puller nets zero
  unless the pulse timing makes the doubles cancel unevenly (c79's left side seems to rely on that).
  Pieces that move ONCE per edge: the corner puller (P_T / E) and U; their watchers sit on the side
  that fires them. Ways out: (a) watch U from the non-lever bottom (one pulse per edge after a merger)
  and climb a wiring column on THAT side (8 wide: the extra column goes on the non-lever side, not the
  lever side); (b) a divide-by-2 (copper bulb + comparator + bulb) turning B's two merged pulses per
  edge into a door-state level for level-held pullers; (c) make U lever-held through dust into the
  static F2 (QC) so its watchers are clean crossings, which needs an edge-aware (asymmetric) top
  because the symmetric train needs BM's move >= 36 ticks after the flip.

## 10. The parity wall cracked by the search (06:15Z)

* cosearch run 6 on t8L (7x8) ended with partials whose LEFT side is wired (`mech/q6-0.txt`; with stone
  frame cells: `mech/q8seed.txt`): the observer on B (0,5) -> repeater delay 4 (-1,5) -> lamp (-2,5) ->
  lamp (-2,4), watched by observers (-2,3) facing up (into the stone (-2,2): ML's puller directly, BL's
  by QC) and (-1,4) facing west (into TL's puller (0,4)). The repeater merges each of B's doubles and the
  lamps' 4-tick off delay plus the observers' busy windows give the pullers an odd net count per edge.
  With the periodic-rest screen (boxproof 2cd7dc3 + 911a18a, cherry-picked) q8seed screens 79/79 with
  5 assists left: A (high, low), B, U and BR's puller. Run 6 had earlier wired exactly those (A, B, U,
  TL's puller) with the left stack + BR's puller assisted (`mech/q6a.txt`, transcribed). Run 8 wires q8seed.
* `mech/t8e.txt` (+ `t8e.ideal`, windows `t8ew.ideal`): 8x8, t8x's lever-held right side, TM goes LEFT
  into C0 (corner E (-1,3)) so the left owns E (moves once per edge, window A1+3..A1+9) and can watch A
  from (0,4); TL/ML/BL left (x=-2), BM down. Ideal WORKS; left stack and BL free 0..60, BM >= 36.
