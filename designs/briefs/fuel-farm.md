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

Estimates at vanilla speed for 64 plants. "Ceiling" is the crop's growth limit per plant: a plant
makes 4.39 items/h as raw bamboo, 5.86 as planks, 9.02 as kelp blocks. Bamboo and kelp both grow
with **no memory** (a fixed chance per random tick), so a harvester that cuts on a clock loses
nothing by cutting early. A clock only loses growth while a plant is blocked: kelp cannot grow into
the piston above it; bamboo is never blocked.

| | layout | blocks / plant (breakdown) | items smelted /h per plant | items smelted /h per block | % of growth ceiling | constraints |
|---|---|---|---|---|---|---|
| A0 | the archived module tiled (bamboo-hc-12b 3 apart, two hopper lines; the first bar, dropped) | 17.0 | 4.32 | 0.254 (measured, robust) | 98% | an off-centre cart per stalk |
| A | **obvious: a bamboo line, observer + piston per stalk over a hopper line** (`farm-gen --layout bamboo-line`) | 13.4: mud, hopper, piston, the stone under it, observer, dust + stone behind it, 3 glass in front with a stone under them, 2 glass roof | 4.37 | **0.326** (measured, 64 stalks, robust) | 99% | the walls are needed: without them the piston squeezes items against the front and they shoot up and out (measured: 20% lost) |
| B | A + a planks line (crafter: 9 bamboo -> block; crafter with one slot -> 2 planks) | 12.2 | 5.8 | ~0.48 | 99% | two crafters, a comparator pulse each |
| C1 | **clocked bamboo carpet** (reviewer 1a): rows of stalks on mud over hoppers, a piston row beside each stalk row, one clock firing every piston through quasi-connectivity (a stone + dust line above the pistons) | ~5.5: mud, hopper, piston, stone + dust; each piston row is the next stalk row's wall; outer walls | 4.3 | ~0.78 raw / ~1.04 planks | 98% | squeezed items shoot up about 1.2 blocks (measured) and must not reach the dust line; items left on stumps after the last pulse |
| C2 | **flying-machine sweeper** (reviewer 1b, like clk-rt's cane-sweeper): a two-way machine at the 2nd block's height runs the field, its blades break every 2nd block | ~3: mud + hopper floor (2), the machine, stations, walls and catchers shared (~1, as in cane-sweeper-10x32: 4.26 b/cane with water) | 4.3 | **~1.4 raw / ~1.9 planks** | 98% | bamboo stumps have a collision column (cane has none): items land on them, and the next pass pushes them off; the slime launches items along the track (walls and catchers); a round trip longer than the judge's 400 gt rest window ends the run with items still on stumps |
| D | kelp with observers -> furnaces -> crafter | ~6.8: hopper 1 (soil and collector), water in the growth cell 1, piston above 1, observer beside the kelp's base 1 (watching the growth cell would be a clock: the head enters it), wiring 2 (stone + dust from the observer's back to a block beside the piston), walls 0.5, line 0.3 | 9.0 | ~1.32 | ~100% | the drying loop; the start-up |
| D' | **clocked kelp carpet** -> furnaces -> crafter: no observers, one clock fires every piston above the growth cells | ~5.6: hopper 1, water 1, piston 1, stone + dust 2, walls and line 0.6 | 8.5 (a 1-minute clock: a kelp at 2 waits on average 0.5 min of its 8.1-min growth step, about 6%) | **~1.51** | 94% | the drying loop; the start-up |
| E | lava: dripstone under a lava source over a cauldron | ~4 | 309 | ~77 | 100% | **ruled out**: no automatic way to empty a cauldron |

