# WR bar — study 3, delay lines

What a world-record entry means here, per N. "Blocks" is `mctas circuit verify`'s count; N, min-pulse and min-gap are the judge's own metrics. Judged in the game or it does not exist.

| variation | record target | ours today | gap |
|---|---|---|---|
| even N, cheapest per gt | 2 blocks per started 8 gt (a repeater line: one repeater + its stone, delay 1..4 = 2..8 gt) | delay-rep2/4/6/8 cards, 2 + 2n blocks | AT THE BAR: a repeater is the only component that buys 8 gt for 2 blocks |
| even N, shortest pulses kept | 2 blocks per 2 gt (a comparator line, min-pulse 2) | delay-cmp card | at the bar for pulse fidelity; a repeater line is cheaper but needs 8 gt pulses |
| N = 1 | 6 blocks | delay-piston-qc-1 (min-pulse 3) | open in principle: 1 gt needs the piston's block-event phase, and no smaller trick is known |
| N = 3 | 9 blocks | delay-piston-or-3 | OPEN: 9 blocks for 3 gt is a first implementation |
| odd N >= 5 | 13 + 2n blocks | delay-odd5 (N 5, 11 blocks) + delay-piston-7/9/11/13 (10 blocks) | N = 5 BUILT 2026-10-21 (11 blocks); N = 1 and 3 look infeasible under R7 (WR notes below) |
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

### Why the piston family is N = 5 + 2d and what N = 1 / 3 / 5 would need (2026-10-21)

Traced delay-piston-7 in redsim (`circuit sim --cells`). Under R7 a piston acts in the SAME tick as
a departure (a block becoming a moving block, block-event phase) or a scheduled-phase signal
(repeater, comparator, torch, observer), but ONE TICK LATER after an ARRIVAL (a moved block lands
in the block-entity phase, after the block events). A moved block departs when its piston acts and
arrives 2 gt later. So every block movement adds 2 gt, and each arrival-triggered piston adds 1.
- Rise of delay-piston-7: P acts t, RB lands t+2, Q acts t+3 (arrival lag), Q's start cell empties
  at t+3, the repeater falls at t+3+2d, P2 retracts at once and its far block departs, Q2 loses power
  and retracts at once, Q2's block lands next to the output at t+5+2d.
