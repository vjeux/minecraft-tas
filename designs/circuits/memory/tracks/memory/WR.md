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

Measured in the game, 2026-10-20 — the naive 5-block version does NOT work. Two droppers facing each other (the rs-dropper structure) with N items in A, both fired by one dust line over their lids: `counter4-items`, 6 blocks, level 1 for every one of 13 pulses. The reason is that both droppers fire on the same pulse: while both hold items they each move one into the other, so the pile never walks and A never empties. A dropper pair only counts if exactly one of them is fired per pulse, which means the direction is extra state the item counts cannot supply: the counts alone cannot say which way the pile is moving (mid-phase both droppers hold items).

So the design needs a phase latch, and these three parts, all proven separately in our archive:
1. Two droppers facing each other, A preloaded with N items (the rs-dropper structure, 4 blocks).
2. Two comparators, one per dropper, each 0 exactly when its dropper is empty. "A empty" happens once every N pulses, "B empty" once every N pulses, half a period apart.
3. A phase latch toggled by those two empty signals, whose two outputs HOLD the dropper that must not fire. A held dropper is a dropper kept continuously powered: it sees no rising edge, so the shared pulse line does not fire it. That is the cheapest possible gate - no gate blocks at all, just a level joined onto the pulse dust.
The latch is either a copper bulb T flip-flop (bulb + comparator) toggled by "A empty OR B empty", or a cross-coupled torch pair (RS NOR) whose two torches ARE the two inverters the empty signals need anyway. The counter's output is free in both cases: the bulb's comparator (toggles once every N pulses) or either torch.

Expected size, constant for every N: about 20 blocks with the torch latch, about 30 with the bulb. That is the record for every N whose ring costs more (today: N >= 19), and it is the whole large-N range at once, since only the preloaded item count changes. Next step: build it, judge it at N = 19, 23, 25, 38, 39 and at N = 576 to show the ceiling, then stamp one tunable group so the catalog shows ONE card for the mechanism.

Geometry notes from the first build attempt, so the next one does not re-learn them:
- Vertical droppers (A above B, facing each other) leave four free horizontal faces each; the horizontal pair leaves only one per dropper, which is where the first attempt ran out of room.
- One dust cell can pulse both: dust on a stone that is adjacent to one dropper, with the other dropper adjacent to the dust itself.
- Routing a hold line past a comparator's SIDE suppresses that comparator (compare mode: side >= rear gives 0). Harmless for this circuit - the suppressed comparator is the one whose dropper is held - but it must be deliberate.
- A standing torch does not power the block it stands on, and dust does not power the block above it; dust DOES strongly power the block below it, and a torch strongly powers the block above it.

## Fold done 2026-10-20

Nine same-N counters that were strictly bigger than another entry with the same metrics line are out of the archive (`circuit archive --remove`): counter12-ring3-ring4 18b, counter16-ring4-ring4 20b, counter18-ring9-bulb1 28b, counter20-ring5-ring4 22b, counter2-ring 8b, counter4-ring9 9b, counter4-ring 10b, counter5-star 12b, counter9-ring9 25b. counter3-ring and counter3-corner are both 8 blocks, so they stay as equivalent layouts. counter 38 at 57 blocks beside counter 48 at 20 is not a mislabel: a composite N chains into small rings while a prime factor forces one long ring, so size tracks the factorisation of N, not N.
