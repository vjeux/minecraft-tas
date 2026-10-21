# Auto-smelters (study 16): notes

Judge: `circuit: smelter N` ([crate::smelter](../../../../crates/circuit/src/smelter.rs), docs:
[docs/circuits.md](../../../../docs/circuits.md) "smelter N"). The player's use: dump items in the
input chest and coal in the fuel chest, collect the results from one chest; more furnaces should
mean faster smelting. The judge's load is 16 items a furnace + 2 of six smeltables (two non-stacking
iron shovels, oak logs that are also a fuel), coal for every item and two more a furnace.

## Records

| design | furnaces | blocks | box | items/min per furnace | why it is the floor |
|---|---|---|---|---|---|
| [smelter-1](smelter-1.txt) (archived) | 1 | 4 | 1x3x2 = 6 | 6.0 | a furnace takes items only from a hopper above it, fuel only from a hopper beside it, and gives results only to a hopper (or minecart) below it: 3 hoppers + the furnace, a column of 3 with one cell beside it |

## Findings

* **The naive array does not share** (calibration/naive-slice.txt, the game and redsim agree):
  a line hopper over each furnace's feeder hopper. A hopper that receives one item at a time is
  emptied by the hopper under it before it can pass the item on (the receiving hopper waits 8 gt,
  the one under it does not), so the first feeder takes EVERY item until it is full (5 kinds, 64
  each, plus the furnace's input slot), and the first fuel feeder takes EVERY coal (up to 5 stacks
  plus the fuel slot). With the judge's load the first furnace smelts 43 of 50 items, the second
  gets one shovel (the kind that did not fit) and no coal: it jams. With a big load of one kind the
  first furnace still takes 384 items before the second sees one.
* Sharing needs the feeders to stop taking: a lock (a comparator reading the furnace, a clock
  that unlocks the feeders briefly) or a moving container (a hopper minecart shuttling over the
  feeders, the classic super smelter). Coal is the hard part: one kind, so a feeder's capacity never
  caps it, and a furnace's comparator mixes its fuel slot with its input (a lock at "the furnace
  holds 14 items" deadlocks once 14 coal sit in the fuel slot).
* Every open hopper top over a furnace's input path fails item intrusion: 64 stray iron ingots
  (they do not smelt) end in the input slot and block the furnace for good. A slice covers its line
  hoppers with full blocks; the first slice's covers are where the input and fuel chests go (the
  judge lets those two chests replace a cover).

## Next ideas

* A tileable slice whose feeders are unlocked by a slow clock for a few gt every ~200 gt (one
  item and one coal per furnace per window, the furnace's own speed); each slice its own clock.
* A hopper-minecart distributor for fixed N (the shuttle's end blocks are not a slice).
* Lava buckets as fuel (stack of 1: a furnace's fuel slot and a filler-limited feeder hold one
  each, so the fuel spreads by itself), as a badge.
