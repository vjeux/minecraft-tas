# Auto-smelters (study 17): notes

Judge: `circuit: smelter N` ([crate::smelter](../../../../crates/circuit/src/smelter.rs), docs:
[docs/circuits.md](../../../../docs/circuits.md) "smelter N"). The player's use: dump items in the
input chest and coal in the fuel chest, collect the results from one chest; more furnaces should
mean faster smelting. The judge's load is 16 items a furnace + 2 of six smeltables (two non-stacking
iron shovels, oak logs that are also a fuel), coal for every item and two more a furnace (at least a
stack). Every furnace must smelt at least two thirds of an even share; at most one coal a furnace
beyond the items' own may burn.

## Records

| design | furnaces | blocks | box | items/min per furnace | how |
|---|---|---|---|---|---|
| [smelter-1](../../smelter/archive/smelter-1.txt) | 1 | 4 | 1x3x2 = 6 | 6.0 | a furnace takes items only from a hopper above it, fuel only from a hopper beside it, and gives results only to a hopper (or minecart) below it: 3 hoppers + the furnace, a column of 3 with one cell beside it (the floor) |
| [smelter-clock-2](../../smelter/archive/smelter-clock-2.txt) | 2 | 31 | 3x5x6 = 90 | 5.3 (17 and 17) | an item clock (hopburn-p65: two hoppers passing two swords, its comparator on 63 of every 65 gt) powers both furnaces through a block and two repeaters; a powered furnace locks every hopper touching it (feeder, fuel feeder, output), so each furnace takes one item and one coal and gives one result per beat; the two hoppers over the furnaces pass the load back and forth, so both feeders always see items; the fuel hoppers do the same. 60 000 annealing moves (`circuit compact`) found nothing smaller that still shares the load |
| [smelter-clock-3](../../smelter/archive/smelter-clock-3.txt) | 3 | 49 | 4x5x6 = 120 | 5.4 (15-19) | the same clock and repeaters, a dust line carrying the beat to the third repeater, the load circling a ring of six hoppers (three over the feeders, three returning), the coal along a plain line |
| [smelter-clock-4](../../smelter/archive/smelter-clock-4.txt) | 4 | 63 | 5x5x6 = 150 | 4.6 (12-22) | the same with a ring of eight; the output hoppers, held still between beats, pass one result each beat, which is what slows it |

## Findings

* **The naive array does not share** (calibration/naive-slice.txt, the game and redsim agree):
  a line hopper over each furnace's feeder hopper. A hopper that receives one item at a time is
  emptied by the hopper under it before it can pass the item on (the receiving hopper waits 8 gt,
  the one under it does not), so the first feeder takes EVERY item until it is full (5 kinds, 64
  each, plus the furnace's input slot), and the first fuel feeder takes EVERY coal (up to 5 stacks
  plus the fuel slot). With the judge's load the first furnace smelts 43 of 50 items, the second
  gets one shovel (the kind that did not fit) and no coal: it jams.
* **Rationing alone does not share either** (calibration/plain-line.txt): with the clock but a
  plain line over the furnaces, the items run on to the line's last hopper between beats, so the
  last furnace smelts 26 of 34. The load must stay where every feeder sees it: the two hoppers
  facing each other (or a ring of hoppers for more furnaces).
* Every open hopper top over a furnace's input path fails item intrusion: 64 stray iron ingots
  (they do not smelt) end in the input slot and block the furnace for good. The clock's own
  hoppers are covered too (strays in them would stop the beat).
* A slice that tiles needs a per-furnace cap or ration that does not depend on how many slices
  follow: the clock-and-ring idea needs the ring's return path (no slice has one), a plain line
  piles the load at the far end, and the furnace's comparator mixes its fuel slot with its input
  (a cap at "the furnace holds 14 items" deadlocks once 14 coal sit in the fuel slot). In a
  one-wide row a furnace has two free faces (its fuel feeder takes one), so a comparator AND an
  observer on it do not fit; their outputs also point away from the fuel feeder.

## The tileable row (smelter-row)

| design | furnaces | blocks | rows of 4 / 8 / 16 | how |
|---|---|---|---|---|
| [smelter-row](../../smelter/archive/smelter-row.txt) | 2 a slice | 14 a furnace + 210 in the end pieces | spread 15-18 / 12-24 / 12-22 (game), 5.7 / 5.5 / 5.2 items/min per furnace | items in a ring through every slice (one furnace under each line), coal in a second ring; each top hopper fed by an intake hopper locked except 8 gt every 12 s, each side hopper locked except 8 gt every 32 s |

What it took (each step a row of 4, 8, 16 on redsim, then the game):
* A clock bus along the row decays (dust, 15 cells): one dust cell per slice reaches 15 slices,
  so 16 furnaces need 2 furnaces a slice. A repeater can't branch sideways and a westward signal
  can't stop after one slice in a period-1 tile, so one dust row per slice it is, strongly
  powering a block between the two furnaces' locks.
* A chain of delay-1 repeaters swallows a 2-gt window (each repeater checks its input again 2 gt
  later): the hopper clocks' 2-gt windows survive one boost repeater and the lock repeater only;
  the latch clock's 8-gt windows pass any number.
* Locking the furnace (all its hoppers) makes its top hopper push only in windows: the furnace
  waits up to a window between two items of different kinds and burns coal idle (the slower the
  clock, the more: a 12 s window burnt 7 coal for 34 items, 1 over the limit). An intake hopper
  above the top hopper, locked instead, rations the take while the top hopper feeds the furnace
  the moment it is free: no idle burn.
* Coal: the fuel chest feeds the ring 1 coal per 8 gt, so with one clock for items and coal the
  coal of each window lies under the first slices' side hoppers when the window opens, and each
  takes one: in a row of 16 the last furnaces get none (calibration/fast-fuel-row). Two slow
  clocks (one per ring line) for the side hoppers let the coal spread round the ring first.
* The latch clock's hopper under its wall torch is an open hopper top (strays reach it through
  the torch's cell): glass on the torch cell's free faces closes it (glass carries no power).
* The item clock's period moves the shares a lot (stone counts 9 and 13 failed the row of 4 at 10
  of an even 16.5; 15 passes every row): a design point, not a law; minimum blocks next.

## Next ideas

* Fewer blocks a furnace: the slice is 28 blocks for 2 furnaces (14 hoppers, 2 furnaces, the lock
  block, its dust, 2 repeaters and their supports, 2 coal-lock blocks and dust, 4 covers); compact
  on the slice with the end pieces kept.
* Smaller end pieces (210 blocks: three latch clocks, the ring turns, covers).
* Lava buckets as fuel (stack of 1: a furnace's fuel slot and a filler-limited feeder hold one each,
  so the fuel spreads by itself), as a badge.
