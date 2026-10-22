# Super smelter (8 and 16 furnaces): design doc

sorter, 2026-10-21, per [docs/process.md](../../docs/process.md). Every rule below was read in the
26.3 source (`mctas src show <Class>`, line numbers from that listing). Measured numbers: the
`chest` run of the smelter judge (`mctas circuit smelter-run --run chest`), game and redsim agree
to the tick on both obvious designs.

## Use

Dump a double chest of raw ore into one input chest, keep a fuel chest stocked with coal, come back
to a chest of ingots.

## The number

1. **Minutes to smelt a full double chest of raw iron** (54 stacks = 3456 items): from a fresh build
   with both chests loaded at tick 0 until the last ingot is in the output chest.
2. **Blocks placed** (the judge's three chests not counted).

Reported with: % of the ceiling, the spread (items smelted per furnace), coal held in the machine
at the end (coal the build swallows).

## Mechanics, from the code

**Furnace** (`AbstractFurnaceBlockEntity`)
- Faces: top -> input slot 0, sides -> fuel slot 1, bottom -> result slot 2 then fuel slot 1
  (`SLOTS_FOR_UP/DOWN/SIDES` 52-54, `getSlotsForFace` 293-300). The bottom only gives up fuel-slot
  items in `FURNACE_FUEL_BOTTOM_TAKEABLE` (empty buckets) (`canTakeItemThroughFace` 307-310).
- `canPlaceItem` 340-352: the input slot takes ANY item (a non-smeltable there blocks the furnace
  for good); the fuel slot only items with `COOKING_FUEL` (or one bucket); the result slot nothing.
  So a hopper facing a side can never jam a furnace with a wrong item, a hopper facing the top can.
- Cook time: `getTotalCookTime` 280-283 = ceil(recipe time / speed multiplier). Raw iron's smelting
  and blasting recipes are both 200 gt (`VanillaRecipeProvider` 2194, 2391); the multiplier is 2 in
  `FAST_FURNACE` blocks (smoker, blast furnace: `LootPredicates` 54, `ContextFloatProviders` 42-47).
  **Furnace: 200 gt = 6 items/min. Blast furnace: 100 gt = 12 items/min.**
- Fuel: coal burns 1600 gt, divided by 2 in fast furnaces (`ContextIntProviders` 74-97): **8 items
  per coal in both**. Coal block 16000 (80 items), lava bucket 20000 (100; leaves an empty bucket
  in the fuel slot, `consumeFuel` 226-240), dried kelp block 4001 (20), blaze rod 2400 (12).
- `serverTick` 148-224: a furnace lights only with an ingredient it can smelt and room in the result
  slot (`canBurn` 242-255: result stack < 64); once lit it burns the coal to the end whether or not
  more input comes (154-155): a furnace fed in trickles wastes coal; one with a steady supply wastes
  at most its last coal. A result slot at 64 stops it: the output hopper must keep up.
- `setItem` 328-338: a different item in slot 0 restarts the cook timer (mixed loads cost nothing
  extra per item, the timer starts on each new item anyway).
- Comparator: `AbstractContainerMenu.getRedstoneSignalFromContainer` 767-783 =
  floor(14 x fill) + 1, fill = mean over slots of count/max. A furnace reads 1 with 1-13 raw iron,
  2 from 14, 3 from 28 (one stack of the input slot is 1/3 of its fill).

**Hopper** (`HopperBlockEntity`)
- 5 slots (`HOPPER_CONTAINER_SIZE` 33), `MOVE_ITEM_SPEED = 8` (32).
- `tryMoveItems` 106-131: when off cooldown and enabled, it first pushes one item into the block it
  faces (`ejectItems` 143-172: slots in order, the first the target accepts), then pulls one from the
  container above (`suckInItems` 218-246: the source's slots for its bottom face in order; with no
  container above, item entities unless a full block covers it); any success = cooldown 8. **2.5
  items/s in and 2.5 out.**
- `tryMoveInItem` 314-345: an item landing in an EMPTY hopper sets that hopper's cooldown to 8 (7
  if it came from a hopper that already ticked): an item advances one hopper per 8 gt.
- A hopper below another hopper pulls from it; a hopper pulls only from the one block above it, so
  every furnace needs its own hopper above it and every hopper its own container above it.
- Locked by any neighbour signal (`HopperBlock.checkPoweredState` 118): locked, it neither pushes
  nor pulls, but other hoppers still push into it and pull from it.
- Hoppers read containers including chest halves as one 54-slot container
  (`getBlockContainer` 378-391 -> `ChestBlock.getContainer`) and container entities when no block
  container is there (`getEntityContainer` 393-398, a random one if several).

