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
  judge's 40 gt missed-harvest limit: those **need a judge rule (the clock period divided by the
  speed-up)**, which goes through this doc first. Layout 4 never reaches 3 blocks and can be judged
  as is, under-read by ~10%.

## Layouts

Estimates per 16 x 16-ish farm; rate as % of 3.30/h per plant, number as items/h per block and %
of the 2.64 ceiling.

| # | layout | plants | blocks | rate (est.) | items/h | items/h per block | % of 2.64 | constraints |
|---|---|---|---|---|---|---|---|---|
| 1 | **The obvious design** (built, measured): double rows on mud, an observer on top of each piston watching the 3rd block, a dust-on-stone relay column shared by the two rows beside it, hoppers under every mud and water cell, one chest | 96 | 818 | **73% measured** (model: 82%, n = 16/32 shared networks) | **230** | **0.28** | 11% | passes every judge rule once w86's backlog fix lands |
| 2 | **Own trigger per plant**: the observer's back strongly powers a stone; a sticky piston under the stone pushes a block (that the observer stands on) into the 2nd block and pulls it back. No dust, no sharing | 96 | ~914 | ~99% | ~313 | ~0.34 | 13% | +22% over the measured bar, but a 4-block mechanism per plant |
| 3 | **Rows + one clock**, no observers: the relay columns driven by one clock (P = 30 s) | 96 | ~730 | ~98.6% | ~312 | ~0.43 | 16% | needs the clock rule; +34% |
| 4 | **Water lattice + pistons from above + clock**: water on `x + 2z = 0 (mod 5)`, every other cell cane on mud with a hopper under it; a piston in each cane's 3rd-block cell faces down and breaks the 2nd block (the 3rd never grows: no air above), pushing its item down into its own column; dust on every piston top, one clock | ~205 | ~1070 | ~98.6% | ~666 | ~0.62 | 24% | judged as is (rts 30 at ~205 plants: under-read as ~88%); 2.2x the bar; edge ring of catchers; ~1300 iron |
| 5 | **Flying-machine sweeper**: rows cane-water-cane on mud + hoppers, one two-way flying machine runs over the farm at the 2nd block's height on a clock and destroys every 2nd block it passes | ~128-205 | ~450-570 | ~95% | ~400-640 | ~0.9-1.1 | 34-42% | needs the clock rule and a judge that runs a machine through a farm; engine, stations and return path are the risk |

## The decision

Search from **layout 4**: it is the simplest mechanism that clearly beats the bar (about 2.2x on
the brief's number and 2.9x on items/h per farm), every plant is identical (no wiring cases), the
dust carpet ties all pistons to one clock, the 3rd block never exists (so no item ever falls back
into the stalk unpushed and there is no missed-harvest at height 3), and it tiles in both
directions. With w86's plant-scaled judge speed it can be judged as is: at ~205 plants the judge
runs at rts 30, where a 30 s clock is under-read as ~88% instead of 98.6%, still about 2x the bar.
A judge rule scaling the clock period by the speed-up would remove the under-read; it would come
back here with w86 first. Layout 5 is the stretch goal once 4 is published. Layout 2 is the
fallback if a clock-driven farm is ruled out: it passes today's judge and beats the measured bar
by ~22%, at the price of a sticky piston and a pushed block per plant.

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

designs/circuits/tracks/clk-rt/cane/cane-obvious-16x16.txt (`mctas circuit cane-gen`): 818 blocks,
96 canes, 16 x 18 footprint plus the collector row.

- **Measured** (redsim twin, `circuit farm-run --sim --rts 60 --ticks 48000`, 20x speed so the
  hoppers keep up): 3071 blocks harvested of about 4219 from free growth = **73% of the plant
  ceiling**, i.e. **2.40 cane/h per plant, 230 items/h for the farm, 0.28 items/h per block (10.7%
  of 2.64)**. The shared dust networks cost more than the 82% the model above gives (one run;
  the 73% is the bar). Longest wait from growth to harvest 1643 gt (a 2nd block waiting for a
  shared trigger), longest at full height 1 gt.
- **Today's judge fails it** for two reasons that are the judge's speed-up, not the farm (the
  judge's single chest fills: 1408 cane + 314 stray-item intruders in one run; and at 100x the
  farm makes 8.8 items/s, over a hopper line's 2.5/s). w86's fix (22:40, landing with their next
  push): chests emptied during the run, the design's hoppers counted as backlog, rts 50 for 96
  canes. The judge's own numbers for this design follow when it lands.
- **A real flaw of mine** it also showed: a few items fly past the catcher row at the south end
  (one escaped at z = 17.75 in 48000 gt); the next version gets a deeper catcher at both ends.
