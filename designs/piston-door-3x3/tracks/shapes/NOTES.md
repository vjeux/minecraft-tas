# Track shapes: door shapes relative to the opening, shape boards

vjeux: "the wall may be good but look for other shapes that could be good". Measure every
working design's shape relative to its door, pick the shape rules that make good boards for
builders, and search for the best design of each.

## Tools
* `crates/piston-door/src/lib.rs`: `Shape` / `shape(d)` (layers in front of / behind the door
  plane, rows below / above the opening, columns left / right of it, blocks below / above,
  bounding box), `ShapeLimits` (`key<=n` lists or presets: wall, wall2, flat, ground, no-above,
  narrow1, narrow2, wall-ground), helpers `in_wall`, `in_wall2`, `on_ground`, `nothing_above`,
  `narrow`.
* `mctas door shapes [--branches] [--rule ...]`: the table of every working design (verdicts from
  the leaderboard cache) and, per shape rule, the best rotation-safe design today per league,
  plus the (volume, blocks) Pareto frontier.

## Shape table summary (2026-10-18 17:00Z, archive + every explore branch, rotation-safe)
Every working design today has at least 2 rows below the door and 3 above it.

| rule | meaning | solid best today | slime-free best | others |
|---|---|---|---|---|
| wall (front<=1, behind<=1) | inside a 3-thick wall centered on the door | 40 fc2 (2-thick, 7x9x2) | 43 lamp-sweep-flat43 | gravity 33 |
| wall2 (thick<=2) | 2-thick wall | 40 fc2 | 43 lamp-sweep-flat43 | gravity 33 |
| flat (thick<=1, slime allowed) | one block thick | 43 lamp-sweep-flat43 | 43 | - |
| ground (below=0) | nothing below the door: no digging | none | none | none |
| no-above (above=0) | nothing above the top row | none | none | gravity 42 (synth-g42) |
| narrow1 (side<=1) | at most 1 column beside the opening each side (5 wide) | 43 riders-r3 / st43-bmobs-43 | none | sculk 42, gravity 33 |
| narrow2 (side<=2) | 7 wide | 38 sf-holdA-b38 | 38 | |
| smallest box | (volume, blocks) frontier | 80 (8x10x1) flat-b-v7 44; 81 flat43 43; 126 fc2 40; 252 sf-holdA 38 | | |
