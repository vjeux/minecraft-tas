# Shulker loaders and unloaders: the useful variations (study 8)

vjeux 2026-10-20: "find what are all the useful variations for builds (no internet lookup)". Each
variation below is derived from the game's code (26.3, `mctas src show`) and from what a build
needs. Each one says how the judge (`mctas circuit verify`, kinds `shulker-loader` and
`shulker-unloader`, docs/circuits.md) measures it. A design passes on the BASE scenario, the drop
stress, the FULL CHEST (2026-10-21, after a loader jammed with ~20 boxes in its chest: the
judge had only ever given 3 boxes and a spare, a player fills the chest): a chest full of boxes
(a loader: 27 empty boxes, 26 boxes' worth of unstackables; an unloader: 27 full boxes), every one
loaded or emptied and delivered, at every rotation; the ENDLESS SUPPLY (2026-10-21, after a
loader delivered nothing in the site viewer's run): that chest kept full, refilled before every gt,
as a hopper line from storage keeps it; and ONE BOX AT A TIME (2026-10-21): the boxes trickle in
one per judge visit, as a hopper feeding the chest brings them. Every other variation is a
separate scenario that gives the design a badge or a number.

## The mechanics that bound everything (read in the source)

* A shulker box holds 27 slots. Hoppers and droppers cannot put a shulker box into one
  (`ShulkerBoxBlockEntity.canPlaceItemThroughFace`). Every face accepts and gives every slot.
* Only a dispenser places a box (`ShulkerBoxDispenseBehavior`). It places into the cell in front
  if that cell is replaceable; item entities do not block the placement. A failed placement keeps
  the box in the dispenser. The box faces outward when the cell under the target is empty, else
  up. A dispenser fires 4 gt after a rising edge of power, its own or the block above it (QC).
* Only a piston breaks a box automatically: its push reaction is DESTROY. `dropResources` pops ONE
  item, the same block with its contents, colour and name (`getDrops` + the box's components), at
  the box's cell centre +-0.25. It moves up 0.2 and sideways up to +-0.1 per tick, at random.
  The box item stacks to 1, so drops never merge. An item entity left 6000 gt despawns: lost.
* A comparator reads a box as floor(14 f) + 1, f = sum(count / maxStack) / 27, so it reads 15
  only when all 27 slots hold FULL stacks. Fullness is relative to each item's max stack, so the
  same full detector works for 64-, 16- and 1-stack items. 14 -> 15 happens on the box's last item.
* A hopper moves 1 item per 8 gt, i.e. 2.5 items/s, into the container it faces. It pulls from the
  container above it, or from item entities in the 1x1 column from 11/16 above it to 2 blocks up,
  unless powered (locked). A hopper UNDER a box empties it, so a loader cannot keep one there
  unlocked. A hopper minecart pulls from the container above it every gt (20 items/s). A dropper
  inserts 1 item per firing into the container it faces (5 items/s on a 4 gt clock); it picks a
  random slot, so the order of mixed items is random.
* So one box fills in 1728 / rate seconds for 64-stack items: 691 s from one hopper, 173 s for
  16-stack items, 10.8 s for unstackables. A box has 6 faces: one goes to the dispenser, one is
  where the piston acts, and the drop needs a way out. That leaves about 3 feeding faces per box,
  so more speed means more feeders per box or more boxes in parallel.
  An unloader can only extract downward (hopper 2.5/s, hopper minecart 20/s), so its speed comes
  from extractors under several boxes, or minecarts.

## The judge's interface (for every variation)

The design declares cells; the judge places a single CHEST at each one (not a design block, not
counted) and manages its contents between ticks:
* `port items[.k]:` loader: an item source the judge keeps topped up until the scenario's total is
  supplied. Unloader: an item sink the judge empties and counts.
* `port boxes[.k]:` loader: the empty-box supply. Unloader: the full-box supply.
* `port output[.k]:` loader: where full boxes must arrive. Unloader: where empty boxes must arrive.
  The judge empties it and records each box with its contents, colour and name.
* `port input: x y z lever` (optional): a start lever the judge turns on once and leaves on.
  Without it the design is always on: the containers are filled at placement.
* Several `.k` cells are allowed, so a fast design is never limited by the judge's chests.
Every scenario fails on: an item lost or duplicated, an item entity alive at the end, anything
escaping the watched box, or a final state that is not at rest (no block changes in its last
400 gt). Times are in gt.

## The variations

| # | variation | why a build wants it | judge scenario / measure | output |
|---|---|---|---|---|
| 1 | BASE: 64-stack items (cobblestone) | the common case | 3 boxes' worth (5184 items), 4 empty boxes; 3 full boxes must arrive. 4 positions x 4 rotations | pass + boxes/min, items/s, first-box latency, blocks, box |
| 2 | 16-stack items (snowballs, pearls, eggs) | ender-pearl and egg storage | the same with 3 x 432 snowballs | badge `16-stack` |
| 3 | unstackables (tools, armour) | gear storage, 27 per box | 3 x 27 iron shovels | badge `unstackable` |
| 4 | mixed items | farm or storage overflow packed as-is | 3 iron ingots once (a rare item: its slot stays partial), then 16 rounds of cobblestone, snowballs and dirt in full and partial stacks, 6 empty boxes; every item must end in a delivered box, every box but the last with 27 occupied slots, the last within 6000 gt of the source running dry | badge `mixed` |
| 5 | filtered (one item kind) | pack one item, pass the rest on | a `port reject:` chest; one box's worth of the item in full stacks with 8 dirt or 2 sticks after each; the box must hold the item alone, everything else must reach reject (the design's own filter items are left out of the inventory) | badge `filter` |
| 6 | speed | big farms (raw items/s) | items/s = the items in the delivered boxes over the gt from the start to the last box (hoppers buffer what leaves the source, so the source's outflow overstates a loader), boxes/min = that over the items per box; also the theoretical ceiling of its feed (feeders x their rate) | number, ranked |
| 7 | compactness | small builds | design blocks (judge chests excluded), volume, footprint, height | numbers |
| 8 | tileable side by side | one loader per item kind, in rows | `tile: dx dy dz [copies]` header: the judge builds 3 copies at that offset, each with its own chests (ports renamed `<port>.t<k>`), all running at once; each copy must deliver its own 3 boxes from its own items | badge `tileable(N)` (N = the step's length) |
| 9 | input forms | how items arrive in a real build | `items: chest` (default), `items: hopper` (a judge hopper pushes into the port cell at 2.5/s), `items: stream` (the judge spawns item entities in the port cell at 10/s, like a water stream) | badge per form |
| 10 | output forms | where boxes go next | `output: chest` (default); `output: stream`: box items arrive as entities in the port cell, and the judge counts and removes them there | badge per form |
| 11 | auto-restart, no interaction | unattended farms | the source pauses 3000 gt after 1.5 boxes, then resumes; all boxes must still arrive with no judge action. No `port input` lever = always on | badges `restart`, `always-on` |
| 12 | box-starved safe | the box supply runs out | 2 boxes for 3 boxes' worth: nothing may be lost while it waits. After 3000 gt the judge adds 2 boxes and the 3rd box must arrive | badge `box-starved` |
| 13 | colours and names kept | colour-coded storage | the box supply mixes colours (red, blue, plain) and one named box; every delivered box keeps its colour and name | badge `colours` |
| 14 | partial-box flush | the input stops for good | 2.5 boxes' worth: the half box must arrive within 6000 gt after the source runs dry (longer than `restart`'s 3000 gt pause: a design that waits in between gets both) | badge `flush` |
| 15 | tops up partial boxes | refill half-used boxes | the 3 supplied boxes already hold 10 stacks and a half, 1 item and 26 stacks (no spare: the dispenser places a random slot); they must leave full | badge `top-up` |
| 16 | lag-friendly | servers and big farms | at rest: the design's hoppers left unlocked (they look for items every gt); later: block state changes in one box cycle | `hoppers awake` |
| 17 | no floor drops | no items left lying around, nothing lost if it lags | item entities polled every gt in the unstackables scenario (the same 3 drops as any item): the longest one alive and whether one touched the ground | badge `drop-free` (picked up within 8 gt, never on the ground) |
| 18 | materials | survival cost | the bill of the design blocks: iron (hopper 5, piston 1, ...), quartz, redstone, and slime or honey if any | numbers |
| 19 | loader + unloader combo | one station for both jobs | `port input.mode: x y z lever`: judged as its kind with the lever off, and as the other kind (unstackables) with it on, on the same chests | badge `combo` |

Unloader variations (kind `shulker-unloader`):

| # | variation | judge scenario / measure | output |
|---|---|---|---|
| U1 | BASE: 3 full boxes of cobblestone | every item reaches `items`, 3 empty boxes reach `output`; 4 x 4 builds | pass + boxes/min, items/s, latency, blocks |
| U2 | any box | the supply mixes full 64-stack, full 16-stack, unstackable, mixed, half-full and already empty boxes; all must be emptied | badge `any-box` |
| U3 | speed | items/s = the steady flow into `items` (10% to 90% of the total); the ceiling of its extractors (hoppers 2.5/s, hopper minecarts 20/s) | number |
| U4 | output forms | `items: chest` (default), `items: hopper` (a judge hopper pulls from the port cell at 2.5/s: feeding a sorter), `items: stream` (item entities counted at the port) | badge per form |
| U5 | colours and names kept on the empties | as 13 | badge `colours` |
| U6 | tileable, compact, lag, drop-free, materials, auto-restart | as 7, 8, 11, 16, 17, 18 | as above |

## Order of work

1. The judge: the BASE scenarios first (1, U1), then the badges in the order of the tables.
2. Hand-built reference designs: the loader (dispenser places, hoppers fill, a comparator reads
   15, a piston breaks, a hopper collects) and its unloader mirror.
3. Per variation, the best design archived: the smallest, the fastest, and the best on each badge.
