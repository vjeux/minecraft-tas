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
