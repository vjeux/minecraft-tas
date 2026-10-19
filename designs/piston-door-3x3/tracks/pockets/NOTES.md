# Track pockets: the pocket-search workhorse (door cosearch pockets, broadly)

Brief (coordinator for vjeux, 2026-10-19 05:47Z): run cosearch2's `door cosearch pockets` broadly on every
mechanism we have (7x8 = 56, 7x9 = 63, 8x8 = 64) and keep every working door; first the tool items
(periodic rest rule, off-plane lever, region closure). Box: devvm49255 (dedicated, 166 cores).

## 1. Tool items (explore/pockets)

* `d976c0f` PERIODIC rest in the screen (main 96a62d6): a closed state may differ from rest as long as the
  door cells are closed and some closed state repeats within cycles + 8 (extra cycles must open with
  frame 12/12 and close); `--exact-rest` keeps the old rule. On c79's bottom-right region the periodic
  screen keeps 116,943 fillings working vs 110,817 exact (bulbs, note blocks, lamps in the palette).
* `586d9e8` OFF-PLANE lever: the grid keeps a lever on a block's front/back face (seeds with one load,
  finds write one, `tracks/pockets/seeds/c79-front.txt` = c79 with the community's front-face lever: WORKS);
  lever candidates on the front face of a filling's own full block or of a static block next to the
  filling (`--no-lever-front`; `--lever-cells x,y,-1`); `--lever-polarity seed|off|on|both`.
  WARM retry (needed for every lever-ON-at-rest door): a filling whose blocks change state while the build
  settles (dust next to the lever, repeaters turning on, a dropper triggered) is screened again placed in
  its settled states (consistent rest, no settling transient). With it c79's lever-held top-right region
  (8 cells + the front-face lever, `--lever-cells 4,5,-1`) is RE-FOUND in full: cosearch2 matched only 4 of
  its 6 groups ((1,3)o2 and (1,4)c4 were lost to the settling transient of the unpowered fillings).
* `d545493` `plan-from --region R --close`: grows R until the door with R erased, driven by the plan,
  goes through the same states as the door (per plane cell, the sequence of states); every stateful
  wiring block next to R (or an observer watching it) that differs joins R. c79: bottom-middle (3 cells)
  closes to 8 (+ bottom-left's observers and dust: cosearch2's finding); the lever's 4 cells close to the
  top-right 8 (+ dropper, its observer, the elevator observers).

## 2. c2e seeds (coordinator top priority)

* m79a (c2e 1e02280, u7-m10 in 7x9, only A's hold ideal): NO PASS, bounded. The 5 free top cells (single
  cells) x 51 states drive nothing. Top-right re-wired too (2,5;3,5;3,4;3,3;4,3 + the dropper (4,2) by
  `--close`, lever kept ON at (4,5)), k=6 x 14 states (16M screens): two disjoint families, {R pair + TRp + B}
  or {R pair + A's hold + TRp} (dot dust (3,5) on (3,4) drops TRp at t0, TR lands in (2,3) at t3, a
  repeater (3,3) d3 on the dust (4,3) powers TR -> A), never the hold with B: B's level can only come in
  through (2,5), anything feeding (2,5) QC-holds TRp until B drops (>= t9), and the hold needs TR in (2,3)
  by t3 (the top is cut at x=1 by A, B and B's head: the right side is the only early source).
  Best true partial: 48 blocks, only B ideal (`runs/m79a-trA-k6s/pockets-partial-1as.txt`).
* m88a (c2e d58e481, 8x8, whole bottom real) + m10top's m10f top transplanted at the same cells
  (`seeds/m88a-top.txt` + `plans/m88a-hold.ideal`): `door ideal` WORKS with only A's hold ideal (t8, window
  t7..t8). Pockets k=4 x 35 states on its free cells (x=-3 column, (-1,3), (3,3)): nothing drives the hold.
  (-1,3) has no floor ((-1,2) is the left frame cell, air at rest), so (0,3) can only be fed from (0,4)
  (dust on it) or A by QC via (0,4) -> (1,4); m10f's observer (-1,4) gives one 2-gt pulse at L1/L2 time.

## 3. Mechanism inventory (frame at open, `door sheet --fill`)

FULL frame as given: w63top m10 / m10r / m10a, seq56 m4 / m5. 12/12 after `door sheet --fill` (static
stone = the door block in the 3 empty frame cells, written to `tracks/pockets/mech/*-ff.*`): w63top m8, m9,
ring56 t8, t10, jf, mechspace c78-001..006 (their smooth-stone statics were the wrong block for the frame
rule: frame cells must hold the door block itself).
