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

## R7 (real input devices, on main 2026-10-20): EIGHT archived gates are now invalid

Re-judged every gate under R7, where the judge drives an input with a repeater pointing into the cell instead of placing a redstone block in it. These FAIL and must be replaced before any of them counts as a record:

| design | what the judge now measures |
|---|---|
| gate-xor-ring-14b | behaves as OR |
| gate-xnor-ring-16b | behaves as NOR |
| gate-and-5b | the output is false (truth table 0000) |
| gate-nimply-cmp-2b | the output is a (0011) |
| gate-nimply-ba-cmp-2b | the output is b (0101) |
| gate-nor-bump-4b | the output is true (1111) |
| gate-nor-torch-3b | the output is not steady at rest |
| gate-imply-torch-3b | the output is not steady at rest |

Survive R7, with their tile pitches measured: gate-or-2b (2), gate-not-torch-3b (3), gate-nand-torch-6b (4). Both delay cards survive too (pitch 1).

The cause, read from the game's source and confirmed in the game. A comparator's or repeater's SIDE input is `DiodeBlock.getAlternateSignal` -> `SignalGetter.getControlInputSignal`, which returns a value only for a redstone block, redstone dust, or a signal source (a diode). A plain block, however strongly powered, gives 0. A redstone block placed IN the input cell satisfied that; the R7 repeater driving the judge's smooth stone there does not. The same rule breaks the dust-ring XOR/XNOR, whose ends read power that a placed block supplied.

So every gate whose input feeds a comparator side, or a dust ring, needs the input cell to hold DUST that the design owns (counted, with its support stone). My first attempt at that for NIMPLY (comparator + dust at the side cell, 4 blocks) still measures `the output is a`, so the side still reads 0: the next step is to dump what the judge actually places at that cell (`MCTAS_INPUT_DEVICE=block` reproduces the old pass, so the difference is isolated) before rebuilding the family. That is the first task of the next round, ahead of XOR/XNOR, since a record that only exists with a block input is not a record.

- `instant` (latency 0 gt) means the output cell changes in the same tick as the input, which only dust-to-dust and the power-bump tricks do; everything through a torch or comparator costs 2 gt.
- A lever input powers its own block, so a lever-input gate can be one block smaller than the same gate driven by a signal. Two different variations, not one record.
- The judge demands the output drive something (`check::update_less`): a dust redirected without a power change does not count as an output.

## Settled facts

- `instant` (latency 0 gt) means the output cell changes in the same tick as the input, which only dust-to-dust and the power-bump tricks do; everything through a torch or comparator costs 2 gt.
- A comparator's or repeater's side input reads ONLY a redstone block, dust, or a diode (`getControlInputSignal`): a powered plain block gives 0. This is the R7 breakage above.
- A lever input powers its own block, so a lever-input gate can be one block smaller than the same gate driven by a signal. Two different variations, not one record.
- The judge demands the output drive something (`check::update_less`): a dust redirected without a power change does not count as an output.
