# Track flat-b: "Solid, flat (one z layer), no slime or honey", a different mechanism

Every block in the door plane z = 0, no slime/honey (`door verify` prints `, flat`).
Record of the board when this track started: 57 (track `flat`, pist19r with the elevator on top).

| file | blocks | open/close | note |
|---|---|---|---|
| v7.txt = BEST.txt (archive/flat-b-v7-44b.txt) | 44 | 30 / 26 | `door resynth` on v6: the top chain is (0,3), (0,4), (0,5) down, (1,5) west -> smooth stone (2,5): TR's puller directly below it, T's puller by QC through (1,5) |
| v6.txt (archive/flat-b-v6-45b.txt) | 45 | 32 / 28 | v5 with the bottom cluster mirrored: lever, a1, P_M on the LEFT, R_B/O/X on the right; the left feed starts at the pm observer (4 observers instead of 5) |
| v5.txt (archive/flat-b-v5-46b.txt) | 46 | 32 / 28 | the top pair fired from TL's rest-cell watcher (0,3) instead of a long chain over the left stack |
| v3.txt (archive/flat-b-min-47b.txt) | 47 | 32 / 30 | v2 with the lever hanging under the observer that watches it (no floor) |
| v2.txt (archive/flat-b-v2-48b.txt) | 48 | 32 / 30 | first working: center goes DOWN (elevator below the door), top pair up, one observer chain |

