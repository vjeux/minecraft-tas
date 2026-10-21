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
* 6-wide topology, worked out (20:30Z), for anyone trying a 6x9 / 6x8 of this kind:
  - Only one side can carry the slime structure, which needs the extra column (x=-2). The other
    wall sits on the box edge, so its door column (x=2) must make a 2-cell vertical move plus one
    horizontal shift (K at (3,-1) or K' at (3,3)). That costs pistons in a corner area.
  - Corner areas are blind: an observer at x=3 or in the edge row that watches a mechanism cell
    always outputs out of the box. The bottom-right can only see B1 from (2,-3), so (2,-3) must
    stay free and B1 must be fed from (0,-3).
  - The top (lever area) reaches the bottom-left only through the slime (a dragged rider). On close
    the slime must therefore return first, so O must leave (0,-1) downwards (D_O at (0,-3)), and
    that takes B1's feed. These two requirements exclude each other in 6x9.
  - The top could see the centre column through an observer at (1,4) watching (1,3) (output into
    (1,5)), but (1,4) is where P_T must sit to pull C up.
  - The bottom-left could push the slime itself (a piston at (0,-1) pushing the rider W) and the top
    could watch a rider at (-1,5) from (0,5) (12 blocks moved, exactly the limit). But Qv's only
    driver cell is (0,-1).
  - Conclusion: this mechanism family is not wireable in 6x9 without a new idea for the box-edge
    column or for driving Qv.
* Next directions (coordinator 20:29Z):
  - (1) toggle-only top, one island: clk-obs.
  - (2) the slime moved from the BOTTOM (Pb at (0,-2) facing west, L down to (-1,-2)), so the
    whole close order (bottom-right -> bottom-left -> slime) lives at the bottom and the top only
    watches the slime (observer at (-2,4) facing down -> row 5). Open issue: (0,-1) next to L's
    slime is poisoned (dragged), so Qv's driver must be at (2,-1) (observer facing east watching
    K) or a rider.
  - (3) corner cells that can watch the mechanism.
  - Rule check: docs/rules.md says the 3 extra blocks must be in place in BOTH rest states, so an
    empty right frame column at rest is not allowed.
