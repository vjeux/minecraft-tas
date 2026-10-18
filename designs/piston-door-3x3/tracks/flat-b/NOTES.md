# Track flat-b: "Solid, flat (one z layer), no slime or honey", a different mechanism

Every block in the door plane z = 0, no slime/honey (`door verify` prints `, flat`).

| file | blocks | open/close | note |
|---|---|---|---|
| v7.txt = BEST.txt (archive/flat-b-v7-44b.txt) | 44 | 30 / 26 | `door resynth` on v6: the top chain is (0,3), (0,4), (0,5) down, (1,5) west -> smooth stone (2,5): TR's puller directly below it, T's puller by QC through (1,5) |
| v6.txt (archive/flat-b-v6-45b.txt) | 45 | 32 / 28 | v5 with the bottom cluster mirrored: lever, a1, P_M (pusher side) on the LEFT, R_B/O/X on the right; the left feed starts at the pm observer (4 observers instead of 5) |
| v5.txt (archive/flat-b-v5-46b.txt) | 46 | 32 / 28 | the top pair fired from TL's rest-cell watcher (0,3) (5 observers + stone) instead of the long chain over the left stack |
| v3.txt (archive/flat-b-min-47b.txt) | 47 | 32 / 30 | v2 with the lever hanging under the observer that watches it (no floor) |
| v2.txt (archive/flat-b-v2-48b.txt) | 48 | 32 / 30 | center goes DOWN (elevator below the door), top pair up, one observer chain |

