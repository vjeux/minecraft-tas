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

### Gadgets contributed (designs/gadgets, `door drive --quiet` PASS)
* `head-qc`: a piston is quasi-powered through another piston's head above it; a dust pointing
  into that head cell fires it (and updates it). Why fc2's X needs no conducting floor.
* `order-flip`: one 2gt pulse, a push-type and a pull-type sticky piston act in opposite orders
  on the two lever edges (the e1 trick: no wiring needed to swap "who moves first").

### Why fc2's control looks pinned (my round, on top of fresh-c's analysis)
* The only pure supports are X's floor and R_q's floor; neither can be a needed block: X's line
  needs its west connector (an observer facing west at (-1,4,0) watching the lever = O_a), and a
  cross X powers Q. Moving X to (1,4,-1) (cross harmless) separates the lever from Q's repeater.
* A one-observer P_L trigger needs P_L two cells from the lever in a line (or the observer's back
  in P_L's quasi cell); none of those lever cells can also feed O_a->X and R_q->Q.
* P_L push-type next to X's floor (gets X's 3 pulses, odd = toggles right) costs 2 slimes = the 2
  observers it saves. Same for P_L under the walkway sharing P_M's pulse (pb): no free source.
* p0 cannot come from a moving part (it is the first event); the window input needs 2 blocks
  (nothing moves near R_B's back cells); P_M needs 2 (observer + conductor or flag + observer).

### More searches on fc2 (nothing below 40)
* `door minimize --moves all --depth 2` on fc2-xglass: 34 level-1 and 255 level-2 verified
  neutral designs, 7650 removals after them: nothing.
* cut {X floor, both P_L observers} + grow --k 2 (418k candidates, x-4..1 y1..6 z-1..2): nothing.
* `regrow --size 3 --any --margin 1 --states obs,block` (2024 cut triples, streamed in 400k batches
  with the fixed tool): see the end of this file for the result.
* Idea that almost works: Q silently quasi-powered by the lever and woken by J's pulse (J next to
  Q) would make Q's delay free (no R_q, no floor), but the lever must then sit next to Q's quasi
  cell (0,5,1) while also being watched by an observer that feeds A (p0), and X must not touch Q
  (X's pulse would wake Q at 2): no cell satisfies all three.
* RESULT of `regrow --size 3 --any --margin 1 --states obs,block` on fc2: stopped after 90 min
  at 24.3M candidates screened (of the 2024 cut triples, in sorted order): 0 passed even the
  2-position screen.

## Final report (11:50Z)
* No design below 40. Best verified this round: `fc2-xglass.txt` (40, same count as fc2, X on
  glass), used only to prove that X's floor needs no conductor.
* New mechanism: e1 (Q-free center, ideal-validated): A's rest-power redstone block rides the
  right structure, T rides the left; a push-type pulse piston on the RB side and a pull-type
  pulse piston on the T side flip the side order between open and close with no wiring
  (gadget `order-flip`). It costs 26 mechanism blocks and its push-type piston can never sit
  next to A, so the wiring ends up >= 41.
* Tool: `door regrow` now streams candidates in 400k batches (size-3 `--any` used to need
  55 GB). Gadgets: `head-qc`, `order-flip`.
* fc2 searched further: depth-2 plateau (255 level-2 neutral designs), cut-3/grow-2 combos, P_L
  relocations, lever-on-O_a, P_L under the walkway sharing P_M's pulse, regrow --size 3 --any:
  nothing. Analysis of why each control pair is pinned is above.
* Next ideas: (1) a layout where X does not touch Q (X at (1,4,-1), a cross is harmless there)
  AND the lever sits next to Q's quasi cell: then Q can be silently quasi-powered by the lever
  and woken by J's pulse, making Q's delay free (-2: R_q + floor); the open problem is getting p0
  from that lever to A. (2) A layout whose one-per-edge pulse block (J, or P_M's conductor) also
  touches a pulse-type side piston (pb shows P_L push-type at (0,-1,0) works with ANY single pulse
  shared with P_M; only a free source into (0,-2,0) is missing). (3) e1 with A at the same end as
  the push-type piston (needs a z-offset RB with <= 1 extra slime).
