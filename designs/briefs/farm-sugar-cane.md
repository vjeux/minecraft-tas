# Full-size sugar cane farm (design doc, gate G1)

Owner: clk-rt. Process: [docs/process.md](../../docs/process.md). Backlog row 3 of
[BACKLOG.md](BACKLOG.md). Every number here is re-derived from the 26.3 server code (`mctas src
show <Class>`); no outside designs were used.

## Use

A player builds it once, beside their base, and leaves it running: sugar cane for paper (books,
maps, rockets) without ever harvesting by hand.

## The number

- **items per hour per block** for the whole farm at vanilla speed (`random_tick_speed` 3), the
  farm judge's `items per hour per block` (every block counted: soil, water, hoppers, wiring).
- **items per hour** for the farm, at a full size: about one chunk (16 x 16) or at least ~100 canes.
- Every result also as a **percentage of the ceiling** (below).

## The mechanics, from the code

1. **Random ticks.** `ServerLevel.tickChunk` (ServerLevel.java:505-544): every tick, in every
   16x16x16 section that holds a randomly ticking block, `randomTickSpeed` (game rule
   `RANDOM_TICK_SPEED`, default 3, GameRules.java:74) positions are drawn with `getBlockRandomPos`
   (mask 15: uniform over the section's 4096 cells, draws independent). A given block gets
   3/4096 random ticks per game tick = **52.7 per hour** (72000 gt). Only chunks the server ticks
   (`ServerChunkCache.tickChunks` -> `forEachBlockTickingChunk`, within simulation distance of a
   player) grow anything.
2. **Cane growth.** `SugarCaneBlock.randomTick` (SugarCaneBlock.java:43-61): only a block with AIR
   above it ages (`isEmptyBlock(pos.above())`), and only while the stalk under and including it is
   under 3 blocks (`height < 3`). Its `AGE` (0-15) goes up by one per random tick; at 15 the next
   random tick places a new cane block above and resets the age to 0. So **one new block per 16
   random ticks of the top block**, and a block that has a block above it does not age at all: a
   cane 2 high whose top was broken restarts the lower block from its age 0, a stalk 3 high
   stops. A block that grew keeps age 0 (the reset), so nothing carries over a harvest.
3. **Where cane stands.** `SugarCaneBlock.canSurvive` (lines 82-101): the block below is cane, or a
   block of `#supports_sugar_cane` (`#substrate_overworld` = dirt, mud, moss, grass, and `#sand`:
   data/minecraft/tags/block/supports_sugar_cane.json, substrate_overworld.json) with, on one of
   its four horizontal sides, a fluid of `#supports_sugar_cane_adjacently` (water: source or
   flowing, waterlogged blocks included) or frosted ice (not placeable in survival). Water above or
   below does not count. A cane that loses its support is not removed at once: `updateShape`
   schedules a tick 1 gt later (lines 64-79) and `tick` destroys it with drops (`destroyBlock(pos,
   true)`, lines 36-40), so **the block above a broken block pops one tick later**.
4. **What breaks cane.** Sugar cane is `noCollision` and `pushReaction(PushReaction.POPPED)`
   (Blocks.java:2178-2188; POPPED is 26.3's name for the old DESTROY). An extending piston whose
   first block is POPPED puts it in `toDestroy` (`PistonStructureResolver.resolve`, lines 41-43)
   and `PistonBaseBlock.moveBlocks` (lines 280-297) calls `dropResources` on it before anything
   moves. Its drop is one sugar cane per block.
5. **Item drops.** `Block.popResource` (Block.java:409-416) spawns the item at the block's centre
   +-0.25 on each axis; the `ItemEntity` constructor (ItemEntity.java:66) gives it a random speed of
   +-0.1 block/gt horizontally and 0.2 up. From a cane's 3rd block an item lands up to ~1.2 blocks
   away (about 14 gt of flight with air drag 0.98): **items can land in the neighbouring columns
   and past the ends of a row**. An item despawns after 6000 gt on the ground (`LIFETIME`,
   ItemEntity.java:44, 189): a farm must collect everything within 5 minutes or lose it.
6. **Pistons push items.** `PistonMovingBlockEntity.tick` (lines 294-319) moves the head half a
   block per tick (2 gt to extend) and `moveCollidedEntities` (line 118) carries every entity in its
   way to the head's face: an item spawned in the cell the head enters is pushed out of it.
7. **Observers.** `ObserverBlock.updateShape` (line 67): a shape change of the block in front starts
   a signal only if the observer is not already powered; `tick` (lines 44-50) turns it on for 2 gt
   (`scheduleTick(pos, this, 2)`) and its back face powers the block behind it strongly (line 110:
   only on its facing). **An observer that watches a cell a piston head moves through re-arms
   itself**: the head's arrival (`PistonMovingBlockEntity` finishing, line 297-316) or its retraction
   lands after the 2 gt pulse ended and starts a new pulse: a clock. So an observer can only watch
   a cell the head never enters: the cane's 3rd block when the head breaks the 2nd.
8. **Piston power.** `PistonBaseBlock.getNeighborSignal` (lines 116-135): any powered neighbour
   (except its front), or quasi-connectivity through the cell above it, but a piston only
   re-checks on a neighbour update. Redstone dust strongly powers the block it stands on
   (`RedstoneWireBlock.getDirectSignal` = `getSignal`, lines 356-374, with direction UP) and
   updates its neighbours' neighbours, so a block with dust on top powers every piston beside it.
   One observer and one piston cannot be wired to each other directly: an observer that sees the
   cane has its back two cells from the cane, a piston that breaks the cane is beside it, and no
   single block touches both (checked for every position of either). Some relay is needed.
9. **Hoppers.** `HopperBlockEntity.suckInItems` (lines 222-244) takes items whose box meets
   `Hopper.SUCK_AABB` (Hopper.java:8: the hopper's top 5/16 plus the whole cell above it), unless
   the block above is a full collision block. **Mud is 14/16 tall** (MudBlock.java:13), so a hopper
   under mud is not blocked and takes items lying on the mud (their bottom at 0.875 < 1.0);
   water has no collision, so a hopper under a water cell takes items floating in it. A hopper
   moves one item per 8 gt (`setCooldown(8)`, line 124): **2.5 items/s per line**, 28 times what a
   96-cane farm makes at vanilla speed (316 items/h = 0.09/s). An item resting on a FULL block
   (stone) over a hopper is not taken.
10. **Water carries items** only where it flows: `EntityFluidInteraction` (line 92) adds the
    fluid's `getFlow` (FlowingFluid.java:54, the slope toward lower levels) to an entity in it; a
    still source has no flow, so an item that falls into a still channel stays there and despawns.

## The ceiling

- **Per plant:** 52.7 random ticks/h / 16 = **3.30 cane per hour** (mechanics 1-2), reached only
  if every grown block is harvested at once (no time at height 3, no discarded age).
- **Per farm:** 3.30 x plants. Plants per floor cell: every soil block needs a water neighbour, and
  one water cell serves at most 4 soils, so water is at least 1/5 of the floor (the lattice
  `x + 2z = 0 (mod 5)` is water, everything else cane: a perfect dominating set of the grid).
  So **at most 4/5 of the floor is cane: 2.64 cane/h per floor cell** (rows cane-water-cane give
  2/3, 2.2/h).
- **Per block:** every cane costs at least its soil plus 1/4 water block, so **at most 2.64 items
  per hour per block**, with harvesting and collection free. Every layout below is a percentage of
  3.30/h per plant for its rate and of 2.64/h per block for the number.

## The trade-offs

- **Trigger per plant vs shared trigger vs clock.** A plant harvested by its own trigger at height 3
  loses nothing (both grown blocks are taken; the 2-4 gt the head stands above the stalk cost
  4/72000 of the time). Every trigger shared with other plants breaks their 2nd block early and
  throws away its age, and the lower block has not aged meanwhile: the plant waits from its 2nd
  block's growth to the next shared trigger. In steady state, with n plants on one dust network
  each harvested at 2, x = (wait / growth time)^-1 solves x^2 + x = n: **n = 16 gives 78% of the
  plant rate, n = 32 gives 84%**. A clock of period P loses P/2 per harvested block against a
  growth time of 21845 gt (16 random ticks): P = 30 s gives 98.6%, P = 60 s 97.3%.
- **Blocks.** Per plant, an observer (watching the 3rd block), a piston and a relay (dust on stone)
  are ~3.5 blocks; soil + water 1.5; hoppers under the mud and the water ~2. Any plant-free
  harvester (a clock, a flying machine) removes the observer; a machine removes the pistons too.
- **Collection.** Mud + a hopper under it takes items wherever they land on the mud, including the
  3rd block's item that falls back into the stalk; a water stream needs every item pushed into it
  (only the 2nd block's), so it fits only farms that never let cane reach 3 blocks; hopper
  minecarts cost a cart and a rail per column. Hoppers cost 5 iron each: ~220 hoppers for 96 canes
  is 1100 iron (survival-possible, the expensive part).
- **Robustness.** Items fly up to ~1.2 blocks: every column next to a cane, and one cell past each
  row end, must be over a hopper or the item is lost. A still water channel loses what falls in
  it, unless a hopper sits under it.
- **Tileability.** Rows along one axis tile along the other by sharing the relay column.
- **The judge's speed-up.** The judge grows crops faster than the game (`random_tick_speed`
  above 3) to see enough growths in a short run. At a fixed 300 (100x) a 96-cane farm makes
  ~8.8 items/s, more than a hopper line carries (2.5/s): the judge's chest filled, hoppers backed
  up and items piled on the mud. A vanilla farm never sees this. w86's fix (2026-10-21 22:40): the
  judge's chests are emptied during the run, items left in the design's hoppers count as backlog,
  and the speed depends on the plant count (`circuit::farm::rts_for`: 300 up to 16 plants, else
  max(30, 300 x 16 / plants): **rts 50, 16.7x, for 96 canes**, ~1.5 items/s). A clock-driven farm
  is still under-read at any speed-up: its wait P/2 stays while growth gets faster (a 30 s clock
  reads 81% at rts 50, 88% at rts 30, against the vanilla 98.6%; at 300 it read 42%). In layouts
  where the cane can reach 3 blocks while it waits (3, 5) a cane also stands at 3 longer than the
  judge's 40 gt missed-harvest limit.

## The efficiency rule (G1 decision, coordinator 2026-10-21; with w86 and clk-misc)

The design's own clock is never scaled (that would judge a different design from the one a player
builds). Instead:

- **Efficiency at vanilla speed:** the farm judge measures `items per hour` and `items per hour per
  block` from one redsim-twin run of the base build at `random_tick_speed` 3 for 720 000 gt (10
  game hours; the twin mirrors the game draw for draw, about 2 s per 10 000 gt for a 200-plant farm),
  counting the items delivered to the judge's chests plus those in the design's containers at the
  end. A ~200-plant farm grows ~6600 items in it (±1.2%).
- **Robustness stays on the fast game runs** (`rts_for`, 16 builds, the tiled run, intrusion): no
  plant lost, no item loose or despawned, rest.
- **The missed-harvest rule** (no cane at 3 blocks for over 40 gt) is dropped: the vanilla run
  charges any wait in the efficiency number itself. (Layout 4 never reaches 3 blocks anyway.)
- **Rest means the items have settled** (w86, 2026-10-21 23:23): no crop or stray item moving and
  every container's crop count unchanged over one rest window; blocks may keep moving (a clock or a
  flying machine is part of the farm). A farm that never stops shows `rests: no (...)` in its table.
- **Stray-item intrusion is bounded** (w86, 23:12): the three intruders go on at most 8 hopper
  intakes and 8 crop columns (a 22 000-item flood into one collector was a judge artifact).
- **The fast runs' speed** scales with the farm (`rts_for`: 300 x 8 / plants, at least 30): rts 30
  for 96 or 204 canes, so a collector line carries the run (layout 4 uses two collectors anyway).
- (The reviewer's alternative, a formula from the measured clock period, free growth x (1 - P/2 /
  21845 gt), is kept as a cross-check on clocked designs.)

