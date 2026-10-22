# Moving items by minecart (loader + track + unloader): design doc

sorter, 2026-10-22, per [docs/process.md](../../docs/process.md). Every rule below comes from the
26.3 source (`mctas src show <Class>`, line numbers from that listing). Measurements come from the
rail study's judge (`mctas circuit cart-run`, `circuit verify`; the redsim twin agrees with the
game).

## Use

Move a farm's or a mine's output to storage far away. The farm's collection chest fills up at the
farm, and the items show up in a chest at the base.

## The number

1. **Minutes to move a double chest of items (3456) over the reference track**, from a loaded
   supply chest at tick 0 until the last item is in the storage chest. Items/s = 3456 / that.
2. **Blocks placed**: the two stations; the track is the same for every layout and is reported
   separately (rails, powered rails, their power).

**Reference track: 128 blocks.**
- Carts replace hopper lines only over distance. A hopper line costs 5 iron a block and moves
  2.5 items/s, so 128 blocks of hoppers is 640 iron.
- 128 blocks (8 chunks) is about the longest link that keeps both ends ticking while the player
  stands at either one. The server's default `simulation-distance` is 10 chunks
  (`DedicatedServerProperties` 105). A cart in a chunk that doesn't tick freezes, so a longer line
  works only while the player is near the middle.

## Mechanics, from the code

**Carts** (`EntityTypes` 560-562, `MinecartHopper`, `MinecartChest`, `AbstractMinecartContainer`)
- Every minecart is 0.98 x 0.7 blocks.
- A hopper minecart holds 5 slots (320 items of a 64-stack; `getContainerSize` 43-45). A chest
  minecart holds 27 slots (1728 items; `MinecartChest` 40-42).
- **A hopper minecart pulls one item per tick, 20 items/s, from the container in the block above
  it** (`tryConsumeItems` 97-102: at most one per tick via `consumedItemThisFrame`; `suckInItems`
  104-117 reuses `HopperBlockEntity.suckInItems` with no cooldown). "Above" is
  `containing(x, y + 0.5 + 1, z)` (`getLevelY` 69-71): on a flat rail that is the block right
  over the rail. A double chest there counts as one 54-slot source (`getBlockContainer` 378-391).
  It also picks up item entities, and a powered activator rail disables it (`activateMinecart`
  48-53).
- **A chest minecart cannot pull.** Only hoppers or droppers pushing into it can fill it.
- A container cart keeps `0.98 + 0.001 x (15 - its comparator level)` of its speed each tick
  (`AbstractMinecartContainer.applyNaturalSlowdown` 101-107): 0.995 empty, 0.98 full. A loaded
  cart coasts about four times shorter than an empty one.
- A destroyed container cart drops its contents as item entities (`remove` 77).

**Track** (`OldMinecartBehavior`; `AbstractMinecart.useExperimentalMovement` 605-607 applies the
new physics only with the experimental `minecart_improvements` pack, which is off by default)
- A cart moves at most **0.4 blocks a tick = 8 blocks/s** on land (0.2 in water; `getMaxSpeed`
  400-402, the clamp at 199-203).
- A powered rail adds **0.06 a tick** while the cart moves (233-238).
- An unpowered powered rail **halves the speed every tick and stops the cart below 0.03**
  (170-177).
- A cart at rest on a powered rail that turns on starts at 0.02, away from a solid block at either
  end of a flat rail (239-262). It gets no such push on a slope, where gravity (0.0078 a tick,
  112-130) starts it instead.
- Keeping 0.4 b/t: a powered rail crossed at full speed leaves the cart at about 0.55. A full cart
  (0.98 a tick) stays above 0.4 for about 16 ticks, about 6 blocks, so a loaded line needs **a
  powered rail every ~7 blocks**; an empty cart (0.995) needs one every ~25.
- A powered rail passes its power to at most 8 more powered rails (`findPoweredRailSignal`
  23-25), so each spaced powered rail needs its own torch or redstone block.
