# Full-size melon and pumpkin farm (design doc, w86, 2026-10-21)

## Use

A player builds it once next to their base and leaves it running for food, compost and emerald
trades: every melon or pumpkin that grows ends up in a chest without anyone touching it.

## The number

**Items per hour per block for the whole farm, at vanilla speed** (random_tick_speed 3), with
**at least 64 stems**. Melon counts slices (what a player gets); pumpkin counts pumpkins. Second
number: % of the growth ceiling (below), so a small farm cannot look efficient by being tiny.

## The mechanics, from the 26.3 code

- **Random ticks.** `ServerLevel.tickChunk` picks `random_tick_speed` random blocks in each 16x16x16
  section every gt (`for (i < tickSpeed) getBlockRandomPos(...)`). So a block gets
  `rts / 4096` random ticks per gt: at vanilla rts 3 that is 72000 x 3 / 4096 = **52.7 random ticks
  per hour**. The judge runs at rts 300, so 100x faster.
- **Stem growth.** `StemBlock.randomTick` does nothing below raw brightness 9 (`getRawBrightness(pos,
  0) >= 9`: daylight or a light source). Otherwise it succeeds on `nextInt((int)(25 / f) + 1) == 0`.
  The first 7 successes age the stem 0 to 7. After that, each success picks ONE random horizontal
  side (`Direction.Plane.HORIZONTAL.getRandomDirection`). The fruit grows only if that side is air
  and the block under it is in `supports_stem_fruit`, which comes to `supports_vegetation`: dirt,
  coarse/rooted dirt, mud, muddy mangrove roots, moss, grass, podzol, mycelium, farmland. Then the
  stem becomes the attached stem facing it (`setBlockAndUpdate` on both).
- **Growth speed f.** `CropBlock.getGrowthSpeed`: f = 1 + (3 for wet farmland under the stem, 1 if
  dry) + 1/4 of that for each of the 8 blocks around the soil. f is halved when the same stem block
  sits on both axes or on any diagonal. Roll = 1 / ((int)(25 / f) + 1):
  - f 4 (wet soil only): 1 in 7
  - f 4.75: 1 in 6
  - f 5.5: 1 in 5
  - f 7 (4 wet diagonals): 1 in 4
  - f 10 (all 8 wet): 1 in 3
- **Farmland.** `FarmlandBlock.isNearWater`: any water fluid within 4 blocks horizontally and 0 to 1
  up keeps it wet (moisture 7). Waterlogged blocks count, so a waterlogged copper grate (a full
  block) waters a 9x9. `FarmlandBlock.canSurvive`: the block above must be non-solid or in the
  `maintains_farmland` tag (stems, attached stems, crops, moving_piston, fence gates). **Melon,
  pumpkin and piston_head are not in it**: a fruit, or a piston head resting on farmland, turns it to
  dirt (updateShape schedules a tick 1 gt later, then turnToBaseBlock). So fruit cells cannot be
  farmland, and f = 10 with fruit sides is impossible in steady state.
- **Attached stem.** `AttachedStemBlock.updateShape`: when its fruit goes, it turns back into the
  age-7 stem. While attached it does not grow, so a fruit left standing stops its stem.
- **Fruit drops.** Melon: `loot_table/blocks/melon.json`, 3 to 7 slices uniform, so 5 on average.
  Pumpkin: itself. Both are `PushReaction.POPPED` (`Blocks.MELON`/`PUMPKIN`): a piston moving into
  the cell pops the fruit as an item.
- **Observer.** `ObserverBlock.updateShape` starts a signal only when it is unpowered and has no
  tick scheduled. `tick` turns it on for 2 gt, 2 gt after the change. It is blind while powered.
  An observer watching the cell a piston head moves into is a clock: the head leaving re-fires it
  (tried in w86's farm notes). So an observer must watch the **stem**, which changes when it
  attaches.
- **Piston.** `PistonBaseBlock.getNeighborSignal` also reads power around the block above the
  piston (quasi-connectivity): a dust on an observer next to the cell above a piston powers it.
  The extension is a block event run the same gt. `PistonMovingBlockEntity` moves 0.5 per gt, so it
  takes 2 gt. A downward head squeezes the popped item against the soil, and it shoots out of the
  cell's nearest open face (seen in w86's stem notes).
