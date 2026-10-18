# Track riders: blocks riding moving structures as timing elements

Direction: beat the rb solid record (47) by giving P_R / Q their asymmetric timing with "riders"
(blocks carried by a moving structure) instead of the 7-block silent hold. Then (coordinator,
08:15Z) re-target the ideas at fresh-c's fc2 (40).

## Results (all `door verify --extra-offsets 28`, 32 positions x 3 cycles)
| file | blocks | open/close | idea |
|---|---|---|---|
| fx1.txt = BEST.txt | 40 | 24 / 26 | fc2 (40) with P_L fired by X's THREE pulses per lever edge (odd count: the pulse-toggled left column ends right); no observer pair for P_L, a 2-slime arm instead |
| r3.txt | 43 | 22 / 33 | `door minimize --moves all --plateau` of r2: the redstone-block rider became an OBSERVER riding the left structure that re-fires Q on each landing |
| r2.txt | 46 | 22 / 23 | rb6-bmobs-47 without the hold: Q held by a redstone block riding the LEFT structure; P_L held by the B window |

## fx1 (40): an odd number of pulses toggles a pulse-driven piston
* fc2's X dust (0,4,0) pulses A three times per lever edge: p0 (O_a, t2) and the two dance
  pulses (O watching R_B, t14 and t22; on close they are swallowed by the extended A but X
  still pulses). Three toggles of a pulse-driven sticky piston = one net toggle.
* P_L moves from (-3,2,0) to (0,3,-1) facing west, right next to X's floor (0,3,0) (weakly
  powered by each X pulse), and pushes an arm (-1,3,-1) + (-1,2,-1) glued to the left backbone:
  push+spit on the first pulse, pull back on the second, push+spit again on the third. The
  left column opens at t2, comes back at t14, opens again at t22 (close: the mirror); the
  center dance does not care (stone door blocks drag nothing).
