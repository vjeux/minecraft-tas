# WR bar — study 4, logic gates

What a world-record entry means here, per function. "Blocks" is `mctas circuit verify`'s count; latency and the `instant` class are the judge's own metrics. Judged in the game or it does not exist.

The headline variation is the SIGNAL-input gate: both inputs are redstone signals from elsewhere in a machine, so the gate composes. A lever-input design is a different (easier) variation, because a lever powers its block from inside the gate; keep those, but list them after the signal ones and say so on the page.

| function | record target | ours today | gap |
|---|---|---|---|
| OR | 1-2 blocks, instant | gate-or-lever-1b, gate-or-2b (0 gt) | AT THE BAR: two dusts meeting is the whole gate |
| NOT / NOR | 2-3 blocks | lever 2b, torch 3b, bump 4b (0 gt) | at the bar; a torch IS the inverter |
| NIMPLY(-BA) | 2 blocks | comparator 2b (2 gt), bump 4b (0 gt) | AT THE BAR: a subtract comparator is one block |
| IMPLY(-BA) | 3 blocks | torch 3b (2 gt, glitches 2), bump 4b (0 gt) | at the bar, but the 3b one GLITCHES twice: a glitch-free 3-4 block IMPLY is open |
| AND | 3-4 blocks | gate-and-5b (4 gt) | OPEN: 5 blocks and 4 gt both look beatable (two comparators in series); try a subtract comparator fed by a torch, and an instant version |
| NAND | 4-5 blocks | torch 6b (2 gt), bump 8b (0 gt) | OPEN: 6 blocks for a NAND is a first implementation |
| XOR | 6-8 blocks | gate-xor-ring-14b (0 gt) | **OPEN, the main gap**: 14 blocks for an XOR is far off the bar. A comparator XOR is about 6-8 blocks |
| XNOR | 6-9 blocks | gate-xnor-ring-16b (2 gt) | **OPEN**: same, 16 blocks |
| tileable row (a bus) | a slice that repeats side by side with no gap, so an 8-bit bus is 8 x the slice | none measured | OPEN: builders put gates in rows per bit; `tile:` now works for study 4's kinds too if the judge is opened to them |

## Priorities

1. XOR and XNOR: the two real outliers. A census over a 4x2x4 box with comparators, k up to 9, is the direct attack (running).
2. AND and NAND: 5 and 6 blocks are first implementations.
3. A glitch-free IMPLY at 3-4 blocks (the archived 3b torch version reports `glitches 2`).
4. Tileable rows once a function is at its bar: measure the pitch with `tile:` rather than claiming it.

## Settled facts

- `instant` (latency 0 gt) means the output cell changes in the same tick as the input, which only dust-to-dust and the power-bump tricks do; everything through a torch or comparator costs 2 gt.
- A lever input powers its own block, so a lever-input gate can be one block smaller than the same gate driven by a signal. Two different variations, not one record.
- The judge demands the output drive something (`check::update_less`): a dust redirected without a power change does not count as an output.