- **Collection.** `Hopper.SUCK_AABB` (a 16-wide column from 11/16 to 2 blocks above the hopper's
  level). `HopperBlockEntity.suckInItems` (line 234): a grid-aligned hopper sucks nothing through a
  full collision block above it. Mud is 14/16 high, so a hopper does suck through mud, but slowly
  (8 gt cooldown), and a squeezed item leaves first. `MinecartHopper` (not grid aligned) sucks every
  gt through any block above it and catches the item the gt it is squeezed. Minecarts push entities
  within 0.2 (`OldMinecartBehavior`, `inflate(0.2, 0, 0.2)`), so two carts side by side push each
  other off their supports: carts need a block of space between them.

## Trade-offs

- **Rate per stem vs blocks:**
  - f (wet farmland around a stem) costs soil blocks.
  - Free fruit sides cost fruit cells, each with its own soil, piston and collector.
  - Sharing a fruit cell between two stems halves its cost and loses nothing: a fruit stands only
    the few gt before its piston pops it.
- **Robustness vs blocks:** a hopper under the mud is one block cheaper than a cart plus a hopper,
  but loses squeezed items. Carts must not touch each other.
- **Materials:** pistons (iron), observers (quartz), hoppers (iron), copper grates. All survival.
  The carts go in with the rail trick: place the rail, the cart, then break the rail, so carts must
  be block-centred.
- **Tileability:** a lattice tiles by sharing its rim. Rows tile side by side.
- **Throughput:** at vanilla speed, 64 stems make at most ~4200 slices/h, about 1.2 items/s. One
  hopper line (2.5 items/s) carries it.

## Layouts

Melon numbers per stem; pumpkin is a fifth of them. A stem makes 52.7 x roll x sides/4 fruits per
hour. Every layout below also needs a hopper line to one chest.

| | layout | blocks/stem | f, roll | fruit sides | slices/h per stem | slices/h per block | % of growth ceiling | constraints |
|---|---|---|---|---|---|---|---|---|
| A | **obvious double row** (the bar) | 8.1 | 4.75, 1/6 | 1 | 11.0 (measured 10.6) | **1.31 (measured)** | 16% | Stems must be 2 apart: side by side, a row's farmland is air-on-soil next to each stem and grows fruit nobody harvests. Grates every 8. |
| B | adjacent stems between two fruit rows | ~9.1 | 5.5, 1/5 | 2 | 26.4 | ~2.9 | 40% | Carts can't stand side by side, so hoppers sit under the mud and lose squeezed drops (or carts every other cell plus walls). The observers' dust joins into one line that fires many pistons (harmless). |
| C | **stem lattice**: stems 2 apart both ways, wet farmland on every diagonal, 4 fruit cells per stem each shared by 2 stems, observer + dust per stem, piston + cart + hopper per cell | 11.3 (8x8: 721 b) | 7, 1/4 | 4 | 65.9 | ~5.1-5.8 | 100% inside, 87.5% with the rim | Rim cells must go: a cell with one owner stalls when its stem re-attaches in its observer's pulse. |
| D | lone stems, 4 own cells (melon-x tiled) | 24 | 7, 1/4 | 4 | 65.9 | 2.5 (measured) | 100% | Robust today; doubles every cell. |
| E | soil-only lattice + one sweeping harvester (flying machine or piston wall) | ~4 soil + ~2 collectors + machine | 7, 1/4 | 4 | ~55-60 (fruit waits for the sweep) | ~8-9 (estimate) | ~85-90% | Moving machine timing; items squeezed onto the diagonal farmland; tiling and stopping. Untested. |

## The ceiling

- **Growth: 13.2 fruits/h per stem** (f 7, 1 in 4, every success grows; 52.7 / 4). f 10 (1 in 3)
  needs farmland on all 4 sides, and a fruit turns farmland to dirt. Fewer fruit sides at a higher
  f lose: 3 sides at f 7.75 give 1/4 x 3/4; 2 sides at f 8.5 give 1/3 x 1/2.
- **64 stems:** 844 fruits/h, so **4218 slices/h** (melon) and **844 pumpkins/h**.
- **Per block, hard bound** (soil only, lattice): 1 farmland, 1 diagonal (shared by 4) and 2 fruit
  soils (shared by 2) per stem, 4 blocks. That gives **16.5 slices/h per block, 3.3 pumpkins/h per
  block**.
- **Per block, least machine:** add 1 observer, 1 dust, 2 pistons and 2 collectors per stem
  (10 blocks): about 6.6 slices/h per block (1.3 pumpkins).