* -3 (o, A-obs, old P_L) +3 (P_L', 2 slimes) = 40, open 24 / close 26 (fc2: 24 / 9).
  `door minimize --moves all --plateau`: nothing more. The arm can't be 1 slime: the only cells
  next to X's floor that can push the left structure west are (0,3,+-1), two steps from the
  backbone; (0,3,+1) would need R_q's floor (-1,3,1) as an arm slime (R_q would pop).

## r3 (43): a self-sequenced close (minimizer result from the r2 seed)
Three minimizer steps: K (0,5,0) removed (R_B2 points into P_L's quasi cell directly: 45); the
rider slime (0,3,-1) + redstone block (1,3,-1) swapped for ONE observer OBS (-1,4,-1) facing
west, glued to the left backbone's top slime (-1,4,0), its back into Q (44); R_B2 dropped and the
glass under it turned into a block (0,4,1) that X's cross dust powers, next to P_L (43).
* OBS rides the left structure. Whenever the structure LANDS, OBS fires 2 gt later (the landing
  shape update), from wherever it landed: in the open position its back faces an empty cell
  (harmless); in the closed position its back is Q, so every return of the left structure gives
  Q a 2 gt pulse.
* P_L is powered by the lever block (quasi) AND by (0,4,1) during each X pulse, so on close
  every A pulse also kicks the left structure out and back: the left structure becomes a clock
  driven by the dance, and OBS turns its landings into Q pulses.
* Close, traced (c = ticks after the flip): c1 the lever drop retracts Q (RB back to A) and P_L
  (left structure home, OBS lands c3); P_R retracts alone (T is blocked by C, the right structure
  stays out). c4 A (RB back) pushes C down one. c5 OBS pulse: Q pushes RB to P_R and SPITS it
  (pulse-driven shuttle), A retracts. c10-18 B dance (C pushed home by A's c12 pulse), X pulses
  kick the left structure out/back twice; c26 OBS pulse after its last landing: Q's sticky head
  PULLS RB back (pulse-toggle), P_R pulls the right structure and T home, RB lands at A, A
  pushes T down: closed at 33 with the right side re-opened once (final state exact).
* Open is rb6's (OBS fires into the empty cell at t5).

## r2 (46): the left structure carries Q's hold
* rb6's Q (shuttle of RB_A) needs power from open t1 until the close dance ends (t18-20); rb6
  pays 7 blocks for a 16 gt silent hold.
* Rider: slimes (0,3,0) and (0,3,-1) glue a redstone block RID (1,3,-1) to the left backbone.
  When the left structure opens (t1-3), RID lands at (0,3,-1), right under Q: Q stays powered
  while the left door column is open, whatever updates it gets (the lever's at close t1, the
  B-push and X updates through its head at t10-14).
* For that the left structure must come back only AFTER the close dance: P_L (QC from the lever
  block, released by A's motion) is also powered by the B window: a second B repeater R_B2
  (0,5,1) (d4) reads the cross dust's floor (0,5,2) and points into a block K (0,5,0) right above
  P_L; K also powers B (harmless: same window as R_B). Glass floor (0,4,1) because X points into it.
* Close: t10 B pushes A (P_L updated, K powered: stays); t18 R_B/R_B2 fall, B lifts A, P_L
  retracts, RID leaves Q, Q retracts, RB_A leaves P_R, P_R pulls T into (1,3,0) (block events in
  that order, all at t18); A, T, RB_A land at t20; A pushes T down. Closed at 23 (rb6: 25).
* Failed first try (r1.txt, broken): with P_L released at t10 (rb6's timing), RID comes back to
  (1,3,-1) during the close dance and powers A at (1,3,0) (A stays extended, B cannot lift it).
  Ideal check that led to r2: a redstone block at (0,5,1) from close t9 to t17 (`door drive`).

## Same trick on the sculk 42 (st43-bmobs): solid 43
st43-bmobs-42 holds Q with a sculk sensor under it at (0,3,-1). Replacing the sensor by r3's
riding observer OBS (-1,4,-1) facing west plus the block (0,4,1) next to P_L gives a SOLID 43
(wip/st-v4.txt, open 22 / close 33; (0,3,0) can be stone, the slime is not needed). Without
(0,4,1) it breaks (wip/st-v1, st-v2): the X-pulse kicks of the left structure are what drive the
riding observer during the close. `door minimize --moves all --plateau`: no gain (43).

## fc2 (40, fresh-c) with riders: nothing below 40 so far
* fc2's structures are far from its control: the left structure is 3 slimes (y <= 2) pulled by a
  pulse-driven P_L at (-3,2,0); the shuttle Q/RB is at y=4; R_B at y=6. A redstone-block or
  observer rider would need 2-3 glue slimes to reach Q, R_B or P_M, and every 2-block group of
  the control already costs exactly 2.
* Ideas checked on paper: a rider holding Q (Q = lever OR "left open") instead of R_q + floor
  (needs >= 3 glue blocks); a riding observer re-firing Q as in r3 (fc2's close relies on A
  being extended during the close's B window, a pulse-driven Q breaks that); the right
  structure's landing as B's trigger (R_B can't be reached from the right structure without new
  blocks); P_M from a rider (2 blocks, same as the observer + block).
* Searches (all on fc2, nothing found): `door grow --k 1` after removing each 2-block group (P_L's
  observer pair, O_b + its block, P_M's observer + block, R_q + floor) with obs/block/dust/rblock/
  repeaters/torch/slime; `door regrow --size 2` (28.7k candidates) and `--size 3` (1.43M
  candidates, states obs/block/dust/rep/rep4/rblock/slime): nothing; seed-and-exploit (below).

## Tool: `door grow --then-search` (seed-and-exploit)
r2 -> r3 showed that a seed which changes the machine (a rider) lets the local search find
removals it could never reach from the record itself. `door grow <design> --k 1 --want 0
--then-search --moves singles,pairs,triples [--seed-minutes M] [--seed-from N]` adds every
single block that keeps the door working, then runs the local search from each such seed and
writes the best result below the start's block count. `--glue` keeps only riders (every added
block touches a slime/honey block, or with `--k 2` an added slime that touches one);
`--changed` skips seeds whose screen timings equal the start's (inert blocks; checked in
parallel, ~3 s per seed per worker). Runs so far, all "nothing below the start":
fc2 single observers/redstone blocks around the left structure (117 seeds) and the right
structure (223 seeds); fx1 riders (`--glue`, 231 seeds: 1-42, 44-85, 87-128 searched; seeds 43,
86 and 129 made a worker JVM run out of memory and wedged the pool, so the run was restarted
past them with `--seed-from`); `door regrow --size 3` on fx1 (1.74M candidates, rider states
included). Caveat for long seed runs: a seed whose machine runs away (a riding observer that
keeps re-triggering) can OOM a worker and hang the pool; restart with `--seed-from`.

## What failed and why (summary)
* rb family below 43: the B window still needs rb6's 6-block bridge (O2 must watch the lever
  from a cell whose back is far from every R_B input), so the rb family stays above fc2's
  observer-chain B trigger.
* fc2 below 40 with riders: its structures are far from its control (left column y <= 2,
  shuttle y = 4, R_B y = 6), so every rider needs 2-3 glue slimes, and each fc2 control pair
  (P_L pulse, Q delay, B trigger, P_M pulse) already costs 2.
* Pulse-driven shuttle Q on open (r3 style): the spit places RB next to P_R in the block-event
  phase while T is still moving, so T is not dragged (fresh-c's finding, confirmed); r3 only
  uses Q pulses on the close.
* Pulse-driven A instead of RB: on close the B window then pushes A, T and C down and the
  dance pulses pull T back up; fc2 needs A extended during the close's B window.

## Round 2 (coordinator, 11:00Z): free P_M in fx1/fc2 -- shown not to fit
P_M (1,-2,0) is pulse-driven (retracted, M two cells away) and costs 2 blocks: observer (2,-1,0)
watching BR, block (2,-2,0) behind it. A 1-block source must be an observer whose BACK is in one
of P_M's power cells: P_M itself (observer at (0,-2,0) W, (2,-2,0) E, (1,-3,0) D, (1,-2,+-1) N/S)
or its QC cell (1,-1,0) (observer at (0,-1,0) W, (2,-1,0) E, (1,-1,+-1) N/S; QC from an observer
does update P_M, it updates its back cell's neighbors). Each of those watches a cell two away
from P_M's column ((-1,-2,0), (3,-1,0), (1,-1,+-2)...) where nothing moves in fx1/fc2.
* Riding observers (fire after every landing, gadget rider-observer): both structures move along
  x, so a rider's two back cells are x-neighbors, while P_M's two power cells are y-neighbors:
  no rider powers P_M in both positions. Powering it in one position only gives the wrong
  parity: the right structure lands once per flip (P_M needs a pulse on both edges); fx1's left
  structure lands out/in/out (open) and in/out/in (close), i.e. 1+2 or 2+1 landings per position.
* A riding redstone block (steady P_M, rb6 style) next to P_M's cells needs 2 glue slimes.
* Searches (all nothing): `door grow --k 1` on fx1 minus P_M's pair, box (-3,-4,-3)..(5,2,3), 24
  states incl. slime/honey/redstone block (3733 candidates); fx1 with the left pusher moved to the
  bottom next to P_M ((0,-1,0) facing west + arm slime (-1,-1,0), 37 blocks) + `--k 2` in the
  bottom box (653k candidates: one 2-block source for both pistons); `door minimize --moves all
  --depth 2` on fx1.
* Cheapest equal-cost variants: observer (0,-1,0) facing west + slime (-1,-1,0) under the left
  backbone (3 pulses per flip in fx1): 40 again.
* Rider seeds on fx1 with `--changed` (after fixing it to compare settle ticks: the first version
  compared only open/close ticks and dropped riders that fire extra but finish on time):
  1-block riders (observer / redstone block / block glued to a slime) in the bottom box: 24 of 64
  seeds change the machine, none searches below 40; in the upper box: 64 of 201 kept, none
  searches below 40 (seed 57, an observer at (-1,2,1) facing north, made a worker run out of
  memory; restarted past it); 2-block riders (slime + payload) in the bottom box: 1823 working
  seeds, 1128 change the machine, only the first ~9 kept seeds searched (30 s each) before I
  stopped it (~9 h for all): low yield, not worth the shared box.
* Verdict: P_M's 2 blocks are minimal in the fc2/fx1 family; a 39 needs a different bottom
  (something that already moves next to P_M's column, e.g. a pulse-driven P_R or the lever at
  the bottom), not a rider.

## Next ideas
* Odd pulse counts are free toggles: any pulse-driven piston can take any existing source
  that fires an odd number of times per edge at harmless moments (X: 3). P_M would need such a
  source moved next to its column (see round 2: nothing moves there today).
* r3's self-sequenced close (riding observer + X-kicked left structure) on fc2's layout would
  remove the B window's need on close entirely; it needs the left structure next to Q.
* `door grow --glue --k 2 --changed --then-search` on fc2/fx1 (2-block riders: slime arm +
  payload); the screen is fast but the timing check is ~3 s per seed, 10k seeds per side.
