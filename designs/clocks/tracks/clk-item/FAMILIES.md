# clk-item: extendable item-clock families

Each family = one base design + one repeated unit; the generator writes any member
(`mctas clock item gen <family> ... --out f.txt`) and `mctas clock item gen-check <family> --max M`
checks the formula against redsim for every member up to M. "Game" = members archived with
`clock archive` (real-game judge, 8 positions x 4 rotations, all robust).

## 1. Hopper ring (`gen loop --n N --m M`)
- Base: the hopper pair (2 hoppers facing each other, comparator reading one, its floor): 4 blocks.
- Unit: lengthen the ring by one column (2 hoppers): ring 2 x (N + 1), L = 2N + 2 hoppers.
- P = 14 (N + 1) / M gt with M items spread evenly (M divides L, M < L); blocks 2N + 4.
  One item: P = 14 + 14N.
- Rule: a hop through an empty hopper is 7 gt in any ticker order; the comparator sees the item
  pass its hopper once per turn (M items: every L/M hops).
- redsim: 34/34 members match (N <= 12, M <= 4). Game: N = 0 (hop2-p14), 1 (loop4-p28),
  2 (loop6-p42), and M = 2: loop6x2-p21, loop10x2-p35.

## 2. Dropper ring (`gen dropring --w W [--one] [--delays D1D2]`)
- Base: 3 x 3 ring of 8 containers, droppers in the bottom row at x = 1 (and x = 2 without
  --one), the comparator in the middle (on stone) reads the top-middle hopper and fires the
  dropper; repeaters (delays 1-4, each on stone) between the comparator and the dropper make the
  ring one row deeper each. The ring runs counter-clockwise; one stone waits in the fired dropper,
  the other starts in the top-right corner.
- Units: one column (W + 1: +2 containers) adds 14 gt; one repeater row adds 7 + 2 x delay gt.
- P = 20 + 14 (W - 3) + 7 (depth - 3) + 7 [one dropper] + 2 x (sum of delays) gt,
  depth = 3 + number of repeaters; blocks 2W + 4 depth - 8.
  e.g. one dropper, no repeater: P = 27 + 14 (W - 3), blocks 10 + 2 (W - 3).
- redsim: 110/112 members match (W <= 9, delays '', 1-4, 11, 14, 44); the 2 that differ are
  W = 3 with two droppers and delays '' or 1 (static with this start: use W >= 4 there).
- Game: the 76 droprep-p* and 8 dropringw-p* archived designs (other starts, same formula).
- Coverage: with W, depth (repeater count) and delays, every P >= 42 and most below.

## 3. Ring lock, item count (`gen ringlock --d D --n N`)
- Base: 3 x D hopper ring (L = 2D + 2), comparator in the inner column at row 1 (on stone) reads
  the west hopper and points into the east one (locks it while the west one holds items); N stone
  in the corner hopper.
- Unit: one more stone (blocks unchanged) adds 8 gt; one more ring row (+2 hoppers) adds 14 gt.
- P = 7L + 8N - 34 gt for 5 <= N <= floor((7L - 2) / 8) - 3; blocks L + 2. (N <= 4: P = 7L;
  above: saturates at the top value.) Every even P >= 154 across ring sizes.
- redsim: 257/257 members match (D <= 20). Game: ringlock3x6n5-p104, ringlock3x6n9-p136,
  ringlock3x20n33-p524.

## 4. Jukebox carousel (disc choice; JVM only)
- 9 blocks (jukebox9-p1446); 3 discs take turns: P = sum over the 3 discs of (20 len + 26) gt
  (len = song seconds: 11: 71, creator_music_box: 73, mellohi: 96, lava_chicken: 134, pigstep: 149,
  stal: 150, far: 174, tears: 175, creator: 176, 13/5: 178, cat/chirp: 185, strad: 188,
  otherside: 195, mall: 197, relic: 218, bounce: 234, wait: 238, ward: 251, precipice: 299,
  blocks: 345). 3 identical discs: one song (1446 ... 6926).
- Game: 1446 (3 x 11), 6926 (3 x blocks), 17978 (blocks + precipice + ward).

## 5. Powder snow dispenser (repeater delay; JVM only, kicked)
- 5 blocks: P = 12 (dust, snowdisp-p12) and P = 12 + 4d with a repeater of delay d = 1..3
  (snowdisp-r1-p16, -r2-p20, -r3-p24); d = 4 stops (the repeater swallows the next pulse).

## 6. Hopper pair + delayed lock (repeater delay)
- 9 blocks, 2 swords: P = 9 + 2d for d = 0 (stone, hop2lock-p9; 8 blocks), 1, 2, 3
  (hop2lockr1-p11, r2-p13, r3-p15); d = 4: 25 (the lock misses a move).
