# buildable: can a survival player build the records?

Track goal (from the coordinator, for vjeux): an integrity check, not a door design. The judge
pastes every block raw (no updates), sends shape updates to dust and a self update to every block,
then settles. That rest state could be one a player never reaches by placing blocks one at a time
(a quasi-powered piston left retracted, a silent hold, observers that fire when their neighbour is
placed, a piston that fires mid-build and pushes half the door away). `door buildable` searches for
a build program a player can follow that ends exactly in the judge's rest state, then runs the
judge's cycles from the built state.

## The tool: `mctas door buildable <design>... [--budget 300] [--workers 16] [--positions 32]`

Designs: files, `@builtin`, or `<git ref>:<path>`. Writes one build script per design
(`--out-dir`) and a markdown row per design (`--table`).

Bridge op `build` (Protocol.java / Sandbox.java; additive, the judge's ops are untouched): a list
of player actions run one by one, the world ticking at least 4 ticks after each (a player clicks
at most every 4 ticks) and then until idle (a slow player):
- `place`: a real `BlockItem.place` (getStateForPlacement, `setBlock` flags 11, `setPlacedBy`, the
  BLOCK_PLACE game event that sculk sensors hear). The click is searched: every face of every
  neighbouring block, 7 pitches x 12 yaws (cardinal and 20 degrees off), and kept only when it
  gives the wanted orientation (facing, face, half...) AND it is geometrically possible: the view
  ray must travel into the clicked face, and it reaches the face through the target cell, so the
  cell it enters from must be open. Clicking a lever, repeater, dust... means sneaking (noted).
  Click-set properties are then set by right-clicks (repeater delay, comparator mode, lever).
  Power, connections, piston extension, torch/lamp/observer state are never chosen: the world
  computes them.
- a block with no neighbour to click (the sandbox is a void) is placed against a temporary dirt
  block put next to it and broken right after; the dirt's placement and removal update the world
  like any block (they are simulated, not assumed harmless).
- `use` (right-click), `break` (like a player: playerWillDestroy, removeBlock, destroy; no drops).

Search (time budget per design): heuristic orders (mechanism / pistons / wiring / observers /
power-last ranks, bottom-up and connected-growth variants), each with the door lever off or ON
while building; then a greedy construction that picks, at every step, a block whose placement does
no damage (no block where the rest state has air, no placed block gone, no drop, no failed
placement), several randomized restarts; then simulated annealing over whole programs, one chain
per worker (moves, swaps, segment moves, lever on/off, temporary helper blocks: stone, glass,
obsidian or a redstone block placed before item k and broken after item m). A found program is
simplified (helpers dropped when not needed, lever off preferred; the notes say why what remains
is needed) and re-checked from scratch at the judge's positions (default 32): the build must end in
the rest state there (every block state of the judge's watch box, no entities, idle), then 3
open/close cycles with the judge's rules from the built state.

## Results

See [BUILDABILITY.md](../../BUILDABILITY.md) (the table) and `scripts/<design>.build.txt` (the
build scripts, one line per action, design coordinates).

## Self-test

`selftest-observer-pair.txt`: two observers facing each other. The judge's rest has them quiet
(raw paste; observers ignore self updates), but a player cannot place them: placing the second is
a shape update the first one sees, then each pulse is a change the other sees (a clock). The tool
must say NOT FOUND (blocking: observer).

## Not modelled

- No ground: the judge's world is a void. Temporary dirt blocks stand in for the pillars a player
  would use; a real build on the ground may need the floor cells checked separately.
- The player's body and reach beyond the one cell the view ray enters through; item pickups.
- Footsteps: walking near a sculk sensor triggers it; sneaking (or standing still) after placing
  the sensor last avoids that, as in the st43 script.
- A faster player acting while pistons are still moving (more options, not needed so far).
