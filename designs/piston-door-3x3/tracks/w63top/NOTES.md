# Track w63top: finish the w63 top (7x8-014 in a 7x9 box, rows -2..6, bottom pockets real)

## Results
- ideal/s1.ideal (pure ideal) and ideal/t1.ideal on w63t-1.txt (A driver chain + right-stack
  observer REAL, (4,4) toggled ideally): **WORK at 32 positions x 2 cycles** (open/close 77).

## Key idea: share (4,4)
- Forced cells (by hand, cell by cell): A's only clean driver at both A positions is (2,4) =
  observer facing EAST (fires A at (1,4) directly, at (1,3) by QC through (1,4) WITH updates at
  both edges; a conductor at (2,4) QC-arms A at (1,3) without updating it: A2 never retracts
  cleanly). (2,4) watches (3,4); (3,4) cannot be powered by anything (any signal into (3,4)
  QC-fires P_T and updates it), so (3,4) = observer facing EAST watching (4,4). The right stack
  (TR puller direct + MR puller by QC) is fed only by (4,3) = observer facing UP, which also
  watches (4,4). So every A pulse comes 2 gt after a right-stack pulse.
- Fix: A1 single, A2 a TRIPLE (pull, push, pull: odd count = one net move, 6 gt apart), A3
  single -> the right stack gets runs (1)(3)(1) = 5 pulses; its bottom watcher sees 2+5+2 = 9
  (odd): BR still nets one toggle. Busy-window counts: a run of k stack pulses 6 gt apart gives
  2,3,5,6 watcher pulses for k = 1..4.
- t1 windows (A1 = 0, even values usable with observer timing): P_T 2..7, B1 9..11, A2 triple
  start 13..18, B2 28..33, A3 37..49, U any, left triple start >= 33.

## Open problem (where it stands)
- P_T's only drivers: (3,5) observer facing UP (QC through (3,4)), (3,4) facing up (ties A to
  P_T+2: fails), (4,3) (taken by the right stack / lever only), a conductor at (4,3) (fires the
  right stack with P_T). With (3,5) = P_T's observer, the right column (4,4)<-(4,5)<-(4,6) has
  only the lever and P_T-coupled cells ((3,6), (3,5)) as inputs, so every A pulse after A1 needs
  a P_T pulse 8 gt earlier: fine for A2a (P_T 6 -> A 14), the question is A2b/A2c/A3.
  P_T extra pulses are harmless before A1 (open: pulls from an empty (1,3); close: blocked if
  TL is at (0,3) and (-1,3) is immovable) and on close after B2+4 (pull from an empty (1,3)).

## v2 (P_T and the right stack share one conductor at (4,3)) and why both plans stop (by hand)
- ideal/v2.ideal on w63t-2.txt (obsidian at (-1,3)): P_T may pulse BEFORE A1 (open: it pulls from
  an empty (1,3); close: its push T, C, TL, obsidian is blocked, so it does not even extend). With
  P_T + the right stack as ONE run of 3 (A1-8, A1-2, A1+4) the mechanism WORKS (4 positions);
  windows (pt = 4): B1 7..16, A2 12..21, B2 21..26, A3 28..40, U >= -4, left triple >= 26.
- Forced cells (each checked against the source rules): (2,4) observer facing east (A's only
  driver with updates at both edges, both A positions); (3,4) holds an observer or a block that
  is never powered by an emitter (an emitter into (3,4) QC-fires P_T AND updates it); the right
  stack's only feed is (4,3) (observer facing up, or a conductor that also fires P_T); P_T's
  drivers: (3,5) observer facing up, (3,4) observer facing up (A = P_T + 2: fails on close),
  (4,3) (the right stack's feed), the lever at (5,3).
- v1 (shared (4,4), P_T at (3,5)): the right column (4,4)<-(4,5)<-(4,6) can only be driven by
  the lever or through (3,5)/(3,6), and every (3,6)/(3,5) change fires P_T. A P_T pulse is
  harmless only before A1 or (both edges) after A3+4, but A3 itself needs a (4,4) change at
  A3-4 (P_T at A3-8: on open it pushes T into the empty (1,3)). DEAD.
- v2 (shared (4,3)): A's chain (2,4) <- trapdoor (3,4) <- conductor (3,5); (3,5) can be powered
  only by (2,5) (an observer watching B: TWO pulses per B motion, so A2/A3 get even counts), by
  (3,6) (dust, or an observer facing up on the lever) or (4,5) (the run chain: shared). The run of
  3 needs two inputs into one right-column cell; only the lever and (3,5)/(3,6) reach it. DEAD
  unless (3,5)'s double pulse is merged (no repeater can point into (3,5): its inputs would be
  (1,5) = B or (5,5) outside).

