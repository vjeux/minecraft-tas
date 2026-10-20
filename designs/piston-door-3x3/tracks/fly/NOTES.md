# fly: a 3x3 door smaller than 56 (from 2026-10-20 17:40Z)

Coordinator brief (vjeux: "the door is still #1 priority"): beat 56 (7x8x1, e78k-h5-41b) on the
Solid board: volume < 56, rotation-safe, FULL frame 12/12 when open, solid category. Slime is
allowed on that board (it has its own "no slime" board); every earlier sub-56 hunt (c70b, w86,
macrosat, pockets3, d67) ran under the slime-free model. My angle: slime / observer
"flying" sub-assemblies (one piston moving a whole group, self-timed by observers riding the
moving parts), and box shapes nobody tried.

## q68: a 6x8 = 48 mechanism, FULL frame, ideal inputs WORK (2026-10-20 19:08Z)
`wip/q68.txt` + `wip/q68.ideal`: x -2..3, y -3..4 (the slime fills x = -2 only when open), 9
sticky pistons + 6 slime + 3 static stones (right wall). `door ideal-rs` WORKS at 32 positions x 3
cycles, `door ideal` (the real game) WORKS at 4 x 2; frame 12/12 when open (trace).
* Left: slime column (-1,0..4) + (0,3), pushed west by Pl (0,4): TL ML BL into the wall in one
  move, and the (0,3) slime drags TM from (1,3) into (0,3).
* Center (vertical Q trick): Qv (1,-1) pushes BM C TM up and pulls BM back (C on (1,2), TM on
  (1,3)); after the slime move P_T (1,4) pulls C into (1,3); B1 (1,-3, extended at rest) pulls Qv
  down to (1,-2) and Qv pulls BM into (1,-1); G (-1,-1, extended at rest: immovable while the slime
  moves) pulls BM on into (0,-1).
