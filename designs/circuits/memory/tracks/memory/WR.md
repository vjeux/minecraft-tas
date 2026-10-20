# WR bar — study 5, memory (latches, D/T flip-flops, shift, counters)

What a world-record entry means here, per variation: the record target, then what we have, then the gap. "Blocks" is always `mctas circuit verify`'s count. Judged in the game or it does not exist.

| variation | record target | ours today | gap |
|---|---|---|---|
| RS latch | 4 blocks (two droppers + comparator + one item) | rs-dropper 4b | at the bar; a 3-block RS is unlikely (a latch needs a readable state block + its reader) |
| D latch, enable high | 5 blocks | d-latch-high 5b (locked repeater + subtract comparator) | at the bar |
| T flip-flop | 3 blocks (copper bulb + comparator + its stone) | tff-bulb 3b | at the bar: the bulb IS the state, the comparator is the only reader |
| T flip-flop with reset | tff + one block that empties the state | tff-reset | measure; a reset that does not grow the cell is the bar |
| D flip-flop (edge) | 5-6 blocks | dff | measure against the latch pair |
| counter 2^K | 3K blocks (K bulb stages) | bulb chain, 3K | at the bar for K <= 5 |
| counter N, N small and composite | chained rings / ring + bulbs, 8-20 blocks | counter3-corner 8b, counter9-ring3-ring3 17b | at the bar |
| **counter N, N large or prime** | **CONSTANT size, independent of N** | ring per unit: counter 19 = 54b, counter 23 = 72b, counter 25 = 72b, counter 38 = 57b, counter 39 = 46b | **OPEN, the one real gap: see "Item counter" below** |
| tileable latch / TFF slice | a 1-wide slice that tiles at pitch 1 with no gap | none archived | OPEN |

## Item counter (critique C36): the mechanism, and what the game actually allows

The idea: hold exactly N items in a container, move one out per input pulse, and read EMPTY with a comparator. A comparator on a container is 0 only when the container is empty and >= 1 for any single item (one item in a 9-slot dropper gives level 1), so "empty" is exact for any N up to 9 x 64 = 576 per dropper. Size does not grow with N, so for every large N this beats a ring, which needs a dropper and a hopper per unit of N.

Measured in the game, 2026-10-20 — the naive 5-block version does NOT work, and the reason is quasi-connectivity. Two droppers facing each other (the rs-dropper structure) with N items in A, both fired by one dust line over their lids: `counter4-items`, 6 blocks, level 1 for every one of 13 pulses, i.e. the pile never walks. Both droppers fire on the same pulse, so while both hold items they each move one into the other and nothing changes. Probes isolated the rule, which decides every layout here:

- A dropper fires when the OR of (a signal on itself) and (a signal on the block ABOVE it) rises - `DispenserBlock.neighborChanged`: `hasNeighborSignal(pos) || hasNeighborSignal(pos.above())`. So a VERTICAL dropper pair cannot be driven separately at all: the lower dropper's quasi-connectivity cell IS the upper dropper, so any power on the upper one fires the lower one too. Measured: with A above B, A holding 2 items, a dust on A's lid, B unpowered and not even reachable by wire, A still never drains - B fires with it and hands the item straight back. Remove B (A facing air) and A drains in exactly 2 pulses, which is the control.
- Therefore the pair must be HORIZONTAL (A and B side by side): then their quasi-connectivity cells are the two separate lid cells.
- A dropper that is held continuously powered sees no rising edge and does not fire. That is the cheapest possible gate: no gate blocks, just a level joined onto the dropper or onto its lid cell's neighbourhood.
- A comparator's side input suppresses it (compare mode: side >= rear gives 0), so no hold line may run beside the comparator that reads the dropper draining in that phase.

So the design needs a phase latch, and these parts, all proven separately in our archive:
1. Two droppers facing each other HORIZONTALLY, A preloaded with N items (the rs-dropper structure, 4 blocks).
2. Two comparators, one per dropper, each 0 exactly when its dropper is empty. "A empty" happens once every N pulses, "B empty" once every N pulses, half a period apart.
3. A phase latch toggled by those two empty signals, whose two outputs HOLD the dropper that must not fire.
The latch is either a copper bulb T flip-flop (bulb + comparator) toggled by "A empty OR B empty", or a cross-coupled torch pair (RS NOR) whose two torches ARE the two inverters the empty signals need anyway. The counter's output is free in both cases: the bulb's comparator (toggles once every N pulses) or either torch.

Working layout rules for the next build (from the attempts, all verified individually):
- Pulse both droppers from one dust line on their lids, or from a dust line one level higher sitting on two stones that cap the droppers (that frees the lid level for routing).
- Hold a dropper either directly (a dust or torch adjacent to it, or a strongly powered block adjacent to it) or through its quasi-connectivity cell (anything powering a neighbour of its lid cell).
- Put the two comparators on opposite sides (west of A, east of B) so neither hold line has to pass a comparator's side.
- A standing torch does not power the block it stands on; dust does not power the block above it; dust DOES strongly power the block below it; a torch strongly powers the block above it. A torch under a hold block is the cheapest inverter, since it both inverts and strongly powers upward.
- The bulb must be declared `lit=false`: the judge's placement leaves B empty, so "B empty" is already high when the design settles and the bulb sees no rising edge then.

Expected size, constant for every N: about 20 blocks with the torch latch, about 30-45 with the bulb and its routing. That is the record for every N whose ring costs more (today: N >= 19), and it covers the whole large-N range at once, since only the preloaded item count changes.

## Fold done 2026-10-20

Nine same-N counters that were strictly bigger than another entry with the same metrics line are out of the archive (`circuit archive --remove`): counter12-ring3-ring4 18b, counter16-ring4-ring4 20b, counter18-ring9-bulb1 28b, counter20-ring5-ring4 22b, counter2-ring 8b, counter4-ring9 9b, counter4-ring 10b, counter5-star 12b, counter9-ring9 25b. counter3-ring and counter3-corner are both 8 blocks, so they stay as equivalent layouts. counter 38 at 57 blocks beside counter 48 at 20 is not a mislabel: a composite N chains into small rings while a prime factor forces one long ring, so size tracks the factorisation of N, not N.
