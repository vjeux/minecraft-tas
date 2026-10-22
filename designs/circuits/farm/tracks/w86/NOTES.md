# Crop farms (study 18): track w86

Coordinator 2026-10-21 18:21Z: a new real-use kind, the AUTO CROP HARVESTER for sugar cane (then bamboo /
kelp if the same judge fits): observer-triggered pistons break the grown block, items are collected
into a chest, tileable per cane. Judge: real random-tick growth in the game, every grown block
harvested, no item lost over a long run (20 min of game time, sped up), tiles side by side, item
intrusion safe, robust. Metrics: blocks per cane, items per hour per cane.

## The judge (crate::farm, docs/circuits.md "harvester CROP")
`circuit: harvester sugar_cane`, `port crop:` (the judge plants the cane, age 0), `port output:` (the
judge's chest), optional `tile: dx dy dz`. Game only (redsim has no random ticks): random_tick_speed
300 for 24000 gt, every crop column recorded every tick (growths counted by age resets, so a block
destroyed in the tick it grew still counts), foreign items dropped on every column halfway, then 800
gt without growth and a rest. `mctas circuit farm-run <design> [--tiled] [--ticks N]` is one run.
Escaped entities (leaving the build's box) and items loose for a whole 500-gt batch are noted.

## Designs (all `circuit verify`: 4 positions x 4 rotations + the copies at 4 rotations)

| design | blocks per cane | box | tile | items/h per cane | how |
|---|---|---|---|---|---|
| [cane4-mc-21b](cane4-mc-21b.txt) | 5.25 | 7x3x3 | 0 0 3 | 2.80 | four canes on one observer/stone/dust: the dust's four arms fire four pistons (east, west, and two east-facing ones on the north and south arms whose canes stand beside the first cane); one grate under the observer touches three soils; a cart on each hopper; robust (3 verifies), archived |
| [cane4-mc-23b](cane4-mc-23b.txt) | 5.75 | 7x3x7 | 7 0 0 | 2.80 | the four canes spread out, one grate each: robust (3 verifies) |
| [cane2-mc-13b](cane2-mc-13b.txt) | 6.5 | 7x3x1 | 7 0 0 | 2.83 | two canes, one observer/stone/dust: the dust's east and west arms fire both pistons (the west cane is harvested at the east cane's next growth step, wait up to ~90 gt at the judge's speed); a cart on each hopper; robust (3 verifies), archived |
| [cane-mc-8b](cane-mc-8b.txt) | 8 | 4x3x1 | 5 0 0 | 2.90 | cane-7b with a hopper minecart standing on the hopper (no rail): the head shoves the item into the cart, which takes items every tick (no 8-gt wait) and the hopper below empties it; robust under the repo's ITEM INTRUSION (3 verifies), archived |
| [cane-mc-9b](cane-mc-9b.txt) | 9 | 4x3x1 | 5 0 0 | 2.90 | the same with the cart on a rail: robust (3 verifies) |
| [cane-8b](cane-8b.txt) | 8 | 4x4x1 | 5 0 0 | 2.90 | cane-7b + a stone capping the cane: robust against items dropped onto the cane, fails the standard intrusion (64 ingots on the hopper) |
| [cane-7b](cane-7b.txt) | 7 | 4x3x1 | 5 0 0 | 2.89 | observer on the bottom cane block -> stone -> dust -> piston at height 2; the head shoves the item onto a hopper; waterlogged waxed copper grate. Loses one item in about one verify in two: the intruders (a judge drop onto the column) get swept onto the hopper, and a cane item arriving during its 8-gt cooldown slides off the rim |

## Log
- Mechanics found: the observer must not be in front of the piston (a piston is not powered through
  its front: the pushed-observer idea, 5 blocks, never fires). A sticky piston given the observer's
  2-gt pulse drops its block (the push-a-block-into-the-cane variant leaves the block in the cane).
  A waterlogged slab leaks water (flowing water around the farm); a waterlogged waxed copper grate
  keeps it in. The hopper must sit at the push height (one block lower loses a third of the items).
- Tiling: copies 4 apart put the next copy's powered stone against this copy's hopper (locked for
  each 2-gt pulse); 1 or 2 items per verify then escaped (seen leaving the box below the hopper). 5
  apart: robust. A dust line (copies 1 apart along the cane row) points along itself, not into the
  pistons.
- The observer watches the BOTTOM block: it fires at every age step (16 per growth), so every piston
  fires often; the harvest comes 2 gt after the growth. A side effect at the judge's speed: the head
  sits in the cane's second block ~3 gt per fire, and the bottom block does not age while it is there
  (about 12% slower at random_tick_speed 300, about 0.2% at vanilla speed).
- Losses, found with `farm-run --turns T --offset X,Y,Z` (a verify build replays exactly: the runs
  are deterministic per build) and the escape watch: every lost item left right after the judge's
  intruder drop (tick 12042). The pistons sweep falling intruders onto the hopper; its 8-gt cooldown
  after each pull leaves a cane item that arrives then on the hopper's rim, and it can slide off.
  A stone above the hopper made it far worse (items bounced back into the cane's column: 27 lost per
  run); a stone capping the cane (cane-8b) keeps the intruders out: robust.
- 2 canes per observer: the dust's other arms power more pistons, and the second cane is harvested at
  the first cane's next age step (cane2-10b: 5 blocks per cane with one water grate for both soils,
  but an item can land on the other piston's top; cane2-11b: 5.5, separate waters). Both lose an item
  in some tiled runs (intruder races, ledges); not archived yet.

## Bamboo (2026-10-21)

| design | blocks | result |
|---|---|---|
| [bamboo-hc-12b](bamboo/bamboo-hc-12b.txt) | 12 | robust x3, archived: a cart on the mud inside the stalk + a cart half over the third cell (takes the third block's item as it appears) |
| [bamboo-cc-12b](bamboo/bamboo-cc-12b.txt) | 12 | 1 of 16: the third item lands on a regrown second block and the run ends first |
| [bamboo-lp6-15b](bamboo/bamboo-lp6-15b.txt) | 15 | repeater pulse extender: the head does not stay long enough |
| [bamboo-shaft-13b](bamboo/bamboo-shaft-13b.txt) | 13 | open north side over a cart: 1 of 16 (an item on the stump at the end) |
| bamboo-shaft-14b/20b/22b, bamboo-8b, bamboo-down-9b, bamboo-mud-10b | | items pushed up through the cap (walled cell), a clock, items flung |

Found with `farm-run --trace` (see WR.md mechanics).

## Kelp (2026-10-21)

| design | blocks | result |
|---|---|---|
| [kelp-17b](kelp/kelp-17b.txt) | 17 | robust with tiling (tile 4 0 0), archived: kelp on a hopper, a piston pushing down into the second block, a refill source with a hopper under it |
| [kelp-down-17b](kelp/kelp-down-17b.txt) | 17 | 1 of 16: an item squeezed by the head into the refill source floats there |
| [kelp-v3t](kelp/kelp-v3t.txt) | 19 | robust x3 with tiling: observer on the bottom kelp, piston into the second block, a falling-water source over the collection cell |
| [kelp-v2](kelp/kelp-v2.txt) | 18 | 1 of 16: the refill source beside the kelp's cell catches an item squeezed by the piston head |
| [kelp-v1](kelp/kelp-v1.txt) | 17 | 13 of 16: the refill current from the collection cell carries items back into the kelp |

## Pumpkin and melon (2026-10-21)

| design | blocks | result |
|---|---|---|
| [pumpkin-mc-7b](stem/pumpkin-mc-7b.txt) | 7 | robust x3 with tiling (tile 0 0 2), archived: observer above the stem + dust on it, a piston above the fruit cell (powered through the cell above it), mud under the fruit, a hopper minecart under the mud on a hopper |
| [melon-mc-7b](stem/melon-mc-7b.txt) | 7 | the same design with a melon stem: robust x3 with tiling, archived (every drop stack counted) |
| [pumpkin-6b](stem/pumpkin-6b.txt), [melon-6b](stem/melon-6b.txt) | 6 | a hopper directly under the mud: the squeezed drop shoots sideways out of the cell first (12 of 16 builds); the melon one is the calibration [melon-lost](calibration/melon-lost.txt) |

## Cactus (2026-10-21)

| design | blocks | result |
|---|---|---|
| [cactus-wool-9b](cactus/cactus-wool-9b.txt) | 9 | robust x3 with tiling (tile 9 0 0), archived: piston beside the second cell + sculk sensor beside the piston, wool cap, stone walls on the two other sides, a hopper minecart on a hopper beyond |
| [cactus-wool-8b](cactus/cactus-wool-8b.txt) | 8 | without the cart: a pushed item falls along the cactus and dies (14 of 16) |
| [cactus-wool-9b-t3](cactus/cactus-wool-9b-t3.txt) | 9 | copies 3 apart: the sensors hear each other (11 lost) |
| [cactus-walls-9b](cactus/cactus-walls-9b.txt) | 9 | stone cap and sensor on the piston: an intruder landing nearby kept the sensor busy as a block grew (3 of 16) |
| [cactus-cap-7b](cactus/cactus-cap-7b.txt), [cactus-sculk-6b](cactus/cactus-sculk-6b.txt) | 7, 6 | the item drifts out of the sides or rises above the head before the sweep |
