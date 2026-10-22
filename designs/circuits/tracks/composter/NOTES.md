# Auto-composters (study 22, track sorter)

Coordinator brief (2026-10-21): an item sink that turns farm output into bone meal: items from a
hopper line into composters, bone meal pulled out into a chest, non-compostables on to an
overflow chest instead of jamming; judge with a mixed crop supply (seeds, wheat, kelp, cane, plus
non-compostables), every compostable composted, all bone meal collected, nothing jams,
intrusion-safe, robust; metrics bone meal/h and blocks; minimum volume, then a tileable row.
The judge: docs/circuits.md "composter N" (crate circuit::composter).

## The model

`ComposterBlock` (26.3): `getContainer` gives an InputContainer below level 7 (one slot, from
ABOVE only, compostable items only: `canPlaceItemThroughFace` checks `direction == UP`), nothing
at 7, an OutputContainer with one bone meal at 8 (from BELOW only). An item put in composts at
once (`setItem` -> `setChanged` -> `addLayer`): the compostable's `layers` provider
(`context_int_provider/compostable/*.json`): level 0 always one layer, else a weighted list
`nextInt(100) < 30 | 50 | 65 | 85` from the LEVEL random (`LootContext` without a sequence).
Level 7 schedules a 20-gt tick to 8. Taking the bone meal empties it (level 0). redsim mirrors
the level random per build (the bridge seeds every sandbox from its watch box), so the compare
is tick-identical: calibration/naive (2241 frames, 5 bone meal) and compost-row's base run (a row
of 4, 5241 frames, 22 bone meal).

## Why a filter per crop

A hopper over a composter can only push into the composter, and it keeps every item the
composter refuses, so non-compostables must never reach it. Nothing in vanilla tells
"compostable" apart from the rest except the composter itself, and the composter only takes from
above (no two-exit hopper can use it as one of its exits). A hopper filter holding several crops
fails too: its drain always takes its FIRST slot, so that crop's anchor runs out after enough
arrivals of the others. So: the item sorter's filter, one crop a slice.

## compost-row (18 blocks a composter, 1x6x5, robust; archived)

storage-classic's slice (study 13) with the composter where its chest was: line hopper L on top,
filter A under it (41 of the crop + 4 named papers: only the crop enters), B under A kept full (64
+ 4 papers), unlocked through the comparator/torch line for a moment whenever A holds one more:
B drops one crop into the composter and A refills B. If the composter is at 7 or 8, B cannot push
and stays unlocked until it can. A hopper under the composter pulls the bone meal into the
slice's chest. A block on every line hopper (the input chest replaces copy 0's) keeps strays out:
without it the strays crowd the line (64 iron ingots on every top) and a crop can be pushed on
before its filter pulls it (calibration/open-tops: 10 wheat to the overflow at rotation 1 in the
game).

Throughput: the line's pace (2.5 items/s); a composter takes one item per hopper move, then
waits 20 gt at level 7 and for its bone meal hopper. Seeds and kelp make a bone meal per ~21
items, cane per ~13, wheat per ~10.

## Next

- Smaller slices: the filter's lock line is storage-classic's (5 deep); `circuit compact` now
  judges composters (composter::sim_score).
- A fixed one-composter build for all four crops needs four filters feeding one hopper over the
  composter: more blocks than four slices (one composter each).
