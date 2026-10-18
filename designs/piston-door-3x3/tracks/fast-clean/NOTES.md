# Track `fast-clean`: the fastest solid doors that respect "nothing in front of or behind"

vjeux's rule (judge on main since 2026-10-18 16:40Z): at rest the 3x3 columns in front of and behind
the opening (x 0..2, y 0..2, every z != 0) hold no block at all, at any distance. Speed boards: open,
close, open + close (worst of 3 cycles at the first position), ties by blocks. k = ticks after the flip.

## Audit (`door verify` on main, 16:52Z)
Every fast solid door breaks the rule in the SAME six cells: the lever net's east branch crosses in
front of the door as a dust staircase at z = -4 (floors (0,0,-4), (1,0,-4), (2,1,-4), dust (0,1,-4),
(1,1,-4), (2,2,-4)); nothing else of the speed family (E/F/A'/Q elevator, R_X/R_Z placers) is in the
columns. Same cells in lamp-sweep a1-107 (10/7), o6-107 (11/7), speed-wire s4-108 (12/7), speed-2
n3-112b (9/7), and tick-door's w9 (its lever line at z = -4, y 1..2).

Why the staircase dips to y 1: dust over a column cell needs its floor outside the column, so a
crossing in front of the door must run at y >= 4 (floors at y 3) or y <= -1 (floors at y -2).

## Tool changes (own commit)
* `door route`, `route2`, the searches (`minimize`, `regrow`, `grow`: search.rs) and `synth` keep the
  whole columns clear (route.rs `keep_clear`: door cells + 32 cells deep each way), not just the 3-deep
  walkway: they no longer propose anything the judge rejects.
* `door route --swept-design <working design>`: swept cells from the design a net was cut out of,
  counting only cells its moving parts visit (static wiring is free), to re-lay a net elsewhere.

## Results (all `door verify --extra-offsets 28`: 32 positions x 3 cycles, rotation-safe)
| file | blocks | open / close | what |
|---|---|---|---|
| n3c-116.txt | 116 | 9 / 7 | speed-2 n3-112b with the staircase cut (9 blocks: the 6 column cells, its west end (-1,2,-4) + floor, its east end (3,3,-4)) and re-laid by `door route --swept-design` over the top at z = -5: (-1,2,-4) up to (-1,3,-5), along y 4 on floors at y 3 (x 0..2), down to (3,3,-5) and (3,3,-4) (13 blocks). Strength at the far ends 3 instead of 5. DOMINATED by speed-2's c1 (112, 9 / 7, the same fix with a hand route at equal size, found in parallel) |
| **a1c-107.txt** | **107** | **10 / 7** | lamp-sweep a1-107 + speed-2's c1.patch (the hand route: S (-1,3,-3) -> (-1,4,-4) -> (0,4,-4) -> step up to (1,5,-4) over a new stone (1,4,-4), so the dust never touches the repeater-powered stone (1,4,-3) -> (2,4,-4) -> M (3,3,-4); 8 cells out, 8 in): equal size, the fewest-block close-7 door under the rule. (a1c-111 was my router version, +4) |
| s4c (work) | 108 | 12 / 7 | speed-wire s4-108 + the same c1.patch: compliant at equal size too (base for a close-7-only resynth) |
| s4c-101.txt | 101 | 12 / 7 | s4c after the first 7 replacements of `door resynth --palette block,dust,dot,rep,obs,torch,cmp,lamp,bulb,target,note --max-size 3 --max-close 7` |
| **s4c-93.txt = BEST** | **93** | **12 / 7** | the same resynth to its end (108 -> 93, 15 replacements: the target taps for the side torches and P_M's torch, the (2,4,0) observer + (2,5,0) dot, shorter lever-net ends): **the fewest-block close-7 door under the rule** (s4 had never been resynthesized; a1-107 and o6-107 had, by lamp-sweep) |
| **c1r-108.txt** | **108** | **9 / 7** | speed-2's c1 (112) + `door resynth` (same palette, `--max-open 9 --max-close 7`): 4 replacements (the dead note-block feeder repeater + stones, obsidian (4,3,0) + stone (5,2,0) -> one smooth stone). **Fastest open (9) and open + close (16) under the rule**, one block under speed-2's c2-109 |
| a1c resynth | 107 | 10 / 7 | `door resynth` size 2..3 `--max-close 7`: nothing (lamp-sweep had already resynthesized a1) |

## Close 6 in the speed family (worked out, not built)
The close is 7 = F's second push [T, C] at c5 (lands c7). Chain: c1 F (BUD-armed through the stone
(1,6,0) that the lock-delayed repeater (1,6,-1) powers from t16) is updated by S_F's head leaving
(1,4,-1) and pushes C from E into N1; c2 the repeater falls, P_X retracts and updates F, F spits C
(placed c2); the T-returner D (3,3,0) pushes T into E at c2 (update from the right top puller's head),
P_D pushes R_F back at c2; c4 F's base, T and R_F land; c5 F extends.
* Close 6 needs exactly: (1) F 0-ticks at c1 (C placed at N1 at c1, F's base lands c3 BE);
  (2) T placed in E before c4's block events; (3) R_F back by c3 BE (S_F sticky: it pulls R_F back
  at c1, and P_D + its observer go away). Then whichever of R_F / F's base / T lands last at c3 BE
  updates F with everything placed and F extends at c4: close 6.
* (1): F's killer must be updated by F's extension (a neighbor of F's base or of E) and must move
  F's arming source in the same phase. The free neighbors (0,4,0) and (1,4,1) only give up-facing
  killers, which self-latch (the block they push sits next to their own quasi cell, tick-door's
  finding). The working geometry is the open's own killer: P_X in F's quasi cell (1,5,0) shoving
  R_X from (1,5,1). Re-arming it for the close needs, after t12: R_Z pushed off (1,6,1) (P_X
  retracts through its head), p pushes R_X back to (1,5,1) (silent for F), R_Z reach-pulled back
  (silent for P_X), and the repeater (1,6,-1) + its lock chain removed (it would power P_X directly
  through (1,6,0)). Two more k pulses and one more p pulse, all wired from the lever.
* (2) is the blocker: D must push T at c1 AFTER F's 0-tick (F's retraction only updates the
  neighbors of (1,4,0), E and N1; D at (3,3,0) is none of them, and anything updated earlier
  queues D before F's retraction, so D's push meets F's moving head and is dropped, no retry), or
  D (sticky) must lose power at c3 for a spit (no c1 instant-on source exists on the close edge to
  feed a torch, and an observer on a c1 mover only gives a c3 RISE), or D needs its own 0-tick
  killer with a silent re-arm window. A different T parking cell (behind or in front of E, a
  reach-through returner next to E) changes Q's push geometry: a new open.
* `close6-ideal.drive` (a1c, open state edited by script: repeater (1,6,-1) removed, P_X retracted,
  R_X back at (1,5,1), R_Z back at (1,6,1), S_F sticky): **the close 0-tick of F works physically**
  (F extends, P_X shoves R_X, F retracts in the same phase: C is PLACED in N1, "air -> stone").
  But it happened at c2, not c1: a sticky S_F pulls R_F into (1,4,-1) as a moving block, so the head
  removal that BUD-updates F at c1 (a normal piston's `removeBlock` of its head) never happens; only
  the old R_F cell's neighbors are updated. So R_F's early return must come from elsewhere (keep S_F
  normal). And with E empty at c2 and T not back, Q pushed A' into E at c4: the T-returner is the
  part that decides whether close 6 exists in this family.
* **`close6-pad.drive`: CLOSE 6 with partly scripted inputs** (door cells all placed at c6, every
  piece physical except the arming blocks and R_F's return). The FIFO block-event queue at c1 runs
  S_F's retraction, F's extension (C pushed E -> N1; E's neighbors updated: Xa), the padding piston
  Xa (1,3,1) (head into (2,3,1): Xb (2,4,1)), P_X (shoves R_X: F loses power), Xb (head into
  (3,4,1): Xc (3,4,0)), F's retraction (C placed in N1, E air), Xc (its base update reaches D
  (3,3,0) and P_D (3,4,-1)), then D pushes T into E (lands c3 BE). F's base lands c3 BE, F
  pushes [T, C] at c4: closed at c6. Rule learned: a piston only queues an extend if its push
  resolves WHEN IT IS UPDATED, so D must be updated after F's retraction, not just run after it;
  a 3-piston chain from E's free neighbor does that.
* **`close6-c1r.drive`: the same on c1r-108 (open 9) with R_F's return physical too**: an armed
  P_D (3,4,-1) is updated by Xc's base and pushes R_F back to (1,4,-1) at c1 (lands c3 BE). Only
  the arming is scripted (R_X/R_Z re-placed, blocks at (2,5,2), (4,4,0), (4,4,-1), the three
  padding pistons placed). A wired c1r with this would open in 9 and close in 6 (open+close 15).
* What a real door still needs (none of it built): (a) the P_X re-arm in the open state after t12
  (R_Z off -> P_X retracts; p pushes R_X back; R_Z back), with the repeater (1,6,-1) no longer
  powering (1,6,0) (its lock chain gives t10/t14/t16 signals to time this); (b) silent late arming
  of Xa (R_X arms it through (1,4,1) during t4..t10, when E changes 5 times: rotate P_X west so R_X
  sits at (0,5,0) instead), Xb (the observer o at (2,4,0) points into it: o looks dead weight), Xc
  (the observer (3,5,-1) updates it at t2/t4 through (3,5,0)), D and P_D; (c) R_F back by c3 BE:
  Xc's base update also reaches P_D (3,4,-1), so an armed P_D pushes R_F at c1 (done in close6-c1r).
* Arming facts checked after that: with close 6 the observer o (2,4,0) is not needed (close6-c1r
  still closes in 6 with o replaced by smooth stone, J's floor; o only served the old close), so Xb
  is safe once o is gone. D and P_D may be armed for the whole open state (every update they get
  meets a blocked push: T moving or A' extended in E for D, S_F's head for P_D), Xc from t6 (after
  the observer (3,5,-1)'s t4 update), Xb any time (silently), Xa only after t12 (E changes until
  t12). A simpler P_X re-arm: make P_X STICKY. Losing R_Z's power it retracts and pulls R_X back to
  (1,5,1) by itself (F stays silent: P_X's base lands before R_X), so p's push is not needed; but
  then R_Z must go out at t2 and t12 and come back at ~t6 and ~t16 (k pulses on the OPEN edge only,
  e.g. A AND NOT B with B a longer delay of the same lever signal: no pulse on the falling edge),
  and P_X stays extended at rest. Not built: about ten new parts, all silent, in a full volume.
