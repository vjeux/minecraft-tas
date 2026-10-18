# Track fresh-e: beat the solid 40 (fc2) with a different geometry

Start: 2026-10-18 09:12Z. Box lease ends 13:10Z (report by 12:45Z).

## Baseline: fc2 (40 = 25 mechanism + 15 control)
fc2 control: lever 2, R_q + floor 2, O_a 1, X + floor 2, O_b + J 2, R_B + O 2, P_L observers 2,
P_M observer + block 2. Searches (minimize/regrow/grow) say fc2 is a local minimum, so the
geometry has to change.

## Log
### 09:25Z e1: a Q-free center (validated with ideal inputs, `e1-mech.txt`, `e1b.sched`)
A's rest-power redstone block RIDES the right structure ((2,4,0), glued to slime (3,4,0)); T rides
the LEFT structure (carrier slime (0,3,0)). The order problem (RB side first on open, T side first
on close) is solved by the piston types alone: the RB side is a PUSH-type pulse piston (P_R
(2,-1,0) + slime (3,-1,0): moves at s on open, at s+2 on close), the T side a PULL-type pulse
piston (P_L (-3,1,0): moves at s+2 on open, at s on close). `door ideal` sweep: works for every
sl in [sr+1, sr+4], p0 <= sr-1, window b anywhere 8..20; P_M shares P_R's pulse block (2,-2,0)
(one pulse fires both). Mechanism 26 (no Q, no R_q/floor, but +2 left slimes).
Why it does not beat 40: the push-type piston can never sit next to A (it must avoid the RB's
6 neighbors and its own quasi cell), so it ends up at the far end (bottom) while p0 (which must
precede it) has to start at the lever next to A; the pull-type side's piston is at x=-3. Every
layout costs >= 4 control blocks more than fc2's Q system. Variants with P_R at the top
((2,5,+-1)) need 2 more slimes (27) and still a far P_L trigger.

### fc2 experiments (all judged with main's verifier)
* `fc2-xglass.txt` WORKS (40): X's floor (0,3,0) does NOT need to be a conductor. In its lower
  position A is quasi-powered through the cell above it (B's head) by the dust X pointing east,
  and the dust's second-order update reaches it. So X's floor is a pure support (any sturdy block).
* `lo-37.txt`: lever hung on O_a's face (no wall block); grow --k 2 for Q's delayed input in
  x-3..1,y2..6,z0..3 (206k candidates): nothing.
* `pb-37.txt`: P_L as a PUSH-type pulse piston under the walkway (0,-1,0) + slime (-1,-1,0),
  sharing block (0,-2,0) with P_M: `pb.sched` proves ANY single pulse there at any tick 0..14
  works. But no 1-block source can pulse (0,-2,0) (grow --k 2 in the bottom box: nothing);
  the only early mover near the bottom is the right column (fc2's BM observer) at (2,*).
* P_L moved to y=0/1 + grow --k 1 (big box): nothing. Lever-on-O_a + synth (k<=5) for Q+P_L: nothing.
* `regrow --size 3 --any` on fc2 blew up to 55 GB RSS in candidate enumeration: killed. Do not run.
