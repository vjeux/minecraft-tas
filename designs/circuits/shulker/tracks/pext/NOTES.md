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
  run, for captions. `--trickle EVERY[,PHASE]` paces the one-box scenarios (the next box at each gt
  t with (t + PHASE) % EVERY == 0); with `--sweep` it runs every phase and counts the failures.
- `mctas circuit shulker trace <design> --stress-run K --out-dir D`: the game judge's drop stress
  run K (its drop seed, a burst every other run, rotation K % 4) traced every gt, written even when
  it fails; `mctas circuit shulker trace-entities <trace> [--id N]`: every entity's life and path
  (with the cells that changed around it): where a lost box went.
- `mctas circuit shulker anneal --sweep 23,29,37`: every candidate must also pass the required
  one-box scenarios (`one-box`, `one-box-burst`) at every phase of a box every 23, 29 and 37 gt
  (`lns::add_sweeps`; a sweep the seed fails is dropped with a warning). This keeps the timing
  holes of the shuttle and both-edges switches out of the search (2026-10-21).

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
- Loader 1x6x9 = 54, 30 blocks (archived `shulker-loader-1wide-1x6x9`). Hand rebuild
  (`loader-north-6tall-seed.txt`, 32 blocks): the bottom line that fired the dispenser after each
  break is gone; instead an observer watching the observer above the breaking piston starts a
  chain on the boxes side (one more column) into a redstone dust that the boxes-chest comparator
  also powers; an observer on that dust fires the box dispenser. Then the soft-volume anneal:
  32 -> 30 blocks at 54. Why not 1x6x8: on the boxes side the column next to the box spot is
  closed at the box's height by the boxes chest and the comparator reading it, so any chain from
  the top line down to the dispenser needs the extra column.
- ENDLESS SUPPLY (2026-10-21, the site viewer's run, now a judge check too): the first 1x6x9 (30
  blocks) delivered nothing when the box chest is refilled before every gt: it fired the box
  dispenser on the chest comparator's level changes (dust + observer), its first fire came before
  a box reached the dispenser, and a chest kept full never changes level; and the after-break pulse
  went into the same dust, masked at 15. Replaced by `shulker-loader-1wide-31b` (1x6x9 = 54, 31
  blocks): both pulses (the chest comparator's observer and the after-break observer) go into one
  dropper, and the dispenser is fired by an observer next to a sticky-piston shuttle; on the very
  first pulse both observers are pushed and the second fire comes 4 gt later, when the first box is
  in the dispenser. Hand seed `loader-north-6tall-late-seed.txt` (33 blocks), anneal to 31.
- FULL CHEST (2026-10-21, webrender's find, now a judge check, `shulker::FULL_CHEST`): the 28-block
  1x7x8 loader and the first 1x6x9 (29 blocks) jammed with ~20+ boxes in the chest: their
  box-arrival line fired the box dispenser on the boxes-chest comparator's on and off (an observer
  sees the comparator's powered state only); with a full chest the first fire comes before a box
  reached the dispenser and the chest never empties, so the comparator never turns off. Both left
  the archive (`rejudged/`). The fix: the comparator powers a dust and an observer watches the
  dust, whose power level changes with every box that leaves the chest.
- Unloader 24 -> 23 (lns) -> 22 blocks (anneal), 1x7x6 = 42, 2.50 items/s, the 7 badges: archived
  `shulker-unloader-1wide-22b`. Soft-volume anneals (slack 1 and 2) find nothing smaller yet.
- ONE BOX AT A TIME (2026-10-21, judge check): the 31b saw boxes arrive through the box-chest
  comparator; boxes trickled in one at a time leave the chest before the comparator updates, so it
  placed none (removed). Read the first hopper of the box line instead (a box waits there 8 gt).
- ONE BOX BURST (2026-10-21, judge check, three visit paces 20/17/23 gt): nearly full boxes one at
  a time. Every sticky-piston shuttle loader (the 28b lineage) misses a box that fills within ~6 gt
  of the previous break; an annealed 31b whose breaker fires on both edges of the full test (no
  switch at all) breaks the next box empty when it lands early (jams at 23 gt, 9 of 37 phases of a
  37 gt trickle). The copper bulb toggles within a gt: every bulb loader passes every phase.
  Before archiving a loader, sweep it: `circuit shulker watch <d> --scenario one-box-burst
  --trickle 37 --sweep` (and 23, 29); a 36-block 1x6x11 anneal find failed 1 of 37 and stayed out.
- Loader 1x6x11 = 66, 41 blocks (archived `shulker-loader-1wide-1x6x11`, the robust 1-wide pick):
  the 39b's full test and copper bulb kept; its bottom row replaced by the boxes-side chain (an
  observer above the breaker and the first-hopper comparator both pulse a dropper, a piston, then
  observers and lamps down to the box dispenser). Clean on every sweep. Then
  `anneal --sweep 23,29,37` took it to 36 blocks at the same 1x6x11 (archived
  `shulker-loader-1wide-1x6x11-36b`, the 41b removed: dominated): the top observer line became
  sticky-piston relays, and the box dispenser is fired through a dropper under it.
- DROPS IN A ONE-WIDE DESIGN (2026-10-21): the same anneal from the 39b found a 1x7x9 = 63 loader
  (34 blocks) clean on every redsim check and sweep that lost 2 of the game judge's 64 drop stress
  runs. `circuit shulker trace --stress-run 5` + `trace-entities --id 2`: the third box's drop
  drifted sideways (0.09 block/gt) out of the design's plane and fell, because the output hopper
  was still on its cooldown from the previous box (a fast burst cycle) and then disabled for 2 gt:
  an observer under the box dispenser strongly powered it, and a strongly powered dispenser
  disables the hoppers next to it. redsim drops have no random drift, so it never saw this; its
  screen now fails a one-wide design whose dropped box waits more than 2 gt for its hopper
  (`rs::BOX_DROP_LIFE`; every archived design passes, the 63 does not). Fire the box dispenser
  through a block beside it (weak power), not by an observer or comparator pointing into it.

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
