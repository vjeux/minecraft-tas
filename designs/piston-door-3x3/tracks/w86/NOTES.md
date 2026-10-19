# Track w86: WIDE 6-tall doors (8x6 = 48b, 9x6 = 54b), 2026-10-19 campaign "smaller than 56"

## Results
- **wip/a3s.txt + wip/a3s.ideal: a 7x6 (42b) MECHANISM, ideal inputs, FULL frame 12/12, flat, slime-free,
  13 sticky pistons + 3 floor statics** (= a3r without KR2: after RP_L escaped to the corner (-1,3) it wiggles
  TL+W right along row 3 and pulls TL back; KR resting at (4,3) pulls W from (2,3) into the corner (3,3)).
  ideal-rs WORKS 4x2, real-game door ideal WORKS 4x2, sheet frame 12/12; nothing leaves x -2..4, y -1..4.
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

## Wiring (the open problem)
- **a3s / a3r / b3s are unwirable as is.** Lemma (6-tall Box A): a piston in the single machine row whose two
  row neighbours are pistons can only be powered from above the box (its quasi cell and top face are outside,
  its front is row 3, pistons don't conduct), i.e. only by the lever, as a level. a3s's row 4 is 5 pistons in a
  row ((-1,4)..(3,4)): Q (1,4), (0,4), (2,4) are boxed in (pockets3: 27 of 30 event groups undriven; families'
  wire analysis: 6 pistons without a free feed cell). b3s (a3s upside down) is boxed the same way in practice.
- **m69t/ (9x6 = 54b, c78l's m69a turned by c70b; plan by w86): m69u.ideal** WORKS ideal-rs + real game, tile AND
  between phases, frame 12/12. Re-timed for the s0 (4,2)/s1 (4,1) QC coupling: s0's home pulls on open are long
  (the coupled s1 push of the (3,1) block is pulled back), on close s0's first home spit also spits s1 (TR takes
  MR's home) and M0 pushes the LINE s0+MR (MR into (2,2)), elevator releases 2 gt after the carried pull lands.
  But (c78l, satwire): R1 (1,-2) has no feed (UNSAT at once); with R1 lever-held (c78l's m69v, lever under it)
  the left (x<=-1) and right (x>=3) wiring regions still can't both learn of the flip: they are separated by the
  door column and nothing the lever moves is observable from both (R1 has no free neighbour). Sculk could.
- **m69t/m69f + m69g (9x6, Box A = m69t upside down, floor statics):** WORK ideal-rs tile+between + real game,
  frame 12/12. m69g: the top trio (0..2,4) facing down EXTENDED at rest, the REAL lever ON at rest on top of
  (1,4) (powers it directly and (0,4)/(2,4) by QC: the 7x8 doors' trick); observers at (-1,4)/(3,4) watching the
  outer pullers can give both halves the flip. Left half fully fed (carried U1 at (-1,1) has its quasi cell
  (-1,2) next to the free (-2,2)). Right half: carried s0 at (3,0) is UNFED (quasi cell (3,1) = a door block
  whose neighbours are s1 and MR'), and W' (3,-1) sits under carried s0 (QC-coupled; obsidian at (3,4) blocks its
  column push but takes the right observer's cell). Rule of thumb: in Box A an elevator in row 1 or 2 is fed,
  one in row 0 is not (its carried puller's quasi cell is a wall cell boxed by pistons/door blocks).

- **m69t/m73 (8x6 = 48b, w86, 22:06Z)**: built around the obstacle list: Box A, lever-held top trio (the real
  lever ON at rest on top of (1,4)), BM leaves by a ROW-0 WIGGLE (ring piston RP0 resting in the right wall cell
  (3,0) pushes BR+BM left and pulls BR back; BL climbs the left wall by W'' (-1,-1); U0 pulls BM into (-1,0)),
  C leaves right by a row-1 elevator (M1 carries s1 into (3,1)); RP0 escapes to (4,0) via R5 (5,0), which rides
  M1's QC coupling and needs no input. WORKS ideal-rs tile+between + real game, frame 12/12. Weak spots: the
  bottom corner pockets are isolated in 8 wide ((-2,-1) = W''s only feed; (3,-1)..(5,-1) = RP0's); on close R5's
  coupled pull at M1's release must be blocked by RP0 being extended. satwire (c78l) running.

## The 6-tall obstacle list (w86, 22:00Z; satwire agrees on every m69 version)
1. The frame row opposite the single machine row (the FAR row) must stay static (blocks entering it come
   back only through a puller inside the opening).
2. One lever must reach both wiring halves (x <= -1 and x >= 3), which the door column separates. Box B (row
   below): nothing the lever moves is observable from both (the bottom trio's middle has no free neighbour), so
   only sculk crosses. Box A (row above): a lever ON at rest on top of the middle top puller holds the whole
   top trio (direct + QC both sides), and observers at (-1,4)/(3,4) watching the outer pullers reach both halves.
3. The center block next to the far row (BM in Box A, TM in Box B) is the crux. It leaves either
   (a) by a far-row elevator: Box B's carried puller at (3,2) has the corner (3,3) as quasi cell, where the
   wall-shift pusher W must sit (TR moves down to make room); Box A's carried puller at (3,0) has quasi cell
   (3,1), boxed by the row-1 side puller and door blocks (feeding it through the door block MR' at (3,2) powers
   it without ever updating it); satwire: the right half is UNSAT solid and sculk in m69v and m69g; or
   (b) by a carried column puller (a3s's W + Q), which puts Q in the middle of the machine-row trio: boxed, and
   not lever-holdable (3 events per edge).
4. C (row 1) is easy: a row-1 elevator on the side WITHOUT a wall-shift pusher is fed via the corner next to
   its quasi cell (satwire: the left half of every m69 version is satisfiable, solid and sculk).
5. Vertically stacked pistons are QC-coupled (powering the upper fires the lower); pulse lengths can make the
   coupled firing net zero (long pulse: push + pull back) or useful (m69u's close).

6. (w86 22:50Z, satwire on m73..m78 and c70b's m69g-12) Box A + lever-held top trio: TL TM TR go up, so the
   6 other blocks fill the 6 wall cells; C leaves by row 1 and BM by row 0, so the side that receives BM via row 0
   also has its own row-0 block (BR or BL) and one of them must CLIMB from (±,0). The climbers all collide with
   something: a bottom-corner pusher (W'' (-1,-1) / W' (3,-1)) is fed only from a bottom-row pocket that the
   row-0 pistons and trajectories cut off (only (x,-1) cells; observers there can only watch row 0 and point out
   of the box; repeaters/dust have no support) — satwire cores: m73 W'' (8 wide), m69g-12/m77 W' (9 wide), m76
   RP0's long close pulse (even 10 wide); a carried top puller (V at (3,4) pushing P3t' into (3,2)) takes the
   crossing cell (3,4), so that half never sees the close flip (m78 core: close P3t'); a ring puller resting in
   (3,2) must escape through (4,2) and boxes s1. So the remaining options are: a pocket connected upward through
   free (x,0),(x,1) cells (no carrier/escape pistons in column 5), or a crossing that does not need (3,4).

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