- Fall: P retracts t', Q retracts at once, its block is back at t'+2, the repeater rises at t'+2+2d,
  P2 pushes its block back (lands t'+4+2d), Q2 extends at t'+5+2d (arrival lag), and the output
  block departs at once. Both edges: 5 + 2d.
- The output stage's rule: a sticky piston Q2 powered at rest by a block X whose absence signal is
  (departs a, back b) gives output (a + 2, b + 1), so it is symmetric only when b = a + 1. The
  repeater (2d) exists to remove the arrival lag between Q's block and P2 on the fall. Every
  lag-remover (repeater, comparator, observer, torch) costs 2 gt, hence N >= 7 for this topology.
- So N = 5 needs a block X that departs at t+3 and is back at t'+4: pushed away by a piston triggered
  by RB1's arrival (acts t+3) and pushed back by another piston acting at t'+2. That second piston
  needs a power-on edge at t'+2 that is off at rest and on the rise (a torch fed by RB1 is lit at
  rest, so its piston sits extended in X's way). Not found. N = 3 needs (1, 2) and N = 1 (-1, 0),
  the latter impossible for this output stage (a >= 0). Open, not proven impossible: other output
  stages (a dust cut by a moving block, QC/BUD pistons) are untried.
- Event-time algebra (R7, offsets after the rise t / fall t'): a piston acts at a departure or a
  scheduled edge (+0) or one gt after an arrival (+1); a moved block departs when its piston acts
  and arrives 2 later. Nothing can happen at t+1 or t'+1, so every output edge is at 0, 2, 3, 4, 5...
  N = 3 needs an output change at t'+3 by an arrival (impossible: arrivals come 2 after an act) or
  by a departure triggered by an arrival at t'+2, and the rise at t+3 likewise. With one input
  piston and its block back at its start cell at t'+2 that is possible on ONE edge only in every
  combination tried; N = 3 and N = 1 look infeasible (reasoned, not proven).
- Two N = 5 topologies on paper, to build:
  (a) Y powered by RB0 (input piston, arrives t+2) OR RB0b (piston behind a delay-1 repeater from the
      input, departs t'+2): Y acts (3, 2), pushes X away at t+3 and pulls it back for t'+4, and the
      output stage above turns X's (3, 4) absence into (5, 5). 10 blocks on paper, but there is a
      PARITY OBSTRUCTION when P0b sits right in front of the repeater: RB0 lands at I + u + 2f0
      (u: P0's offset from the input I) and RB0b at I + 2v + 2f1 (the repeater at I + v, P0b at
      I + 2v), so the two landing cells differ by an odd vector and can never share a neighbour (Y).
      THE FIX COSTS ONE BLOCK: the repeater powers a solid block S at I + 2v, and P0b stands next to
      S at I + 2v + u (S strongly powered powers it sideways). Then RB0 and RB0b land 2 apart and Y
      sits between them. With u = x the pushes must be vertical, and an up-facing piston is held by
      its own landed block through quasi-connectivity (the head cell is its QC cell), and a
      down-facing layout QC-powers Y from the blocks at rest; with u = y (P0 standing ON the input
      stone) and both pistons pushing east it works: delay-odd5, below. Y fed by RB0 through
      quasi-connectivity has the right parity but gets no block update when RB0 lands (a BUD).
      **BUILT: delay-odd5, N = 5 gt both edges, 11 blocks, 6x4x3, min-pulse 6, min-gap 5, robust in
      the game, tick-identical in redsim** (archive/delay-odd5.txt): input stone I, P0 on top of it
      (sticky, east) with RB0; repeater (facing I) + its stone + S; P0b on S (sticky, east) with
      RB0b; Y (sticky, east) between the two landing cells, X (RB) in front of it; Q2 (sticky, east,
      extended) on top of X with its head and RB2; the output next to the head.
  (b) output = OR(X at its near cell (5, 3), cut block Z absent (6, 5)): P2 powered at rest by K
      (K pushed off at t+3 by a piston on RB0, back at t'+2) pulls X near; P3 on X pushes Z out of a
      dust step-up cut at t+6 and pulls it back at t'+5.

## Settled facts (older)

## Catalog shape (critique 2026-10-20)

- The four `delay-rep2/4/6/8` bases are one mechanism — a single repeater on a stone, set to 2, 4, 6 or 8 gt, extended into a line — and they ARE one card: they share `extend group: delay-repeater-line`, and the site shows a card of several families by its first base alone (`card_member` on the others; the live site at gh-pages eed47a38d1 already had a single repeater card, so the "four cards" view was an older deploy). An earlier version of this note said they could not be folded; that was wrong — it looked at the circuit catalog's tunable-group pass, not at the family cards that already do it. What WAS still wrong is fixed in crates/site/src/generate.rs: that one card read "2-tick delay · 2 blocks", its base's own value, while it stands for every repeater line. A card of several families now carries the union of their values as its span, so it reads "2- to 88-tick delay · 2 to 22 blocks · 44 versions" (and the piston-extender cascade card, the only other multi-family card, "reach 2 to 16").
- Study 2's nine dominated T flip-flops ARE folded now: the dominance pass keyed on (kind, axis), and study 2's axis is the block family, so every mechanism was its own card by construction and nothing could ever fold. Keyed on kind alone for tff, the catalog goes from 10 cards to 3: tff-bulb and the two designs that win something real.

## delay-qc-0 removed (critique 2026-10-20)

It was archived as "the only circuit whose output follows its input with no delay", 6 blocks with a sticky piston. Judged side by side, a plain dust on a stone does the same N = 0 in 2 blocks with a shorter min-pulse (2 gt vs 4) and min-gap (2 gt vs 3): a 0-tick delay is a wire, and this one was a dominated wire, so it is removed rather than re-captioned. The delay study now starts at N = 2 (one repeater), which is where a page about making things happen later should start; odd N stays the open gap above.

## ONE design per length (coordinator's call on vjeux's rule, 2026-10-20)

The delay kind is one design per N: the repeater line (2 blocks per started 8 ticks, tileable side by side at pitch 1). The comparator line (2 + 2n blocks) and the torch tower (6 + 4n blocks) are removed: at every N they are larger. What they had over it — a 2-3 tick pulse passes through unstretched, where a repeater set to d stretches anything shorter than 2d ticks — is now said in the repeater line's own caption, with the fix (a line of repeaters set to 1 keeps short pulses, at 2 blocks per 2 ticks). Odd N remains the open gap.

### Hopper chains do not give an odd DELAY (2026-10-20, from HopperBlockEntity)

A hopper receiving an item while empty gets cooldown 8, or 7 when it already ticked this gt before the sender (`tickedGameTime`): that 8 - 1 is study 1's odd timer length. A delay needs it on BOTH edges. The rising edge can take it (unlock the first hopper; the item lands in a hopper read by the comparator after a 7-gt hop: 7 + 2 = 9). The falling edge cannot: the output hopper has to hold the item while the input is on (locked), and `pushItemsTick` decrements the cooldown every tick, locked or not, so the item leaves the moment the lock opens and the output falls 2 gt after the input — even. An odd falling edge would need a second item and a second 7-gt hop timed by the fall, which is a machine of ~20+ blocks, not a delay line. Not found yet, with this as the reason; candidates left: 0-tick piston timing and a comparator reading a container that changes in the block-entity phase of the input's own tick.

## ODD N FOUND under R7: delay-piston-7/9/11/13 (2026-10-20)

One 10-block layout gives every odd delay from 7 to 13 ticks, robust in the game; the repeater between the two stages sets it. The mechanism comes straight from the tick phases:
- A piston set off in the scheduled-tick or block-event phase (by a repeater, or by another piston's block leaving) acts in that tick, and the block it pushes arrives 2 ticks later, in the block-entity phase (PistonMovingBlockEntity finishes one tick after its progress reaches 1).
- A piston set off in the block-entity phase (by a block ARRIVING) can only act in the NEXT tick's block events: one tick later. That is the odd tick.
- A stage = piston X (its block, at the pushed position, powers piston Y) then piston Y (its block, at the RETRACTED position, powers what follows). Input on: X's block arrives (+2, block-entity phase), so Y acts at +3 and its block leaves: the stage output falls at +3. Input off: X's block leaves at once, Y acts in the same block-event loop, and its block arrives back at +2: the output rises at +2. One stage is an inverter with 3 ticks one way and 2 the other.
- Two stages with a repeater between them (the repeater re-times every edge to the scheduled-tick phase, so the second stage sees the same phases as the first) add up to 3 + 2 + 2 = 7 on BOTH edges; each notch of the middle repeater adds 2.
- Without the repeater the second stage sees its rising input in the block-entity phase and the edges come out 5 and 6: measured reasoning, which is why the repeater is there.
Still open: odd N below 7 (1, 3, 5). The same phase algebra rules out every chain of these stages, torches and repeaters shorter than this, so a smaller odd delay needs a different odd source (the hopper's 8 - 1 only works on one edge, see above).
