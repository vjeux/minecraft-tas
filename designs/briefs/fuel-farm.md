# Fuel farm for the super smelter (design doc, w86, 2026-10-22)

## Use

A player builds it next to their super smelter and never fetches fuel again: the farm grows a crop,
turns it into fuel, and puts the fuel into the smelter's fuel chest by itself.

## The number

- **Fuel made per hour per block of farm, in items it can smelt**, at vanilla speed
  (random_tick_speed 3). A furnace smelts one item per 200 gt of burn time, so fuel in items =
  burn time / 200: a bamboo is 0.25 items, a dried kelp block 20.
- **Second:** how many plants keep one 8-furnace super smelter at full speed, and how long a 64-plant
  farm takes to fuel one double chest of ore (3456 items).
- The super smelter at full speed: 8 furnaces x 1 item per 200 gt = 0.8 items/s = **2880 items/h**,
  which burns 576,000 gt of fuel an hour. One double chest of ore needs 691,200 gt (3456 items),
  about 7 stacks of coal.

## The mechanics, from the 26.3 code

- **Random ticks.** `ServerLevel.tickChunk` gives every block rts / 4096 random ticks per gt: at
  vanilla rts 3 that is 72000 x 3 / 4096 = **52.7 random ticks an hour**.
- **Burn times.** Burn times are data: `ContextIntProviders.bootstrap` registers each fuel's burn
  time, and `Items` attaches it with `cookingFuel(...)`.

  | fuel | burn time (gt) | items smelted |
  |---|---|---|
  | bamboo, scaffolding (`COOKING_TIME_BAMBOO`) | 50 | 0.25 |
  | stick, dead bush, saplings (`COOKING_TIME_DRY_PLANTS`) | 100 | 0.5 |
  | bamboo slab | 150 | 0.75 |
  | bamboo block, planks, mosaic, stairs, fence, trapdoor, pressure plate (`COOKING_TIME_WOOD_BLOCKS`) | 300 | 1.5 |
  | coal | 1600 | 8 |
  | dried kelp block (`COOKING_TIME_DRIED_KELP_BLOCK`) | 4001 | 20 |
  | lava bucket | 20000 | 100 |

  A smoker or blast furnace (the `FAST_FURNACE` predicate) halves the burn time and doubles the speed
  (`ContextFloatProviders`, `AbstractFurnaceBlockEntity.getTotalCookTime`), so the fuel per item
  is the same in any furnace.
- **Bamboo growth.** `BambooStalkBlock.randomTick`: the top block grows on `nextInt(3) == 0` when the
  block above is empty and the light is 9 or more. That is **17.6 bamboo an hour per stalk** with no
  memory: a stalk cut at any height loses nothing. It needs `bamboo_plantable_on` soil (dirt, mud,
  sand...).
- **Kelp growth.** `GrowingPlantHeadBlock.randomTick` with `KelpBlock.GROW_PER_TICK_PROBABILITY`
  0.14 gives **7.38 kelp an hour per plant**.
  - The cell it grows into must be water. `KelpBlock.canGrowInto` is `state.is(Blocks.WATER)`, so
    flowing water counts.
  - It stops at age 25. After a harvest the new top gets a random age 0..24
    (`GrowingPlantHeadBlock.getStateForPlacement`), so a farm that harvests every growth almost
    never stops.
  - It survives on any sturdy top face except `cannot_support_kelp` (`GrowingPlantBlock.canSurvive`).
    w86's kelp-17b stands on a hopper.
- **Turning bamboo into more fuel.** The crafting recipes:
  - 9 bamboo make a bamboo block (300 gt: less than the 450 the bamboo burns raw).
  - The block makes 2 bamboo planks (600 gt). That is **66.7 gt per bamboo, 1.33 x raw**, the
    best bamboo product.
  - Slabs and mosaic equal planks. Stairs, trapdoors, pressure plates, signs and rafts lose. Sticks
    and scaffolding equal raw bamboo. Hanging signs gain but eat iron chains.
- **Turning kelp into fuel.** Kelp itself is not a fuel. Kelp dries in a furnace or smoker
  (`dried_kelp_from_smelting` / `_smoking`, cookingtime 200), and 9 dried kelp craft a dried kelp
  block (4001 gt).
  - Drying costs 200 gt of fuel per kelp. Fed with the farm's own blocks, the farm nets
    4001/9 - 200 = **244.6 gt per kelp = 1.22 items smelted**.
  - A campfire dries kelp without fuel (`dried_kelp_from_campfire_cooking`, 600 gt), but nothing
    automatic can load it. Only a player's use calls `CampfireBlockEntity.placeFood`
    (`CampfireBlock.useItemOn`): there is no dispenser behavior for food, and a campfire is not a
    hopper container.
