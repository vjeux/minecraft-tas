# sculk-ew track: the sculk doors facing east/west

Ask (vjeux): sculk-2's four sculk doors (s4, s4b, s4c at 36, s1 at 38) work facing north/south
(0 and 180 degrees, also mirrored) and break facing east/west (90/270). Find why, and build a
variant that works facing east/west with as few extra blocks as possible.

Coordinates below are the north/south design's (`archive/sculk2-s4-36b.txt`): door x 0..2, y 0..2,
z 0; A = the sticky piston at (1,4,0) holding T, Q = the shuttle piston (0,4,-1) facing east,
RB = its redstone block (1,4,-1), B = the elevator (1,5,0), R_B = the repeater (1,6,1) fed by the
calibrated sensor (1,6,2), O' = the observer (1,5,1) pulsing A through the dust X' (1,4,1).

## 10:20 s4r: 36 blocks, sculk, ROTATION-SAFE (all 8 orientations), open 22 / close 37
`s4r.txt` (= BEST.txt) is s4 with ONE change: R_B delay 1 -> 2. Same 36 blocks. It works in all
8 orientations (4 turns x mirrored) at 32 positions (`door verify --extra-offsets 28
--orientations`), so the north/south door and the east/west door are the same design.
`s4r-ew.txt` is it turned 90 degrees (`door turn --turns 1`, `door-axis: z`): the east/west-facing
file, also verified at 32 positions in all 8 orientations. Archived: `archive/sculk-ew-s4r-36b.txt`,
`archive/sculk-ew-s4r-ew-36b.txt`.

### Why s4 breaks facing east/west (traced with `door trace --close` on `s4-ew0.txt` = `door turn` of s4)
* Close, t1: the lever goes off, K loses power, Q retracts and pulls RB back next to A (P_R's
  early pull fails against P_L's moving base, in both orientations). t2: the hold sensor re-powers Q.
* t3, block-entity phase: Q lands BEFORE RB (its moving block entity was created first), but it
  cannot queue its re-extension: `PistonBaseBlock.checkIfExtend` only queues the block event when
  `PistonStructureResolver.resolve()` succeeds, and RB in front of it is still a moving block.
  Then RB lands (`setBlock` flag 1) and updates its neighbours in the order W, E, D, U, N, S. Both
  A (powered by RB) and Q (now able to push) queue their extension in that update.
* North/south: Q is WEST of RB (east at 180) and A is SOUTH (north at 180): Q is queued first; at
  t4 Q pushes RB away, then A's event finds no signal and is dropped (`triggerEvent` re-checks).
* East/west: A is WEST/EAST of RB and Q is NORTH/SOUTH: A is queued first. At t4 A extends and
  starts pushing C down from (1,3,0) into (1,2,0); Q then pushes RB away, A retracts in the same
  block-event phase and spits C at (1,2,0). A's base is a moving piston until t6, so B's push at
  t6 fails (retried at t7, when A lands), A reaches (1,3,0) only at t9 and misses O''s pulse
  (t8-t10): C is never pushed home. The second pulse (t18-20) pulls C back up to (1,3,0), where it
  blocks the right structure's return when the hold ends (t32: T would push C into P_L), so the
  right column, C and M (P_M fires on the right column's return) stay open.
* Every east/west orientation has A and RB side by side across the door plane (W/E) and Q behind
  RB along the door's axis (N/S), so no mirror or turn of this geometry fixes it.

### The fix: R_B delay 2 (no extra block)
The B window and both O' pulses come 2 gt later (B at t8, pulses t10-12 and t20-22).
* East/west close: A still spits C to (1,2,0) at t4, but A has landed by t6; B pushes A down to
  (1,3,0) at t8, A lands at t10 inside the pulse, extends at t11 and pushes C home to (1,1,0)
  (spit at t12). From there the close is s4's: the second pulse moves nothing, the hold ends at
  t32, P_R pulls T back, A pushes it home (closed 37).
* North/south close: no spit at t4; B pushes A and C down together at t8 and the same pulse pushes
  C home.
* Open: B pushes A down at t8, A pulls C up at t11-12 and again at t21-22 after B lifts it: open 22
  (s4: 20). The hold (30 gt from t2) still outlasts the close dance (done at t22 < t32).
