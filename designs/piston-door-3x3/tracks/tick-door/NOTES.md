# Track `tick-door`: the fastest 3x3 door with 0-tick and 1-tick mechanics

Door cells x 0..2, y 0..2, z 0. T=(1,2) C=(1,1), TR=(2,2). k = game ticks after the lever flip on,
c = after the flip off.

## Result
**`BEST.txt` (= `w14.txt`): opens in 4 ticks, closes in 15, 229 blocks, category solid (uses slime),
rotation-safe, front/back rule OK.** `door verify --extra-offsets 28` passes (32 positions x 3 cycles).
Open 4 is the proven floor for a solid door (below), so this is the fastest possible opening; it beats
the previous solid open of 10 and the any-category open of 5. Open+close is 19 (the solid open+close
board has 17).

How it opens (every input comes from the lever):
* k1, one block-event phase: S1 shoves R_TR, P_TR loses power and pulls TR up; W (3,2,0), quasi-powered
  by R_W and blocked by the door at rest, is updated when TR leaves, extends into (2,2) and retracts
  at once because its killer K_W (3,3,0, facing south, armed silently by the lever's block B_L) shoves
  R_W: reach-through, T is pulled into the corner. T leaving N1 updates A' (at E=(1,3,0), quasi-powered
  by R_A): A' extends into N1 and its killer K_A (armed by lever dust, blocked at rest by a normal
  piston P_YA that the lever shuttle S3 releases in k1) pushes R_A up: A' retracts and pulls C into N1.
* k4: a torch (fed by the lever through repeaters, off from k4) drops Q, P_WN and the ring pullers.
  P_WN pulls W north, V (4,2,0) reaches through (3,2,0) and pulls T east (killer K_V armed quietly at k4
  by a repeater); Q pulls A' west, F (1,4,0, powered by R_A) reaches through E and pulls C up (killer
  K_F armed quietly at k4 by repeater RKF). TL+L are pulled west, the whole bottom row drops on a slime
  row pulled by P_S, R is pulled east. All 9 cells are empty at k4.
* Close: observer pulses F (C back to N1) and V (T back to the corner) at c2; the torch comes back at
  c6 (Q puts A' back in E, P_WN puts W back, ring and slime row return); a 4-observer chain powers A'
  at c8 (C home), observers on P_WN's head arm K_W through B_L at c12 while a 6-observer snake updates
  W: W pushes T into N1 with a 0-tick; observers on P_YA update P_TR, which pushes TR home (c15) and
  resets K_W.

## First principles (26.3 source)
* No block event runs at k0 (the flip is between ticks): the first piston move is k1.
* A PULLED block leaves its cell at once but is a moving_piston (immovable) until k+2 BE:
  it can move again at k+3 at the earliest. Nothing finalizes a retracting moving block.
* A PUSHED block can be placed at once by a sticky 0-tick (retract in the same block-event
  phase, or before it lands): only the block directly in front of the head (`finalTick` of
  pos+2*facing); the rest of a pushed line still lands at k+2.
* Reach-through: a sticky piston that extends into an empty cell and retracts in the same
  phase (or before its head lands) pulls the block two cells away.
* Events queued during a block-event phase run in that same phase (FIFO). Power changes in the
  scheduled phase act in the same tick's block events; landings (BE phase) act next tick.

## Lower bounds
* C has no outside neighbor: it needs two in-plane moves (C -> edge cell N1 -> exit E). A push
  of C needs a piston on the opposite edge cell (inside the door), impossible by k1, so both
  moves are pulls: k1 and k4 at the earliest. **Open >= 4 (solid)**.
* Reaching C at k1 needs a puller sitting in E at rest (A') and N1 empty with E free, so the
  edge block T cannot go to E: it is pulled SIDEWAYS into the corner TR's cell (TR pulled up
  first), by W at (3,2) reaching through (2,2). Corner topology, all in tick 1.
* Close: pushed blocks can be placed instantly (spits), so close >= 1 in principle.

## Log
* `k1chain`: the tick-1 half proven with ideal inputs. `designs/gadgets/killer-chain`: the gadget
  (two BUD-triggered reach-through 0-ticks chained in one tick).
* `o4b`: open 4 with ideal inputs. `o4c`: full cycle with ideal inputs, open 4 / close 10.
* `w2`/`w3.sched`: the real lever drives S1, arms K_W silently, arms K_V at k4; W closes by a 0-tick.
* `w6`: lever line over x=0 to the north side: S3 releases K_A's blocker, X arms K_A; RKF arms K_F.
  The whole k1 stage and the k4 arming are wired (open 4 / close 9, rest ideal).
* `w7`: P_WN extended at rest on a torch fed by the k2..c4 net (RB2 reads the lever directly and
  holds the net until c4). `w8`: Q on the same torch's net. `w9`: ring on the torch net (P_TL+P_L,
  slime bottom row on P_S, P_R) and S1 held until c8 by a repeater: **the open is fully wired**.
* `w10`-`w13`: close pulses one by one (F: observer on a lever-dust corner; V: observer on lever dust
  south of V; A': 4-observer chain; P_TR: observer pair on P_YA). `w11` moved the torch net above the
  door columns for the new front/back rule.
* `w14`: K_W's close arming (observers on P_WN's head cell -> B_L) and W's close update (observer
  snake from lever dust): **no scripted input left, verified**.

## Wiring lessons
* An up-facing sticky killer self-latches (the block it pushed sits next to its own quasi cell);
  killers sit in the target's quasi cell facing sideways (K_W, K_V) or push the block on again.
* A killer that must fire after a k1 event needs k0 arming: silently (lever block next to its quasi
  cell) or noisily behind a blocker that a lever shuttle releases in k1. Blockers must be normal
  pistons (a sticky blocker pulls the power block away).
* A killer armed early fires early: R_YA landing next to K_F's quasi cell in the same BE phase as R_A
  landing in front of K_F fired K_F before F. Arm such killers in the scheduled phase (repeaters).
* Dust next to a block powered by other dust is NOT lit (the dust-under-a-powered-floor trick does
  not exist); repeaters and observers do read dust-powered blocks.
* A powered floor powers pistons next to it: use glass floors next to quasi cells (P_YA, Q, P_TL).
* Hidden step connections merged nets three times: a dust one block lower and diagonal connects
  unless the cell above the lower dust is a conductor. Check every new dust's diagonal neighbors.
* `door patch` settles the design: a broken intermediate state is written back (pistons retracted,
  blocks dragged). Apply multi-part changes in one patch from the last good file.
* No-retry: a piston whose push is blocked when its power arrives never retries.

## Next ideas
* Close 15 -> ~10: the close waits for the torch (c6) because P_WN and Q must not race the F/V
  heads (c4); a separate earlier source for the ring/Q/P_WN, or F/V pulses at c0 (lever-direct
  1-tick pulse), would let everything shift by 2-4 ticks.
* Blocks: `door minimize --objective total` is running on BEST; the many single-purpose observer
  chains and the long torch net are the obvious candidates.
* Slime-free: the slime bottom row can go back to three pullers (P_BL west, P_M, P_BR) on the torch
  net; costs blocks.