- 128 blocks at 0.4 b/t = **16 s each way**.

**Hoppers and carts** (`HopperBlockEntity`)
- A hopper reaches a cart the same way it reaches a block container. The block it faces, or the
  block above it, has no container, so it takes a container entity whose box intersects that
  cell's 1x1x1 box (`getEntityContainer` 393-398; with several, a random one).
- It moves one item per 8 gt: **2.5 items/s per hopper** (`MOVE_ITEM_SPEED` 32, `tryMoveItems`
  106-131), into a cart (`ejectItems` 143-172) or out of one (`suckInItems` 218-246).
- **A cart's 0.98-wide box intersects two cells along the rail unless its centre sits within 0.01
  of a cell's centre.** A cart resting against an end block is centred exactly (0.51 from the face:
  one cell); a cart halted by a powered rail, or held on a slope, is not. **Two hoppers under two
  rail cells can therefore empty one cart at 5 items/s.** Across the rail the cart stays centred,
  so four hoppers would need a cart resting mid-curve; curves can't be powered rails.
- A rail can sit on a hopper (the hopper's top is sturdy), and the hopper pulls from the cart on
  it.
- Two carts can't be stacked so that one's hopper minecart drains the other. A rail needs a block
  under it, and that cell would be the lower cart's.

**Sensing** (`DetectorRailBlock`)
- A detector rail sees a cart overlapping its inner box (0.2 to 0.8 of the cell, 0 to 0.8
  high).
- A comparator reading it gives the cart's fill (`getAnalogOutputSignal` 143-157,
  `AbstractContainerMenu.getRedstoneSignalFromContainer`): 15 only when all 5 slots hold full
  stacks.
- The reading refreshes on entry and then every 20 gt while the cart stays (`checkPressed`
  80-111). "Full" and "empty" are therefore seen up to 1 s late.

## The ceilings

- **Loading:** 20 items/s per parked hopper minecart (16 s for 320 items). Extra hoppers pushing
  into the cart's two cells could add up to 12.5/s, which loading never needs.
- **Unloading:** 2.5 items/s per hopper under the cart, and at most two hoppers under a railed
  cart: **5 items/s per unloading spot** (64 s for 320 items).
- **Travel:** 8 blocks/s. The 128-block round trip is 32 s.
- **One cart, 128 blocks:** 16 + 64 + 32 = 112 s per 320 items = 2.86 items/s, about 20 min for a
  double chest.
- **One loader and one unloader spot, any number of carts:** the unloader's 5 items/s, about
  12 min for a double chest.

## The obvious design, measured (gate G2)

The obvious system is one hopper minecart shuttling on the 128-block line between two terminus
stations. At the loader it parks **under the supply chest** (the 20/s pull is the one thing
everybody knows about hopper minecarts). At the unloader it parks **over a hopper** into the storage
chest. Each station sends the cart back when it is full or empty (the supply dry) through a
detector rail and its comparator.

| station | design | blocks | judge (12-13 carts, mixed loads, 0.02-2 b/t) |
|---|---|---|---|
| loader, cart under the chest | [ld-chest](../circuits/tracks/cart-transport/ld-chest.txt) (new) | 34 | 17.46 items/s, departs 15 gt after full or dry |
| loader on the site, hopper beside the cart | rail/archive/cart-loader-t5 | 35 | 2.48 items/s, 9 gt |
| unloader on the site, one hopper under the cart | rail/archive/cart-unloader-t3 | 22 | 2.48 items/s, 15 gt |

ld-chest is cart-loader-t5 with its side hopper replaced by the supply chest above the cart.
- **The chest had to stay clear of the cart's path.** The cart climbs the detector slope, and the
  rail lifts it without collision checks. Rolling back west with its top above the chest's
  bottom, it caught on the chest's east face and stuck.
- **Fix:** an open trapdoor on the east side of the block above the detector stops the climb
  while the cart is still under the chest.
- Game verify passes at 4 positions x 4 rotations. Two rotations measure 17.48 instead of 17.46:
  a one-tick difference in the rate metric, which the rail study's exact-match rule calls
  "rotation-dependent".