## Mechanism `cd19` (19 blocks): the center goes DOWN, the elevator sits below the door
- `cd19.txt` + `cd19.ideal` (ideal inputs, `door ideal --independent --sweep`): the mirror of
  mech-synth's pist19r. A' (1,-2) sticky up (pull-type, gap (1,-1)), B' (1,-3) sticky up (the
  elevator), P_M sticky pull-type next to M's exit (v7: (-1,-1) facing east, M parks at (0,-1)).
  M is pulled down and parked aside, then C is pulled down twice (A' pushed up by B', B' back).
- v7's columns: left = 3-stack (-2,0..2) facing east; right = pair (4,0..1) facing west + TR pulled
  UP by (2,4); T pulled up by (1,4).
- Symmetric schedule (same pulses on both lever edges), ticks after a1: pm 3..10 (real +4..7 in
  v7), b1 8..12, a2 = b1+2..3, b2 >= a2+5, a3 = b2+2..4, **T's puller >= a3** (on close a2
  overshoots M and C up one cell and T's late push puts the column back: the mirror of
  pist19r's late M), left and right columns anytime.
- Why down: the late piston is at the TOP, where quasi-connectivity lets one powered block above
  the pullers (facing down, y=4) fire several of them, and TL's REST cell (0,2) can be watched from
  (0,3) (free because TL goes left), which turns "the left stack moved" into a clean single pulse.

## Control of v7 (25 blocks), t = game ticks after the flip (close timings in brackets if different)
```
y\x  -3 -2 -1  0  1  2  3  4
 5            d  w  S              top: (0,3)d (0,4)d (0,5)d (1,5)w -> smooth stone (2,5)
 4            d  D  D              D = T's (1,4) and TR's (2,4) pullers
 3            d                    TL rest-cell watcher
 2    X  E  .  #  #  #  w  C       dust (-3,2); TR rest-cell watcher (3,2) -> stone (4,2)
 1    C  E  .  #  #  #  .  W       stone (-3,1): left 3-stack feed
 0    d  E  .  #  #  #  .  W
-1    d  .  E  .  .  X             P_M (-1,-1) east; X dust (2,-1) on O
-2    e  e  d  C  U  d             (-3,-2)e (-2,-2)e (-1,-2)d; a1 stone (0,-2); A'; O (2,-2)d
-3          e  d  U  R  x          (-1,-3)e; (0,-3)d holds the lever; B'; R_B (2,-3) d4 east; dust
-4             L  w  w  C          lever (0,-4); (1,-4)w (2,-4)w; stone (3,-4)
```
- **Cluster (12)**: lever (0,-4) hangs under observer (0,-3) (facing down: t2) which strongly
  powers stone (0,-2) = a1 into A'. (-1,-3) east (t4), (-1,-2) down (t6) fires P_M = pm.
  (1,-4) west watches the lever (t2; under B' it emits east only), (2,-4) west (t4, R_B's floor)
  powers stone (3,-4): the dust (3,-3) on it feeds R_B (2,-3) (repeater d4 facing east) into B':
  window t12..t20. O (2,-2) down watches R_B: t14 and t22 into the cross dust X (2,-1) = a2 (A'
  high at (1,-1)) and a3 (A' low, quasi cell (1,-1); A' lands while X is lit and extends at once).
- **Left feed (6)**: (-2,-2) east watches the pm observer (t8), (-3,-2) east, (-3,-1) down, (-3,0)
  down (t14) -> stone (-3,1): (-2,1) directly, (-2,0) by QC, and the dust (-3,2) on the stone
  (back-fed) points into (-2,2).
- **Top (5)**: TL leaves (0,2) at t16 [is pushed back at t14]; (0,3) fires t18 [t16], (0,4), (0,5),
  (1,5) -> stone (2,5) at t24 [t22 = a3]: TR's puller below it, T's puller by QC.
- **Right pair (2)**: TR leaves (2,2) -> (3,2) west strongly powers stone (4,2): (4,1) directly,
  (4,0) by QC.
- The one chain lever -> (0,-3) -> (-1,-3) -> (-1,-2) -> ... -> (-3,0) -> [stack] -> (0,3) -> ... ->
  (1,5) is 11 observers = 22 gt, exactly a3: it is as short as the timing allows, and every hop
  also does geometry (a1, pm, the left feed, the top). The rest is 14 blocks: lever, a1 stone,
  R_B + its 3-block input cross, O, X, stack stone + dust, top stone, right pair.

## Searches run on v3..v7 (all at 32 positions)
`door minimize --moves all --plateau` (also `--plateau-pairs --walk 60`), `door regrow --size 2|3`,
`regrow --any`, `door resynth --max-size 5|6` (found v6 -> v7), `door resynth --max-size 6
--palette block,dust,dot,rep,obs,torch,cmp,lamp,bulb,target` (lamp-sweep's wide palette, after
it took the flat board to 43 with a lamp in track flat's pist19r design), `door route2 --max-rip
6|7 --pairs`, and explore/lamp-sweep's resynth with `note` added (size 5): nothing below 44.

## Why 44 is tight for this layout
- a3 = R_B's input + 18 gt, and R_B's input is at least a1 + 2 >= t4 (a1 needs its own lever
  watcher, R_B's input sits across the door): a3 >= t22. T's puller must fire at or after a3, and
  in v7 the only path to it is the chain lever -> ... -> left stack -> TL watcher -> top: 11
  observers = 22 gt, exactly a3, and each hop also does geometry (a1, pm, the walk to the left
  stack, the walk to (1,5)). A repeater (8 gt for 2 blocks) never fits where the chain also has
  to turn.
- The other 14 blocks each do one job: lever, a1 stone, R_B + floor + 3-block input cross, O, X,
  stack stone + dust, top stone, TR watcher + stone.
- Going the other way round (chain through the cross and up the right side, right 3-stack
  mid-chain, left pair from TL's watcher) costs one more: then a1's and pm's observers are off
  the chain.

## What did not work
- v1: three top pullers fed by (1,5) with the chain arriving along y=5: the observer (-1,5)
  facing west quasi-powers D0 through (0,5) 2 gt before the stone does: a 4 gt pulse, so on close
  TL was pushed home and pulled back out.
- v4: the top triggered by watching ML's park cell (-1,1): a pull-type piston's gap cell sees the
  head go in and then the block: two pulses, the top went up and back down.
- The "trick" (cdM.txt / cdM.ideal): a2/a3 delivered only through M parked next to A' (M strongly
  powered by O), so a2/a3 are no-ops on close and T's puller can fire ANY time (verified with ideal
  inputs: top 0..40 all work). It needs the M-mover to be a pusher on the far side (a puller next
  to M's park gets powered with M), and then a1/pm sit on the pusher side while R_B's input sits on
  the park side, needing a slow cross. Since the chain to the top is geometry-bound anyway, it
  never paid off.
- route2 `--need` cannot judge a half-wired door (every candidate fails the judge), so it is no
  help to wire a mechanism from scratch; wire by hand, then search.

## Lessons
- Watch a door block's REST cell (the block leaves it, or comes back while the watcher is still
  on): one pulse per edge. Watching a pull-type piston's gap cell double-fires.
- A lever can hang under (face=ceiling) the observer that watches it: no support block.
- A dust on a stone that an observer strongly powers lights (back-feed): stone + dust feed a
  whole 3-stack (stone: middle directly, bottom by QC; dust: top).
- One strongly powered block above two pullers facing down fires one directly and the other by QC
  through its quasi cell; an observer next to a puller's quasi cell quasi-powers it (watch which way
  every observer emits).
- The bottom cluster always splits: X (next to A' high) and P_M sit on opposite sides of (1,-1),
  and a1 must come from P_M's side (O sits under X). R_B's input then needs a 3-block cross under B'
  (observer under B' emitting sideways, R_B's floor as an observer, stone + dust on the far side).
  Putting the lever on P_M's side is cheaper than crossing pm (pm's window is a1+4..7).
- In the plane, repeaters cannot be locked (their sides are N/S), and a repeater cannot sit on a
  piston that extends.
