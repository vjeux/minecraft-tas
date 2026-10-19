# seq56: a 7-wide flat FULL+hall 3x3 door designed by hand, self-sequencing (clean sheet)

Box target 7x8 = 56 (x -2..4, y -2..5), fallback 7x9 = 63.

## 1. Open state and motion set chosen for wirability
- 3-stacks on both sides seal the top from the bottom in 7 wide (x=-2 pistons, x=-1 gaps fill
  rows 0..2); a side links top and bottom only if one corner moves vertically.
- m1 (m1.txt / m1.ideal, WORKS with ideal pulses, 4 positions x 2 cycles, open 33 / close 15):
  side stacks of TWO (L2 (-2,2), L1 (-2,1), R2, R1) pull TL/ML/TR/MR out; BL, BM, BR go DOWN
  (U0 (0,-2), UM (1,-2), U2 (2,-2) facing up); center UP: elevator A (1,4) + B (1,5) facing down,
  TM parked at (0,3) by L (-1,3) east. Open A L B A B A, close A L A (A2 at (1,3) needs direct power:
  QC through (1,4) is silent).
- Open ring: walls (-1,1..2),(3,1..2), floor (0..2,-1), roof (0,3) TM, (1,3) C. Empty S: (-1,0),(3,0),(2,3).
- Regions (free cells) in 7x8: TL 7 cells (feeds L2, L1 by QC, L, A (0,4), B (0,5)), TR 9 (R2, R1, A (2,4),
  B (2,5)), bottom-left / bottom-right 6 each; L1 seen from (-2,0), R1 from (4,0) (the open corner
  columns link top and bottom). Weak spot in 7x8: UM (1,-2) sits between U0 and U2: only QC through
  a parked BL/BR, and that also QC-powers U0/U2 (they push their block back). In 7x9 (y -3..5) UM is
  fed from (1,-3) and the whole bottom is ONE 19-cell bus.

## 2. FULL FRAME (vjeux 03:36Z): m2 = m1 + 3 static frame stones -> frame 12/12
- m2.txt / m2.ideal: m1 plus plain stones at (-1,0), (3,0) (wall cells nothing passes: BL/BR go
  down) and (2,3) (ceiling cell nothing passes: TM parks LEFT at (0,3)). `door ideal`: WORKS
  (4 positions x 2 cycles, open 33 / close 15); `door sheet`: frame 12/12. 7x8 box x -2..4, y -2..5.
- The static stones are conductors in useful places: (2,3) touches A when it sits at (1,3) (A2's
  feed), (-1,0)/(3,0) touch (-2,0)/(4,0) and the corners (-1,-1)/(3,-1).

## 3. Links between regions in 7 wide (why every approach dies, and the one clean crossing)
- In 7 wide, rows 0..2 of each side are pistons + heads: the top and bottom wiring regions meet
  only through the side pistons. A PULSE piston watched by an observer gives 2 pulses 4 gt apart
  (extend, landing; the retract falls in the busy window): w2.txt/trace (L1 pulsed, the bottom
  pullers pull on the 1st pulse and push back on the 2nd). Every push+spit piston needs <= 2 gt,
  so a 2-pulse link needs a merger; in the 2-wide bottom-left every repeater spot outputs into
  U0 (0,-2) or its QC cell (0,-1), so no merger fits there (7x8 or 7x9).
- TORCH LEMMA: a torch can never hold a piston from directly above it: the torch's input block is
  a horizontal neighbor of the torch's cell = the piston's QC cell, so whenever the torch is off,
  the piston is QC-powered. (w1: torch at (-2,3) on the lever block (-1,3): L1 retracts, L2 stays.)
  Torches hold pistons only from BELOW (or beside).
- The lever next to a piston's QC cell powers it (lever weak power counts for pistons): a lever
  under (-1,0) QC-powers U0 via (0,-1); a lever on top of the left stack QC-powers L2 (w2).
- CLEAN CROSSING (bottom -> top, one pulse per edge): a single side piston L1 (-2,1) (TL goes UP,
  BL goes DOWN, so (-2,2) and (-2,0) are free), held extended at rest by a STANDING torch at
  (-2,0) on block (-2,-1), the lever OUTSIDE the box at (-3,-1) on that block's west face (no
  neighbor of the lever is in the box; the lever is excluded from the volume). Lever on: torch off,
  L1 retracts once (level piston: its watcher at (-2,2) fires exactly once). The lever block also
  hands the lever's LEVEL to the bottom region ((-1,-1), (-2,-2)). Cost: TL up puts P_TL at (0,4),
  so the elevator piston A (1,4) is then reachable only from the right ((2,4)) or through B's cell.

## 4. Single-U (vjeux's hint: one sticky piston facing UP in the box's bottom row)
- m3.txt / m3.ideal: 7x8 box x -2..4, y -3..4, center DOWN: TM up by T (1,4); C down to (1,-1) by the
  elevator A' (1,-2) + B' (1,-3) (B' = the ONLY bottom-row piston, sticky facing up); BM pulled
  down by A' and parked at (2,-1) by K' (3,-1); BL down by U0 (0,-2) (row -2, not the bottom row);
  TL/ML by L2/L1 (-2,2..1); TR/MR/BR by a 3-stack (4,2..0). Static frame stones (0,3), (2,3), (-1,0).
  `door ideal`: WORKS (4 positions x 2 cycles, open 33 / close 15); `door sheet`: frame 12/12.
- Regions: top-left (5 cells) - L1 - bottom-left (8) - B'/A' - bottom-right (7); top-left - T - top-right (5).
