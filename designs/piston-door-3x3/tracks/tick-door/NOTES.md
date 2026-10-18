# Track `tick-door`: the fastest 3x3 door with 0-tick and 1-tick mechanics

Door cells x 0..2, y 0..2, z 0. T=(1,2) C=(1,1), TR=(2,2). k = game ticks after the lever flip.

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
  first), by W at (3,2) reaching through (2,2). Corner topology, all in tick 1:
  TR up (P_TR) -> W 0-tick pulls T into (2,2) -> A' (at E, facing down) 0-tick pulls C into N1.
  Tick 4: X pulls TR from (2,3) east, P_TR reaches through (2,3) and pulls T up; Q pulls A'
  out of E, F (above E) reaches through E and pulls C up. **Open 4 is reachable on paper.**
* Close: pushed blocks can be placed instantly (spits), so close >= 1 in principle.

## Log
* `k1chain.txt/.drive`: the tick-1 half proven in the simulator: TR up, W 0-tick (T into the
  corner), A' 0-tick (C into N1), all in k1. The 0-tick module: piston B quasi-powered at rest
  by a redstone block R with its push blocked by the door; a sticky killer K next to B's base
  (facing so R is its front, R adjacent to B's QC cell) is armed by the lever but blocked by a
  normal piston's head until k1; B's extension updates K, K shoves R, K's move updates B, B
  retracts in the same phase (reach-through pull). Gotcha: blockers must be NORMAL pistons (a
  sticky blocker pulls R away when it retracts).
* `o4b.txt/.drive`: **OPEN AT 4 with ideal inputs** (all 9 cells clear at k4 and stay clear).
  Tick 4: P_WN (pull-type pulse) pulls W north out of (3,2), V (4,2) 0-ticks through (3,2) and
  pulls T east; Q (pull-type pulse) pulls A' to (0,3), F 0-ticks through E and pulls C up.
  Killers K_V, K_F armed silently at k4 (quiet repeater power into their QC cells). Blockers kept
  until k4: TL (V's line), M (F's line), R/BR (none needed in this variant). Not wired yet; close
  not designed yet.
* `o4c.txt/.sched` (`door ideal`): **full cycle WORKS with ideal inputs, open 4 / close 10**
  (32 positions x 3 cycles). Killers moved so they do not self-latch: an up-facing sticky killer
  keeps the block it pushed next to its own quasi cell and can never retract; K_W now sits in W's
  quasi cell (3,3,0) facing south, K_V in V's quasi cell (4,3,0) facing east. K_W's blocker is
  released by a second lever shuttle queued after P_TR's: TR's arrival at (2,3,0) updates K_W while
  it is still blocked, the blocker leaves before W extends. Close (scripted): F and V spit C/T one
  cell (c1-2), P_WN and Q put W and A' back (c3-4), A' pushes C home (c4-5), W pushes T into N1
  (c6-7, must wait for A's head to leave N1: no-retry), R_TR pulled back so P_TR pushes TR (lands
  c10).
* `w2.txt` + `w3.sched`: the REAL lever (south, (3,5,1) on B_L (3,4,1)) now drives S1 (P_TR's
  shuttle), arms K_W silently (B_L is next to K_W's quasi cell, not to K_W) and arms K_V at k4
  through a quiet repeater chain (Rep1 (3,5,3) reads the lever's dust: k2; RK (5,4,1) into the
  arming block (5,4,0): k4). W's close is a 0-tick too: K_W resets at c1 (B_L's de-power updates its
  head), so W is quasi-powered again; at c5 a quiet arming of K_W plus a pure update of W (glass next
  to W) make W push T into N1 and retract in the same phase. `door ideal w3.sched`: **WORKS, open 4 /
  close 9, 32 positions x 3 cycles** (remaining scripted inputs: K_A arming + its blocker, Q/P_WN
  pulses, K_F arming, ring pullers, F/V/A' close pulses, resets).
* `w4.txt` (WIP, does not open): a lever line over the top (crossing z=0 at (4,7,0)) fires S3
  (shuttle of P_YA's power, pushing R_YA to (1,7,0) where it also arms K_F at k3 BE) at k1; the
  K_A arming branch runs out of dust strength / gets its step cut.

## Why it is not wired yet (constraints found)
* An up-facing sticky killer self-latches (the block it pushed sits next to its own quasi cell),
  so killers must sit IN the target's quasi cell facing sideways (K_W, K_V), or the block must be
  pushed on again (R_A: K_A then K_F).
* Every 0-tick piston that must fire after a k1 event needs its killer armed at k0 (lever level):
  silently (B_L next to its quasi cell) or noisily with a blocker released at k1 by a lever shuttle.
  Only one lever block exists: it can arm K_W or K_A silently, not both. The other needs a blocker
  + shuttle, and all of S1, S3 and that killer's arming must be lever dust within 15 blocks.
* Placement conflicts that killed layouts: R_TR next to K_A's quasi cell (re-arms K_A when it comes
  back); B_L next to W's displaced quasi cell (W extends into the walkway after the open); floors
  under dust next to P_TR's / P_WN's / K_V's quasi cells; dust next to R_A (lit at rest); a dust
  corner where a straight end was needed to point into a piston; a step cut by the floor of the
  dust above.
* No-retry bites the close: a piston whose push is blocked when its power arrives (a head still in
  the way) never retries; every close step must wait for the previous head to leave.

## Next steps (to wire o4c/w3 for real)
1. K_A: arm it from the north with lever dust ending in a straight N-S segment into (1,4,-3) (route
   the descent along x first; the zigzag cut its own step), or move the lever north and give K_W the
   blocker instead (S2/P_YW from o4c).
2. Q, P_WN: extended at rest, one torch net (input = repeater d1 OR d3 from the lever: off k4, on
   c8), the torch net also powering P_TL (TL stays as V's blocker until k4).
3. M blocker: torch-driven P_M; R, BR: pull-type pistons on an observer of a lever dust (k2..k4).
4. Close windows from an observer chain off the lever (F, V at c2..c4 are harmless on the open
   edge; A' via a stone at (1,3,1); W via glass + quiet K_W arming; P_TR via glass at (2,5,0)),
   resets last (K_V on K_W's move, K_F on the P_TR glass pulse, P_YA pushes R_A home).