- **Furnace faces** (`AbstractFurnaceBlockEntity.getSlotsForFace`): a hopper into the top feeds the
  input, into a side the fuel, and a hopper under it takes the result.
- **Crafter** (`CrafterBlock`): it crafts 4 gt after a rising redstone edge (`CRAFTING_TICK_DELAY`).
  It puts the result into the container in front (`HopperBlockEntity.addItem`), else spits it out.
  `CrafterBlockEntity.canPlaceItem` fills its slots evenly (`smallerStackExist`), so a hopper line
  fills all 9 slots one item at a time. A comparator reads the number of filled slots
  (`getRedstoneSignal`), so a signal of 9 means ready.
- **Lava** (the fuel that would win, ruled out):
  - `PointedDripstoneBlock.maybeTransferFluid` fills the cauldron under a dripstone with lava on 15
    of 256 random ticks of the stalactite's top. A full lava cauldron is one bucket: 3.1 buckets an
    hour = **309 items smelted an hour per dripstone**, from a lava source that never runs out.
  - Nothing automatic can take the lava out. The dispenser's empty-bucket behavior
    (`DispenseItemBehavior`, `Items.BUCKET`) only picks up from `BucketPickup` blocks: liquid
    blocks, powder snow, bubble columns. Cauldrons are not one. So a lava farm needs a player with a
    bucket, which fails "fully automatic".
- **Not farmable automatically:**
  - Logs and charcoal: a tree farm needs a player's axe, or TNT, which eats gunpowder.
  - Blaze rods and coal: mob farms, which this judge cannot judge.
  - Saplings and sticks: they come from leaves and dead bushes, nothing that regrows on its own.

## Trade-offs

- **Fuel per plant against blocks per plant:** kelp makes 2 x the fuel of bamboo per plant (9.0 vs
  4.4 items/h), but needs water and a drying line.
- **Processing against blocks:** the planks crafters (bamboo) and the smoker + crafter line (kelp)
  are a fixed cost of about 10-20 blocks. One line serves a whole farm (a smoker dries 720 kelp an
  hour, about 97 plants), so it costs well under 1 block per plant at full size.
- **The drying fuel:** fed back from its own blocks, the farm loses 45% of each block's burn time.
  Fed with bamboo instead, each block needs 36 bamboo, about 2 stalk-hours, and that costs more
  blocks than it saves.
- **Materials:** hoppers, pistons, observers, a smoker, a crafter (iron, redstone). All survival;
  the water is placed with buckets.
- **Robustness:** every crop item and every intermediate item (dried kelp) must reach a container.
  The loop that feeds fuel back must not starve the smoker or flood it.

## Layouts

Estimates at vanilla speed for 64 plants. "Ceiling" is the crop's growth limit per plant; a plant
makes 4.39 items/h as raw bamboo, 5.86 as planks, 9.02 as kelp blocks.