**How a popped kelp reaches its hopper (D, D').** An item in water does not fall:
`ItemEntity.tick` calls `setUnderwaterMovement` (fluid height > 0.1), which adds 0.0005 a tick
upward up to 0.06, so a kelp item floats up. In kelp-17b (w86, robust) the piston above the growth
cell pushes down, so the moving head pushes the popped item down into the kelp's base cell. The
hopper under the kelp sucks that whole cell (`Hopper.SUCK_AABB`: from 11/16 to 2 blocks above the
hopper) before the item can float out. So the piston must be above the growth cell, pushing down,
and the observer (D) beside the base.

**Sizing for one 8-furnace super smelter at full speed (2880 items/h):**

| | plants needed | hours for a double chest at 64 plants |
|---|---|---|
| A (raw bamboo, measured rate 17.4/h per stalk) | 662 stalks | 12.4 |
| B, C2 with planks | 497 stalks | 9.3 |
| C2 raw | 670 stalks | 12.6 |
| D' (kelp blocks) | 339 kelp plants | 6.4 |

## The ceiling

- **Per plant:**
  - raw bamboo 17.6 x 0.25 = 4.39 items/h
  - planks 5.86 items/h
  - kelp (self-fed drying) 7.38 x 1.222 = 9.02 items/h
- **Per block, soil only:**
  - bamboo: 1 block per stalk, so 4.39 raw, 5.86 planks
  - kelp: 2 blocks per plant (soil + the water it grows into), so 4.5
- **Per block with a shared harvester:** a sweeper needs only the soil and the collector under it,
  2 blocks per stalk: 2.2 raw, 2.9 planks.
- Lava would be about 77 per block, if it could be automated.

## The obvious design, measured (step 2)

The reviewer's bar: `circuit farm-gen --layout bamboo-line --n 64` writes `designs/circuits/farm/
tracks/w86/fuel/bamboo-line64.txt`. It has a stalk per block in a row on mud over one hopper line,
a piston beside each stalk's 2nd block, and an observer above the piston watching the 3rd. The
observers' dust behind them is one line, so any stalk at 3 fires every piston.

- **Without walls it loses 20% of its items.** The head squeezes each popped item against what is
  in front of it, and the item shoots up about 1.2 blocks and out of the row (8 stalks, game run:
  868 of 4357 lost).
- **Glass in front, at the ends and over the stalks and observers keeps them all.** Glass lets the
  daylight through, and bamboo grows only at light 9. With it, 8 stalks pass a game run: 17.42
  bamboo/h per stalk, 15 blocks per stalk at that size (12 at 64).
- **64 stalks, `circuit verify`: robust (16 of 16 builds).** 858 blocks, 1119.2 bamboo/h on the
  vanilla twin (17.49 per stalk, 99% of the ceiling). That is **279.8 items smelted an hour = 0.326
  items/h per block**: the bar.
  - **A judge fix.** 9 of the 16 game builds first failed on one item left resting on a bamboo
    stump at the end. In a player's farm the stump's next growth pushes it off (an item inside a
    new block moves to the closest space, `ItemEntity.tick`), so the judge was wrong, and it got
    fixed (step 2). After the run, growth now resumes while a crop item lies on a crop, up to
    1600 gt at rts 300 (scaled to the run's speed), and an item still there then fails. The
    coordinator once approved this rule for bamboo (2026-10-21), then dropped it when
    bamboo-hc-12b's carts made it unneeded.
  - **A farm fix.** 2 rotation-2 builds then still missed 1-2 of about 4500 items. The judge's new
    `left:` note (crop items seen below or beside the build) found one: the head pushed it down
    along the front wall and it slipped out under the wall's lowest glass (the front column had
    no block at the mud's height). A stone row under the front wall closed it (+66 blocks: 858).
- **The first bar** (the archived module tiled, bamboo-rows64): 0.254 items/h per block, robust.

## Results (C2, the bamboo sweeper; 2026-10-22)

`circuit farm-gen --layout bamboo-sweeper --n W --len L` (`canefarm::bamboo_sweeper`) is clk-rt's
cane sweeper (cane-sweeper-10x32) over an all-mud bamboo field:

- **The field:** mud over a hopper under every cell, a stalk on every field cell but the one under
  the second half's observer (so the planting starts only the first half).
- **The machine:** the 6-block two-way docking machine with a wing of slime and stone blades on
  each half, sweeping at the 2nd block's height.
- **Walls:** up to y = 4 along both long sides instead of the cane farm's curb. A bamboo item
  pushed out of a block by the machine left sideways at 0.2 block/gt as high as y 3.2 (the
  `left:` note found it).
- **The width limit:** a field is at most 10 wide. Each push moves the half's slime, its observer,
  its wing (2 blocks per column) and the other half's piston, at most 12 blocks
  (`PistonStructureResolver`). 12- and 16-wide fields never start.

- **The station traps:** a stone over the catcher row in front of each station's hopper pair. An
  item that came in at y ~1 slipped under the upper hopper's bowl, where neither hopper takes it
  (the lower one pulls from the hopper above instead of sucking items), and despawned (the
  judge's despawn note found it, 10x32 at rts 9).
- **End walls up to y = 5:** one more row at both ends (24-28 blocks), against an item launched over
  the y = 4 row from on top of the machine.
- **Open at 10x64 (not archived):** its tiled run at rotation 3 loses one item in about 30,000.
  The `left:` note's trail shows the cause. An item riding on top of the machine is squeezed into
  the north end wall as the machine docks. An item inside a block moves to the closest free space
  (`ItemEntity.tick` -> `moveTowardsClosestSpace`), and here that is behind or over the 1-thick
  wall, so it falls out of the world.
  - Tried, did not stop it: a second wall layer at y = 1..4, and closing the station pockets to
    the floor (reverted).
  - **The fix (in the generator now):** a catcher behind each end wall. It is mud over hoppers
    one cell behind the wall, a hopper under the wall into the catcher row, and a back wall up to
    y = 3; a 1-high curb caught an expelled item on its top, where it despawned. With it, the
    failing tiled run (rotation 3, rts 4, 96,000 gt) passes: 2492 blocks, 1.11 items smelted/h per
    block.
  - The full archive run of 10x64 with the catchers (about 25 min) is the next step; it is past the
    time box. The archived 8x8 and 10x32 predate the catchers (they passed robust without them).
  - The 16 single builds of 10x64 pass, at 2360 blocks and about 1.17 items smelted/h per block.

All robust (16 game builds, the tiled copies too), with their rates from the vanilla twin run.
The 8x8 and 10x32 are archived (`designs/circuits/farm/archive/`); 10x64 is not (above).

| design | stalks | blocks | bamboo/h | items smelted /h | per block | x bar (0.326) | iron, slime | a double chest's fuel | fields for the full smelter |
|---|---|---|---|---|---|---|---|---|---|
| bamboo-sweeper8x8 (the 64-plant field) | 63 | 496 | 984.1 | 246.0 | **0.496** | 1.5 | 672, 74 | every 14.0 h | 11.7 |
| bamboo-sweeper10x32 | 319 | 1336 | 5429.7 | 1357.4 | **1.016** | 3.1 | 2232, 92 | every 2.5 h | 2.1 |
| bamboo-sweeper10x64 (not archived) | 639 | 2360 | ~11,080 | ~2770 | ~1.17 | ~3.6 | 2232+, 92 | every 1.25 h | 1.04 |

- **Card, 64-plant field:** "a 64-stalk field fuels a double chest of ore every 14 hours; 12 such
  fields keep the 8-furnace smelter at full speed".
- **Sizing:** at the measured 17.0 bamboo/h per stalk (4.26 items smelted), 677 stalks keep the
  smelter at full speed: one 10 x 68 field (one machine) or two 10 x 34.
- **Rows:** a field grows by rows of 10 along its length. Each row adds about 32 blocks (field, ring,
  walls) for 10 stalks, so the per-block number climbs toward ~1.3 as the field gets longer.
- **Start-up:** the first planting starts the machine (the stalk under its first observer). No fuel
  or item is needed, and it never stalls: it shuttles for as long as it stands (`rests: no`).
- **Planks:** not built (time box). A crafter line (9 bamboo -> block, block -> 2 planks) would add
  a third (x 1.33) for two crafters and their comparator pulses. It is the next step, with the
  start-up rule applying to its crafters too.

## Decision

Build **C2, the flying-machine bamboo sweeper**, raw bamboo first, then the planks line:

- **Per block:** its estimate (~1.4 raw) is within 7% of the best kelp layout (D', ~1.51), so it is
  within the coordinator's 15%. With planks (~1.9) it beats every kelp layout.
- **Simpler for a player:** it needs no water refill, no drying furnaces and no fuel loop, and it
  never stalls on start-up.
- **The risks**, each with its test on the first 2-row field:
  - **Items on stumps:** each pass pushes the previous pass's items off. The judge's 400 gt rest
    window must see a pass, so the round trip stays under 400 gt: a field at most 16 stalks long at
    10 gt per block (a longer field gets two machines).
  - **Slime launching items:** walls past the ends and a curb, as in cane-sweeper.
  - **A two-way machine 1 wide enough to fit the rows:** clk-rt's 6-block shuttle with wings.
- **The fallback** if C2 is not robust by 11:30 is D' (the clocked kelp carpet).

**After the build (measured).** C2 is robust, but it measured below its estimate: 1.02 items
smelted/h per block at 10x32 and ~1.17 at 10x64, against ~1.4 estimated. Its machine, stations,
ring and walls cost 3.7-4.2 blocks per stalk, not ~3. So the measured C2 is 23-33% under D''s
*estimate* (1.51), outside the coordinator's 15%. D' is unbuilt, though, and the same overheads
(ring, walls, the drying furnaces and crafter line) would cut its estimate too. A fair re-decision
needs D' built; that is the next step if the coordinator wants it. The items on stumps never needed
a short field: with the judge's regrowth rule and the rest rule's settled-items check, a 10x64 field
(a round trip of ~1400 gt) passes.

## Start-up, card and size (coordinator's additions, for whichever layout wins)

- **Start-up:** the farm must start from an empty build (the judge plants the crops) with at most
  one starter fuel item, and never stall.
  - C2 needs none: the machine starts when the first stalk under its station observer grows.
  - D' must feed a block back to its furnaces before it sends any to the smelter. The first 9 kelp
    dry on the starter coal (1600 gt dries 8: the 9th waits for the first block). The split sends
    blocks to the furnaces' fuel slots first and the rest to the fuel chest. The judge's run starts
    with that one item and fails a stall: a furnace with kelp but no fuel for over 1000 gt.
- **Card:**
  - items smelted per hour
  - per block
  - "N stalks keep an 8-furnace super smelter running"
  - "a double chest's fuel in X h at this size"
- **Size:** 64 plants first, extendable by rows. At the measured 17.4 bamboo/h per stalk the
  smelter needs 662 stalks raw (497 with planks), about 10 such 64-stalk fields. With D', 339 kelp
  is about 5 fields.

## The judge (needs approval: a new kind)

`fuel-farm CROP`, built on the harvester judge (same runs, same robustness builds, same vanilla twin
run for the rates). Three changes:

1. **The number** is the burn time of the fuel delivered into the fuel chest: every item's
   `cooking_fuel` burn time, read from the game's data, / 200 per hour at vanilla speed, per block.
   Fuel still in the design's containers at the end counts, as the farm judge's backlog does. The
   design's own furnace, its fuel slot included, is part of the farm: a block that goes back into
   it is fuel the farm used, not fuel it made.
   - For a bamboo farm (C2, raw), the harvester judge already counts it: the bamboo delivered x
     0.25. So C2 raw needs no new kind; the planks line does.
2. **Nothing lost:** a crop-derived item (kelp, dried kelp, a block) must never be left loose or
   despawn. Exact item accounting through a furnace and a crafter is not needed for that. The
   judge fails any such item still loose at the end, and any seen with an age over 5900 gt
   (about to despawn).
3. **Regrowth before the end check** (found with the obvious design, needed by every bamboo
   layout): a crop item resting on a living crop is freed by that crop's next growth. So after the
   run, growth resumes while one lies there, up to 1600 gt at rts 300 (scaled by 300 / rts), and an
   item still there then fails.
   - In the farm judge and its twin now (`farm::finish`, `sim_finish`, `REGROW`), with a
     `regrown:` line on the card.
   - It changes no archived verdict: no archived farm leaves an item on a crop.

The twin already models furnaces drying kelp (200 gt), crafters (clk-misc's `redstone/crafting.rs`,
with the game's recipes) and the dried kelp block's 4001 gt, but not smokers or bamboo as fuel. So
D's first version would use **furnaces**, which cost the same fuel per kelp: a furnace dries 360
kelp an hour, so 64 plants need 2. Bamboo items are modeled (the harvester twin runs bamboo farms).
A design the twin does not model keeps the game runs' rates, as now.

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