**Hopper minecart** (`MinecartHopper.tryConsumeItems` 97-102): pulls one item per tick from the
container above it, no cooldown: **20 items/s**; it never pushes, hoppers under it pull from it.

**Stacks**: raw iron, coal, iron ingots stack to 64; a double chest = 3456 raw iron = 432 coal.

## The ceiling

- Furnaces: N furnaces always busy from tick 0: 3456 / (6N) min = **72 min at 8, 36 min at 16**
  (blast furnaces: 36 and 18).
- Feeding: one input hopper moves 150 items/min, so the chest takes **23 min** to enter the machine
  at best: never the limit for furnaces up to 25; for 16 blast furnaces a double input chest (one
  hopper under each half) feeds 300/min.
- Output: one output line also carries 150/min: enough for 25 furnaces, not for 16 blast furnaces
  (192 ingots/min: two output lines).
- Fuel: 432 coal is all a double chest needs; anything the build keeps is coal the player must own
  beyond that.

## The obvious design, measured (gate G2)

[obvious-8.txt](../circuits/tracks/super-smelter/obvious-8.txt), obvious-16.txt (explore/sorter):
input chest over a hopper line two above the furnaces (each line hopper passes items on, the hopper
under it pulls into its furnace's top), fuel chest over a second line one above beside them (the
hopper under each feeds a furnace side), a hopper under each furnace into a bottom line to the
output chest. 6 blocks a furnace, no redstone. The judge as it was failed it (even-share and coal
accounting rules); with the `chest` run's must-haves it passes.

| design | blocks | minutes | % ceiling | per furnace | coal kept |
|---|---|---|---|---|---|
| obvious-8 | 48 | **87.1** | 83% | 522, 506, 489, 472, 453, 434, 414, 166 | 5632 |
| obvious-16 | 96 | **87.1** | 41% | the same 8, then **0** for furnaces 9-16 | 11264 |

Why: every position buffers 704 items (line hopper 320 + down hopper 320 + furnace input 64) before
anything passes it, so the first five positions swallow the whole chest while the rest wait; the
fuel line does the same with coal (704 a position: 5632 coal held for 8 furnaces, ~88 stacks the
player must own before the last furnace lights). **A 16-furnace hopper line is an 8-furnace one with
8 idle furnaces.** Quarters out at 28 / 46 / 67 / 87 min.

## Trade-offs

- Blocks vs minutes: 6 hoppers-and-furnace a position is the floor (each furnace needs a hopper on
  three faces, and each feeding hopper a container above it); every minute won costs blocks that cap
  or meter what a position takes.
- Minutes vs materials: hoppers are 5 iron each (obvious-8: 200 iron); a blast furnace adds 5 iron
  and 3 smooth stone but doubles the rate (raw metals and ores only, no sand, cobble or food).
- Fuel: stackable coal hoards in feeding hoppers (320 each); 4 filler items in each side hopper
  (the side never accepts them) cap it to 64 + 64 a furnace for free. Lava buckets stack to 1 but
  their empty buckets leave through the bottom into the output (fails "only ingots out").
- Clock vs rest: a metering clock runs forever (hoppers awake = lag, AUDIT's complaint about the
  rows); a comparator per furnace rests but costs more blocks.
- Robustness: the side hopper cannot jam a furnace; the top hopper can (any stray in the input
  chest). Entities (minecarts) bring chunk-border and timing risks a block design does not.

## Five layouts

| # | layout | est. blocks 8 / 16 | est. minutes 8 / 16 | % ceiling | constraints |
|---|---|---|---|---|---|
| A | **obvious** hopper lines (measured) | 48 / 96 | 87.1 / 87.1 | 83% / 41% | no redstone; swallows 704 coal a furnace |
| B | **metered line**: one slow clock (~180 gt) locks every down hopper and side hopper except one tick a period (block + dust beside each, 2 a hopper), side hoppers with 4 fillers | 85 / 167 | ~73 / ~37 | ~98% / ~97% | a clock runs forever; even shares of ore AND coal from the first pulse, coal kept <= 128 a furnace |
| C | **self-capping line**: each furnace's comparator (reads 2 from 14 items) locks its own down hopper; fillers in side hoppers | ~80-90 / ~160-180 | ~75 / ~42 | ~96% / ~85% | no clock, rests; coal still reaches furnace k only after 128(k-1) coal (51 s a position); dust of neighbouring positions must not touch |
| D | **blast furnaces** in A (or B) | 48 / 96 (B: 85 / 167) | ~45 / ~45 (B: ~37 / ~24) | ~80% / ~40% (B: ~97% / 75%, input-bound) | ore only; +40 / +80 iron |
| E | **chest-minecart shuttle** over the down hoppers instead of the input line (every down hopper pulls from the cart as it passes, so all fill at the same rate) | ~60 / ~110 | ~75 / ~40 | ~95% / ~90% | entities, powered rails and a reverser; fuel line as A (needs its own fix) |

