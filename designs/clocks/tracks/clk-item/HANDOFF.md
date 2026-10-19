# clk-item handoff (item clocks)

Track `clk-item` (session 90b04d7f, box devvm55676, branch `explore/clk-item`). Board: Space memory
`minecraft-tas/clocks/families/item.md`. Every clock below passed `mctas clock verify` (real game, 8
positions x 4 rotations) and is archived in `designs/clocks/archive/` (`clock archive`).

## Infrastructure I added
- Design format `nbt: x y z {snbt}` (container contents, absolute coords like `port:`), applied after
  the raw paste like `data merge block` (bridge op `bedata`); `step` can record block-entity dumps
  (`record.block_entities`). redsim: `redstone/container.rs` parses SNBT Items + TransferCooldown.
- `crates/clock-item` = `mctas clock item ...`:
  - `run <design> [--waves]`: redsim cycle, visible vs whole-state period, family, per-cell waves.
  - `census` (k blocks in a box), `ccensus` (constructive: container cycles + comparator readers +
    extras), `fill <skeleton> --free box --k a..b --palette ...` (complete a skeleton; the workhorse:
    it found the delayed locks and the burnout hybrids).
  - `sweep <design> --at cells --counts a..b --items ... --cooldowns ...` (contents/cooldown sweep).
  - `sweep-loop`, `sweep-ringlock`, `sweep-dropring [--delays 1,2,..,14,..]` (parametric families,
    families.rs generators `hopper_loop`, `ring_lock`, `drop_ring_delayed`).
  - `promote <designs...> --name 'x-p{p}' --caption '...'` (clean name + caption + redsim facts).
  - `verify` (my own game check: offsets x rotations, redsim cross-check).

## Mechanics that matter (26.3, measured)
- Hopper: cooldown 8 after a move; an EMPTY receiver gets 8, minus 1 if the sender already ticked this
  tick -> a hop through empty hoppers is 7 gt in any ticker order; a non-empty receiver keeps its own
  cooldown (two hoppers that never empty push every 8 gt independently: analog P=8).
- Hopper eject + refill in one move reuse the first slot: hopper queues are not FIFO (limits the
  jukebox carousel to 3 discs).
- Lock = `!hasNeighborSignal`, flag 2. Comparator on a hopper: floor(14 fill)+1, 1-gt dips swallowed,
  but a compare-mode comparator updates the cell in front on EVERY scheduled tick (pokes burnt-out
  torches: the burnout hybrids).
- Dropper: rising edge (QC too) -> 4 gt -> 1 item; a strongly powered dropper fires its dropper
  neighbours. 2+ stacks -> random slot (avoid).
- Jukebox: 20*len + 20 gt, emits 15 all around while playing; torch on top is off while it plays.
  Dust floor 2-deep updates are position-dependent: relay through a note block.

## Families and formulas (block counts = whole structure incl. floors)
| family | blocks | periods |
|---|---|---|
| hopper pair slosh | 4 | 14 (1 item); 8 analog (3 swords, cooldowns 5 apart) |
| hopper rings, m items | L + 2 | 7L/m: 21, 28, 35, 42, 56 ... (every multiple of 7 >= 14) |
| pair + delayed lock (repeater d) | 9 (8 with stone) | 9, 11, 13, 15, 25 |
| ring lock 3x3, swords | 10 | 15 (rot-dep), 30, 46, 49, 56, 98, 106, 158, 218, 654 |
| ring lock 3xd, N stone | L + 2 | 7L + 8N - 34 (5 <= N <= ~7L/8 - 3): every even P >= 154 |
| dropper ring w x 3, 2 stones | L + 2 | 7L - 29 / 7L - 36: every P = 6 mod 7 from 20 |
| dropper ring + repeaters | L + 2 + 2/repeater | every P from 42 up (and 29, 31, 33, 35, 36, 38, 40) |
| burnout + hopper hybrids | 8-9 | 64, 65, 126, 129, 192 (rot), 195 (rot), 210 |
| jukebox carousel (3 discs) | 9 | 20*len + 26 per disc: 1446 ... 6926; 3 different: sum, up to 17978 |

Small periods still missing in my family: 10, 12, 16-19, 22-24, 26, 32, 37, 39 (and digital 8).
All redstone delays are even and hops are 7/8, so even P needs an 8-gt (non-empty receive) phase or
a dropper ring.

## Negative results (exhaustive searches)
- No item clock <= 3 blocks. Pure hopper/dropper/comparator/stone <= 5 blocks: only P=14.
- Cycles <= 4 containers + comparator + <= 2 extra blocks: only 14, 28.
- Vertical 3x3 dropper rings: none (suction drops items straight through).
- Classic shuttle hopper clock (2 sticky pistons + redstone block given): no completion with <= 3
  wiring blocks; latch hopper clocks (P ~ 16N) need >= ~12 blocks: not built yet.

## Next ideas
- Latch hopper clock (copper bulb T flip-flop toggled by "A empty or B empty" torches): P = 16N + c,
  N up to 320 per hopper, the real item-count long end.
- Crafter clocks (9 dust <-> block between two crafters; observers + relays, JVM only).
- Small even periods: dropper-hopper pairs with a self-trigger (fill k=4..5 on scratch/drophop.txt).
