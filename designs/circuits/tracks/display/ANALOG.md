# clk-obs8: the analog decoders (study 15, 2026-10-21)

`mctas circuit display analog-stack [--down [--w 8,2,4,1] [--slack N]] --y0 N --out F`
(crates/circuit-display/src/agen.rs, astack.rs, analog.rs), then `mctas circuit display settle`
(it now keeps comparator outputs as `OutputSignal` nbt, so the stack is at rest the moment it is
placed). analog-v5 = `analog-stack --down --y0 1` then `settle`.

| design | blocks | box | latency | glitches | game |
|---|---|---|---|---|---|
| analog-v2 | 632 | 9x16x9 (1296) | 86 gt | 76 | robust |
| analog-v3 | 565 | 7x16x9 (1008) | 36 gt | 48 | position-dependent (46 in one build) |
| analog-v4 | 565 | 7x16x8 (896) | 40 gt | 64 | position-dependent (61 in one build) |
| **analog-v5** (w [8,2,4,1]) | **566** | **7x16x7 (784)** | 34 gt | 64 | position-dependent (63 in one build) |

How it works:
- The value as one signal strength: s = 15 - (8 !a0 + a1 + 4 a2 + 2 a3) (code w [8,1,4,2] neg 0001,
  17 inner windows, every segment <= 4 runs). A chain of 4 subtract comparators from a full hopper;
  a complemented bit's weight is a comparator with a composter rear (w) and the bit on its side, a
  plain bit's is a comparator with the bit as rear and a hopper-reading comparator (15 - w) on its side.
- One cross ring per segment, stacked every 2 levels: the lamp in the middle is lit through 4
  stones, each driven by a window comparator W at distance 2. W: rear a hopper (constant k), sides
  an s dust contact (distance d2) and an offset comparator O (rear a hopper C, side an s contact at
  d1, out C - s + d1). W is on iff s in [C + d1 - k + 1, k + d2 - 1] (needs d1 <= lo - 1). Top runs
  drop W's s contact, bottom runs drop O.
- Each ring's s network: radius-3 dust arcs, joined under the rear hoppers by comparators reading
  the support block of one arc end and powering the other's (lossless), plus optional copies under
  the corner hoppers (arc cell -> W contact). Those support-level comparators also power the dust of
  the ring below (a strongly powered block powers the dust under it): the solver goes top down and
  feeds each ring with what the ring above couples into it.
- v2: a lossless riser (comparator -> block -> dust on top, 2 cells per level) spirals on the
  radius-4 square from the chain at the bottom: 9x9 footprint.
- v3/v4 (`--down`): no riser. The chain sits on top; its output block powers a dust, a short line of
  comparators/blocks at the level above the top ring ends in a block over a top-ring arc cell; s then
  flows DOWN the stack through the rings' bridges. v4's descent stands on top-ring arc cells (left
  out of that ring) and uses one row outside the 7x7.

Open: with w [8,1,4,2] a descent fully inside the 7x7 finds no assignment (the bottom ring runs out
of feeds); w [8,2,4,1] and [4,2,8,1] (neg 0001) do (v5). Glitches come from the unequal delays of W's
two sides (O adds 2 gt) and the bridge cascade (2 gt per ring).
