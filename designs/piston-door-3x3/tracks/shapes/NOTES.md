# Track shapes: door shapes relative to the opening, shape boards

vjeux: "the wall may be good but look for other shapes that could be good". Measure every
working design's shape relative to its door, pick the shape rules that make good boards for
builders, and search for the best design of each.

## Tools (all merged to main by the coordinator at 8e1c46d, newer ones on explore/shapes)
* `crates/piston-door/src/lib.rs`: `Shape` / `shape(d)` (layers in front of / behind the door
  plane, rows below / above the opening, columns left / right of it, blocks below / above,
  bounding box; along the door's own axes, so `door-axis: z` designs work), `swept_shape(w, d)`
  (the same over every cell the door occupies at rest AND at any tick of an open/close cycle: the
  space a builder must keep free), `ShapeLimits` (`key<=n` lists or presets: wall, wall2, flat,
  ground, no-above, narrow1, narrow2, wall-ground), helpers `in_wall`, `in_wall2`, `on_ground`,
  `nothing_above`, `narrow`, and `ground_check(w, d, opts)` (the judge's checks on the door standing
  on a stone floor one below its bottom row: slime must not drag the ground, nothing may push or
  power through it).
* `mctas door shapes [--branches] [--rule ...]`: the table of every working design (verdicts from
  the leaderboard cache; swept shapes + ground checks cached in `archive/shapes-v2.tsv`) and, per
  shape rule, the best rotation-safe design today per league, plus the (volume, blocks) frontier.
* Search filter: `--shape <preset|key<=n,...>` on every SearchArgs command (minimize, harvest,
  regrow, grow, mech-synth --then-search...): candidates must keep the static shape, the judge also
  checks the swept shape, and the shape replaces the "a flat base stays flat" league rule.
* Leaderboard boards (leaderboard.rs render, showcase.rs BLOCK_BOARDS + boards()): "Solid, in a
  3-thick wall", "Solid, on the ground (nothing below the door)" (swept below = 0 and the ground
  check), and "Solid, smallest box" (swept bounding-box volume, ties by blocks). All rotation-safe.
* `mctas door mech-synth <base> --sym-bfs "<open events>"|auto`: symmetric control by product
  BFS: the shortest single event sequence S with S(rest) = open and S(open) = rest, searched over
  PAIRS of macro states under the same events (an event that changes nothing in one run leaves
  it). Exact where mech-synth's DFS/split searches give up; prints where each pulsed piston stands
  in both runs. `door ideal`: `let` may be sums of earlier variables and `--set` re-derives them.

## Shape table summary (archive + explore branches, swept shapes, rotation-safe)
Every working design today has at least 2 rows below the door and 3 above it.

| rule | meaning | solid best today | slime-free best | others |
|---|---|---|---|---|
| wall (front<=1, behind<=1) | inside a 3-thick wall centered on the door | 40 fc2 (2-thick, 8x9x2 swept: the right structure slides one further right) | 43 lamp-sweep-flat43 | gravity 33 |
| wall2 (thick<=2) | 2-thick wall | 40 fc2 | 43 flat43 | gravity 33 |
| flat (thick<=1, slime allowed) | one block thick | 43 flat43 | 43 | - |
| ground (below=0) | nothing below the door: no digging | none | none | none |
| no-above (above=0) | nothing above the top row | none | none | gravity 42 (synth-g42) |
| narrow1 (side<=1) | 5 wide | none (riders-r3 / st43 are 5 wide at rest but their side structures slide out to 7) | none | none |
| narrow2 (side<=2) | 7 wide | 38 sf-holdA-b38 (= the main record) | 38 | |
| smallest box | (volume, blocks) frontier | 80 (8x10x1) flat-b-v7 44; 81 flat43 43; 144 fc2 40; 252 sf-holdA 38 | | |

Boards chosen: WALL (a real constraint: the record 38 is 4 thick), GROUND (no design yet),
SMALLEST BOX (compactness). Not chosen: narrow (5 wide forces the side columns to move vertically
by 3: no design, likely huge; 7 wide is the main board), wall2 (same leader as wall), no-above
(mirror of ground: the top-middle block must leave downwards or two cells sideways).

