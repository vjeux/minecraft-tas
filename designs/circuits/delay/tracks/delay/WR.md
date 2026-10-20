# WR bar — study 3, delay lines

What a world-record entry means here, per N. "Blocks" is `mctas circuit verify`'s count; N, min-pulse and min-gap are the judge's own metrics. Judged in the game or it does not exist.

| variation | record target | ours today | gap |
|---|---|---|---|
| even N, cheapest per gt | 2 blocks per started 8 gt (a repeater line: one repeater + its stone, delay 1..4 = 2..8 gt) | delay-rep2/4/6/8 cards, 2 + 2n blocks | AT THE BAR: a repeater is the only component that buys 8 gt for 2 blocks |
| even N, shortest pulses kept | 2 blocks per 2 gt (a comparator line, min-pulse 2) | delay-cmp card | at the bar for pulse fidelity; a repeater line is cheaper but needs 8 gt pulses |
| N = 1 | 6 blocks | delay-piston-qc-1 (min-pulse 3) | open in principle: 1 gt needs the piston's block-event phase, and no smaller trick is known |
| N = 3 | 9 blocks | delay-piston-or-3 | OPEN: 9 blocks for 3 gt is a first implementation |
| odd N >= 5 | 13 + 2n blocks | delay-piston-or5/7/9/11 cards | OPEN: same mechanism, same question |
| **tileable stack** | a slice that repeats with no gap, so k parallel lines cost k x the slice | none measured | OPEN: builders run buses of equal delays side by side; `tile:` now judges study-3 kinds if opened |
| flat (one layer) | a repeater line already is | - | at the bar |

## Priorities

1. Measure `tile:` on the repeater and comparator lines: a bus of n equal delays is what builders actually place, and the pitch is a number we can publish rather than guess. Two dust-free lines side by side should tile at pitch 1, which no other study here can claim.
2. Odd N: 9 and 13 blocks are the outliers of the whole study. The N=1 design's tricks (a power-bumped dust, a quasi-connected piston) are the thing to re-apply.

## Settled facts (do not re-derive)

- Every redstone component moves an edge by an EVEN number of gt (dust 0, repeater 2/4/6/8, comparator 2, torch 2, observer 2). Odd N therefore needs a piston, which acts in the block-event phase: an input changing between ticks makes it act 1 gt later.
- A repeater line is N = 2 x the sum of its delays, 2 blocks per repeater: blocks = 2 x ceil(N / 8).
- A comparator line is 2 gt per comparator and keeps 2-3 gt pulses, where a repeater line needs 8 gt ones.
