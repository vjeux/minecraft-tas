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

## Next ideas

* 3 and 4 furnaces: the same clock, a 2x2 ring of hoppers over 3 or 4 feeders (no return hopper
  wasted), the furnaces' outer faces for the fuel feeders and the repeaters.
* A tileable slice: a 2-wide slice whose second column carries the cap (a comparator reading the
  furnace, threshold by one dust step) and an observer on the furnace that lets one coal in when it
  goes out; or slices with end caps (a hopper-minecart shuttle) if the judge's tiled run grows end
  pieces.
* Lava buckets as fuel (stack of 1: a furnace's fuel slot and a filler-limited feeder hold one each,
  so the fuel spreads by itself), as a badge.
