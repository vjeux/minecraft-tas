# Track shapes: door shapes relative to the opening, shape boards

vjeux: "the wall may be good but look for other shapes that could be good". Measure every
working design's shape relative to its door, pick the shape rules that make good boards for
builders, and search for the best design of each.

## Tools
* `crates/piston-door/src/lib.rs`: `Shape` / `shape(d)` (layers in front of / behind the door
  plane, rows below / above the opening, columns left / right of it, blocks below / above,
  bounding box), `swept_shape(w, d)` (the same over every cell the door occupies at rest AND at
  any tick of an open/close cycle: the space a builder must keep free), `ShapeLimits` (`key<=n`
  lists or presets: wall, wall2, flat, ground, no-above, narrow1, narrow2, wall-ground), helpers
  `in_wall`, `in_wall2`, `on_ground`, `nothing_above`, `narrow`, and `ground_check(w, d, opts)`
  (the judge's checks on the door standing on a stone floor one below its bottom row: slime must
  not drag the ground, nothing may push or power through it).
* `mctas door shapes [--branches] [--rule ...]`: the table of every working design (verdicts from
  the leaderboard cache; swept shapes + ground checks cached in `archive/shapes-v1.tsv`) and, per
  shape rule, the best rotation-safe design today per league, plus the (volume, blocks) frontier.
* Search filter: `--shape <preset|key<=n,...>` on every SearchArgs command (minimize, harvest,
  regrow, grow, mech-synth --then-search...): candidates must keep the static shape, and the
  judge also checks the swept shape.
* Leaderboard boards (leaderboard.rs render, showcase.rs BLOCK_BOARDS): "Solid, in a 3-thick wall",
  "Solid, on the ground (nothing below the door)" (swept below = 0 and the ground check), and
  "Solid, smallest box" (swept bounding-box volume, ties by blocks). All rotation-safe.

## Shape table summary (2026-10-18 17:20Z, archive, swept shapes, rotation-safe)
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

Boards chosen: WALL (a real constraint: the record 38 is 4 thick), GROUND (no design yet: the
bottom-middle block has to leave upwards or two cells sideways), SMALLEST BOX (compactness). Not
chosen: narrow (5 wide needs the side columns to move vertically by 3: no design, likely huge;
7 wide is the main board), wall2 (same leader as wall), no-above (mirror of ground).
