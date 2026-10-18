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
