# clk-item: door wiring (overnight push, from 2026-10-21 01:00Z)

Joined fly's sub-56 door push (vjeux's #1). Line taken: copper-bulb open/close memory for q69m's wiring.

## q69m (fly, explore/fly designs/piston-door-3x3/tracks/fly/wip/q69m.txt, 6x9 = 54)

### Copper bulbs do not rescue the areas (exhaustive, redsim)
Against fly's ORIGINAL q69m.ideal (island-consistent timings; the compacted plan acts at t0 everywhere),
each area's ideal inputs removed (and its rest-on redstone block, where it had one), every filling of
the area with 25 states per cell: air, stone, redstone block, dust, observers (up/down/east/west),
repeaters (east/west, delay 1-4), comparators (east/west, compare/subtract), torch, wall torches
(east/west), copper bulb lit and unlit:
- top-left (-2..1,5), P_T and Pl removed: 0 of 390,625.
- bottom-left (-2,-3) (-1,-3) (-2,-2), N, D_O and W removed: 0 of 15,625.
- bottom-right (2,-3) (3,-2) (3,-3), K, A and B1 removed: 0 of 15,625.
Calibration: the same machinery passes the all-air filling with the inputs kept.
Why bulbs cannot help much in one plane: a bulb is read only by a comparator (2 cells with its
support), and an in-plane comparator has no side inputs, so it cannot subtract. A bulb would buy a
LEVEL output from a once-per-edge trigger (Pl is exactly "on while open"; D_O is its inverse, with
the bulb lit at rest): about 3 cells next to the piston's feed, which no area has to spare.

### Blocker: W has no in-plane feed
`door feeds` on q69m.ideal (`--box=-2..3,-3..5`): "no feed at all: close t46 NE(-1,-1)". Checked on
the plane at close t46: W (-1,-1) faces east, its back (-2,-1) is empty (it is W's own open-state
cell: the slime drags W there on open, which is the bottom-left's open trigger through O), above is
the slime (-1,0), whose other neighbours are all mechanism cells, below is N, and its front holds
BM. No free cell touches W or its QC cell, so q69m cannot be wired as it stands: W's close push of
BM from (0,-1) to (1,-1) needs a piston with a free neighbour.

## s69r (clk-rt's s69 with the riding observer O at (0,-2)): B1 c66 and Qv@ c18
- Qv@(1,-2) close t18: SOLVED in redsim with O itself. O watches (-1,-2), a free cell of the
  bottom-left area, and fires east into (1,-2), which is exactly the ideal input's power pattern
  (direct into Qv, and quasi-connectivity into B1). A change at (-1,-2) at close t16 that reverts
  anywhere in t40..t60 WORKS (redsim 4 x 2; the second O pulse is harmless). In wiring terms: any
  component at (-1,-2) whose state changes at close t16 and back in that window.
- B1 (1,-3) close t66: NOT through O. O's pulse also powers Qv (then at (1,-2)) directly, Qv's
  block event is queued first, and an extending Qv cannot be pushed, so B1's lift fails (every
  revert tick 62..66: BROKEN, Qv left at (1,-2)). The only signal that fires B1 but not Qv enters
  (1,-2) from ABOVE (Qv ignores its front face), i.e. a source in (1,-1), which is the cell Qv must
  move into. In box y -4..4 instead of y -3..5, B1 gets a direct feed from (1,-4), but the top loses
  row 5 (P_T is then fed only from (0,4), and fly's lever-on-observer at (1,5) is gone).
- `door shuttle` (clk-rt) on s69r for both close pulses: 0 placements.

## m69T (pext's m69T-mm02-oc + fly's bot6 train): R0's level from the bottom-right
fly asked for a copper-bulb T flip-flop in the bottom-right free cells (1..4,-3..-1 minus U, K and
the floor frame cells) feeding R0 (4,0) through (4,-1). Measured in redsim on m69T-bot6-train:
- R0's level really is timing-free: the (5,0) stand-in switched at open t20 / close t20 WORKS.
- But (4,-1) is K's BACK cell (K (3,-1) faces west): any level into R0 from (4,-1) also powers K.
  A redstone block at (4,-1) switched like a perfect flip-flop (off open t20, on close t20), (5,0)
  removed: BROKEN, and the plane at open t1 shows K extended at closed rest (head in (2,-1)).
- Nothing in (4,-1) powers R0 alone: an observer facing down gives pulses (an extended piston needs
  continuous power), comparators/repeaters cannot face up, dust does not power upward, a torch
  powers K, and a wall torch hung on K (powers R0, not K) can never switch off.
- R2 is the mirror case: its only in-box feed (4,3) is C's back (C (3,3) faces west). One lever on
  the right holds two of R0/R1/R2 (at (5,1): R1 + R0 by QC; at (5,2): R2 + R1 by QC).
So m69T needs K (or C) moved off R0's (or R2's) only feed; memory does not help.
