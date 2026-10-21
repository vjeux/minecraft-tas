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
| **rand4-compact** (archive; tracks/clk-item/rand4-compact.txt, `circuit display rand-search --k 2 --bound 179 --all --wide` + `rand4 --pic`) | 4 | **43** | **7x5x4 (140)** | game robust, style hold, latency 8 gt; uniform: chi2 2.5 over 1536 presses in 16 seeded builds (374/387/367/408); trace-compare IDENTICAL (5081 frames) |
| rand4-cross (archive; tracks/clk-item/rand4-cross.txt) | 4 | 49 | 6x5x6 (180) | game robust, style hold, latency 8 gt; uniform: chi2 1.9 over 1536 presses in 16 seeded builds (395/371/400/370); trace-compare IDENTICAL (5081 frames) |
| rand4-plus (archive; tracks/clk-item/rand4-plus.txt, `circuit display rand4`) | 4 | 53 | 5x6x7 (210) | game robust, style hold, latency 8 gt; uniform: chi2 3.3 over 1536 presses in 16 seeded builds (414/375/367/380); trace-compare IDENTICAL (5081 frames) |

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
The layout search (`rand-search --k 3`) places no such output even once in the near dropper
arrangements: the three dusts around one comparator need their three readers on three sides,
within reach of three different pairs.

### The layout search (`circuit display rand-search`)

- Backtracking: outputs one by one. Each output's rear and side cells must be dusts of the right
  signals at the same depth: either all are readers' front dusts (2 or 1), or all are second dusts
  (1 or 0). Missing dusts become new taps on a free face of the right dropper, reading it
  directly or through a block. Every partial layout is checked: supports, no dust of one signal
  touching another's (diagonals included), nothing at a comparator's side, no dust pointing into
  a dropper or a through block, climbs with their corner free.
- The press network is routed at the end. Every output also needs a free straight line out of
  the box.
- `--all` keeps every layout within `--bound`. Build them with `rand4 --pic` and screen them on
  `circuit sim`.
- Quasi-connectivity hazard: a dropper also fires from power at the cell above it. Take a block
  that a dust powers (the dust's support) next to that cell: when it turns off, the update
  misses the dropper below, which is diagonal to it. The dropper stays triggered (a BUD) and
  never fires again, so its coin stops.
  - It hit r4-0: the support of the press dust climbing from A0's top dust sits next to A0's top
    cell. On redsim and in 12 of 16 game builds, coin A froze after the second press, and output
    a never lit.
  - It depends on the order in which the dusts turn off, which depends on position. rand4-compact
    has the same pattern, yet passes 64 game builds (16 positions x 4 rotations).
  - `--qc-safe` refuses every such layout. The search keeps them by default, and the redsim
    screen catches the ones that freeze.

## Eight outputs: status (clk-item, 2026-10-21, open)

No design yet. What was tried, and why it failed:
- **One stage, three pairs.** Each output's rear and both sides are threshold dusts of three
  pairs. `rand-search --k 3`, near and `--wide` arrangements: not even one output fits. Its three
  dusts need three readers on three sides, within reach of three different pairs.
- **One stage, two pairs, every output twice** (`rand-search --dup`). The other side of each
  output gets a sink for a third coin's bit, a repeater bit. Timing works: the repeater ticks
  before comparators in the same tick. But the eight outputs exceed what the four droppers' taps
  can reach: no layout.
- **Two stages, one global coin.** rand4's outputs are split by a coin routed to all eight
  copies. Dust runs from one coin to copies all around the build exceed 14 blocks, so the signal
  dies.
- **Two stages, a coin per side** (`rand8`, crates/circuit-display/src/rand8.rs). This is valid
  because the coin only has to be independent of stage 1: exactly one stage-1 output is lit.
  Stage 2 needs room in front of every output for:
  - a relay;
  - a hub;
  - two copies, each with a free line out;
  - side repeaters, the coin, and the routes.
  rand4's compact outputs leave too little room. `rand-search --stage2` reserves room in front;
  the copies still don't fit, and the backtracking over variants gets slow.
- Parked (coordinator 2026-10-21: N = 2 and N = 4 cover real use). The plan, when it resumes,
  is stage 2 outside rand4's box, one module per stage-1 output:
  - a comparator relays the output one cell out (over a plain block if its front cell has no
    footing);
  - a hub dust feeds the copies: one copy straight on, the other sideways along the outside of
    the box face, so both lines out are free;
  - each copy's side is fed by a chain of two comparators from a local coin's threshold dust
    (each output can have its own coin);
  - everything stays analog, and every copy's two inputs change at t+6.
  Expect about 150 blocks and a box of about 400. Alternatively, search stage 2 jointly with
  stage 1.