## The wall, stated once (hand case analysis, bounded: flat, slime-free, this mechanism and bottom)
1. A (both positions) is driven only by (2,4) = observer facing east watching (3,4).
2. (3,4) may not hold an emitter's target (QC-fires P_T and updates it); it holds an observer
   (facing east: watches (4,4); facing down: watches P_T) or a non-updating block (trapdoor,
   lamp, dropper) fed only through a CONDUCTOR at (3,5) or (4,4).
3. P_T's real drivers: (3,5) observer facing up (watches (3,6)), (4,3) (observer facing east on
   a lever at (5,3), or a conductor that also fires the right stack), (4,4) emitting west
   (only on a lever at (5,4)), (3,4) observer facing up (A = P_T + 2).
4. The right stack's only feed is (4,3) facing up (or the shared conductor of 3).
5. A2 and A3 must not be P_T-coupled: on close, any P_T pulse between B1 and B2+2 pulls A out of
   (1,3); on open, any P_T pulse after its real pull pushes T into an empty (1,3) unless (1,3)
   and (0,3) are full and (-1,3) is immovable.
6. Watching B (the only other late motion near A) gives TWO pulses per B motion (+2, +6), so A
   gets even counts per phase; no repeater can point into (3,5) to merge them (its input would
   be (1,5) = B or (5,5) outside).
=> A's late pulses need the right column (4,4)<-(4,5)<-(4,6) fed from (3,5)/(3,6) or a lever;
   with P_T at (3,5) every such input is P_T-coupled (1+5), and with (3,5) used as the column's
   input or A's conductor, P_T has no driver left (3+4). A stone at (3,6) is an invisible relay
   (power is no state) but then P_T's observer never sees its one change.
- Tested and failed: r7 (P_T-coupled 7-pulse run feeding A, the stack and P_T, every B1/B2
  timing): 0/561.

## Full frame (vjeux 03:36Z) and the lamp trick
- With the 3 static frame stones (0,-1), (-1,2), (3,0) (frame-stones.patch): t1f (w63t-1f, the
  real A chain + right-stack observer) and lamp-f WORK at 4 positions; `door sheet` says frame
  12/12 when open. So nothing in this track depends on those 3 cells being empty.
- LAMP TRICK (new): a redstone lamp watched by an observer gives exactly one observer pulse per
  lit interval edge, and an interval that ends 4 gt after its power ends MERGES with a power
  pulse starting that tick (the lamp's off tick was scheduled first: off then on in one tick,
  one observer pulse). B watched from (2,5) gives two pulses 4 gt apart per B motion; through a
  conductor (3,5) into a lamp at (3,4) (flag-2 state changes: no update reaches P_T), B1 at A1+8
  and B2 at A1+16 keep the lamp lit from A1+10 to A1+28: the observer (2,4) fires exactly at
  A2 = A1+12 (the merge) and A3 = A1+30 (the off edge). ideal/lamp(-f).ideal: WORKS (open/close
  81) with P_T pre at A1-2 + real A1+4 and the right stack run A1-8, A1-2, A1+4.
- What the lamp plan still lacks: A1's interval (power into (3,5) at A1-2 for 8 gt) and the stack
  run before A1, both from the right column, which has no clean input (the wall above).

## Pivot (04:12Z): single-U 7x8 mechanisms, extended-at-rest family
- u7/m7.txt + u7/m7.ideal (levels via u7/m7i.txt): the 009 block moves with every side puller,
  U and the T parker P (3,3) EXTENDED AT REST (heads in frame cells) and driven by LEVELS, only the
  elevator A (1,4) / B (1,5) pulsed. `door ideal --extra-offsets 28`: WORKS at 32 positions x 2
  cycles, open 41 / close 43; `door sheet`: frame 12/12 (static (0,3), (0,-1), (2,-1)). Windows
  (A1 = z): P's level change z+5..z+7 (both edges), B1 9..14, A2 12..19, B2 19..24, A3 26..36,
  U's level change >= z+28 (on close U must push the column back after A3), side levels any time.
- But m7 cannot be wired with one lever: with 3-high side stacks the bottom pistons (L0, R0, U)
  touch only the bottom rows, and nothing the top moves is visible from there (and vice versa).
  Every catalog single-U mechanism (the 9 in the 04:03Z artifact) has the same split.
- Wiring tricks for this family (by hand, from the source rules):
  * a WALL TORCH hung on a static frame stone at (-1,2) (torch at (-2,2)) powers L1 directly and
    L0 by QC (it emits into (-2,1)): a level-held 2-stack, turned off by powering the stone, e.g.
    a lever at (-1,3) on top of it (the lever then also weakly powers (0,3), (-2,3), (-1,4));
  * a level-held piston moves once per edge, so an observer on its base fires exactly ONE pulse
    per edge (retract: base -> moving -> landed 2 gt later, inside the busy window): the clean
    top->bottom link (box72's torch link), e.g. R0's watcher at (4,-1) driving L0 and U below.
  * still open: every elevator design has its T parker's QC cell next to A's driver (the w63 top
    wall, mirrored for a parker at (-1,3)).
