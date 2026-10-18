# Track `pareto-mid`: the middle of the solid Pareto frontier (open ticks, close ticks, blocks)

Direction (coordinator): the solid frontier at the start was 38 (36/32), 40 (24/9, fc2), 41 (22/9,
fc2f), 43 (16/16, mp8), then nothing until 107 (10/7, lamp-sweep-a1-107). Fill the middle: every
design not dominated on all three counts. `mctas door frontier --check <file>` (this track's tool)
says whether a design is on the frontier and what dominates it.

## Frontier points found (all `door verify --extra-offsets 28`, rotation-safe, category solid)

| file (archive/) | blocks | open / close | from | what changed |
|---|---|---|---|---|
| `pareto-mid-mp8-leafU-43b.txt` | 43 | 15 / 17 | mp8 (43, 16/16) | the bottom-right flag is an oak log, the first observer of U's 3-observer chain becomes persistent leaves next to it (2 swaps, same count): U at t13 open / t15 close |
| `pareto-mid-mp8-lampU-44b.txt` | 44 | 14 / 16 | mp8 | the bottom-right flag is a redstone block; a lamp (4,-2,0) next to the cell it moves into, observer (3,-2,0) watching the lamp, dust (2,-2,0) on stone (2,-3,0) beside U: U at t12 open / t14 close (+1) |
| `pareto-mid-mp7-repU-46b.txt` | 46 | 14 / 14 | mp7 (45, 16/16) | U at t12 on both edges: flag -> observer (3,-2,0) -> block (3,-3,0) -> delay-1 repeater (2,-3,0) on a floor (2,-4,0) -> U (+1). Needs mp7's pump that does not trip on close (below) |
| `pareto-mid-mp7-leafC-46b.txt` | 46 | 15 / 13 | mp7 | log flag; leaves (4,-2,0) next to the cell it moves into; observer (3,-2,0) watching them -> dust (2,-2,0) on stone (2,-3,0) beside U: U at t13 open / t11 close (+1) |
| `pareto-mid-mp7-repU-pgF-50b.txt` | 50 | 14 / 12 | mp7-repU | F asymmetric (`pgF.patch`, +4): R_F and its floor out; repeater d1 (0,4,-2) reads the lever's block and powers a sticky piston Pg (0,4,-3) facing east, which pushes a redstone block from (1,4,-3) to (2,4,-3), where a delay-2 repeater (2,4,-2) reads it and powers F's stone (2,4,-1). Opening: the block lands at t4, F moves at t8 as before; closing: Pg pulls it away at t2 (instant-off), F goes off at t6 instead of t8 |
| `pareto-mid-mp7-leafC-pgF-50b.txt` | 50 | 15 / 11 | mp7-leafC | the same F shuttle |
| `pareto-mid-mp7-dustU-47b.txt` | 47 | 14 / 13 | mp7-leafC | `dustU.patch` (+1): the bottom-right flag is a redstone block and the leaves become dust (4,-2,0) on a floor (4,-3,0) under the cell it moves into: lit when it lands (t10), dark the moment it leaves (t8, instant-off), so the observer (3,-2,0) fires U at t12 opening / t10 closing |
| `pareto-mid-mp7-dustU-pgF-51b.txt` | 51 | 14 / 11 | mp7-dustU | plus the F shuttle: F off at t6 closing, U at t8, C lands t11 (with ideal inputs U may fire anywhere from t5 to t9 closing for 11) |

Frontier after these (solid, all branches): 38 (36/32), 40 (24/9), 41 (22/9), 43 (15/17), 43 (16/16),
44 (14/16), 46 (14/14), 46 (15/13), 47 (14/13), 50 (14/12), 50 (15/11), 51 (14/11), 93 (12/7,
fast-clean), 107 (10/7), 108 (9/7). For a new door with 52..92 blocks to count it now needs open
<= 13, or close <= 10 with open <= 21.

All eight also pass `door vanilla-check` (the unmodified 26.3 server, tick for tick at 4 positions x
3 cycles, plus a real-time 20 tps cycle): the leaves, lamp, dust and shuttle timings are vanilla's.

## The mp family (fresh-b's mirrored pump) and its timing
Mechanism (mp5..mp8): H (lever, t1) pushes the left side west; A1 (A's first pulse, t3, observer on
the left structure's top flag) lifts TM into E (1,3,0); the pump P under a 2-slime row (BM + corner)
pushes C into TM's cell (t7..t11, flag observer + delay-2 repeater) and pulls the row back; F (lever
+ delay-4 repeater, t8) pushes the right structure east and its carrier drags TM aside; A2 (t10,
observer on the right structure's top flag) pulls C into E; U (t14, 3-observer chain off the right
structure's bottom flag) reach-pulls the row down: open 16. Close: the same events push C down
(A1, spit), bring TM back (F), push TM + C home (A2, spit, C lands at t13), lift the row (U).

### The pump trip (why mp8 cannot simply fire U earlier)
mp8's pump repeater R_P points straight into the pump P (0,-2,0). On the close edge the lowered P
sits at (0,-3,0) under the lowered slime row; R_P's t7 pulse strongly powers the slime at (0,-2,0)
and its turn-on updates that cell's neighbours, so the lowered P fires: the row goes up at t7 (U,
glued under it, rides up), P pulls it back at t11, and U is pushed down again, landing at t13 BE,
before the row (block-entity order). So on the close edge U needs a NEW power edge at t14 or later
(an extend queued by its landing finds the slime still moving and is dropped, no retry): close >=
16 with 2 gt pulses, whatever the open edge does. mp7 (fresh-b's previous step, 45 blocks) feeds
the pump through dust on obsidian (R_P -> dust (0,-1,-1) -> obsidian (0,-2,-1) -> P): dust updates
reach 2 cells, the lowered pump is 3 away, so it is never updated and never fires on close (it is
quasi-powered, but stays a silent BUD until the door's own close lifts it back). In that no-trip
world U can fire at the same tick on both edges: U at t12 gives 14 / 14.

Measured with ideal inputs (`mp8-PU.sched`: P and U by redstone blocks, no trip; `mp8-PUr.sched`:
P through its real repeater turned to delay 1, trip modelled), open edge: pump [7,11) -> U must
extend at t12 (2 gt) or t11 (3 gt): open 14; pump [6,9) and U at t10: open 12. Close edge, no trip:
U at t10..t11 -> close 13 (A2's C lands at 13: F's return at t8 sets it); with the trip, close 13
only if the trip ends by t8, 14 if by t9, 15 if by t10, else 16.
`mp5-close.sched` (fresh-b's mp5 mechanism, separate close timings): the mechanism closes in 8
(A1 at t1, F back at t3, A2 at t5/6, U by t5) and opens in 12 with ideal inputs; every one of those
close timings is earlier than its open counterpart, so it needs per-edge (asymmetric) control.

### What bounds the wired mp doors now
* open: the pump [7,11) (left flag observer t3 + delay-2 repeater) lands the row at t13, so U
  retracts at t14 at the earliest: open >= 14. Open 13 needs the pump to end by t10 (a 3 gt pulse,
  odd edges near the bottom-left), open 12 a pump [6,9) plus U at t10.
* close: F returns at t8 (lever + delay 4, symmetric), TM lands t10, A2 pushes at t11, C lands t13:
  close >= 13 unless F returns earlier on the close edge only. The shuttle (`pgF.patch`, +4) does
  that: F off at t6, A2 at t9, C lands t11, so close 11 (leafC) or 12 (repU, U at t10 + spit).
* U's timing per edge is free within the toolbox below (offsets from the right structure's move at
  t8): pairs (open, close) = (9,11)+2k, (11,9)+2k [leaves], (12,14)+2k, (14,12)+2k [lamp + observer],
  (12,10)+2k [dust + observer: the 47], (10,10)+2k [observers]. I first missed the dust pair and
  thought 14 / 13 needed (12, <= 11) from a 1 gt asymmetry; (12, 10) does it, since C lands at t13
  anyway.

`door grow --k 1` (states obs, block, dust, rep1/2, torch, leaves, log; box -4..5 x -6..7 x -2..2):
no single added block gives mp7-repU open <= 13, or close <= 13 with open <= 21; none gives
mp8 / mp8-leafU open <= 14, or (open <= 15, close <= 16). `door minimize --moves all --plateau` at
their speed bounds: mp8-lampU 44 and mp7-repU 46 are local minima.

## New timing elements (gadgets `log-leaves`, `rb-lamp`, rows in docs/mechanics.md)
* **Persistent leaves next to a moving log** (odd, asymmetric): leaves re-read their distance in a
  scheduled tick 1 gt after a neighbour changes and never random-tick. A log LEAVING their neighbour
  cell at tick m changes them at m+1; a log ARRIVING is a moving block (distance 7) until it lands
  at m+2, so they change at m+3. One leaves block per log: two leaves re-read each other and ripple
  (1 -> 3 -> 5 -> 7, extra observer pulses).
* **Lamp next to a moving redstone block** (even, asymmetric): lit at once when the block lands
  (m+2), dark 4 gt after it leaves (m+4).
* **Dust next to a moving redstone block** (even, asymmetric the other way from the lamp): lit
  when the block lands (m+2), dark the moment it leaves (m, instant-off). With an observer: m+4 on
  the move out, m+2 on the move back (mp7-dustU).
* **Shuttle** (a level whose falling edge comes 2 gt early): lever + delay-1 repeater -> sticky
  piston -> redstone block pushed next to a repeater: on at 4 + 2d, off at 2 + 2d (mp7-*-pgF).
* **Dust on a block beside a piston**: an observer whose output cell sits next to a slime row (where
  an observer or block would be dragged) can drive the piston through a dust there (dust is never
  dragged) standing on a block next to the piston; the dust also updates 2 cells deep.

## fc2 family: an earlier B window would open in 18..20 (not wired)
`fc-asym.sched` (fresh-c's fc2 ideal mechanism with Q's timing per edge): with fc2's own Q (t4 on
both edges) the B window can start at t7 already: window [7,14) opens in 18, [8,16) in 20, close 9
in every case (fresh-c's bound "b >= d + 6" is not tight). With Q at t1 on open (Q powered by the
lever directly OR by its delay-2 repeater, off at t4 on close) the window [6,14) opens in 18 and
[5,12) in 16. The blocker is the window itself: B must turn on at t7..t8 and off 7..8 ticks later,
and A's two dance pulses must come 2 ticks after each edge (fc2's O watches the window). A delay-4
repeater only gives [8,16) from an input pulse starting at t0; from the lever observer (t2) it gives
fc2f's [10,18) = 22. An OR of two delay-3 repeaters fed at t2 and t4 gives [8,16), but then O must
watch the OR (a dust carrying both) and the geometry around B/O/X/R_B has no room for it in fc2f's
layout. A cheap t0 pulse (a cut of a lever-fed line at t1..t8 on open) would give 20 at +1..2 blocks.

## Tools (own commits)
* `door frontier [--all] [--check FILE...] [--any-category] [--cache-dir D]`: the frontier from the
  leaderboard's verdict cache (designs not in it are verified), every working design with its ticks
  (`--all`), and each candidate placed against it (ON THE FRONTIER, which frontier doors it pushes
  off, or which doors dominate it).
* `door grow --states leaves,log` shorthands.

## Log
* Background: `door resynth` on a1-107 with `--max-open 12 --max-close 8` (sizes 2..3, full palette
  with lamps/bulbs/notes/targets): 107 -> 107, nothing (the 107's wiring is resynth-minimal even at a
  relaxed speed). `door minimize --moves all --max-open 16 --max-close 8` on a1-107: nothing (no
  removal, pair, swap or move works even at open 16). s4-108 (12/7) is not legal any more (dust and
  stone at z = -4 in the columns in front of the door, rule of 16:40Z), so the speed family can only
  shrink after a re-route.
* `door minimize --objective open --slack 6 --moves all` on mp8: nothing. `--max-open 14
  --max-close 14` on mp7-repU: nothing (46 is a local minimum at 14/14).
* `door minimize --moves all --plateau` at their own bounds: mp8-lampU (14/16) and mp7-leafC (15/13)
  are local minima too.
* fast-clean's compliant 93 (12/7, `fast-clean-93.txt`, copied from origin/explore/fast-clean
  BEST.txt) at relaxed speeds, where any smaller door would be a frontier point: `door minimize
  --moves all` at `--max-open 13 --max-close 20` and at `--max-open 21 --max-close 12` (1.7M
  candidates each, none even passes the 2-position screen) and `door resynth --max-open 13
  --max-close 12 --max-size 3` (full palette): nothing; `--plateau` at both relaxations (180
  verified neutral designs, 14940 removals and 2976 reroutes after them): nothing. Its wiring has no
  slack even 1 tick slower on open or 5 slower on close.
