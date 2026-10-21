# Study 18: crop farms (WR board)

An automatic harvester: the crop grows by itself, the farm breaks every grown block and sends the item
into a chest, forever, with no player. Kind `harvester CROP` (sugar cane so far).

## The judge (`crate::farm`, docs/circuits.md "harvester CROP")
- Ports: `port crop:` per plant (an air cell on the design's soil: the judge plants the crop there,
  age 0; not counted), `port output:` (the judge's chest, an air cell, not counted); `tile: dx dy dz`
  for copies side by side.
- In the game only (redsim has no random ticks): random_tick_speed 300 (100 times vanilla) for
  24000 gt, so about a hundred growths per plant; every crop column recorded every tick; halfway the
  judge drops foreign items onto every column (item intrusion). Then growth stops and the run rests.
- PASS: no plant lost, no cane at full height (3) longer than 40 gt, every harvested item in the
  judge's chests (none loose, despawned or kept), nothing else loose, rest. Robust = every position
  and rotation passes (the rate follows the random growth: the card shows the builds' mean).
- Metrics: blocks per cane (the main number), items per hour per cane at vanilla speed (one cane
  grows about 3.3 times an hour; a farm that harvests every growth delivers that), harvest height
  (2 = as soon as it grows), wait (the longest a grown block stood).

## Board (sugar cane)
| design | blocks per cane | box | tile step | items/h per cane | status |
|---|---|---|---|---|---|
| [cane-mc-8b](archive/cane-mc-8b.txt) | 8 | 4x3x1 | 5 0 0 | 2.90 | robust under ITEM INTRUSION (w86, 2026-10-21): the hopper minecart stands on the hopper, no rail |
| [cane-mc-9b](tracks/w86/cane-mc-9b.txt) | 9 | 4x3x1 | 5 0 0 | 2.90 | robust; the same with the cart on a rail |
| [cane-8b](tracks/w86/cane-8b.txt) | 8 | 4x4x1 | 5 0 0 | 2.90 | fails ITEM INTRUSION: 64 iron ingots on its hopper keep it busy, and a cane item arriving then can drift off |
| [cane-7b](tracks/w86/cane-7b.txt) | 7 | 4x3x1 | 5 0 0 | 2.89 | the same, and items dropped onto the cane get swept onto the hopper |

## Mechanics (w86, 2026-10-21)
- A cane needs sand or dirt below with water beside that block; a waterlogged block counts. A
  waterlogged slab leaks (its water flows out of its open sides); a waterlogged waxed copper grate
  (a full block) keeps its water in. Only the top block of a cane ages; at age 15 it grows a block
  and restarts at 0; a cane stops at 3.
- A piston is not powered through its front (an observer pushed into the cane by the piston behind
  it never fires it). A sticky piston given an observer's 2-gt pulse drops its block.
- The piston head shoves the dropped item along its push: a hopper right past the cane, at the push
  height, catches it (one block lower loses about a third). A hopper powered by a neighbour is locked
  for that pulse, and an item can then get away.
- An observer on the BOTTOM cane block fires at every age step (16 times per growth), so its piston
  fires often; the growth itself is harvested 2 gt later. Watching the second block directly needs a
  longer wire (no single block links an observer watching it to a piston pushing into it).
- Item intrusion (crate::intrusion, like every kind: a cobblestone, 64 iron ingots and a shovel on
  every open hopper intake and above every cane, at a quarter, half and three quarters of the run):
  a hopper waits 8 gt after each item it takes, so 64 ingots keep it busy for about 512 gt; a cane
  item that arrives then is not taken in flight and can drift off the hopper. A HOPPER MINECART in
  the cell the head shoves the item into takes items every tick with no wait (5 slots), and the
  hopper under its rail empties it into the chest: cane-mc-9b. The cart works standing on the hopper
  without a rail (the hopper pulls from a container entity in the cell above it): cane-mc-8b.
- The judge's rate is measured at random_tick_speed 300 and scaled: a design whose piston often sits
  in the cane's second block (the bottom block does not age then) reads lower than it would at
  vanilla speed, where every design that harvests every growth gives about 3.3 items per hour.
