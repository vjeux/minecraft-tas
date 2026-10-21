# Study 13, item sorters: world-record targets

The coordinator (2026-10-20 23:06Z, from the critique: "the most common survival redstone build,
missing from the site"): ONE world-best item sorter, the classic 1-wide tileable hopper/comparator
filter: one item kind per slice, overflow-safe, full hopper speed, no jams, works for stackables,
and says plainly what it does not handle. Judge: `mctas circuit verify` on the new kind `sorter`
(docs/circuits.md, "Item sorters"): items in, the filtered kind out at its output, everything else
on to the end of the line, every item counted.

## What a slice is
A row of slices sorts several kinds: an item line (hoppers each pointing into the next) runs across
them; under each slice's line hopper sits its FILTER hopper A, which holds the slice's kind and
filler items in all 5 slots, so it only takes more of that kind from the line; a comparator reads A;
when A holds one item more than its recipe, the signal crosses a threshold and unlocks the DRAIN
hopper B under A, which takes that one item down into the slice's chest and locks again. The judge
counts blocks per slice: the line hopper is part of the slice (tiling repeats it); the chests are the
judge's.

## Mechanics that set the bounds
- A hopper moves one item per 8 gt: the line carries 2.5 items/s, and B drains A at the same rate,
  so a slice keeps up with a line full of its own kind.
- A hopper pulls from the container above it; a locked hopper (powered) neither pulls nor pushes,
  but still RECEIVES pushed items. So A must not point into B (it would push its own recipe into the
  locked B): A points sideways into something that is not a container, and B pulls from A.
- A comparator reading a hopper outputs floor(1 + 14 x F), F = (sum over the 5 slots of count / max
  stack) / 5. Recipe 41 + 4 (41 of the kind in slot 1, one non-stacking filler in each other slot,
  64-stack items): signal 2 at rest, 3 with one more item, and 3 when slot 1 is full (64): the
  signal NEVER exceeds the trigger level, so an overflowing slice (its chest full: B full, A full)
  sends the same signal as a working one. Recipe 18 + 4: signal 1, 2, but 3 when A fills up.
  16-stack items: 10 + 4 (signal 2, 3, and 3 full).
- The threshold is distance: dust loses one level per block. With recipe 41 + 4 the lock must turn
  off at signal 3 and not at 2: three dust steps (or an equivalent). With recipe 18 + 4, two steps,
  but at pitch 1 the slices' dust touch: an overflowing slice (signal 3) reaches its neighbour's
  second dust at 1 and unlocks it, which then drains its own recipe. So 41 + 4 with three steps is
  the overflow-safe 1-wide recipe (or a threshold that cannot leak sideways).
- The lock: B must be locked at rest and unlocked on the trigger, so the threshold drives an
  inverter (a redstone torch next to B).
- Non-stackable items (tools, armour, potions): a comparator cannot tell one more of them in a
  full slot from none, and they never stack into A: a slice cannot sort them; they pass through.
  Items with other names or enchantments do not stack with the recipe: they pass through too.

## Targets
| variation | target | holder | status |
|---|---|---|---|
| blocks per slice (overflow-safe, tileable at pitch 1, full speed) | <= 9, then the census minimum | [sorter-classic](../../sorter/archive/sorter-classic.txt), 15 blocks | held; no 12-14 block design within 3 removals / 2 additions (below) |
| flow | 2.5 items/s through a slice (the line's own speed), the kind sorted at 2.5/s | sorter-classic: 2.50 items/s | held |
| overflow | a full chest: the slice keeps the line moving, the other items pass, no slice loses its recipe, and the slice resumes when its chest is emptied | sorter-classic: badge overflow | held |
| stack sizes | 64-stack kinds (recipe 41 + 4) and 16-stack kinds (10 + 4) | sorter-classic, and its snowball recipe (10 + 4 / 16 + 4): both pass with both badges in the game | held |
| not handled, said plainly | non-stackable items, items with other names or enchantments | in the caption | |

## The record (sorter-classic, 15 blocks, 2026-10-20)
- The drain hopper B is kept FULL (64 of the kind + 4 named fillers that differ from the filter's),
  and the filter A points straight down into it: while B is locked, A cannot push anything into
  it; when B unlocks it drops one item into the chest and takes one from A (pull) or A refills it
  (push). Nothing stays behind (a B that only pulls keeps the last item of each kind until the next
  one comes, and the judge counts that item as not delivered), and no extra hopper is needed.
- The lock: the comparator sits on a block that is also B's lock: a wall torch under that block
  powers it from below. The threshold: three dust (signal minus 2), the last on the block behind a
  1-tick repeater that powers the torch's block. Why the repeater: the chain starts at the
  comparator's output, whose support is the only cell above the torch's block where dust could
  sit, and dust that merely points at a block stops pointing at it once the slices stand side by
  side (each dust joins its neighbours' and a three-way dust has no fourth side); a repeater
  always takes the dust.
- Census (`mctas circuit sorter-census`, redsim, 2026-10-20): every design with up to 3 of its 11
  non-hopper, non-comparator blocks removed and fewer blocks added (stone, glass, dust, torches,
  wall torches, repeaters) in the box x 0, y -2..3, z -2..5: 6.3M candidates; up to 4 removed and 3
  added in x 0, y -1..3, z -1..4: 142.6M candidates. None passes the base, tiled and overflow
  runs. (The census finds the record again from a copy with one useless block added.)
- Calibration (designs/circuits/tracks/sorter/calibration/, game and redsim agree): the classic
  18 + 4 recipe with a 2-step threshold (leaky-18) passes the base and tiled runs but not
  `overflow` (the full slice's signal 3 unlocks its neighbour, which drains its recipe and then
  sorts cobblestone); an empty drain hopper (b-empty) and a recipe the threshold does not match
  (recipe-18) fail the base run.

## Open
- Fewer blocks: other core layouts the census keeps fixed (the comparator reading through a block,
  the drain hopper facing sideways into its chest so a torch fits under it, a lock device other
  than a torch).

## The judge (`circuit: sorter <item>`)
- Ports: `items` (the judge's source chest, above the slice's first line hopper), `pass` (the chest
  where the line ends), `output` (the slice's chest). Chests are the judge's, not counted.
- Base run: a stream of the kind mixed with other items (64-stack, 16-stack and non-stackable, full
  and partial stacks, runs of the kind back to back); every item of the kind must reach `output`,
  every other item `pass`, nothing else; the design's containers end as they started (the recipe
  intact); no item entity; rest. Metrics: blocks, items/s through the slice.
- Tileable: 3 slices side by side at the `tile:` step, each with its own kind (the judge rewrites
  the recipe's item), one stream of all three kinds and other items.
- Overflow: in the tiled run, the middle slice's chest starts full but one slot and gets 8 stacks
  of its kind: during the overflow nothing may reach a wrong chest and no slice may lose its
  recipe; then the judge empties that chest and the slice must sort again.
