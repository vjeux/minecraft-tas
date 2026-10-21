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
| **rand4-plus** (archive; tracks/clk-item/rand4-plus.txt, `circuit display rand4`) | 4 | **53** | **5x6x7 (210)** | game robust, style hold, latency 8 gt; uniform: chi2 3.3 over 1536 presses in 16 seeded builds (414/375/367/380); trace-compare IDENTICAL (5081 frames) |

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

## Four outputs: two crossing pairs (clk-item, rand4-plus)

The output needs one pair's reading and the other pair's reading side by side, with both changing
in the same tick, or the output flickers. The obvious pieces do not fit in one layer:
- Two dusts after each reader (levels 1 or 0) feeding subtract comparators: a planar search found
  no layout (crates: none; the one-off searches are described here). Each reader's second dust
  sits an odd distance from its dropper, so one pair's dusts only ever meet the other pair's dust
  for ONE of its droppers. Two of the four outcomes cannot be formed.
- Comparators reading the droppers directly or through blocks: the same parity, no layout.
- Repeater bits routed anywhere: about 120 blocks.
The working layout is three layers:
- Coin A on the ground. Coin B two blocks up, crossing over it.
- Readers read through a block where parity needs it.
- Each pair's second dusts climb up or down one block to height 1.
- Each output is a subtract comparator at height 1: rear on one pair's dust, side on the other's
  (A0 - B0, B0 - A0, B0 - A1, A1 - B0). Only one dropper of coin B is read: the pair's other
  dropper is its complement.
- Every input comes through exactly one comparator from a dropper, so every output sees both
  inputs change in the same tick.
The press is one dust network on top of all four droppers. `circuit display rand4` builds the
design with a small router for it: the shortest dust path that touches no threshold dust, no
comparator's rear or side, and no through block.

Eight outputs, the same way: three pairs. A subtract comparator takes the rear and BOTH sides
(rear minus the larger side), so o_xyz = A_x - max(B_y', C_z') with all three inputs at depth 1.
