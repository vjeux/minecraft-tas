# Track pext on study 8 (shulker loaders and unloaders): smaller 1-wide designs

Job (coordinator 2026-10-21, vjeux: "1 works but it's massive, can you make it smaller?", standing
rule: minimum volume, the design co-designed as a whole): shrink the 1-wide loader and unloader.

## Tools (crates/circuit-shulker)
- `mctas circuit shulker rs <designs>`: the judge's scenarios run in redsim (`rs.rs`, a port of the
  judge's run: placement, raw chests, settle, supply, levers, 20-gt visits with every output chest
  read each gt, rest, inventory), decided by the judge's own `shulker::judge`. On the 1-wide loader
  and unloader it reproduces the game's metrics to the gt (base 13875 / 13872 gt, 2.49 items/s;
  unloader 13859 / 13838 gt, 2.50) and runs every scenario in about 0.2 s. Not modeled: colours (no
  coloured boxes), mixed (no dirt item); rotations and the drop stress stay with the game.
- `mctas circuit shulker lns`: remove every r-subset (plus every block outside `--box`), grow back at
  most `--add` blocks near the removed cells; a candidate must pass every scenario the original
  passes on redsim at its rate (less `--tol`). About 28 000 candidates/s on 40 threads.
- `mctas circuit shulker anneal`: random edits (delete, re-state, add, move by 1-2, swap, shift a run,
  move a port chest), batches judged in parallel, simulated annealing on (volume, energy), energy
  = 100 x sum over the box faces of sqrt(blocks on the face) + 10 per block.
- `mctas circuit shulker repair`: the design cut to a smaller box, then edits near the cut guided by a
  graded screen. No find yet: the graded landscape is flat (nothing counts until a box goes out).

## Results (every one game-verified with `mctas circuit verify`, all badges, robust)
- Loader 39 -> 38 (lns r=2/add=1) -> 34 -> 33 blocks (anneal), 1x7x10 = 70, 2.49 items/s, the 9
  badges: archived `shulker-loader-1wide-33b`.
- Unloader 24 -> 23 (lns) -> 22 blocks (anneal), 1x7x6 = 42, 2.50 items/s, the 7 badges: archived
  `shulker-unloader-1wide-22b`.
- Volume: not yet smaller. Every face of the loader holds 3 or more blocks; the unloader's bottom
  face holds one (the support of the bulb reader), but cutting it needs the bulb and its reader
  moved up together.

## Why the loader is tall (my analysis, 2026-10-21)
- The piston trigger must cross the box column (dispenser, box, feed hopper, items chest) from the
  read side to the piston side: over the items chest (y=3) or under the dispenser. Moving the items
  chest off the column does not help: its feed hopper chain and the trigger line cross each other
  in the slice, and the piston's three free neighbours are then all containers, which never give it
  the block update its quasi-connectivity needs.
- The bottom route (trigger along y=-3, up the piston side) would make the design 5 tall instead of
  7 if the dispenser lines fit on y=-2. That needs the output chest moved into the read-through
  container's place (the hopper under the read pushes south into it), which frees y=-2 at the box's
  front. Not built yet.
