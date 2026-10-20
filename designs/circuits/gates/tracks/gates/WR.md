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

## R7 (real input devices, on main 2026-10-20): FIFTEEN archived gates were invalid, and are out

Re-judged every gate under R7, where the judge drives an input with a repeater pointing into the cell instead of placing a redstone block in it. Fifteen of the twenty-three failed and are removed from the archive (a design that fails the judge cannot sit in it claiming a record):

| design | what the judge measured |
|---|---|
| gate-xor-ring-14b | behaves as OR |
| gate-xnor-ring-16b | behaves as NOR |
| gate-and-5b | always false (0000) |
| gate-nimply-cmp-2b / -ba-cmp-2b | passes a / passes b through |
| gate-nor-bump-4b, gate-not-bump-4b, gate-imply-bump-4b, gate-imply-ba-bump-4b, gate-nand-bump-8b | always true |
| gate-nimply-bump-4b / -ba-bump-4b | behave as OR |
| gate-nor-torch-3b, gate-imply-torch-3b, gate-imply-ba-torch-3b | the output is not steady at rest |

What survives, with the tile pitches I measured: gate-or-2b (pitch 2), gate-or-lever-1b, gate-not-torch-3b (3), gate-not-lever-2b, gate-nor-lever-2b, gate-nand-torch-6b (4), plus the two rebuilt below. Both delay cards survive too (pitch 1).

THE RULE, from the game's source and confirmed by the judge: a comparator's or repeater's SIDE input is `DiodeBlock.getAlternateSignal` -> `SignalGetter.getControlInputSignal`, which answers only for a redstone block, redstone dust or a diode. A plain block, however strongly powered, gives 0. Every removed design read its input through a plain block that a placed redstone block used to be: the whole "bump" family is that trick, and the dust rings read ring power the placed block supplied.

A design's own dust AT the input cell does not fix it (measured: the judge drives it correctly, `probe-side`, yet a comparator beside it still reads 0). What works is giving the design its own REPEATER at the input cell, whose output feeds a dust that feeds the comparator's side: the judge drives a receiving repeater at its back, and the dust is then a real signal source.

Rebuilt and archived that way: gate-nimply-r7-6b and gate-nimply-ba-r7-6b, 6 blocks, latency 4 gt, robust. The same shape rebuilds AND (two subtract comparators, the second's side fed by the first's dust) and NAND; XOR and XNOR need a different mechanism entirely, since the dust ring is gone.

## Priorities (rewritten after R7, updated as they are done)

Done: AND 7b (torch), NOR 3b, IMPLY and IMPLY-BA 4b, NIMPLY and NIMPLY-BA 6b, XOR 32b — all with real signal inputs, all judged robust.

1. XNOR: the XOR core plus one inverter. The torch must hang on a block the XOR's output dust powers, and in the current 5x5 footprint every neighbour of that block is a floor stone carrying a dust; run the output dust one cell out into free space first, then invert there.
2. Shrink the XOR below 32 blocks. 16 of the 32 are floor stones, so a layout sharing floor lands near 24. The two detours are not optional: each input must reach the far comparator's side at full strength, which is what the repeaters restore.
3. NAND (6b, torch) and AND (7b) both survived or were rebuilt and look at the bar.
4. Re-measure the tile pitches for the rebuilt gates.

- `instant` (latency 0 gt) means the output cell changes in the same tick as the input, which only dust-to-dust and the power-bump tricks do; everything through a torch or comparator costs 2 gt.
- A lever input powers its own block, so a lever-input gate can be one block smaller than the same gate driven by a signal. Two different variations, not one record.
- The judge demands the output drive something (`check::update_less`): a dust redirected without a power change does not count as an output.

## Settled facts

- `instant` (latency 0 gt) means the output cell changes in the same tick as the input, which only dust-to-dust and the power-bump tricks do; everything through a torch or comparator costs 2 gt.
- A comparator's or repeater's side input reads ONLY a redstone block, dust, or a diode (`getControlInputSignal`): a powered plain block gives 0. This is the R7 breakage above.
- A lever input powers its own block, so a lever-input gate can be one block smaller than the same gate driven by a signal. Two different variations, not one record.
- The judge demands the output drive something (`check::update_less`): a dust redirected without a power change does not count as an output.
- A comparator's or repeater's `facing` is the direction it READS (its back); its output goes the opposite way. Getting this backwards silently gives a dead gate, and cost me several builds.
- A dust input at a driven cell works (clk-rt's driver is right): what fails is dust that does not POINT into its reader. Dust only connects to a neighbour it points at, so a comparator's side sees 0 unless the dust's shape bends into it.
- An exact comparator gate needs both inputs to arrive at the SAME strength: a detour costs 1 per cell, so the detoured branch must pass through a repeater, which restores 15. Without it the gate degrades to OR.