Estimates: B from the clock period (each furnace gets an item and a coal per period; the input line
carries 16 items per 180 gt < 2.5/s); C from the coal arrival times (sum over furnaces of 6 x (T -
t_k) = 3456); D from A's buffering at twice the rate; E from equal pull rates while the cart passes.

## Decision

Build from **B, the metered line**, at 8 and 16 furnaces (and its blast-furnace variant as the ore
smelter): it is the only layout whose distribution is even for both ore and coal from the first
period on a fresh build, so it reaches the ceiling at 16 where A cannot use its last 8 furnaces at
all; its cost is about 4 blocks a furnace and a clock. C is the fallback if the reviewer rules the
forever-clock out (it rests, but loses ~6 min at 16 to coal arrival and costs as much). E is cheaper
in blocks but trades them for entity risk. Searches then cut B's blocks (shared lock lines, clock
size) while keeping its minutes.

Beats are claimed per furnace count against A: at 16, anything under 87.1 min (B at ~37 min, 167 b);
at 8, under 87.1 min (B ~73 min, 85 b). B-8 beats A-16 on both numbers.

## Must-haves (the `chest` run)

- Any load from 1 item to a double chest smelts completely.
- **Works with the coal the load needs plus a stack** (coordinator 2026-10-21): the fuel chest
  starts with 432 + 64 = 496 coal and is never refilled (most players own a few stacks, not the ~90
  a hoarding fuel line swallows). Coal left at the end is reported.
- The input and the fuel chest may be double chests (`port items.b:`, `port fuel.b:`; the judge
  places the two halves joined): a player dumping a double chest of ore needs one, and the hopper
  under each half can start its own line.
- Every raw iron becomes an ingot in the output chest; nothing else there, never coal.
- Never jams; nothing left in the machine but fuel; nothing on the floor.
- Input, fuel and output chests where a player reaches them.

## Not goals

- An even share between furnaces (only the minutes count).
- Exact coal accounting (coal left in the machine is reported, not failed, as long as the load
  smelts with need + a stack).
- Rest (reported; the AUDIT's lag concern is a trade-off, not a gate).

## Results (the `chest` run, need + a stack of coal; game = redsim, 16 builds each)

| design | furnaces | blocks | minutes | % ceiling | coal left | iron | rests |
|---|---|---|---|---|---|---|---|
| obvious-8 | 8 | 48 | never: 30% smelted | - | 366 | 200 | yes |
| obvious-16 | 16 | 96 | never: 30% smelted | - | 366 | 400 | yes |
| obvious-16d (double chests) | 16 | 96 | never: 26% smelted | - | 382 | 400 | yes |
| **bs8** | 8 | 108 | **72.2** | **100%** | 64 | 240 | yes |
| **bs16** | 16 | 221 | **36.2** | **100%** | 64 | 505 | yes |
| **bs8-blast** (blast furnaces) | 8 | 108 | **36.2** | **100%** of 36 | 64 | 280 | yes |
| **bs16-blast** | 16 | 221 | **18.2** | **99%** of 18 | 64 | 585 | yes |

The obvious rows fail on coal before ore: the first position's side hopper and fuel slot take 384
coal, the second the rest, and the other furnaces never light. With unlimited coal (the first bar)
they took 87.1 min, both sizes.

**What was built: B, self-clocked (no clock).** C was dropped: a furnace that pulls only what it
needs leaves the load queued in the line, and a queue in a hopper line drains through the furnaces
nearest its end only (the last furnaces finish long after the first idle). bs8: per furnace a
locked hopper under the line hopper (one raw iron per window) over an open hopper that drains it
into the furnace (a locked hopper cannot push either: without the open one the buffers stay locked
when the windows stop), a locked side hopper (one coal per window). The last line hopper faces a
comparator: when the line has filled up to it, a torch goes off for a few ticks and every locked
hopper acts once. No clock: with the chest empty nothing moves. bs16 = two bs8 modules back to back
under the double chests, each module's output line running into the output chest between them (16 blast furnaces make 192 ingots/min, more than the 150/min one hopper line carries: with one output line bs16-blast took 23.2 min).

**Mechanics found on the way** (HopperBlockEntity `tryMoveInItem`, measured): an item entering an
empty hopper leaves it 7 gt later, while the chest feeds the line every 8 gt, so one line hopper in
eight is empty at any tick and the empty one moves back a position per tick; the window must open
8 gt after the last hopper fills (comparator 2 + repeater 4 + torch 2), or the position whose hopper
is empty at that tick never gets an item (a hopper below ticks before the one above: the block map
is ordered x, y, z, so the top hopper takes the item that would have refilled the next hopper).