## Layouts

Estimates per 16 x 16-ish farm; rate as % of 3.30/h per plant, number as items/h per block and %
of the 2.64 ceiling. **Iron** (reviewer 2026-10-21: a must-have number): a hopper is 5 iron, a
piston (sticky or not) 1, a minecart 5; observers, dust, repeaters and comparators none.

| # | layout | plants | blocks | iron | rate (est.) | items/h | items/h per block | % of 2.64 | constraints |
|---|---|---|---|---|---|---|---|---|---|
| 1 | **The obvious design** (built, measured): double rows on mud, an observer on top of each piston watching the 3rd block, a dust-on-stone relay column shared by the two rows beside it, hoppers under every mud and water cell, one chest | 96 | 818 | **1191** (219 hoppers, 96 pistons) | **73% measured** (model: 82%, n = 16/32 shared networks) | **230** | **0.28** | 11% | passes every judge rule once w86's backlog fix lands |
| 2 | **Own trigger per plant**: the observer's back strongly powers a stone; a sticky piston under the stone pushes a block (that the observer stands on) into the 2nd block and pulls it back. No dust, no sharing | 96 | ~914 | ~1290 | ~99% | ~313 | ~0.34 | 13% | +22% over the measured bar, but a 4-block mechanism per plant |
| 3 | **Rows + one clock**, no observers: the relay columns driven by one clock (P = 30 s) | 96 | ~730 | ~1200 | ~98% | ~310 | ~0.42 | 16% | a cane can reach 3 blocks while it waits: needs the missed-harvest rule adapted |
| 4 | **Water lattice + pistons from above + clock** (counted below) | 204 | **1068** | **1836** | ~98% | ~660 | **~0.62** | 23% | 2.2x the bar; an edge ring of catchers; the most iron, the least iron per item/h (2.8 against the bar's 5.2) |
| 5 | **Flying-machine sweeper**: rows cane-water-cane on mud + hoppers, one two-way flying machine runs over the farm at the 2nd block's height on a clock and destroys every 2nd block it passes | ~128-205 | ~450-570 | ~1300-1650 | ~95% | ~400-640 | ~0.9-1.1 | 34-42% | the missed-harvest rule adapted, and a judge that runs a machine through a farm; engine, stations and return path are the risk |
| 6 | **No hoppers but one: canes beside FLOWING water** (`#water` holds `flowing_water`, data/minecraft/tags/fluid/water.json, so `canSurvive` accepts it): rows land-cane-channel-cane-land, the piston on the land side pushes the 2nd block's item toward the channel, the stream carries it to one hopper at its end | 96 | ~605 | ~126 | 73% | (230) | (0.38) | (14%) | **fails "no lost items"**: see below |

**Why layout 6 does not work:** the head leaves the 2nd block's item just past the channel's edge,
but the item keeps `ItemEntity`'s random sideways speed (+-0.1 block/gt, ItemEntity.java:66, and
0.2 up) for its ~15 gt fall to the water, so roughly half drift back onto the cane's own soil
(where an item lies still and despawns after 6000 gt) or past a 1-wide channel; the 3rd block's
item pops a tick later (mechanic 3) with no push at all. Every cell an item can land on needs a
hopper (or a stream under it), so hoppers stay; the decision stands.