**The bar, from the stations' measured rates** (one cart, 128 blocks; trip = 320/load + 320/unload
+ 2 x 16.5 s):

| system | trip | a double chest | items/s |
|---|---|---|---|
| site pair (t5 + t3) | 291 s | 52 min | 1.10 |
| **obvious (ld-chest + t3)** | 180 s | **32 min** | **1.77** |
| hopper line, 128 hoppers (reference) | - | 23 min | 2.5 (640 iron, 128 blocks) |

The obvious system is already 1.6x the site pair, but its one unloading hopper caps it at 72% of
a hopper line's rate.

## Trade-offs

- **Unloader vs everything else:** the unloader is the bottleneck in every single-cart layout
  (64-129 s against the loader's 16-18 s). Gains come from hoppers under the cart, or from more
  carts, not from the loader or the track.
- **Carts vs track:** more carts keep the unloader busy (the ceiling's 5/s). But two carts on one
  two-way line meet head-on, so it takes a loop (twice the track, two turnarounds) and stations
  where a cart can wait behind another.
- **Chest minecart vs hopper minecart:** 5.4x the load per trip, but it can't use the 20/s pull,
  and every unloading spot is still 5/s. Fewer trips save only travel time, which is the smallest
  term.
- **Materials:**
  - Hoppers are 5 iron, and a hopper minecart is 10 iron.
  - Rails cost 6 iron per 16.
  - A powered rail costs 1 gold, plus a torch for its power. A loaded line needs ~18 per 128
    blocks; an empty return line needs ~5.
  - The cart line beats a hopper line on iron at any length over ~20 blocks.
- **Robustness:**
  - The cart freezes in a chunk that doesn't tick (the 128-block reference).
  - Mobs or players can knock a cart off the rail.
  - A station's open hopper under the rail takes strays: the existing judge drops them, and they
    end in the storage chest.
  - A cart that leaves "dry" with a few items is fine for a trickle farm (the next trip carries
    more).

## Five layouts (a double chest over 128 blocks)

**Materials per layout** (reviewer note 4; track = 112 rails, 16 powered rails and 16 torches =
42 iron, 16 gold, ~19 redstone; plus the cart(s) at 10 iron each). A and B are measured (Results);
the others are estimated:

| layout | iron | gold | vs a 128-hopper line (640 iron) |
|---|---|---|---|
| A obvious | 60 | 22 | 9% |
| B two-hopper unloader | 65 | 24 | 10% |
| C 3-cart loop (2x track) | ~150 | ~40 | 23% |
| D chest minecart (2 loading hoppers, chest minecart = 5 iron) | ~70 | ~21 | 11% |
| E rolling unloader (16+ hoppers) | ~140 | ~25 | 22% |

| # | layout | station blocks (est.) | minutes (est.) | % of the matching ceiling | constraints |
|---|---|---|---|---|---|
| A | **obvious**: one hopper minecart; cart under the chest; one hopper under the cart (measured stations) | 56 | 32 | 63% of the one-cart 20 min | none new |
| B | **two-hopper unloader**: as A, but the unloader holds the cart on a slope (t5's geometry: an unpowered powered slope with a detector slope above it). The cart straddles two rail cells there, with a hopper under each; the slope also launches it back by gravity when powered | ~70 | 21 | 95% of 20 min | the cart must stop off-centre (the slope guarantees it: t5's carts rest at 0.78-0.94 of the cell) |
| C | **loop, 3 hopper minecarts**: B's stations as through stations with a waiting spot, a one-way loop (128 out, 128 back, two turnarounds) | ~110, + 128 more track | 12.5 | 96% of 12 min | 2x the track; carts queue behind each other at both stations |
| D | **chest minecart** (1728): two hoppers under a double supply chest push into it (5/s); B's unloader (5/s) | ~75 | 24 | 83% of 20 min | the cart can't pull: loading drops from 20/s to 5/s; 2 trips |
| E | **rolling unloader**: the cart shuttles back and forth over a line of 16 hoppers at full speed (each fresh hopper takes one item as it passes: 8 items/s while rolling) | ~110 | ~20 | ~100% | 16+ hoppers, a reversing mechanism at both ends, and still ~5/s once the reversals are counted |

Estimates:
- A: measured station rates.
- B: A's loader plus 320 items at 5/s per trip.
- C: unloader-bound at 5/s plus the first trip.
- D: 2 x (346 + 346 + 33) s.
- E: 20 passes of 2 s plus reversals.

## Decision

Build **B, the two-hopper unloader with the obvious loader (ld-chest)**, single cart.
- It is 1.5x faster than the obvious system (about 21 min against 32) for about 15 more blocks.
- The cart is unloaded at the unloading spot's ceiling, and a single cart reaches 95% of the
  one-cart ceiling at 128 blocks.
- It needs no loop and no second cart.
- **What the gain is for** (coordinator and reviewer 2026-10-22): even the obvious system
  (1.78 items/s) carries more than the biggest farm on the site makes (the melon lattice, about
  0.9 items/s) and keeps up with the 16-furnace super smelter. B's speed-up matters for bulk
  loads: emptying a full chest from a mining trip or a farm's backlog. The double-chest time
  stays the number.
- **C is dropped** unless someone names a use above 2.9 items/s (one cart's ceiling at 128
  blocks).
- D and E lose to B on the same blocks: D can't use the 20/s pull, and E needs 16 hoppers for no
  gain.

## Results (end to end: both stations and the 128-block track in one design; game = redsim)

`mctas circuit cart-system <line>` (crate::cartsys): the judge fills the double supply chest with
3456 items (54 stacks), puts one empty hopper minecart on the track, and counts what reaches the
double storage chest. Every item must arrive, none on the floor or left in the line.
`mctas circuit cart-line --loader --unloader` writes a line from two station files: the unloader
turned to face east, 128 blocks of track (a powered rail every 8, each lit by a torch), the
loader. The bill includes the track and the cart; the judge's chests are not counted.

| line | stations | a double chest | items/s | trips | bill | blocks (track 293) |
|---|---|---|---|---|---|---|
| [line-site](../circuits/tracks/cart-transport/line-site.txt) | site's cart-loader-t5 + cart-unloader-t3 | 52.4 min | 1.10 | 11 | 65 iron, 21 gold, 67 redstone | 344 |
| [line-a](../circuits/tracks/cart-transport/line-a.txt) (obvious) | ld-chest + cart-unloader-t3 | 32.4 min | 1.78 | 11 | 60 iron, 22 gold, 66 redstone | 353 |
| **[line-b](../circuits/rail/system/line-b.txt)** | ld-chest + **ul-gate** | **20.9 min** | **2.75** | 11 | 65 iron, 24 gold, 80 redstone | 363 |
| a hopper line, 128 blocks (reference) | - | 23 min | 2.5 | - | 640 iron | 128 |

- line-b is 96% of the one-cart ceiling (20 min) and 1.55x the obvious system.
- line-b beats a 128-hopper line on time, at a tenth of the iron (plus 24 gold for the
  powered rails).

**ul-gate** ([file](../circuits/rail/archive/ul-gate.txt), 31 blocks; rail judge
`circuit verify`: robust, 4.79 items/s, a full cart emptied in 64 s against t3's 128 s).
- The proof the reviewer asked for comes first: a cart held off-centre, drained by two hoppers.
- Boosters send the cart up a powered slope toward a closed fence gate. The gate's collision
  plate sits in the middle of its cell, so the climb ends at 0.885 of the slope's cell.
- A detector rail at the foot of the slope, with a plain rail between them (a pressed detector
  rail powers every neighbour, the slope included), cuts the slope's power through a repeater
  just as the cart reaches the top. The halted slope holds it there.
- Any rest point between 0.51 and 0.94 of the slope's cell puts the cart's box over the cell of
  the hopper under the slope AND the gate's cell above the second hopper, which also supports
  the slope's upper end. Both hoppers pull.
- The comparator on the second hopper keeps the slope off while items flow. When that hopper
  stays empty, a torch powers the slope again and the cart rolls back down by gravity.
- An idle station keeps the slope powered: an empty cart is boosted up and rolls straight back.

**Block counts** (reviewer's note 3): ld-chest (44 with the dry-supply fix) and ul-gate (31) are slope stations. The
reviewer estimates a plain terminus at ~12-15 blocks each, so no block beat is claimed. The
reviewer compares the stations with a plain terminus at G3.

Beats are claimed against the obvious system's 32 min / 56 station blocks on the reference track.

## Must-haves

- Every item delivered: none lost, none left in a cart or a station. Any load from 1 item to a
  double chest, including 16-stackables and non-stackables.
- The cart returns by itself, never stuck. It leaves the loader when full, or when the supply ran
  dry and it holds something, and leaves the unloader when empty.
- **An empty cart at an empty supply waits** (coordinator and reviewer 2026-10-22: an empty cart
  shuttling forever is the first thing a player notices at an idle farm). The rail judge's loader
  rule now demands it: an empty cart at a dry supply may not move for 600 gt, then it must take a
  late supply of 10 items and leave. The site's earlier loaders (cart-loader-t2, t4, t5) sent the
  empty cart away and left the archive; ld-chest replaces them.
- Supply and storage chests where a player reaches them.
- Safe from stray items dropped on open hopper tops (the existing station judge already checks
  this).

## Not goals

- Exact cart timing or a departure delay under a second (reported, not ranked).
- Keeping the loader from sending partly filled carts when the supply runs dry (a trickle makes small loads; the cart still parks when there is nothing).

## The system run (built: crate::cartsys, `mctas circuit cart-system`)

A system run on top of the station judges. The two stations (each still passing its own
`cart-run` judge) and the 128-block track form one design. The judge fills the loader's double
chest with 3456 items (54 stacks of 64: the station judges already cover 16-stackables and
non-stackables), starts one cart, and counts arrivals in the unloader's double chest. The number
is minutes until everything has arrived, with every item accounted for (none on the floor, none
left in a cart or a station).

**The trickle run** (`cart-system --trickle`): the supply is fed slowly, like a farm's output (5
items every 100 gt up to 300), then nothing more. Every item must arrive, and the cart must end
parked at the loader (still for 2000 gt within 4 blocks of the supply chest).
- line-b (game = redsim): all 300 delivered by 5.9 min; the cart ends parked under the supply
  chest at (133.83, 0.89, 0.50).
- The line built with the loader before the dry-supply fix
  ([calibration](../circuits/tracks/cart-transport/calibration/line-shuttles.txt)): with the chest
  empty at the start, it sent its empty cart away; the cart came to rest at the foot of the
  loader's slope and never moved again. 10 of 300 delivered in 115 min.

**ld-chest with the fix** ([file](../circuits/rail/archive/ld-chest.txt), 44 blocks, 16.87
items/s): one comparator reads the cart's fill through the detector rail, in compare mode. Its side
input is the chest's own comparator, lifted to full strength by a repeater (a double chest holding a
few items reads 1). So it passes the fill only when the cart is full (15 against 15), or when the
chest is empty and the cart holds something (fill against 0). A repeater after it carries a cart
holding one item (fill 1) to the slope. A fourth booster before the slope makes every cart, even a
crawling one, climb into the detector rail's view before it halts (with three, a cart entering at
0.05 b/t was halted 0.6 block up the slope, out of view, and would wait forever). Robust in all 16
game builds. Rule (coordinator 2026-10-22): a cart station is robust when every build passes with
every item delivered, and its rate is the worst build's. In a few builds the 0.02 b/t test cart
rests 0.1 block lower (floating-point slope physics at other coordinates) and is full 16 gt sooner
(17.05 items/s); the card shows the worst, 16.87.
