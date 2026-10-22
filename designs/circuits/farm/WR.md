# Study 18: crop farms (WR board)

An automatic harvester: the crop grows by itself, the farm breaks every grown block and sends the item
into a chest, forever, with no player. Kind `harvester CROP` (sugar cane so far).

## Full-size farms (64+ plants; the ranking since 2026-10-21)

Ranked by items per hour per block for the whole farm at vanilla speed (the redsim twin at rts 3
over 720,000 gt); every one is robust in 16 game builds. Design docs: designs/briefs/.

| crop | design | plants | blocks | items/h | items/h per block | bar (obvious design) | iron |
|---|---|---|---|---|---|---|---|
| melon | [melon-lattice8](archive/melon-lattice8.txt) | 64 stems | 800 | 3210 slices | **4.012** | melon-rows64 1.250 (3.2x) | 2187 |
| pumpkin | [pumpkin-lattice8](archive/pumpkin-lattice8.txt) | 64 stems | 800 | 663.5 | **0.829** | pumpkin-rows64 0.243 (3.4x) | 2187 |
| sugar cane | [cane-sweeper-10x32](archive/cane-sweeper-10x32.txt) (clk-rt) | 256 canes | 1091 | 800.1 | **0.733** | see farm-sugar-cane.md | 2232 |
| sugar cane | [cane-sweeper-10x24](archive/cane-sweeper-10x24.txt) (clk-rt) | 192 canes | 880 | 595.4 | **0.677** | see farm-sugar-cane.md | 1752 |
| sugar cane | [cane-lattice-16x16](archive/cane-lattice-16x16.txt) (clk-rt) | 204 canes | 1234 | 646 | **0.524** | see farm-sugar-cane.md | 1836 |

- **The stem lattice (w86, `mctas circuit farm-gen --crop melon --n 8`):** stems two blocks apart
  on wet farmland, with a fruit cell on mud between every two stems (four per stem, each shared)
  and wet farmland on every diagonal.
- **Why it is fast:** wet farmland around a stem makes it roll 1 in 4, the game's best with fruit
  sides, and with four free sides every roll grows a fruit.