* fc2f (41, 22/9): `door grow --k 1 --max-open 21` (any close; obs, block, dust, rep1..4, torch,
  leaves, log, redstone block, lamp; box -5..5 x -3..8 x -2..2, 7839 candidates) and `--k 2` around
  its B chain (obs, block, dust, rep1..3; box -4..2 x 3..8 x -1..1, 366750 candidates): nothing
  passes even the screen. One or two blocks cannot move fc2's B window (see the fc2 section).

## Next ideas
* fc2 family, open 18..20 with close 9 (see the fc2 section): build B's window [8,16) as the OR of
  two delay-3 repeaters fed at t2 (lever observer) and t4 (observer on O_a), feeding a dust that O
  watches; estimated +4..8 blocks, so (45..49, 20, 9), a frontier point at any of those counts.
* mp7 with F asymmetric, measured with F driven by an ideal input (`mp7-F.sched` on
  `mp7-repU-idealF.txt` / `mp7-leafC-F.sched`: R_F and its stone replaced by a scheduled redstone
  block): F off at t6 on close gives repU 14 / 12 and leafC 15 / 11 (built: the two 50s); F cannot
  move before t8 on open, nor go off before t6 on close. With F off at t6, `pgF-U.sched` (U by ideal
  input) closes in 11 for U anywhere from t5 to t9: 14 / 11 is this mechanism's floor (built: the
  51). Going lower needs C to land before t11 (A2 earlier, so TM back earlier, so F off before t6,
  which breaks the close).
* The shuttle is a general tool: any lever + delay level that should end earlier on the falling
  edge (by 2) can become lever + delay 1 -> sticky piston -> redstone block -> repeater, +4 blocks.
* mp family open 13: a 3 gt pump pulse [7,10) and U at t11 (rest-side leaves + one observer on the
  right structure's flag, if a cell for that observer can be found next to U).
