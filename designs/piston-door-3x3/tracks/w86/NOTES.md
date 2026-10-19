# Track w86: WIDE 6-tall doors (8x6 = 48b, 9x6 = 54b), 2026-10-19 campaign "smaller than 56"

## Results
- **wip/a3r.txt + wip/a3r.ideal: an 8x6 (48b) MECHANISM, ideal inputs, FULL frame 12/12, flat, slime-free,
  14 sticky pistons + 3 floor statics.** `door ideal-rs` WORKS (4 positions x 2 cycles), real game `door ideal`
  WORKS (4 x 2), `door sheet`: frame 12/12 when open. Nothing ever leaves x -2..5, y -1..4. NOT wired.

## How a3r works (Box A: the one extra row is ABOVE the ceiling; door at (0..2, 0..2))
- Frame when open: ceiling (0,3) TL, (1,3) BM, (2,3) C; walls (-1,2) TM, (3,2) TR, (-1,1) ML, (-1,0) BL,
  (3,1) MR, (3,0) BR; floor (0..2,-1) = 3 statics that never move.
- Column 1 goes UP. W = a sticky piston facing down RESTING IN the ceiling cell (1,3); Q (1,4) carries it.
  1. (0,4) pulls TL up. 2. RP_L (sticky east RESTING IN the wall cell (-1,2)) pulls TM to (0,2).
  3. W pulls C up to (1,2). 4. RP_L wiggle (long pulse): pushes TM, C, TR right (TR into the wall (3,2),
     C to (2,2)) and pulls TM back to (0,2). 5. (-1,4) pulls RP_L up into the corner (-1,3).
  6. (-2,2) pulls TM into the wall (-1,2). 7. Q pushes W down to (1,2); W pulls BM up to (1,1); Q pulls W
     back; W pulls BM to (1,2). 8. W escapes to the corner (3,3): KR (3,3) pulls it to (2,3), KR2 (5,3) pulls
     KR to (4,3), KR pulls W again. 9. Q pulls BM into (1,3); (2,4) pulls C into (2,3). 10. side pullers.
- Close: (2,4) pushes C down, Q pushes BM down, KR pushes W to (2,3), (3,4) pulls TR UP into (3,3), KR pushes
  TR+W left (W home at (1,3), TR to (2,3)), KR2 pushes KR home, W/Q push BM down to (1,0), (2,4) pushes TR+C
  down (TR home, C into (2,1)), MR's puller pushes MR+C left (C home), then TM, RP_L, TM, TL, BL.

## Lessons (6-tall)
- The frame row with NO machine row behind it must stay static: a block that enters it can only come back
  through a puller inside the opening (a carried piston at y = 1 facing down for the floor).
- Vertically stacked pistons are QC-coupled: powering the upper one powers the lower one (its quasi cell is
  the upper piston's own cell), and the upper one's move updates it: it fires. a3 (KR escaping onto TR's
  puller (4,2)) broke on close for exactly that; a3r removes (4,2) (TR enters the wall by the wiggle, leaves
  it upward by (3,4)).
- fullmech --check drops statics/helpers and uses the old hole rule (air allowed in ring cells); it hit its
  cap on 11-14 piston layouts (independent side pullers blow the graph up). ideal-rs plans by hand are faster.
- `phase tile` puts a torch support at z+2: inputs behind opening cells (a carried piston inside the
  opening) need `phase between`.