## Mechanism `cd19` (19 blocks): the center goes DOWN, the elevator sits below the door
- `cd19.txt` (+ `cd19.ideal`): mirror of mech-synth's pist19r. A' (1,-2) sticky up (pull-type,
  gap (1,-1)), B' (1,-3) sticky up (the elevator), P_M (3,-1) sticky west (pull-type: M parks at
  (2,-1)); M is pulled down, parked aside, then C is pulled down twice (A' high, B' back, A' low).
- In v2 the left column is a 3-stack (-2,0..2) facing east, the right column a pair (4,0..1) +
  TR pulled UP by (2,4), and T pulled up by (1,4) (two top pullers facing down).
- Symmetric schedule (same on both edges; `door ideal --independent --sweep`, ticks after a1):
  pm 3..10, b1 8..12, a2 = b1+2..3, b2 >= a2+5, a3 = b2+2..4, **top (T's puller) >= a3**
  (on close a2 overshoots C/M up one cell and T's late push puts the column back: the mirror of
  pist19r's late M), left/right columns anytime.
- Why down: the late piston is now at the TOP, where QC lets ONE powered block at (1,5) fire every
  puller facing down at y=4 ((1,4) directly, (0,4)/(2,4) through their quasi cells (0,5)/(2,5)).

## Control of v6 (26 blocks)
- Bottom cluster (12), mirrored from v2/v5: M parks at (0,-1) (P_M (-1,-1) facing east, pull-type),
  X dust (2,-1) on O (2,-2), O watches R_B (2,-3) (repeater d4 facing east, floor = observer
  (2,-4)), R_B reads the dust (3,-3) on stone (3,-4). Lever (0,-4) hangs under observer (0,-3)
  facing down (t2) which strongly powers stone (0,-2) = a1 into A' (1,-2). (-1,-3) east watches
  (0,-3) (t4), (-1,-2) down watches it (t6) and fires P_M = pm. (1,-4) west watches the lever (t2,
  under B': emits east only), (2,-4) west (t4) powers stone (3,-4): R_B window t12..t20, O t14/t22.
- Left feed (6): (-2,-2) east watches the pm observer (-1,-2) (t8), (-3,-2) east, (-3,-1) down,
  (-3,0) down (t14) -> stone (-3,1) + dust (-3,2): the left 3-stack.
- Top (6) and right pair (2): as v5. (v7: top = 5, see the table.)

## Control of v5 (27 blocks)
- Bottom cluster (12): as v2 below, but the lever hangs under the observer (2,-3) (face=ceiling).
- Left feed (7): (-2,-3) east watches the R_B-input dust (t6), (-3,-3) east, (-3,-2..0) down,
  stone (-3,1) + dust (-3,2) on it: the left 3-stack at t14.
- Top (6): TL's rest-cell watcher (0,3) facing down (TL leaves (0,2) at t16 on open / is pushed
  back at t14 on close: one pulse per edge), (0,4) down, (-1,4) east, (-1,5) down, (0,5) west ->
  stone (1,5): T's and TR's pullers at t26 (open) / t24 (close), after a3 (t22). The observers next
  to the pullers all emit away from them and from their quasi cells.
- Right pair (2): as v2.

## Control of v2 (29 blocks)
Bottom cluster (13, lever included):
- Lever (2,-4) on stone (2,-5). Observer (2,-3) facing down watches it: t2, strongly powers stone
  (2,-2) = **a1** into A' (1,-2) (B' gets quasi power from it too, harmless: A' extends first).
- (3,-3) observer west watches (2,-3) (t4), (3,-2) observer down watches it (t6) and fires P_M
  (3,-1) = **pm** (a1+4).
- (1,-4) observer east watches the lever (t2, under B': it emits west only), (0,-4) observer east
  (t4, it is also R_B's floor) strongly powers stone (-1,-4): the dust (-1,-3) on it lights and
  feeds **R_B** (0,-3) (repeater d4 facing west) whose output powers B' (1,-3): window t12..t20.
- O (0,-2) observer down watches R_B: t14 and t22 into the cross dust X (0,-1) on O = **a2** (A'
  high at (1,-1), right next to X) and **a3** (A' low, quasi cell (1,-1)). fc2's dance cluster,
  mirrored below the door.
Late chain (14): (-2,-3) observer east watches the R_B-input dust (t6, once per edge), then
(-3,-3) east, (-3,-2..0) down (t14), stone (-3,1) strongly powered: the left 3-stack fires
((-2,1) directly, (-2,0) by QC, and the dust (-3,2) on top of the stone points into (-2,2)); the
chain goes on through an observer (-3,3) watching that dust, (-3,4..5) down, (-2,5), (-1,5),
(0,5) west -> stone (1,5) at t26 (>= a3 = t22): T's and TR's pullers.
Right pair (2): observer (3,2) facing west watches TR (2,2) and strongly powers stone (4,2):
(4,1) directly, (4,0) by QC.

## Lessons
- Watch a door block's REST cell (the block leaves it, or comes back while the watcher is still
  on): one pulse per edge. Watching a pull-type piston's gap cell double-fires (the head goes in,
  then the block): v4 tried ML's park cell (-1,1) as the top's trigger and the top went up and
  back down.
- The bottom cluster always splits: X (a2/a3, next to A' high) and P_M sit on opposite sides of
  (1,-1), and a1 must come from P_M's side (O occupies X's side under X). R_B's input then needs a
  3-block cross under B' (observer under B' emitting sideways, R_B's floor as an observer, stone +
  dust on the far side). Putting the lever on P_M's side costs less than crossing pm (pm has a
  tight window a1+4..7).
- A pull-type piston facing down at y=4 next to an observer whose back points at its quasi cell
  (0,5) is quasi-powered by that observer (first try v1: D0 got the chain's pulse AND the (1,5)
  block's: a 4 gt pulse, so on close it pushed TL home and pulled it back out).
- A dust on a stone that an observer strongly powers lights (back-feed): it lets the chain end in
  a stack feed and continue (an observer watching the dust).
- The trick "a2/a3 only through M parked next to A'" (M strongly powered by O) frees the top's
  timing (top anytime, `cdM.ideal`), but it needs the M-mover to be a pusher at (0,-1) (a puller
  at (3,-1) is next to M), which puts a1/pm and R_B's input on opposite sides of the door.
