# Track sf-holdA: pist19r with A held extended at rest ("Solid, no slime or honey" board)

| file | blocks | note |
|---|---|---|
| **b38.txt = BEST.txt** | **38** | b39 with the lever at (1,5,-3) hung on O: it weakly powers a NOTE BLOCK (1,4,-3) that O_a watches (p0 t2, nothing passed on to P_T's quasi cell) and (1,6,-3) watches the lever itself (observer (1,5,-3) gone). Open 36 / close 32, BUILDABLE |
| b39.txt | 39 | b40 with ONE chain off the lever watcher for both the B window and P_T: (1,5,-3) -> (1,6,-3) -> Y (1,6,-2) facing north powers R_B directly (J gone); O (1,5,-2) turned UP watches Y and fires into O_a's cell = P_T's quasi cell (P_T's 3-block feed gone). Open 40 / close 36 |
| b40.txt | 40 | TL and TR both up on X's three pulses, P_T south at (1,3,-2), 2-observer M trigger (section b40 below). Open 38 / close 34 |
| r41.txt | 41 | w42 + `door resynth --palette block,dust,dot,rep,rep4,obs,torch,lamp,bulb --max-size 4`: X dust + floor + the TR chain (2 observers) -> a plain block X' (1,4,-1) (O_a and O' strongly power it: A-high directly, A-low by quasi-connectivity through B's head) + a cross dust (2,4,-1) on a floor (2,3,-1) next to it, pointing into the TR puller (2,4,0). The TR puller now gets X's THREE pulses (t2, t14, t22): three toggles end with TR up, and the right pair (watching TR's cell) toggles three times too. Open 28 / close 26, 32 positions |
| hA.txt | 43 | the A-held wiring of w42 (below) |

Goal: beat wire19sf's w42 (42, slime-free) with a pist19r variant whose close ends with A
EXTENDED and powered (fresh-c's symmetric close, as in slime-free's s7q), so that the bottom-middle
piston M may fire early (w42 spends 4 blocks on M's late pulse).

## Ideal-input results (door ideal, 4-8 positions x 2 cycles)

| file | mechanism | result |
|---|---|---|
| h1-mech.txt / h1.ideal | w42's pist19r (P_T (-1,3,0) east, TR pulled up, right pair, left column of 3) + A extended at rest, RB (1,4,1) south of A, Q (2,4,1) sticky facing west (pushes RB to (0,4,1)) | WORKS: open 24 / close 13 |
| h3-mech.txt / h3.ideal | TL pulled UP (puller (0,4,0)) + left pair, right column of 3, P_T (3,3,0) west (T parks at (2,3,0)), RB (1,4,1), Q (0,4,1) facing east (RB to (2,4,1)); the TL puller fires with Q from ONE powered block (0,5,1) (Q directly, TL puller by quasi-connectivity, updated by Q's motion) | WORKS: open 24 / close 13 |
| h2-mech.txt / h2.ideal | RB (2,4,-1) powering X dust AND a level-type TR puller | FAILS: a redstone block next to X makes the dust connect to it (corner shape), so X stops pointing into A |

Windows (ideal ticks, one variable at a time; real observer ticks are +1):
* h1: **M, left column, right pair, TR puller: ANY tick 0..40** (A held: B's close window is
  blocked by the extended A and A2/A3 are swallowed); q 3..6 (pt 7); pt 6..9 (q 5), i.e.
  pt - q in 1..4; a1 0..4; bon 9..11; a2 13..14; a3 >= 21; boff 18..19.
* h3: q 3..6, pt 6..9, a1 0..4, bon 9..11, left pair / right column / M anytime.

## Why the hold is hard to make pay (count analysis)
s7q (44) IS pist19r with A held (RB (0,4,0) west of A, Q (0,4,-1), P_T (1,3,2) north). Versus
w42: + Q + RB (2 mechanism blocks) + Q's feed (2 observers off the lever) - M's late trigger (4)
+ M's early trigger (observer watching BR + block, 2) = +2. To beat 42 the hold must share:
* Q needs ONE pulse per edge at real t4..t7 (P_T at t8). Every cell next to Q is >= 2 observer
  hops from the lever cluster, except Q (0,4,-1) (s7q: lever on an observer watching it, then one
  more observer).
* A's free neighbours: (0,4,0) (needed by a TL puller or RB), (1,4,1). P_T's quasi cell must not
  touch RB's rest cell, and P_T's head is always directly below RB's rest cell, so a BUD-ed P_T on
  RB's level retracts at once (before T lands).
* RB cannot double as a level source for a level-type edge piston: no cell is next to both A and
  an edge puller; next to X it changes X's shape.

## e41 (41): the same trick on wire19sf's mirrored w42e
`e41.patch` on w42e: X dust + floor + the TL chain ((0,4,-2), (0,4,-1)) -> block X' (1,4,-1) +
cross dust (0,4,-1) on floor (0,3,-1) pointing into the TL puller (0,4,0). Open 28 / close 26,
32 positions. **General lesson: X (A's pulse hub) fires three times per lever edge (p0, A2, A3);
any "anytime" pull-type piston that X can reach through one dust (+ floor) costs 2 blocks and no
chain.** A solid block X' in place of the X dust powers A-high directly and A-low by
quasi-connectivity (through B's head), and needs no floor, which frees (1,3,-1).

## b40 (40) = BEST.txt: TL AND TR up, both on X's three pulses
Built from r41 (`b40core.patch`, `b40nom.patch`, `b40p.patch`, then `door synth` for M):
* **TL and TR are both pulled UP**, by pullers (0,4,0) and (2,4,0) on either side of A, each fed by
  a cross dust next to the X block (1,4,-1): (0,4,-1) on (0,3,-1) and (2,4,-1) on (2,3,-1). Both get
  X's three pulses (p0, A2, A3). The left pair (-2,0..1,0) follows TL (observer (-1,2,0) watching
  TL's cell + block (-2,2,0) above the pair), the right pair follows TR (observer (3,2,0) + block
  (4,2,0)): every side piston toggles three times per edge.
* **P_T moves to (1,3,-2) facing south**: T parks at (1,3,-1), the cell X's old floor held (free
  now that X is a block). P_T's quasi cell is O_a's cell (1,4,-2): the lever must not touch it, so
  the lever moved to (2,4,-3) on an observer (1,4,-3) facing east that watches it (O_a watches that
  observer: p0 at t4, everything 2 gt later than w42). X' also quasi-powers P_T during X's pulses,
  but nothing updates P_T then (BUD-safe).
* P_T's feed: observer (0,5,-2) watching O (the B chain's first observer, t6) -> observer
  (0,4,-2) facing up (t10) -> block (0,3,-2) next to P_T. Window (ideal): pt 6..10.
* **M's trigger is 2 observers** (found by `door synth`, k=2): (-1,-1,0) watching L0's gap
  (-1,0,0) -> (0,-1,0) -> M's quasi cell (1,-1,0). L0 toggles three times per edge, which gives M
  the late push on close that w42 needed a lamp for (ideal window: m >= 23). Mirror variant
  b40r: (3,-1,0) watching RP0's gap -> (2,-1,0) -> M's quasi cell.
* Budget: 19 mechanism + lever 1, lever watcher 1, O_a 1, X 1, two dust + floor 4, B chain 4
  (O, J, R_B, O'), P_T feed 3, left pair 2, right pair 2, M 2 = 40. Open 38 / close 34.

## b39 (39) = BEST.txt: the B window and P_T on one chain
`b39.patch` on b40. P_T (1,3,-2) is quasi-powered through O_a's cell (1,4,-2), so an observer
ABOVE that cell facing up fires straight into it and updates P_T. That observer is O (1,5,-2),
turned up: it now watches Y (1,6,-2), an observer facing north that powers R_B directly (no J).
Y is fed by two observers climbing from the lever watcher: (1,5,-3) facing down (t4) -> (1,6,-3)
facing down (t6) -> Y (t8) -> R_B (B window t16-24, A2 t18, A3 t26) and O (t10) -> P_T.
Budget: 19 mechanism + lever 1, lever watcher 1, O_a 1, X 1, two dust + floor 4, (1,5,-3),
(1,6,-3), Y, O, R_B, O' 6, left pair 2, right pair 2, M 2 = 39. Open 40 / close 36, 32 positions.

## Searches that found nothing smaller
* r41: `minimize --moves all --plateau` (336k swaps, 34k neutral edits); resynth max-size 5.
* b40: minimize --moves all --plateau; regrow --size 2 --any (67k), --size 3 (1.78M).
* b39: minimize --moves all --plateau (31k neutral edits); resynth max-size 4 (245 groups).

## Why the hold (the brief's direction) does not pay
* hA (43, `hA.patch` on w42): RB (0,4,0) west of A, Q (0,4,-1) south, fed by w42's P_T-chain head
  (0,4,-2) turned north + an observer (0,4,-3) watching the lever (s7q's feed). Trick worth
  keeping: P_T (-1,3,0) is EXTENDED at rest (RB quasi-powers it); when RB leaves it retracts
  empty with A, its t8 pulse then pulls T; on close the pulse ends after RB is back, so P_T stays
  extended. A's extension is blocked while T is still moving into (1,3,0); T's landing updates
  A, which then pushes T and C home. Open 24 / close 14 (fast close).
* Count: Q + RB + Q's feed (>= 2, Q needs ONE pulse at real t4..7 and every cell next to Q is
  >= 2 hops from the lever) cost >= 4; M's late trigger it replaces costs 4 in w42 but only 2 in
  b39 (L0 toggles three times, which gives the late close push for free). Net >= +2.
* The X-block trick cannot be combined with hA: the X block (1,4,-1) would power Q (0,4,-1).
  With both TL and TR up (b39), A's only free neighbour is (1,4,1), and RB's open cell would then
  sit next to a puller.

## Ideas examined for 38 (none pays yet)
* Lever at (1,5,-3) on O (one observer fewer in the Y chain): Y fires at t4, P_T at t6 = p0 + 2,
  but on open P_T must come >= 4 gt after p0 (T lands 4 gt after p0). A lamp under the lever
  (weakly powered, so it does not quasi-power P_T) makes p0 t2 on open but t6 on close, where P_T
  then collides with p0.
* P_T on the far side (south) frees the lever cell next to O_a (no lever watcher) but its feed
  must cross over B.
* The side-pair follower blocks: an observer firing into air quasi-powers only the upper piston
  of the pair; the lower one then needs its own feed (>= 2).

## b38 (38) = BEST.txt: a note block as the lever's silent relay
`b38.patch` on b39. The lever sits at (1,5,-3), hung on O (1,5,-2). Its weak power reaches the
NOTE BLOCK at (1,4,-3) (powered state flips at once on both edges, no lamp-style delay), which O_a
watches: p0 at t2. A weakly powered note block passes nothing on, so P_T's quasi cell (1,4,-2)
(= O_a's cell) stays dark; a lever there (or a block the lever is hung on) would hold P_T
extended. (1,6,-3) watches the lever itself: Y t4 -> R_B (B window t12-20), O t6 -> P_T, i.e.
P_T = p0 + 4 (T lands 4 gt after p0). Budget: 19 + lever 1, note block 1, O_a 1, X 1, dusts +
floors 4, (1,6,-3), Y, O, R_B, O' 5, followers 4, M 2 = 38. Open 36 / close 32, 32 positions,
`door buildable`: BUILDABLE.
**Trick worth reusing: a note block (or any block whose state flips on weak power) next to a
lever is a zero-delay, non-conducting watch target, so an observer can read the lever without
the lever touching the observer's neighbourhood.**
* b38r.txt (38): b38 with M fired from the right pair (observer (3,-1,0) watching RP0's gap ->
  (2,-1,0) -> M's quasi cell), 32 positions.

## Searches on b38 that found nothing smaller
minimize --moves all --plateau (30k neutral edits); resynth max-size 4 (block, dust, dot, rep,
rep4, obs, torch, lamp, bulb) and max-size 5 with `note`; regrow --size 3 (1.42M, with note);
regrow --size 2 --any --margin 2 (254k, with note).

## Where b38's 38 blocks go (and why each part is hard to cut)
* 19 mechanism (door 9, TL/TR pullers, two side pairs, M, A, B, P_T).
* The x=1 control cube (x=1, y 4..6, z -3..-1, all 9 cells used): lever, note block, O_a, X block,
  (1,6,-3), Y, O, R_B, O'. Timing is exact: p0 t2 is the earliest (one observer after the lever),
  P_T must be >= p0 + 4, and every other cell around P_T's quasi cell is either a lever
  neighbour or too far.
* TL/TR feeds: dust + floor each (the only non-inverting relay from the X block to a puller
  diagonal to it; a wall torch on X inverts every pulse into a retract + extend pair).
* Side pairs: observer watching TL/TR + block above the pair (an observer firing into air
  quasi-powers only the upper piston).
* M: 2 observers (no cell next to M's piston or quasi cell changes, so >= 2).

## Next ideas
* A cheaper side handler: each side costs puller feed 2 (dust + floor) + follower 2. A relay
  that turns X's strong power 90 degrees without a floor, or a follower whose one block powers
  both pistons of the pair from a cell that already changes, would give 37.
* The note-block relay generalises: any observer that must read a lever (or other weak power)
  without the lever touching its neighbourhood. Worth trying on the other boards' lever clusters
  (fc2's, the flat board's).
* `door synth --palette ...,note` now knows the relay; resynth/regrow accept `note` too.
* Hold variants (h1/h3 ideal schedules) give a 13-14 gt close; a fast-close slime-free door
  could start from hA (43, close 14) if a speed board for slime-free doors appears.

## Round 2 (after the 38): looking for 37
Ideal-input studies (all WORK, none wired below 38):
* `sb-ideal.patch` / `sb.ideal` (SOLID board): the bottom row BL, M, BR glued to a slime row
  (0..2,-1,0) and pulled down by ONE pull-type piston (1,-3,0); L1 and R1 become single pistons
  fired by O_L / O_R alone (an observer firing into the air cell above a piston quasi-powers it and
  updates it). 35 blocks without the bottom feed; works for any bottom pulse at m >= 21. But the
  bottom piston sits 4 rows below the nearest changing cell (L1's gap), every cell beside the slime
  row drags whatever stands there, and `door synth` finds no feed of <= 3 blocks (static model
  finds no path): 38 at best.
* `bld-ideal.patch` / `bld.ideal`: BL pulled DOWN by (0,-2,0) instead of left (L0 and the follower
  block gone, O_L fires L1 alone): 35 blocks without feeds, BL works at any tick, M at >= 21. BL's
  puller and M's piston are horizontal neighbours, so no single observer or block powers both
  (an observer powers at most two VERTICALLY stacked pistons: the one it points into and, by
  quasi-connectivity, the one below); `door synth` k <= 3 for BL + M: nothing.
* The bottom row of three down-pullers (BL, M, BR) has the same problem: horizontal neighbours need
  a dust line (6 blocks with floors).
Searches on b38, all nothing: `door synth` k = 1 for the TL feed (dust + floor; 13-state palette
incl. note, lamp, bulb, target, comparator) and for the left follower (--loose: only 2-block
followers exist, the current one and its note/lamp/target variants); k = 3 for {TL feed + left
follower}, {left follower + M}, {right follower + M}, {both TL/TR feeds}; `minimize --moves all
--depth 2 --plateau-pairs`; `regrow --size 3 --any` (obs/block/dust/note, ran for 90 min);
`resynth --margin 2 --max-size 3` (with torches, comparators, note blocks).
Why the cube (x = 1, 9 blocks) is pinned: P_T must fire >= 4 gt after p0 (T lands 4 gt after A's
pull) and B >= 4 gt after P_T; R_B (delay 4 = 8 gt) is the only one-block window, so its input must
come at t4 = two observer hops after the lever (W, Y), and P_T at t6 = three hops (O watching Y).
O_a must sit at (1,4,-2) (the only free cell outputting into X), which is P_T's quasi cell, so the
lever cannot touch it (hence the note block); O' must watch R_B from (1,5,-1).
Note-block relay on other clusters: `resynth --palette ...,note --max-size 3` on fc2 (40) and on
flat-b v7 (44): nothing. Their lever neighbourhoods are already full of watchers (fc2: K feeds R_q;
flat-b hangs the lever on the observer that watches it), so the relay has nothing to replace.