**Layout 4, counted** (a 16 x 16 field, `x + 2z = 0 (mod 5)` water):

| part | blocks | iron |
|---|---|---|
| floor: 204 mud (canes) + 52 water sources | 256 | 0 |
| hoppers under every floor cell, in rows into a collector (items land up to ~1 block from their cane: ±0.25 spawn offset + ≤0.8 drift in ~9 gt, so every cell catches) | 256 | 1280 |
| edge ring (68 cells): mud on a hopper, its east column the collector into the chest | 68 + 68 | 340 |
| pistons at the 3rd-block height facing down | 204 | 204 |
| dust on every piston top: a carpet joined through the cane cells (the water cells are isolated points, so no block is needed over them; a downward piston's back is a full face, `Block.canSupportCenter`) | 204 | 0 |
| repeaters (dust carries 15: a spine of 2-3 repeaters on piston tops reaches every cell within 14) | ~4 | 0 |
| clock: a sealed hopper clock (2 hoppers, 2 comparators, 2 sticky pistons, 2 blocks), period ~25 s | ~8 | 12 |
| **total** | **~1068** | **~1836** |

(A glass wall 3 high instead of the ring: 192 blocks, no iron: 0.58/h per block for 1576 iron.)

## The decision

Search from **layout 4**: it is the simplest mechanism that clearly beats the bar (about 2.2x on
the brief's number and 2.9x on items/h per farm), every plant is identical (no wiring cases), the
dust carpet ties all pistons to one clock, the 3rd block never exists (so no item ever falls back
into the stalk unpushed and there is no missed-harvest at height 3), and it tiles in both
directions. It is judged by the efficiency rule above (vanilla-speed twin run): no scaling of its
clock, no under-read. Layout 5 is the stretch goal once 4 is published. Layout 2 is the
fallback if a clock-driven farm is ruled out: it passes today's judge and beats the measured bar
by ~22%, at the price of a sticky piston and a pushed block per plant.

Status: **G1 approved** (reviewer and coordinator, 2026-10-21 22:42), with the additions above.

## Must-haves (from real use)

- Fully automatic: no player action after building; output into a chest the player reaches.
- No lost items: every drop reaches the chest; nothing rests on the ground (it would despawn after
  6000 gt), nothing in still water.
- Survival materials only (mud from a water bottle on dirt; no frosted ice, no commands).
- Tileable along at least one axis, so a player who wants more builds copies side by side.
- Safe from stray items (the judge's intrusion run): a dropped cobblestone or tool must not jam it.
- Robust in every rotation and position (the judge's 16 builds).

## Not goals

- Fairness between plants, harvest height, the wait before a harvest: only the farm's rate counts.
- Items per hour per plant on its own (a 1-plant harvester is not a farm).
- Sizes beyond about a chunk: the number is per block, the farm tiles.
- Collection throughput above vanilla rates (the judge's 100x backlog is not a player's problem).

## The obvious design's measured numbers

designs/circuits/tracks/clk-rt/cane/cane-obvious-16x16.txt (`mctas circuit cane-gen`): 866 blocks
(818 + a second catcher row at both ends), 96 canes, 1191 + 60 iron.

- **Vanilla speed** (redsim twin, `farm-run --sim --rts 3 --ticks 720000`, the judge's efficiency
  run, on the 818-block version): 2234 harvested in 10 game hours of about 3164 from free growth =
  **71% of the plant ceiling: 223 items/h, 0.273 items/h per block (0.258 with the deeper
  catchers), 10% of 2.64**. The shared dust networks lose more than the model's 82%: a 2nd block
  waits up to 31 390 gt for some cane on its network to reach 3.
- At the judge's speed (rts 60, 48 000 gt) it read 73%: the bar does not depend on the speed-up.
- **Before G3** the bar is re-measured by the fixed judge in the game, like every candidate
  (reviewer 2026-10-21). With the deeper catchers no crop item escapes in 48 000 gt.

## Results (layout 4)

designs/circuits/tracks/clk-rt/cane/cane-lattice-16x16.txt (`mctas circuit cane-gen --lattice
16,16`): 1234 blocks, 204 canes, two collectors (two chests), 1836 iron.

- **Vanilla speed** (the judge's efficiency run: twin, rts 3, 720 000 gt): 6460 harvested of about
  6724 from free growth = **96% of the plant ceiling: 646 items/h, 0.523 items/h per block (19.8%
  of 2.64)**, 1.9x the bar per block and 2.9x per farm; 2.8 iron per item/h (the bar: 5.6).
- At the judge's speed (rts 30) the clock's wait reads 83%; the card takes the vanilla run.
- The game run of the base build agrees with the twin (1868 grown, 1868 harvested at rts 30).
- Its remaining judge failures were the judge's (the 22 000-item intrusion flood, the rest check
  on a clocked farm), both fixed by w86 (above); the official card follows with w86's push.

## Layout 5 (stretch, in progress)

designs/circuits/tracks/clk-rt/cane/cane-sweeper-10x16.txt (`mctas circuit cane-gen --sweeper
10,16`): the layout-4 field (lattice, hopper floor, ring, two collectors), no pistons, dust or
clock. ONE flying machine sweeps the 2nd-block height: the docking machine's 6-block core (track
fly, fly-docking-15b-narrow: two halves of observer, slime, sticky piston), stations over the ring
rows (an observer that sees it arrive, the stone it powers, a cross of dust on the stone behind the
leading piston, a wall), and a wing per half: slime at y = 3 stuck to the half's slime, a stone
blade under each. 569 blocks, 128 canes, about 1190 iron (238 hoppers, 2 pistons).

Mechanics that shaped it, read from the code:
- A piston moves only what a sticky block of its structure reaches:
  `PistonStructureResolver.addBranchingBlocks` branches from `isSticky` blocks only, and
  `addBlockLine` walks back along the push axis only while the block is sticky. A wing hung on the
  observer stays behind; it must hang from the half's slime. 12 blocks per push: 4 wing columns
  per half (slime + blade), 10 columns per machine.
- A moving slime block LAUNCHES the entities its box sweeps: `PistonMovingBlockEntity.
  moveCollidedEntities` sets that axis of their motion to 1 block/gt. A cane item pops with 0.2 up
  and rises into the slime layer, then flies ~9 blocks along the machine's track: obsidian walls
  (immovable, so the slime never drags them) past both ends, and a catcher row under each end. A
  slime blade at y = 2 would launch every item.
- Honey is no redstone conductor (not a full block), so a honey core's observer cannot reach its
  piston; honey and slime never stick (`canStickToEachOther`). A second machine beside the first
  (16 wide) would have stuck to it through the wings: one 10-wide machine, a 10-wide field.
- An all-`none` dust is a dot and powers only the block under it: the stations need a cross.
- Each station watches a ring cell (no cane), so only the machine changes it; it starts when the
  judge (or the player, last) plants the cane under its first observer.

Measured so far (game, rts 30, 12 000 gt): 565 grown, 565 harvested, longest wait 323 gt; 80% of
free growth at rts 30, where the 358 gt round trip weighs 10x what it does at vanilla speed (about
98% there). Estimated card: about 410 items/h, **0.72 items/h per block (27% of 2.64), 2.6x the
bar per block**. The twin cannot run it yet (the fall distance of a launched item near water;
clk-misc models it next), and the intrusion flood still strands ~45 items (w86's bound fixes it).