## Ground: a working mechanism with nothing below (ideal inputs), not wired
Why it is hard: with nothing below, the bottom-middle block M has no neighbor a piston or a slime
can reach at rest (below is forbidden, z +-1 are the walkway columns, the rest are door cells),
so it has to leave UP the middle column (three cells, after T and C) or two cells sideways
(after a side column moved two cells). Slime at y = door.y is out too: standing on real ground it
drags the floor block under it (the ground check catches that).

`bases/ground20.txt` (20 mechanism blocks): side columns on 6 pull-type sticky pistons (x -2 and
x 4, y 0..2, gaps at x -1 / 3); the middle column on an elevator: A (1,4) on B (1,5) on D (1,6),
all sticky facing down; P_T (-1,3) east parks T at (0,3), P_C (3,3) west parks C at (2,3), M
ends at (1,3). A pulls T up from height 4, C from heights 3 then 4, M from heights 2, 3, 4 (D + B
form a double extender to reach height 2).
* `ground20.ideal`: open 15 + close 15 pulses, every one a 2 gt pulse (pull-type with a gap in
  front, push-and-spit otherwise). WORKS (4 positions x 2 cycles).
* `ground-sym.ideal`: ONE pulse train for both lever edges (found by `--sym-bfs` on
  `bases/ground-mid.txt`, the middle column alone; the sides fire at tick 0 on both edges): 24
  pulses `A4 T D A3 T C C B4 D B5 C B5 T A2/A3 D T B4 A3 D B5 T C C A4`. WORKS with 6 or 5 ticks
  per step (`--set s=5`: open/close 123 ticks), not 4. Shortest symmetric sequence for that open
  state (BFS); parking in z instead (`bases/ground-mid-z.txt`) is also 24. With level toggles the
  pair space explodes (8M pairs, none found).
* Why it is not wired: 24 timed pulses into 7 cells around a vertical piston stack. Each pulse is
  a 2 gt observer pulse (spits need short pulses: a repeater would stretch them), so roughly one
  observer per pulse plus a ~120 tick delay line and routing: ~70-80 control blocks, ~95 in all.
  Stacked pistons also quasi-power the piston below whenever the one above is pulsed by an
  observer or dust (its neighbors-of-front update reaches it); harmless when the upper piston
  pushes the lower one (its block event runs first and moves it), but the two no-op steps (8 and
  10 in the run from open) would fire A underneath.

## Wall: what was tried
* `door minimize --shape wall --moves all --plateau` from lamp-sweep-flat43 (flat, 43): 356k swaps,
  14 neutral designs, nothing smaller (`wall/flat43-wall.txt` = unchanged).
* The 38/39 slime-free record family (sf-holdA b38/b39) is 4 thick: its X hub (1,4,-1) is fed from
  z -2 (O_a) and all its other neighbours are taken (two dusts, O', T's parking cell), so the
  lever/P_T chain cannot fold into z -1..1 without re-laying the hub. P_T can become a push-type
  piston at (1,3,1) facing north (the same 2 gt pulse pushes T to (1,3,-1) on open and pulls it
  back on close), which frees z -2 of P_T but not of the chain.
* fc2 (2-thick) and the m05 / fx1 40s (3-thick) were searched without any shape limit before, so a
  wall limit cannot find anything new there.

## Next ideas
* Ground: wire `ground-sym.ideal` (first entry of the board, ~95 blocks), or a mechanism whose
  symmetric sequence is shorter / more regular (the product BFS evaluates a base in seconds); a
  coupling search (observer watching piston X fires piston Y) on top of the macro model could
  replace most of the delay line with the mechanism's own motion.
* Wall: re-lay b38's control with the hub fed from above (one observer merging p0 and the R_B
  edges) and P_T as the push-type (1,3,1).
* Smallest box: only flat doors can beat 80 (a 2-thick door needs W x H < 40); the flat designs
  were minimized for blocks, not volume: flat43 minus one row or column (9x8 / 8x9 = 72), or
  flat-b-v7 without its x -3 column (7x10 = 70).
