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

## R7 (real input devices): odd N is NOT FOUND YET, and five designs were dominated

Re-judged the whole study under R7. Every piston design shifted DOWN by exactly 1 gt, because R7's input changes during a tick, so the piston acts in that same tick instead of the next one:

| design | was | now |
|---|---|---|
| delay-piston-qc-1 | N 1 | N 0 |
| delay-piston-or-3 | N 3 | N 2 (9 blocks) |
| delay-piston-or5/7/9/11 | N 5/7/9/11 | N 4/6/8/10 (13 blocks) |

So the five odd-N designs became even-N ones, and every one of them is dominated by a 2-block repeater line or a comparator line. They are removed. The N=0 design is kept and re-archived as `delay-qc-0`: it is the only archived circuit whose output follows its input with no delay at all while still driving a block update.

ODD N UNDER R7 IS AN OPEN GAP, not an impossibility. An earlier version of this file claimed a proof; it was wrong, and the correction matters more than the claim. The argument was: every redstone component moves an edge by an even number of gt (dust 0, repeater 2/4/6/8, comparator 2, torch 2, observer 2), the only odd source is a piston acting in the block-event phase, and that needs an input changing BETWEEN ticks, which R7 never does. The hidden assumption is "every component": it silently excluded BLOCK-ENTITY timings, and those are exactly where odd lengths live. Study 1 has `pulse-limiter-hopper-timer-L7/15/23/31`, robust under R7, at L = 8n - 1: a hopper moves an item every 8 gt in the block-entity phase, and the comparator reading it sees the transfer one gt before the next cooldown ends, so the chain is odd by construction. Other untried odd sources: a dropper's 4 gt scheduled tick, 0-tick piston retraction, the arrival frame of a moving block, an observer watching a block a piston moves, and a comparator reading a container in the same tick the container changes.

So the right statement is: no odd-N delay line has been FOUND under R7, the piston route is closed (its half-tick came from the input's own timing), and the hopper/dropper route is open and untried for this kind. Next: a delay line whose stage is a hopper (8 gt per item, read by a comparator) rather than a repeater.

What the study holds now: the repeater line (2 blocks per started 8 gt, every even N), the comparator line (2 gt per comparator, shorter pulses kept), the torch tower (N = 6 + 4n) and delay-qc-0. Both line cards tile side by side at pitch 1.

- Every redstone component moves an edge by an EVEN number of gt (dust 0, repeater 2/4/6/8, comparator 2, torch 2, observer 2). Odd N therefore needs a piston, which acts in the block-event phase: an input changing between ticks makes it act 1 gt later.
- A repeater line is N = 2 x the sum of its delays, 2 blocks per repeater: blocks = 2 x ceil(N / 8).
- A comparator line is 2 gt per comparator and keeps 2-3 gt pulses, where a repeater line needs 8 gt ones.

### Odd N on a hopper stage: tried, and why it does not transfer (2026-10-20)

Study 1's hopper item timer gives odd LENGTHS (L = 8n - 1) because it measures how long items keep flowing: the length is the item count times the 8 gt cooldown, less the gt the comparator sees the last transfer. A DELAY is a different demand — both edges must shift by the SAME fixed N — and the hopper's cooldown is free-running relative to the input: when the lock releases, the next transfer lands anywhere in the remaining 0..7 gt of the current cooldown, so the shift is not a constant. Judged: the L7 timer used directly as a delay fails with "only the 1 longest pulse(s) come out shifted", i.e. it limits rather than delays.
So an odd-N delay needs a block-entity event whose phase is SET by the input rather than free-running: a dropper fired by the edge (its scheduled tick is 4 gt, even, but the item's arrival in a hopper below is a block-entity event in the same tick), a 0-tick piston retraction, or an observer watching a block a piston moves. Those are the untried candidates; the hopper-lock route is closed for this kind.

- Every REDSTONE component moves an edge by an even number of gt (dust 0, repeater 2/4/6/8, comparator 2, torch 2, observer 2). That is not the whole game: BLOCK-ENTITY timings give odd lengths (a hopper moves an item every 8 gt and its comparator reads the transfer a gt before the cooldown ends: study 1 L = 8n - 1, robust under R7), and so may a dropper tick, a 0-tick retraction, a moving block arrival, or a comparator reading a container mid-tick.
- A repeater line is N = 2 x the sum of its delays, 2 blocks per repeater: blocks = 2 x ceil(N / 8).
- A comparator line is 2 gt per comparator and keeps 2-3 gt pulses, where a repeater line needs 8 gt ones.

## Settled facts (do not re-derive)