| | layout | blocks / plant | items smelted /h per plant | items smelted /h per block | % of growth ceiling | constraints |
|---|---|---|---|---|---|---|
| A | **obvious: bamboo rows into the fuel chest** (bamboo-hc-12b tiled 3 apart, observer + piston per stalk, two hopper lines to one chest; raw bamboo) | 17.0 | 4.32 | **0.254 (measured, 64 stalks, robust)** | 98% | an off-centre hopper minecart per stalk (rail trick, then a push) |
| B | A + a planks line (crafter: 9 bamboo -> block; crafter with one slot -> 2 planks) | 17.5 | 5.6 | ~0.32 | 96% | two crafters, a comparator pulse each |
| C | dense bamboo rows: stalks side by side, one dust line under the observers fires every piston (no memory, so early cuts lose nothing), mud-over-hopper floor, raw or planks | ~6.5 | 4.2 / 5.6 | ~0.65 / ~0.86 | 96% | items from the third block scatter onto stumps (w86's bamboo4-qc lost 1-2 in 9-11 of 16 builds) |
| D | **kelp rows -> smoker -> crafter**: kelp on a hopper line, a piston above each growth cell pushing down, an observer on each kelp. The smoker dries, the crafter makes blocks, and a split feeds blocks back to the smoker's fuel. | ~5.5 + ~20 for the line | 9.0 | **~1.5-1.6** | ~100% | water refill of the growth cells; the fuel loop; the judge must count fuel |
| E | lava: dripstone under a lava source over a cauldron | ~4 | 309 | ~77 | 100% | **ruled out**: no automatic way to empty a cauldron |

**Sizing for one 8-furnace super smelter at full speed (2880 items/h):**

| | plants needed | hours for a double chest at 64 plants |
|---|---|---|
| A (raw bamboo) | 666 stalks | 12.5 |
| B (planks) | 514 stalks | 9.6 |
| D (kelp blocks) | 319 kelp plants | 6.0 |

## The ceiling

- **Per plant:**
  - raw bamboo 17.6 x 0.25 = 4.39 items/h
  - planks 5.86 items/h
  - kelp (self-fed drying) 7.38 x 1.222 = 9.02 items/h
- **Per block, soil only:**
  - bamboo: 1 block per stalk, so 4.39 raw, 5.86 planks
  - kelp: 2 blocks per plant (soil + the water it grows into), so 4.5
- **Per block, least machine** (an observer, a piston and a collector per plant): bamboo 4 blocks,
  so 1.10 raw and 1.47 planks; kelp 5 blocks, so 1.80.
- Lava would be about 77 per block, if it could be automated.

## The obvious design, measured (step 2)

`circuit farm-gen --layout bamboo-rows --n 64` writes `designs/circuits/farm/tracks/w86/fuel/
bamboo-rows64.txt`. It is the robust archived single-stalk harvester bamboo-hc-12b, tiled 3 apart,
with two hopper lines into one chest.

- The 4-stalk version passes in the game and on the twin at rts 300: 16.83 bamboo/h per stalk,
  0.975 bamboo/h per block, so 0.244 items smelted per hour per block.
- **64 stalks, `circuit verify`: robust** (16 game builds). 1089 blocks, 1106.8 bamboo/h on the
  vanilla twin (17.29 per stalk, 98% of the growth ceiling), 3269 iron.
  - That is **276.7 items smelted an hour = 0.254 items/h per block**: the bar.
  - It would take 666 stalks (about 11,300 blocks) to keep the 8-furnace smelter at full speed.
  - At 64 stalks, a double chest's fuel takes 12.5 hours.

## Decision

Build **D, the kelp line**:

- **Fuel per plant:** about 2 x bamboo (dried kelp blocks are the densest fuel a farm can make
  automatically).
- **Blocks per plant:** about the same, because a kelp on a hopper is soil and collector in one
  block.
- **Overall:** about 6 x the bar per block.
- **The smoker:** half the furnaces of a furnace line for the same fuel per kelp.
- **The order:** first one row of 8 kelp with the line, then 64 plants. B is the fallback if the
  line cannot be made robust.

## The judge (needs approval: a new kind)

`fuel-farm CROP`, built on the harvester judge (same runs, same robustness builds, same vanilla twin
run for the rates). Two changes:

1. **The number** is the burn time of the fuel delivered into the fuel chest: every item's
   `cooking_fuel` burn time, read from the game's data, / 200 per hour at vanilla speed, per block.
   Fuel still in the design's containers at the end counts, as the farm judge's backlog does. The
   design's own furnace, its fuel slot included, is part of the farm: a block that goes back into
   it is fuel the farm used, not fuel it made.
2. **Nothing lost:** a crop-derived item (kelp, dried kelp, a block) must never be left loose or
   despawn. Exact item accounting through a furnace and a crafter is not needed for that. The
   judge fails any such item still loose at the end, and any seen with an age over 5900 gt
   (about to despawn).

The twin already models furnaces drying kelp (200 gt), crafters (clk-misc's `redstone/crafting.rs`,
with the game's recipes) and the dried kelp block's 4001 gt, but not smokers or bamboo as fuel. So
D's first version uses **furnaces**, which cost the same fuel per kelp: a furnace dries 360 kelp
an hour, so 64 plants need 2. That makes D's vanilla run measurable now. A design the twin does not
model keeps the game runs' rates, as now.

## Must-haves

- Fully automatic into the smelter's fuel chest.
- Survival materials (carts placed with the rail trick).
- No crop item or fuel lost, ITEM INTRUSION on.
- Rests when growth stops, or noted as clocked.
- Extendable: more rows feed the same line.

## Not goals

- A fixed fuel mix.
- Fuel delivered within a fixed time.
- Chest capacity.
- Tiling the processing line (one line serves a full farm).