- **The harvest:** an observer over each stem fires the four pistons over its cells; a hopper
  minecart under each cell's mud catches the squeezed drop; hopper lines carry it to one collector
  line and one chest (the farm makes about 1 item/s at the game's speed; a hopper line carries 2.5).
- **Needs** open sky or light 9 at the stems.
- **Tiles** every 17 blocks. A 12 x 12 square makes 7725 slices/h at 4.29 per block (measured on
  the twin).

The single-plant modules below stay as variants.

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
| [cane4-mc-21b](archive/cane4-mc-21b.txt) | 5.25 | 7x3x3 (4 canes) | 0 0 3 | 2.80 | robust (w86, 2026-10-21): one observer, stone and dust fire four pistons (one per dust arm); one water grate touches three soils; a cart on each hopper |
| [cane4-mc-23b](tracks/w86/cane4-mc-23b.txt) | 5.75 | 7x3x7 (4 canes) | 7 0 0 | 2.80 | robust; the four canes spread on the four arms, a grate each |
| [cane2-mc-13b](archive/cane2-mc-13b.txt) | 6.5 | 7x3x1 (2 canes) | 7 0 0 | 2.83 | robust (w86, 2026-10-21): one observer, wire and stone for two canes (the dust's two arms fire both pistons; the second cane waits for the first one's next growth step); a cart on each hopper |
| [cane-mc-8b](archive/cane-mc-8b.txt) | 8 | 4x3x1 | 5 0 0 | 2.90 | robust under ITEM INTRUSION (w86, 2026-10-21): the hopper minecart stands on the hopper, no rail |
| [cane-mc-9b](tracks/w86/cane-mc-9b.txt) | 9 | 4x3x1 | 5 0 0 | 2.90 | robust; the same with the cart on a rail |
| [cane-8b](tracks/w86/cane-8b.txt) | 8 | 4x4x1 | 5 0 0 | 2.90 | fails ITEM INTRUSION: 64 iron ingots on its hopper keep it busy, and a cane item arriving then can drift off |
| [cane-7b](tracks/w86/cane-7b.txt) | 7 | 4x3x1 | 5 0 0 | 2.89 | the same, and items dropped onto the cane get swept onto the hopper |

## Board (cactus)
| design | blocks per plant | box | tile step | items/h per plant | status |
|---|---|---|---|---|---|
| [cactus-wool-9b](archive/cactus-wool-9b.txt) | 9 | 3x5x3 | 9 0 0 | 2.9 | robust x3, tiled, ITEM INTRUSION on (w86, 2026-10-21) |

A piston beside the cactus's second cell (solid: a block growing there breaks itself a tick later), a
sculk sensor beside the piston (it hears the block break and powers the piston 30 ticks), a wool block
above that cell and a stone on each of its two other sides; the head sweeps the falling item into the
next cell, where a hopper minecart standing on a hopper takes it at once. Copies 9 apart: a sensor
hears every block break within 8 blocks.

## Board (pumpkin, melon)
| design | blocks per stem | box | tile step | items/h per stem | status |
|---|---|---|---|---|---|
| [melon-x](archive/melon-x.txt) | 24 | 4x6x3 | - | 60.2 (slices); 2.507 per block | robust, ITEM INTRUSION on (w86, 2026-10-21) |
| [pumpkin-x](archive/pumpkin-x.txt) | 24 | 4x6x3 | - | 11.9; 0.497 per block | robust, ITEM INTRUSION on (w86, 2026-10-21) |
| [pumpkin4-mc-16b](archive/pumpkin4-mc-16b.txt) | 4 | 3x6x3 (4 stems) | 3 0 0 | 0.57 | robust (32 builds), tiled, ITEM INTRUSION on (w86, 2026-10-21) |
| [melon4-mc-16b](archive/melon4-mc-16b.txt) | 4 | 3x6x3 (4 stems) | 3 0 0 | 2.8 (slices) | robust (32 builds), tiled, ITEM INTRUSION on (w86, 2026-10-21) |
| [pumpkin-mc-7b](archive/pumpkin-mc-7b.txt) | 7 | 2x6x1 | 0 0 2 | 0.98 | robust, tiled, ITEM INTRUSION on (w86, 2026-10-21) |
| [melon-mc-7b](archive/melon-mc-7b.txt) | 7 | 2x6x1 | 0 0 2 | 4.9 (slices) | robust, tiled, ITEM INTRUSION on (w86, 2026-10-21) |

The -x designs lead by efficiency (items per hour per block, 2026-10-21). A stem's roll is 1 in
(int)(25 / speed) + 1, where the speed is 1, plus 3 for wet farmland under it (1 if dry), plus 3/4
for each wet farmland around it (CropBlock.getGrowthSpeed). A lone stem on dry farmland rolls 1 in
13; on wet farmland with wet farmland on its 4 diagonals it rolls 1 in 4. Every roll after the 7
that grow the stem picks one of 4 sides, so 4 free fruit cells grow a fruit on every roll: 17 times
the lone 1-cell stem's fruits. The fruit cells stand on mud, not farmland: a piston head above
farmland turns it to dirt (a moving piston may stand there, a head may not), and an item resting on
dirt is out of reach of the cart under it. Farmland cannot have a block on top either, so the
waterlogged grate stands beside a fruit cell, within 4 of every farmland.

The 4-stem designs: four stems around one fruit cell (every fruit grows there), each stem with its own
farmland, observer and dust; the piston, the mud, the hopper minecart and the hopper are shared. Their
stems grow slower (a stem with another stem on a diagonal grows at half speed).

Both: an observer above the stem watches it (the stem turns to an attached stem when a fruit grows)
and fires into the dust on its top; the dust powers the cell above the piston, which stands above
the fruit's cell facing down (a piston is also powered through the cell above it), and the dust's
update reaches the piston. It breaks the fruit and presses the item onto the mud (dirt for the fruit,
lower than a full block); a hopper minecart under the mud reaches it, and the hopper under the cart
empties it into the chest. The stem's other sides have nothing below them, so the fruit grows only on
the mud. Dry farmland: the stem keeps it farmland, and the fruit grows at the dry rate.

## Board (kelp)
| design | blocks per stalk | box | tile step | items/h per stalk | status |
|---|---|---|---|---|---|
| [kelp-17b](archive/kelp-17b.txt) | 17 | 4x4x4 | 4 0 0 | 7.1 | robust, tiled, ITEM INTRUSION on (w86, 2026-10-21): kelp planted on a hopper; an observer on the bottom kelp (it turns from kelp to kelp_plant at each growth) fires a piston above the second block, which pushes it down: the item goes down through the kelp (no collision) to the hopper; a source beside the second block refills it, and a hopper under that source takes an item the head squeezes into it |
| [kelp-v3t](tracks/w86/kelp/kelp-v3t.txt) | 19 | 5x4x3 | 0 0 3 | 7.0 | robust, tiled: a piston beside the second block; water falling from a source above the collection cell pushes the item onto the hopper and refills the second block's cell |

## Board (bamboo)
| design | blocks per stalk | box | items/h per stalk | status |
|---|---|---|---|---|
| [bamboo-hc-12b](archive/bamboo-hc-12b.txt) | 12 | 3x6x4 | 16.8 | robust (32 builds), tiled 3 0 0, ITEM INTRUSION on (w86, 2026-10-21): observer on the third block, piston breaking the second; a hopper minecart on the mud inside the stalk takes whatever reaches the second block's cell; a second cart, half on a hopper beside the stalk at the third block's height, takes the third block's item as it appears |

## Mechanics (w86, 2026-10-21)

- EFFICIENCY ranks the farms (vjeux 2026-10-21): items per hour at vanilla speed ÷ blocks. Free
  growth per plant (items per hour): cane and cactus 3.3, bamboo 17.6, kelp 7.4, a stem 1.0 fruit
  for each free side it owns. The records harvest 85-100% of their plants' free growth: cane 85-89%
  (the piston head in the cane's second cell after every age step wastes the random ticks that land
  then, since a cane ages only with air above it), bamboo 94%, kelp 93%, stems 87-100%, cactus 89%.
  The judge drains the collection after growth stops, so backed-up hoppers and carts at the judge's
  100x speed still deliver.
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
- Bamboo (`circuit: harvester bamboo`, judged the same way; no water): a stalk grows a block every
  3 random ticks on average (about 5 times faster than a cane), up to 16. An observer cannot watch
  the bottom block (it does not change when the stalk grows). Watching the second block, the piston
  head entering that block re-fires the observer: a clock (bamboo-8b, observer on top of the second
  block: restless, and growths destroyed in the tick they appear go uncounted). A piston pushing down
  into the second block flings the items away (bamboo-down-9b). Open: watch the third block and push
  the second (the third block's item then falls into the stalk's own cell).
- bamboo-shaft-14b/20b (track): bamboo on MUD (14/16 high, so the hopper under it takes items lying
  on it), an observer on the third block, the piston below it breaking the second block into a cart,
  the stalk walled in with a cap. It harvests every growth (16.5 items an hour per stalk at vanilla
  speed), but 13 of 16 builds lose 1-2 items: the third block's item can land on the stump's top;
  when the stalk regrows into that cell the item is inside the new block, and an item inside a block
  moves without collision toward open space, through the walls. A hopper minecart under the mud
  (bamboo-shaft-22b) still loses items in 10 of 16 builds: one ended at rest outside the shaft on a
  minecart's top, others flew off into the void.
- What `farm-run --trace` showed (one tick per batch, every crop item's last 40 moves): an item
  inside a block is pushed toward the nearest face whose neighbour is not a full block, never down;
  with every side walled it goes UP, through the cap and over the walls. Opening the third cell's
  north side over a cart fixed that (bamboo-shaft-13b, 1 of 16 builds lost an item). The rest: the
  third block's item flies about 10 ticks, and at random_tick_speed 300 the stalk sometimes regrows
  under it, so it lands on the second block; the next growth pushes it out, but when the run ends
  first it stays. A cart standing on the mud inside the stalk takes items up to y 2.875 (stump and
  second cell), not on the second block's top (bamboo-cc-12b, 1 of 16). A cart standing half over
  the third block's cell takes the third block's item before it flies (bamboo-hc-12b: robust).
- A hopper minecart can stand inside the bamboo (the stalk grows through entities), and a hopper
  under mud reaches a cart standing on it (mud is 14/16 high).
- Kelp (`circuit: harvester kelp`): the crop cell is a water source of the design. Kelp grows into
  water (still or flowing), about once every 100 ticks at random_tick_speed 300. When a block is
  harvested the kelp below becomes the top again with a random age, so ages cannot count growths.
  A piston breaking kelp leaves air: the cell must be refilled. With the refill water flowing out of
  the collection cell into the kelp's cell, its current carries items back into the kelp
  (kelp-v1: 13 of 16 builds lose one); with the refill source beside the kelp's cell, an item
  squeezed by the piston head escapes into it (kelp-v2: 1 of 16). Water falling from a source above
  the collection cell pushes items down onto its hopper and refills the kelp's cell from there
  (kelp-v3t: robust). A piston above the second block pushing down sends the item through the kelp
  (no collision) to a hopper the kelp is planted on; an item the head squeezes sideways goes into the
  refill source and floats up to its surface, inside the reach of a hopper under that source
  (kelp-17b: robust; without that hopper 1 of 16 builds lost one).
- Pumpkins and melons (the stem crops): a piston breaks the fruit (both are destroyed when pushed).
  An observer can only watch the stem (watching the fruit's cell, the piston head entering it
  re-fires it: a clock) and its back points away from the stem, so no piston beside the fruit's cell
  takes its output directly; a dust on an observer above the stem reaches one: the piston above the
  fruit's cell, powered through the cell above it, the dust's update (it updates every block within
  two) reaching it. Pushing down, the head squeezes the drop between itself and the mud: the item
  shoots toward the nearest open face of its cell and, near an edge, leaves before a hopper under the
  mud ticks again (pumpkin-6b, melon-6b: 12 of 16 builds lose one). A hopper minecart under the mud
  ticks with the entities, before the squeezed item moves, and takes it (pumpkin-mc-7b, melon-mc-7b).
  Copies 2 apart: side by side the dusts join into a line, which no longer powers the cell above the
  next piston. Four stems can share one fruit cell (pumpkin4-mc-16b): with one piston for four
  observers, a fruit may be broken in the very tick it grows (another stem's observer fires the
  piston), unseen by the stem's state: the judge counts every stem crop's drops (one stack a fruit).
- Cactus (26.3 source, CactusBlock): it grows on sand (a falling block: it needs a block under it); a
  block that grows next to a solid block (or with liquid above it) breaks itself a tick later and
  drops its item in that cell, and that also stops cactus flowers (a flower grows only where a cactus
  could survive). A cactus hurts every entity inside its cell (the whole cell, not its shape): a
  cactus item lying on the plant dies after 5 ticks, a minecart touching it breaks. Observers cannot
  watch it (an observer is solid: beside the plant it breaks it; watching the second cell, the piston
  head entering it re-fires the observer). A sculk sensor hears the block break and powers the piston
  beside that cell (also the block that breaks it), but: an item lying on a sensor triggers it
  (cactus-cap-6b: intruders on the sensor kept the piston out half the time), anything landing nearby
  starts its 40 ticks of being busy, and a growth in its last 10 ticks (the head already back) goes
  unswept: items landing on wool make no vibration (the wool cap); the head reaches the item 2 ticks
  after it appears, by when it may have drifted out of the cell's sides (the stones) or risen above the
  head (the cap); without the hopper minecart, a pushed item falls along the cactus and dies
  (cactus-wool-8b). Copies 3 apart hear each other (11 items lost); 9 apart they do not.
- The judge's rate is measured at random_tick_speed 300 and scaled: a design whose piston often sits
  in the cane's second block (the bottom block does not age then) reads lower than it would at
  vanilla speed, where every design that harvests every growth gives about 3.3 items per hour.
