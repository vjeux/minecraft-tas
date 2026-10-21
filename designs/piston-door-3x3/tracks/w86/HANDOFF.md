# w86 HANDOFF (smaller doors; box devvm59213 since 2026-10-20 05:20Z)

## State (2026-10-21 ~10:05Z, final push for vjeux's 09:00 PT deadline)
No wired door under 56. Best lead: the 7x7 (49) lever-held-trio family (NOTES "7x7 (49)" items 10-11).
- seven/trio7.{txt,ideal}: c70b's u2 with the 7x8 records' lever-held TOP TRIO (one real lever on SD+(1,4)'s top
  (1,5)); WORKS redsim 32x2 + game 4x2 (69/63), FULL frame, 7x7. Breaks u2's island-start wall.
- seven/trio7b.ideal: trio7 + Q's pull fed in-plane from (-2,2) (arrival-fire, o42 4 gt) + SD(-1,3) merge + SW(4,1)
  at o4/c4 (bottom right starts early). WORKS 32x2 + game.
- seven/fr22.{txt,ideal}, fr22b.ideal: (2,-2) freed (BR right into R0 via SW(4,0)), SU(1,-2) fed from (2,-2).
- seven/fr22ms / fr22st: macrosat SAT seeds on fr22 (plain / --strict), real lever, WORK 32x2 (templates only).
Walls (team: pockets3 lead, clk-item, clk-rt, fly, clockinfra, c70b):
- C1 (0,-2) on trio7 (lockstep only); fixed by fr22's (2,-2).
- A2 (-2,-2)(-2,-1): drives nothing with real blocks on any seed (clk-item); macrosat with A2 feeding nothing is
  UNSAT for fr22 at h16/h20 (sound --wire-dir 74e837865).
- B4 top-left: 6 ticks from 4 cells, no filling (pockets3 exhaustive k=4).
- trio7 fixed layout: --wire-dir UNSAT at h16 and h20 (clockinfra). fr22: --wire-dir SAT, --strict --wire-dir SAT,
  but A2:0 UNSAT.
Running at handoff (devvm59213 ~/agents/w86/runs/ms, macrosat with clockinfra's exact rules 0bbf27982 in
~/agents/w86/ci3; outputs x-*.out): fr22 --free bottom-left (x-fr22-freeBL), fr22 --free left two columns + bottom
middle at h16/h20 (x-fr22-freeL, x-fr22-freeL20), trio7 the same at h16 (x-trio7-freeL). fr22's fixed layout is UNSAT
at h16 and h20 under these rules. A SAT there is a new layout family: replay its export with a real lever (replace
the trio's z=1 rest hold and the (1,5) stand-in by a lever on (1,5) used at both flips), then satwire + pockets3's
island check + clk-item's A2 screen. Known model gaps I reported (2026-10-21 ~12:50Z): dust always powers the piston
under it, and dust pops off a piston that extends (redsim probe runs/bf/probe.*), so (-2,2) dust on SE(-2,1) is
impossible; bf16 (clockinfra's best seed, all rules) fails satwire exactly there (core open Q (-1,0)).

Earlier verdicts (still valid): 7x7 w1 class unwirable; t1 dead under the strict extras reading; inside-carried q7*
not wirable; 6x8 unwirable solid and sculk; u2-T start-order wall (macrosat UNSAT h16 with observers + physics).

## Tools on devvm59213
- ~/agents/w86/minecraft-tas (explore/w86); ~/agents/w86/mm (explore/c70b mechmut with --lever-held/--max-unfed);
  ~/agents/w86/ci (explore/clockinfra 38cb06cac), ~/agents/w86/ci2 (74e837865: the sound --wire-dir) for macrosat.
- `door ideal-rs --extent` (mine, on main): the real footprint of a plan over a cycle.
- Per-pulse windows: write one `let` per pulse and `ideal-rs --sweep v=lo..hi --independent --ok-only`
  (runs/u2t/u2TW.ideal, runs/trio/trioW.ideal).