* Right (box edge x = 3, static wall): P_TR (2,4) pulls TR into (2,3); A (2,-2) pulls BR into
  (2,-1); K (3,-1) spits BR west into (1,-1); B (2,-3) lifts A, A pulls MR to (2,0), B lowers A, A
  pulls MR into (2,-1) (e78k's elevator on the right column).
* Close: the reverse with spits; L's return drags TM back to (1,3), and P_T pushes the column
  TM C BM down by one into its rest (the column was rebuilt contiguous at (1,1..3)).
Open 18 inputs, close 17 (one per piston action). Next: wiring inside the 48 cells.
`wip/vq78.*`: the same center in a 7x8 with classic right pullers (7 pistons), WORKS 32 x 3.
q68 also WORKS in the real game at 32 positions x 2 cycles (`door ideal --extra-offsets 28`).

### Wiring progress (pockets3 + me, 19:15Z-)
* pockets3: satwire UNSAT with core Qv (boxed: G's head, the transit cell (2,-1), BM, B1's head);
  free areas bottom-left 7 cells, top 8 (with row y = 5, box 6x9 = 54), bottom-right 2 (overloaded:
  A, K, B need 8 distinct ticks); the three areas are islands (none can start without the lever).
* q68c (P_T / P_TR extended at rest, e88's lever trick): the lever re-extends them at close t0 and
  blocks the slime's return, so not usable as is.
* q68e: Qv's boxed actions come from a REAL observer O at (0,-1) facing west, fired by a normal
  piston N at (-2,-1) whose head enters / leaves (-1,-1): the two 2 gt pulses are a spit (BM, C
  up) and a reach-and-pull (BM back) = the wiggle. G is gone: K's spit pushes BR, BM and O west
  (O lands in the corner (-1,-1)); on close N pushes O and BM back east and stays extended while the
  slime returns (a retracted N next to (-2,0) gets dragged east), then N's retraction fires O for
  Qv's last spit. Qv's other actions happen at (1,-2), fed from (0,-2). WORKS redsim 32 x 3.
* q68f: q68e with q68's pull-type ceiling (only B1 powered at rest). WORKS 32 x 3.
* q68g: the TOP starts the bottom-left: a normal piston W rests at (-1,-1) stuck to the slime; the
  slime's move drags it out, O fires (Qv spits BM C up), N (-1,-2, facing up) puts its head in (O:
  Qv reaches and pulls BM back) and out after B1 dropped Qv (O's signal into the EMPTY (1,-1)
  QC-powers Qv at (1,-2): it reaches and pulls BM in). On close W pushes O + BM back east and its
  retraction fires O (Qv at (1,-2) spits BM up through QC), and the slime's return drags W home
  (O: Qv's last spit at (1,-1)). Qv has NO input; the bottom-left needs N, W, B1 only. Ceiling as
  q68c (P_T held at rest). WORKS 32 x 3.
* q69h (6x9 = 54, rows -3..5): q68g's left + center, and the right column as a second vertical Q
  from the ceiling: Qr (2,3) sticky down spits TR MR BR down (BR into (2,-1)) and pulls TR back,
  Br (2,5) (extended at rest) lifts Qr to (2,4), Qr pulls TR into (2,3); K spits BR BM O west, A
  pulls MR into (2,-1); close: A spit, K pull, Qr spit, Br on, Qr spit, A spits the column up.
  B removed; the bottom-right (K, A + free (3,-2) (3,-3) (2,-3)) can watch B1 from (2,-3). Open
  chain: top lever -> slime -> bottom-left (W/O) -> B1 -> bottom-right. Close problem: the order
  must be K pull (bottom-right) -> W push (bottom-left) -> slime return (top), and nothing the top
  does is visible from the bottoms before the slime returns. WORKS 32 x 3.
* q69c (6x9): no wiggle: a top elevator (A' (2,4) + B' (2,5)) lifts MR two cells, K' (3,3)
  moves TR west into (1,3), an e78k-style center elevator, G moves BM west, A pulls BR down.
  WORKS 32 x 3, but G at (-1,-1) blinds the bottom-left (on close the slime drags anything at
  (-2,-1) into G), so it cannot be wired with one lever. Dropped.
* q69i (6x9 = 54): q69h made wireable on paper, so every area starts on something it sees:
  - The lever is in the top-right (Qr, Br), the only part of the top that has no view at all.
  - The top-left watches Br from (1,5).
  - The bottom-left sees W, which the slime drags.
  - The bottom-right sees B1 from (2,-3).
  - D_O (0,-3) pulls O down to (0,-2) before B1 drops Qv. K then moves only BR and BM, so
    (-1,-1) stays free and the slime returns FIRST on close.
  - The close does not retrace paths (all door blocks are stone): B1 blinks BR up into (1,0),
    W pushes BM east, K + A send it up the right column, and D_O brings O home LAST, while the
    column BR C TM is jammed under P_T's extended head, so O's arrival pulse cannot move Qv.
  - WORKS 32 x 3 (open 105 / close 81).
* Rules learned on the way:
  - An observer in the top row (or at (3,3)/(3,4)) watching a mechanism cell always outputs out
    of the box, so the top runs on timers.
  - A moved observer fires on arrival.
  - A powered piston whose push is jammed does nothing, which makes stray pulses harmless.
* q69m: q69j with B1 pull-type (retracted at rest, (1,-2) empty). O arriving at (0,-2) QC-powers
  it through the empty (1,-2), and it reaches up and pulls Qv down: the open needs no B1 input.
  At N's retraction, Qv (powered directly by O) moves first and B1 (QC) is blocked. WORKS 32 x 3.
* Wiring verdict on q69i/j/m (pockets3: per-edge island check and exhaustive pockets k=4 per
  area; pext's interleave --islands):
  - The BOTTOM-RIGHT is isolated. Its only views are B1 (from (2,-3)) and K (from (3,-2)).
    With D_O at (0,-3), B1 can only be fed from (2,-3), and on close the floor must be cleared
    by A and K before anything else can move.
  - Every area needs pulse shapes its cells cannot make (e.g. Qr's single one-edge 6-gt pulse in
    3 cells). Cheap signals are: a lever level (lever area only), 2-gt observer pulses on visible
    changes, repeaters, torches.
  - Toggle semantics fit cheap signals. A sticky piston given 2 gt spits a block it touches and
    reaches back for a block one cell away, so one pulse train can do open and close:
    - Pl: spit on open, reach-pull on close.
    - P_T (pull-type): reach TM / reach C on open; spit C / spit TM+C on close.
    - Br as a pull-type toggle, plus Qr fed from (3,3) and (3,4): train [Qr3, Qr3, Br, Qr4].
      On close, Br's spit lowers Qr and TR together.
    - O returning to (0,-1) at the end fires into Qv; that is harmless only if the column is
      jammed (P_T extended), or it can do the job of spitting the floor stone at (1,-1) up into
      (1,0).
  - Core flaw: the box-edge right column needs bottom pistons (A, K), and they cannot be
    reached. Next: a right column with no bottom-right pistons, or a centre escape that frees
    (1,-3) so the bottom becomes one island.

## Box shapes under 56
A FULL frame needs the 12 frame cells in the door plane, so the door plane is at least 5x5.
* depth 2: only 5x5x2 = 50 (5x6x2 = 60). Never tried (the smallest 3D door on the boards is
  126b, 7x9x2).
* flat: 7x7 49, 6x8 / 8x6 48, 6x9 / 9x6 54, 5x10 / 10x5 50, 5x11 / 11x5 55, 6x7 / 7x6 42.

## 5x5x2 = 50: ruled out (argument, 2026-10-20)
Planes A (z = door, 5x5: opening O, 12 frame cells F, 4 corners) and B (z = door+1: ring Br of
16, center Bc must be air at rest and open).
1. No block ever changes plane inside the box: a piston pushing across z pushes out of the box
   (z = -1 or 2), and a sticky pull across z needs its base at z = -1 or 2.
2. Bc can only be entered by piston HEADS of Br pistons facing in (anything else would need a
   force from outside the box or from a piston resting in Bc); a head is not sticky sideways, so
   nothing in Bc can ever drag a door block. Plane B can only drag plane-A frame / corner blocks.
3. So every door block leaves O by a plane-A force: an edge-middle block (ML) moves west only by
   a row push from the east (a pusher in O or in the opposite frame cell) or by a slime next to it
   in O moving west, and a slime can enter O only from F, which needs a force from outside the
   5x5 or from inside O. The 9 blocks + the 12-block push limit leave no way to empty O and fill
   all 12 frame cells with plane-A pistons standing in the 4 corners and the frame cells (which
   must hold door blocks when open). Dropped.

## Flat 7x7 / 6x8 / 6x9 with slime
State of the art (c70b smaller/NOTES.md): 7x7 mechanisms exist (c77 w1, u0 / u2) but need 17-19
piston actions per edge from free cells that form 5 islands (wiring UNSAT). Slime lets one
piston move a whole column or row (up to 12 blocks) and an observer riding the moved group
sequences the next piston with no wire, so the action count per edge can drop a lot.

### Side structures (redsim, 2026-10-20 17:56Z, /tmp/fly/s0.txt + s0.ideal)
7x7 (x -2..4, y -2..4). Left: slime in the wall cells (-1,0..2), up through the corner (-1,3) to
(-1,4), pushed west by a sticky piston at (0,4) facing west; right mirrored ((3,0..4), sticky at
(2,4) facing east). ONE level action per side: the slime goes to x = -2 / 4 and drags TL, ML, BL
(TR, MR, BR) into the wall cells; the sticky retraction brings everything back. 8 blocks per push
(limit 12). Constraints it leaves: x = -2 / 4 (y 0..4) empty at rest, and every cell next to a
moving slime that is not meant to move must be air at rest: (0,3) and (2,3) (next to the
corners) and (-1,-1), (3,-1). Routing through the bottom corner instead frees (0,3)/(2,3) and
blocks (0,-1)/(2,-1). A slime next to a stone that should end in a frame cell drags it there
(e.g. slime at (0,-1) drags a stone from (1,-1) into (0,-1)).
Open problem: the center column. C is 2 cells from every frame cell; the 7x7 has one row above
the ceiling and one below the floor, so a single pull reaches only 1 cell.

### Why the center stays hard with slime (2026-10-20 18:50Z)
* A slime that drags a block into a frame cell ends beyond it (outer ring) in the drag direction;
  a slime (or piston) inside the 3x3 needs 2 moves to leave it. So C's two moves are: a first
  move onto an edge cell (a push of a line from a ring piston, or a pull by a sticky face in the
  3x3) and a final pull from 2 cells beyond the frame (P_T at (1,4) pulling C from (1,2) into
  (1,3): cheap).
* Every moving slime "poisons" its neighbours: a cell next to a moving slime that must not move
  must be AIR at that moment. A routed wall column therefore blocks (0,3)/(2,3) (top route) or
  (0,-1)/(2,-1) (bottom route) from holding a static frame block. Dragged extras fix that: the
  corner route through (0,3) drags whatever sits at (1,3) into (0,3) (a stone, or TM after P_T
  pulled it up), and the same at (0,-1)/(2,-1) from (1,-1).
* Q trick (new, redsim-checked pieces): a sticky piston Q in the left wall middle (-1,1) facing
  east, stuck to the left structure's slime. Q pushes ML, C, MR east (MR pushes the right
  structure's slime (3,1): that push moves the whole right structure, TR / BR dragged into the
  wall, no pusher needed), Q's sticky retraction pulls ML back: the CENTER IS EMPTY. Then the left
  structure drags Q into the margin (-2,1) (one escape move) and Q pulls ML into (-1,1). w1's
  lemma (the ring piston needs >= 2 escape moves) assumed a separate margin puller; here Q is it.
  What remains: C sits on (2,1) next to MR, still 2 cells from a free frame cell ((2,3), (2,-1)).
* Vertical version (Q in the floor middle pushing BM C TM up, TM dragged into (0,3) by the left
  corner slime, P_T pulling C into (1,3)) works up to Q's escape: Q must leave (1,-1) downward,
  and a down-moving slime next to Q needs a pusher in a cell the wall structures need.

* Reversibility (paper, 19:00Z): the horizontal Q push sends MR into (3,1); on close MR needs a
  westward force, which only a piece in the outer column (4,1) can give, so a 6-wide box can't
  close it. Vertical version with an e78k-style escape: Q_v sticky up at (1,-1), B1 sticky up at
  (1,-3) EXTENDED at rest holding Q_v; open: Q_v pushes BM C TM up, pulls BM back, the left
  structure's (0,3) slime drags TM into (0,3), P_T (1,4) pulls C into (1,3), B1 pulls Q_v down to
  (1,-2) and Q_v pulls BM into (1,-1); close: L returns TM to (1,3), Q_v + B1 push BM up 2, P_T
  pushes TM C BM down 1 = rest. Fits y -3..4; open problem there: the right column in 6 wide
  (MR has no frame cell 1 move away once (3,1) is static).

### Searches
* `fullmech --anneal 7x7 --slime --helpers` from scratch, all offsets, 30 min: nothing.
* Seeded with the two frozen side structures (`--anneal-seed /tmp/fly/s0.txt --anneal-freeze
  99 --frame-full`), 20 min, 265k evaluations: nothing.
* New exhaustive `fullmech --complete` (every placement of k pieces from a palette in given
  cells added to a frozen seed, each solved exactly). Calibrated: fm-7x8-000 minus 2 pistons
  re-found (5 of 1313 candidates, 7 s). Seed sW (left top-routed with (0,3), right bottom-routed
  with (2,-1), P_T (1,4), P_B (1,-2), statics (2,3) (0,-1)): k <= 2 over 16 free cells (pistons,
  slime, stone, obsidian) 0 of 24,688; k <= 3: 0 of 1,579,384.
