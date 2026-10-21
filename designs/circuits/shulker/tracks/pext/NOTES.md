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
  graded screen (boxes placed, filled, broken, delivered). No find: the cuts tried need several
  blocks moved at once.
- `mctas circuit shulker anneal --face y,-3`: the energy counts the blocks left on one face. No face
  emptied this way; `--soft-volume` (above) is what worked.
- `mctas circuit shulker watch <design> --cells ...`: the state changes of chosen cells in a redsim
  run, for captions.

## Results (every one game-verified with `mctas circuit verify`, all badges, robust)
- Loader 39 -> 38 (lns r=2/add=1) -> 34 -> 33 blocks (anneal), 1x7x10 = 70, 2.49 items/s, the 9
  badges.
- Loader 1x7x8 = 56, 28 blocks (anneal `--soft-volume --slack 1`: the volume is part of the energy,
  1000 per cell, so the box may grow for a while; 33 -> 30 blocks at 1x7x9 = 63 -> 1x7x8 = 56 at 29,
  then 28): archived `shulker-loader-1wide-28b` (the 33 removed: dominated). The search replaced the
  copper bulb and its comparator (2 columns) with a sticky-piston shuttle: an observer pulses a sticky
  piston on every change of the full test, a short pulse leaves the pushed observer one cell up, the
  next change pulls it back; up there it fires the piston relays to the breaking piston, down there
  into air. Traced with `mctas circuit shulker watch`.
- Loader 1x6x9 = 54, 29 blocks (archived `shulker-loader-1wide-1x6x9`; the 28-block 1x7x8 stays
  as the fewest-blocks one). Hand rebuild (`loader-north-6tall-seed.txt`, 32 blocks, passed on
  the first try): the bottom line that fired the dispenser after each break is gone; instead an
  observer watching the observer above the breaking piston starts a chain on the boxes side (one
  more column), into a piston whose head an observer watches, under the box dispenser. Then the
  soft-volume anneal: 32 -> 29 blocks at 54. Why not 1x6x8: on the boxes side the column next to
  the box spot is closed at the box's height by the boxes chest and the comparator reading it, so
  any chain from the top line down to the dispenser needs the extra column.
- Unloader 24 -> 23 (lns) -> 22 blocks (anneal), 1x7x6 = 42, 2.50 items/s, the 7 badges: archived
  `shulker-unloader-1wide-22b`. Soft-volume anneals (slack 1 and 2) find nothing smaller yet.

## Why the unloader stays at 1x7x6 (2026-10-21)
- Soft-volume anneals (6 runs, slack 1-2, up to 12 000 steps) find nothing smaller.
- The loader's shuttle does not carry over: the unloader must break the empty box within the
  emptying hopper's 8-gt cooldown after its last item (else the hopper under the box grabs the
  drop), and a shuttle (observer, sticky piston, landing observer) takes about 8 gt. Hand seeds
  in /tmp (5 tall) failed on exactly that, and on the breaking piston's quasi-connectivity cell
  sitting next to the dispenser (every dispenser fire would also fire the breaker).

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
