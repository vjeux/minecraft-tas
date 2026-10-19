# clk-item handoff (item clocks)

Track `clk-item` (session 90b04d7f, box devvm55676, branch `explore/clk-item`). Board: Space memory
`minecraft-tas/clocks/families/item.md`. Every clock listed passed `mctas clock verify` (real game,
positions x 4 rotations) and is archived in `designs/clocks/archive/` by `clock archive` on my branch
(robust unless flagged). Extendable families are archived as cards: see FAMILIES.md.

## Infrastructure I added
- Design format `nbt: x y z {snbt}` (container contents and hopper TransferCooldown, absolute coords
  like `port:`), applied after the raw paste like `data merge block` (bridge op `bedata`); `step` can
  record block-entity dumps (`record.block_entities`). redsim `redstone/container.rs` parses SNBT
  Items + TransferCooldown for its item table (stone, redstone, snowball, ender_pearl,
  wooden_shovel, diamond_sword).
- `crates/clock-item` = `mctas clock item ...`:
  - `run <design> [--waves]`: redsim cycle, visible vs whole-state period, family, per-cell waves.
  - `census`, `ccensus`, `fill <skeleton> --free box --k a..b --palette ...` (completes a skeleton;
    found the delayed locks, self-locks and burnout hybrids; refuses a skeleton that does not place).
  - `sweep <design> --at cells --counts a..b --items ... --cooldowns ...` (contents sweep).
  - `sweep-loop`, `sweep-ringlock`, `sweep-dropring [--pairs --triples --delays=...]`.
  - `gen <family> ... --n N --out f` (loop, dropring, ringlock, ringlockd, snowdisp, lockpair),
    `gen-check`, `cards --check | --write DIR` (the extendable cards of extend.rs).
  - `promote <designs...> --name 'x-p{p}' --caption '...'`, `unarchive <files>`, `verify`.

## Mechanics that matter (26.3, measured)
- Hopper: cooldown 8 after a move; an EMPTY receiver gets 8 minus 1 if the sender already ticked
  this tick -> a hop through empty hoppers is 7 gt in any ticker order; a hopper that never empties
  pushes every 8 gt on its own phase (analog P=8 pair). A hopper under a container pulls at once.
- Hopper eject + refill reuse the first slot: hopper queues are LIFO when the upstream hopper ticks
  first (why the jukebox shift register fails and only 3 discs rotate in the carousel).
- Lock = `!hasNeighborSignal`, flag 2. Comparator on a hopper: floor(14 fill)+1; a compare-mode
  comparator updates its front cell on EVERY scheduled tick (pokes burnt-out torches).
- Dropper: rising edge (QC too) -> 4 gt -> 1 item; a strongly powered dropper fires its dropper
  neighbours; 2+ stacks -> random slot (avoid).
- Dispenser buckets: powder snow placed / picked up (no cauldron use in Java). A failing dispense
  spits the item (an entity): every fire must succeed.
- Jukebox: 20*len + 20 gt per song, 15 all around while playing; a torch on top is off meanwhile.
  Dust floor 2-deep updates are position-dependent: relay through a note block.

## Results (archived; r = rotation-dependent, p = position-dependent)
- Cards (FAMILIES.md): hopper rings (14 + 14n, 7 + 7n), ring locks (76 + 14n, 104 + 8n,
  300 + 8n), lock pair (11 + 2n), lock rings 2x2 (33 + 2n, 41 + 2n), powder snow dispenser
  (16 + 4n), 14 dropper-ring cards (every P from 42 to ~460, and 27, 31, 33-36, 38, 40), and the
  LATCH hopper clock (17 blocks, P = 16N - 2 for N = 2..320 stones: up to 5118 gt): a waxed copper
  bulb remembers the direction; comparators light torches when their hopper is empty; the torches
  (one under the bulb, one under a stone beside it) toggle the bulb; a comparator on the bulb locks
  A through a dust on the stone above A, whose wall torch locks B.
- Individual: 8 analog (pair + 3 swords, 4b), 8 DIGITAL (the same pair read by a subtract
  comparator against a constant 3 from a dead-end hopper, 7b), 9 (pair + lock, 8b), 12 (powder
  snow dispenser, 5b), 14 (hop2, 4b), 15/17/20 (self-locking pair, 9b), 18 (pair + lock, 1 stone),
  13 (double-torch locks x2), 16r/24 (digital 2x2 mixed-item rings, 9b), 25 (pair + lock delay 4),
  18/19/21/22/23/25/26/28/32 (dropper rings, 3 filled cells), 20/29 (dropper rings, other
  starts), 54/81 (3x3 dropper rings with swords), ring locks with sword placements (3x3: 15r, 30,
  46, 49, 56, 98, 106, 158, 218, 654; 3x4: 44, 52, 74, 76, 78, 148, 156, 228, 236; 3x5: 58, 84,
  90, 102r, 103r, 104, 107, 206r, 211r, 214, 275, 306r, 314, 414, 422, 1230r), burnout hybrids
  (64, 65, 84, 126, 129, 140, 210 robust; 192r, 195r, 384p, 389r, 882r, 1344r), jukebox
  carousels 1446, 6926, 17978 (9 blocks).
- Coverage of the item family: every P from 8 to ~460 except 10, plus the long ones above.

## Negative results (exhaustive searches)
- No item clock <= 3 blocks; pure hopper/dropper/comparator/stone <= 5 blocks: only 14.
- Cycles <= 4 containers + comparator + <= 2 extras: only 14, 28. Vertical dropper rings: none.
- Dropper-hopper pair: no self-trigger with <= 4 wiring blocks; 2x2 ring + dropper: none <= 4.
- Pair + lock fills k = 5 (stone, dust, repeater, torch; 1 stone / 2 swords / 3 swords): the
  periods above only (no 10; 37 and 39 came later from the same wiring on a 2x2 ring).
- Two-torch burnout loops: always position/rotation-dependent (12 of 12 judged).
- Classic shuttle hopper clock (two sticky pistons + redstone block given): no completion with <= 3
  wiring blocks; the bulb latch (clk-item-latch) does the same job in 17 blocks.

## Next ideas
- P10: not found. Every redstone delay is even and a hop is 7 (empty receiver) or 8 gt, so 10 needs
  an 8-gt hop + 2 gt per cycle; the analog pair + delayed lock variants re-synchronize to 8.
- Latch hopper clock: built (clk-item-latch, 17 blocks). Smaller layouts may exist (the OR of the
  two empty-torches and the lock routing cost 6 blocks).
- Crafter clocks: handed to clk-misc (session da291dc5).