## The obvious design, measured (step 2)

It is `circuit farm-gen --layout rows --n 32` (designs/circuits/farm/tracks/w86/stem/
melon-rows64.txt and pumpkin-rows64.txt), one game run each, 24000 gt at rts 300, ITEM INTRUSION
on.

| crop | stems | blocks | items/h | items/h per block | % of growth ceiling |
|---|---|---|---|---|---|
| melon | 64 | 518 | 680 slices | **1.313** | 16% |
| pumpkin | 64 | 518 | 134.7 | **0.260** | 16% |

The redsim twin does not model this design yet. In its 720,000 gt run at rts 3 it grew 1,313 fruits
(93% of the expected 1,408), but 2,839 slices stayed in its hoppers and carts, which never happens at
vanilla speed, and 897 were lost. The game runs keep every slice. So until clk-misc fixes the twin,
the card takes the game builds' rates (`circuit verify` re-measures at rts 75). Either way, the bar
is about **1.1 to 1.3 slices/h per block** (melon) and **0.26 pumpkins/h per block**.

The judge first failed it, so I fixed the judge, as step 2 says. All three fixes are speed-up
artifacts, in crate::farm:

1. The judge's two chests filled (3456 slices). The judge now empties its chests every 500 gt and
   counts what it takes, as a player empties them; at 100x a chest fills 100x faster.
2. 16,900 slices were backed up in the hopper lines. At rts 300 they arrive at 19 items/s, while a
   line carries 2.5. Crop items still in the design's own hoppers and carts at the end now count as
   collected (a `backlog:` line). Items on the ground, or despawned, still fail.
3. `rts_for` (with clk-rt, whose 96-cane farm piles items on mud over full hoppers): a farm of more
   than 16 plants runs at 300 x 16 / plants (at least 30), 75 for 64 stems. It then makes items
   about as fast as a 16-plant farm at 300. Every archived farm has 16 plants or fewer and keeps
   rts 300.

## The judge for this topic (coordinator's decision, 2026-10-21 22:41)

- **Efficiency comes from a vanilla-speed run.** It is measured at random_tick_speed 3 on the
  redsim farm twin, in one long run (`VANILLA_RUN` = 720,000 gt, 10 game hours). That removes every
  100x artifact at once: the backlog, clocked harvesters, speed-up add-backs. melon-x: 13 s, 137
  fruits, 67.2 slices/h = 2.80 per block. melon-rows64: about 7 min.
- **A design the twin does not model, or whose twin run fails, keeps the game builds' rates**,
  with a note on the card.
- **Robustness stays on the game builds:** 16 builds, the tiled run, ITEM INTRUSION. They run at
  `rts_for` (300 up to 16 plants, then 300 x 16 / plants).
- **A missed harvest is a crop still standing at full height (a stem still attached) at the end of
  the run.** A wait during the run is a note: it costs rate, which the efficiency counts. This
  replaces "full for over 40 gt", which the 8x8 lattice's stem waiting for its neighbour's next
  harvest failed. At vanilla speed that wait is about 4 minutes, once in about 1,400 harvests.

## Decision

Build from **C, the stem lattice**. Every interior stem is at the growth ceiling, and its fruit
cells cost half because two stems share each one. It is the best per block of the robust-able
layouts: ~4x the bar. A full 8x8 (721 blocks) has made 3116 slices/h = 4.32 per block in one run.
To do before it counts:

- **The stalled stem in that run:** one stem stayed attached over 40 gt. I'll trace it, then fix the
  layout, or show it is the judge's 100x speed. A stem waiting for its neighbour's next harvest
  costs rate only, which the number already counts.
- **A few slices resting on the diagonal farmland:** fix the layout.
- **Robustness:** 16 builds and tiled, judged on clk-misc's farm twin with game spot-checks.

E could beat C on blocks but has no robust collection yet. It is the follow-up if C stalls below
~5 per block.

## Must-haves

- At least 64 stems.
- Fully automatic: every fruit popped and in a chest. No item lost under ITEM INTRUSION.
- Survival materials only: carts block-centred (rail trick), no command blocks.
- Tileable side by side.
- Rests when growth stops (no clock).
- The page says it needs daylight or light 9 at the stems.

## Not goals

- Chest capacity, and hopper throughput above the farm's vanilla rate (see the fixes above).
- Per-stem fairness.
- Harvest within a fixed number of gt: the rate already prices any wait.
- Night operation.