* Direction analysis (20:30Z-21:00Z), with clk-obs measuring on q68f:
  - Close order (q68f-type bottom): bottom actions -> P_T first spit (must END before the slime
    leaves) -> slime return -> P_T second spit. clk-obs: the slime cannot return first with
    q68f's bottom (0 of 3751 timings). q69i/m reverse it (O pulled down by D_O): top first.
  - The top's only eyes are (-2,4) facing down (the slime at (-2,3)) and (3,4) facing down
    ((3,3)). So the bottom can only signal the top through the slime, or through the x=3 static
    column used as a BUS: pushed up by one ((3,2) into (3,3)), then pushed back down from above
    (a sticky retraction brings back only one block). A bus pusher must sit under (3,0), which is
    K's cell, and K has no feed if it rides on the bus. Unresolved.
  - Every 6-wide assignment of the 9 blocks needs one 2-cell move (an elevator) for the centre
    column and one for the box-edge column, plus one horizontal shift (K or K'). An elevator needs
    2 rows beyond the frame, so the possible 6x9 row splits are:
    - y -3..5: centre elevator at the bottom, Qr + lifter at the top. The lifter sits in row 5,
      so the top has no wiring row and splits into two areas.
    - y -2..6: wiring row 6, but only 1 row at the bottom, which leaves no room for a centre
      elevator.
    - y -4..4: both elevators and a wiring row at the bottom, but the top row (Pl, P_T) has no
      feed. An outside lever on P_TR's back face (2,5) holds P_T (QC) and P_TR, but then P_T's
      head at (1,3) blocks the slime's return.
  - Every one of these gaps has a structural cause. The hand-design space of this family looks
    exhausted for 6x9; the next step is a search with island/feed checks inside the mechanism
    search, not after it.
* New elements (21:10Z), not yet used in a design:
  - SELF-INVERSE TOGGLE ELEVATOR: a puller Q (sticky, retracted) and a lifter L under/over it,
    every input a 2 gt pulse. The train L, Q, L, Q moves a block 2 cells on open, and the SAME
    train undoes it on close (checked by hand). A top elevator for C, P_T (1,4) + M (1,5), with
    Pl and a pull-type Qv (1,-2), gives the centre train P_T, Pl, M, P_T, M, P_T, Qv, which opens
    the centre (TM to (0,3) via the slime, C to (1,3), BM to (1,-1)) AND closes it when replayed
    in the same order. A single train for both edges is exactly what observer dominoes / `door
    interleave` need.
  - J-SLIME: L = (-1,-2..2) + (0,-1), pushed by Pb (0,-2) facing west (6 slimes + 4 dragged).
    The (0,-1) arm drags (1,-1)'s block into the bottom-left corner. It cannot also take the
    (0,3) corner (13 blocks), so (0,3) becomes a 4th static and one door block must hide.
  - BOTTOM-ROW SHIFTER: a slime arm at y=-2 under the bottom frame row ((0,-2),(1,-2), pushed
    from (2,-2)) shifts the row west when L moves: (1,-1) to (0,-1), and (0,-1) to (-1,-1), which
    hides a block there (7 slimes + 5 dragged = 12).
  - Invariant obstacle in 6 wide: the box-edge column's middle block MR. Hiding it in the x=2
    column conflicts with that column's pullers, so it always needs a corner slider (K at
    (3,-1) or K' at (3,3)).
* `door selffeed --anywhere` (tool, 49a191c8b5): carried-observer candidates (every free cell
  and facing). On q69m: 0 of 11 inputs (its inputs are levels and multi-pulse; selffeed needs a
  2 gt train). `door interleave --search 1M --levels --segments`: no train for q68f / q68g /
  q69j / q69m (18-20 cells off). Next: toggle-form plans (every input a 2 gt pulse), designed so
  that the open train replayed closes.
* r69 (21:20Z, 6x9 = 54b, frame 12/12, 9 pistons): built from the self-inverse toggle idea.
  - Pieces: top elevator M (1,5) + P_T for C; the slime for the left column and TM; Qv (1,-2)
    pulls BM; A pulls BR; K pushes BR + BM west; bottom elevator A + B for MR; P_TR pulls TR; G
    (-1,-1) is a sticky that rides the slime (dragged to (-2,-1) on open, brought home on close)
    and pushes BM back east on close.
  - Runs from ONE hand-made train of 15 2-gt pulses, the same on both edges.
  - WORKS: redsim 32 x 3 and the game 32 x 2, open 87 / close 87.
  - Unwireable top: P_T (1,4) is boxed by Pl, P_TR and M. Its only feed is QC through (1,5),
    which also powers M, and M is updated first and pushes P_T away (clk-obs).
  - Co-firing A with B (A's feed into (2,-2) QCs B) is harmless in redsim.
* r69a: r69 without M. Qv (1,-1) touches BM at rest (its spit raises the middle column so P_T
  reaches C), and B1 (1,-3) drops Qv. P_T is fed from (1,5) and the top is one area. 2 gt
  pulses, with different open and close sequences. WORKS redsim 32 x 3.
* r69t: `door interleave --search` found ONE train for r69a in 19k pairs.
  - 17 2-gt pulses, the same ticks on both edges, last pulse t33, open 36 / close 32.
  - WORKS: redsim 32 x 3 and the game 32 x 2.
  - Train: P_T 0 9 | P_TR 0 | Pl 5 | Qv@(1,-1) 2 4 | Qv@(1,-2) 0 17 | B1 12 | G 9 24 | K 22 |
    B 25 29 | A 0 33 | A@(2,-1) 26.
  - The first single-train sub-56 door. Wiring next: island-aware trains, pockets3 / clk-obs.
* Wiring verdicts on r69t (21:30Z-21:50Z):
  - clk-obs (exhaustive, 4.5M fillings of row 5 + (3,4), 6 lever spots): the top can't be
    wired. P_T (two pulses) and Pl (one pulse between them) both draw from the (1,5) hub, and
    nothing in the top can time a pulse after the bottom's Qv spit.
  - pockets3: the bottom-right {(3,-2),(3,-3)} sees only A, B and K, so it can never start, and
    it would have to make 5 ticks from 2 cells. Qv@(1,-1) and A@(2,-1) have no feed at all.
  - interleave --islands: no train in 3M pairs.
* s69 (slime-free, mechanism box 6x8, frame 12/12, 12 pistons):
  - Left column: L0 and L1 at (-2,0),(-2,1) are held by a lever outside the box on L1's back
    face, extended at rest and moving at t0 of each edge. L2 (-2,2) is a toggle that must fire
    at t0 too: its feed QCs L1 and would re-extend it.
  - P_C (-1,3) pulls TM into (0,3). G (-1,-1) is static and pulls BM into (0,-1) / pushes it
    back.
  - Pl is gone, so P_T has free sides ((0,4), and (1,5) by QC also for P_T at (1,3)), and the
    top is one area.
  - WORKS with ideal inputs, redsim 32 x 3, open 81 / close 63. No single train in 4M pairs (2
    cells off).
* STRUCTURAL RESULT for 6-wide one-sided designs (the extra column on one side only):
  - The box-edge column (x=2) has 3 blocks and 2 vertical exits, so one block moves
    horizontally (K at (3,-1) or K' at (3,3)), and the middle block needs a 2-cell move (an
    elevator: puller + lifter).
  - In 6x9 every placement puts that machinery in a blind corner:
    - Bottom elevator A + B with K: {(3,-2),(3,-3)} is enclosed by K, A and B. It sees only K,
      and the lifted puller at (2,-1) has no feed.
    - Top elevator P_TR + M_R with K': M_R at (2,5) cuts {(3,3),(3,4),(3,5)} off from row 5, and
      observers in the x=3 column can only watch the column itself.
    - Without K, by moving the extra row (y -4..4): the top then has no row 5. s69's top needs
      only (0,4) and (-2,3), but P_TR is alone with {(3,3),(3,4)}.
  - Wiring needs every corner piston fed from outside its corner. The only conductors there are
    the statics and transit door blocks, and power does not chain through stone.
  - So this family is out for 6x9. The next family worth it is 7 wide (both sides have a spare
    column: lever-held side stacks + toggles), i.e. 7x7 = 49.
* 7x7 check (22:00Z): every 3x3 door needs C, the centre block, to make a 2-cell move. In 7x7 no
  side has 2 spare rows or columns, so:
  - an elevator is impossible;
  - a column push from (1,3) or (1,-1) needs a piston parked in a frame cell, with no room to
    vacate (the S-stone trick pushes the column down by 1, but then TM is stuck);
  - sideways moves need a puller reaching from the frame cell (-1,1).
  So 7x7 is blocked for these families too (consistent with macrosat's strict UNSAT at short
  horizons). Sub-56 boxes: 54 (blind corner), 49 (C's 2-cell move), 48 (worse than both).
* pockets3 on m69T (22:00Z): with the lever in the BOTTOM-RIGHT area every area starts on both
  edges; with the lever-held stack trick (lever on R1's back), the bottom-right's first pulse
  must move to t3 or later so that an observer sees R0 retract.
* `fullmech --complete` cannot check the box-edge column claim at this size: calibration with
  r69a's own 4 right pieces (k=4) refinds nothing (the exact solver's horizon is shorter than
  r69a's 17-pulse sequence).
* m69T (c70b's 6x9, pext's one-train p33; with pockets3, 22:10-22:30Z). New `door shift-plan`
  re-times a plan; a shift can be `@name` for `ideal-rs --sweep`. Results, redsim 32 positions:
  - Whole train delayed k = 1..6 behind the R0..R2 levels: WORKS.
  - R0 or R2 as a TOGGLE (retracted at rest, one late pulse) fails after tick 1.
  - As LEVELS (extended at rest, as in mm02), the stack is TIMING-FREE: R1+R2 switch at tick r,
    R0 at tick q, after each flip. All 1681 combinations of r, q in 0..40 WORK (the open just
    ends at max(33, r+1)); the real game agrees at r=17 q=29 (32 x 2).
    wip/m69T/m69T-p33-stackfree.ideal.
  - So the stack needs no flip-time signal. R0 takes a level into (4,-1) from the bottom-right.
    R2+R1 take a level into dust at (4,3) on R2 (R2 directly, R1 by QC through R2's cell), or
    the lever on R2's back.
  - Each end can be a toggle clocked once per edge by anything its own area sees. The lever can
    go where the areas' island starts need it.
  - Open problem (pockets3's island check): on the OPEN edge the two bottom areas see nothing
    from the top before their first pulses. The bottom-left (-1,-3..-1) only touches (0,-3),
    (0,-2), (0,-1) and a static, and its observers facing x=0 output out of the box.
  - pockets3 (22:30Z): no lever placement starts every area. The areas form two groups,
    TOP = sealed top-left + top-right and BOTTOM = bottom-left + bottom-right, with no visible link
    between them. The stack doesn't bridge them: R0 is seen only from (4,-1) (bottom) and R2 only
    from (4,3) (top).
  - Per-group re-timing of p33: delaying the bottom inputs fails from d=1; delaying the top fails
    from d=3. So with p33 the groups must start within 2 ticks, which no visible link
    (observer + block move, at least 3-4 ticks) can deliver.
  - But other schedules let one group wait. interleave --search with `--after=cell=6`:
    - bottom held to t6 or later: ONE TRAIN, 26 pulses, open 40 / close 40
      (wip/m69T/m69T-bot6-train.ideal);
    - top held to t6 or later: ONE TRAIN, 26 pulses, open 43 / close 43
      (wip/m69T/m69T-top6-train.ideal).
    Both WORK at redsim 32 x 3. With the lever on R1's back, the bottom-right starts from R0
    (observer at (4,-1)), so top6 needs only a bottom-to-top link before t6. bot6 is the mirror:
    the top's (1,3) spit at t4 lands BM on (1,-1) at about t6, but no free cell touches (1,-1).
  - Link candidates: the 3 extras may move mid-cycle (rules: in place when open and when closed).
    m69T's extras column (-1,0..2) runs from the top-left to the bottom-left, so pushing it 1 down
    (or up) is a visible crossing. Catch: a 1-wide column's arrival cell is seen only from the
    column's continuation, because the side cells are outside the box or mechanism cells. That is
    exactly where the restoring piston must sit. The same holds for the stack column (x=4).
    `mechmut --reserve -1,3 --validate --repair` on mm02 (to free W's cell for a pusher): 0 of 14
    neighbours work.
  - General: every sub-56 box (6x9, 9x6, 7x7, 6x8) splits into two groups (top/bottom, or
    left/right). The door's 3 rows plus the static/stack columns wall them off, so links must be
    moving elements, and the schedule must let one group wait.
  - RIDING OBSERVER (a link that gets round the end-cell catch): a frame cell may hold anything at
    closed rest (only the 3 extras must be in place). Park an observer in the END frame cell of a
    door column, or on top of the extras column at (-1,3), and it rides when the other group
    pushes that column. It fires on arrival (the s69o rule, verified) and outputs SIDEWAYS along
    its facing axis into a free cell of the far region. Example: extras pushed UP by a bottom
    piston at (-1,-1); an observer at (-1,3) facing west lands at (-1,4) and powers s0 (0,4). A top
    piston at (-1,5) pushes the column back down.
    Cost: in m69T those cells hold W / U / the (1,3) and (0,-1) pistons. An observer at (1,-1)
    breaks bot6 in every facing (redsim), so it needs a mechanism redesign.
  - bot11p (wip/m69T/m69T-bot11p-train.ideal): with the centre spit forced at t0 (`--prefix
    1,3,1@0`) and the bottom held to t11 or later, a 21-pulse train works (open 31 / close 31).
    BM sits on (1,-1) from t3, but only U, K's front and the (0,-1) B piston border it, and U
    can't go: C's descent at t17 needs a puller below. clk-obs: the bottom's first pulse may
    only come at t0-4, t8, t9 or t11, while an observer on (1,-2) would fire at t5. m69T stopped.
  - Tool fix: interleave's --after was not enforced on the evenly spaced start (compaction only
    pulls steps earlier), so trains made before ab90d91ce8 could fire below their floor.
* Ranking by pockets3's corrected island check (23:14Z):
  1. q69m: the only one whose islands pass (lever in the top-right; the slime column links top
     and bottom). Pulse shapes fail in every area.
  2. s69: every area starts except the bottom-right 2-cell corner, with the lever outside on L1's
     back. Exhaustive pockets: the corner drives ONE of K o48 / B c42. Never drivable: Qv (1,-1)
     o6/o18 (boxed, no feed cell), B1 o24/c66, P_T's t0 pulses, A and A@(2,-1) entirely,
     B o54/o66/c54, K c36. Drivable: L2 t0, P_T t12, P_TR o78+c60, P_C t6, G o36+c24, L0 t0,
     Qv@(1,-2) o30 or c18.
  - Why the box-edge column is load-bound: its middle block needs an elevator (2 pistons, about
    4 events), and those pistons are fed only from x=3 (the blind corner) or x=1. The centre's
    own elevator holds x=1 ((1,-2) is needed for BM's descent in every allocation I tried).
  - clockinfra's macrosat now has movable and riding observers (`--observers`; s69o checks SAT).
    It is running s69 with x 1..3 of the bottom and the top-right free, corner budget 1 action
    per edge, observers on, `--physics`.

## Overnight push (2026-10-21 00:46Z ->, door lead; deadline 16:00Z)
The coordinator made this the final time-boxed push and gave me the consolidated write-up:
docs/history.md "Below 56: why 7x8 holds". The team's results are there; this is my own part.
* s69r (O_Q resting in (0,-1), pushed up into (0,0) by D_Q at (0,-2), to fire Qv through BM):
  BROKEN at every timing. D_Q before t4 pushes the left column up (L0 hasn't pulled BL yet).
  Later, O_Q back in (0,-1) sits in G's t36 target cell. No interleave train (6M pairs, 2 cells
  off). clk-rt's shuttle tool then fed Qv properly (s69r on explore/timer, game 4x2).
* s69's left stack is timing-free like m69T's: L0+L1's levels can switch at any tick 0..40 after
  the flip (41/41).
* q69c-sh0: q69c with its boxed Qv fed by `door shuttle` (S (3,-3) up carries O (3,-2) east onto
  (3,-1); it also absorbs A's two t20 pulses). WORKS redsim 32x3 + game 4x2. q69c-sh0p = prune-plan
  of it (4 close pulses), same checks. pockets3: every piston has a free-cell feed; solid islands
  fail at ONE cell on ONE edge.
  - Why (sweeps, /tmp/fly/q69c-shuttle/sh0v.ideal): S's open pulse window is o0..23 (BROKEN
    without it). Its close pulse works anywhere in c0..49 and also c51, c53, c56-58. B1's open start
    window is o24..33. (2,-3) touches only B1, A and S, so nothing it sees moves before o24.
* `door prune-plan` (new): drops the pulses a plan works without, together. Greedy latest-first,
  or `--exhaustive` over <= 16 candidates; `--only` limits it to one area. On q69c's bottom-left,
  at most 2 can go together (Qv c62 + (1,-2) c50): wip/q69c-blp.ideal.
* Tool fix: `ideal-rs --compact-dir` / `--offplane-dir` dropped the plan's `phase` line (q69m-cpt
  replayed BROKEN in the default phase).
* Tricks vjeux asked about (copper bulbs, BUD, carried redstone blocks, rails, honey, 0-ticks,
  sculk), ranked and farmed out. Carried RBs and BUD were already in macrosat. Bulbs gave nothing
  (clk-item). Sculk works only through an observer reading the sensor (the echo, clk-item).

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
