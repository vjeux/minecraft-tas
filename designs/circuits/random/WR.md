# Randomizers (study 16, `circuit: randomizer N`)

One press lights exactly one of N outputs, uniformly at random (coordinator 2026-10-21). The
judge: [docs/circuits.md](../../../docs/circuits.md) (randomizer N), [crate::random](../../../crates/circuit/src/random.rs).

The randomness is the game's level random: a dropper picks a uniformly random filled slot
(`DispenserBlockEntity.getRandomSlot`). Every judged build's sandbox is seeded from its box, and
redsim mirrors the level random draw for draw (`circuit trace-compare rand2-token`: IDENTICAL,
2561 frames), so the screen predicts the game's outcomes exactly.

## Board

| design | N | blocks | box (volume) | status |
|---|---|---|---|---|
| **rand2-token** (archive; tracks/clk-item/rand2-token.txt) | 2 | **19** | **10x3x1 (30)** | game robust, style hold, latency 8 gt; uniform: chi2 0.2 over 768 presses in 16 seeded builds (378/390) |

## The token pair (clk-item)

Two droppers face each other: one holds an iron shovel (the token), the other a cobblestone. A
press fires both in the same tick (a dust on the first powers it and points at the cell above the
second, which a dropper also fires from). Whichever fires first hands its item over; the other
then holds two items and fires one back at random. Either order leaves the shovel in either
dropper with chance exactly 1/2, whatever came before, and each dropper with one item again (no
hopper: no item intake to seal). The comparators fire 2 gt after both droppers, so they only see
the final state (no glitch). A comparator reading a dropper gives 2 with the shovel, 1 with the
cobblestone; two dusts bring that to 1 or 0 and the output repeater (the tap) is on while the
shovel is there.

Larger N: k independent pairs give k fair bits; an output for each combination is a subtract
comparator, rear = one pair's dropper reading (2 or 1), side = the other pair's OTHER dropper
reading at full strength (1 when that token is where the output wants it, 2 when not): on (1)
only when both tokens are where the output wants them.
