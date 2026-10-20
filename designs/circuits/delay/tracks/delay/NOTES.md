# Track delay (study 3: delay lines) — notes

Owner: session 7c07d5cb (clk-rt), box devvm55879, branch explore/delay. Judge: `mctas circuit sim`
(redsim screen) and `mctas circuit verify` / `circuit archive` (the real game, 4 positions x 4
rotations); stimulus and metrics in [docs/circuits.md](../../../../../docs/circuits.md). Track
commands: `mctas circuit delay ...` (crates/circuit-delay).

## Results (all archived robust)

| N (gt) | blocks | design | how | min-pulse / min-gap |
|---|---|---|---|---|
| 1 | 6 | delay-piston-qc-1 | update-correct: power-bumped dust + quasi-connected piston (below) | 3 / 3 |
| 2 | 2 | card delay-rep2 (n=0), card delay-cmp (n=0) | one repeater (delay 1) / one comparator on a stone | 2 / 2 |
| 3 | 9 | delay-piston-or-3 | sticky piston's 3/1 copy OR'ed with itself 2 gt later | 4 / 3 |
| 4, 6, 8 | 2 | cards delay-rep4/6/8 (n=0) | one repeater, delay 2/3/4 | N / N |
| 5, 7, 9, 11 (+8n) | 13 + 2n | cards delay-piston-or5/7/9/11 | N=3 design, the OR on a dust, repeater tail | 4..8 |
| 2 + 8n ... 8 + 8n | 2 + 2n | cards delay-rep2/4/6/8 (group: N = 2 x sum of delays) | repeater line | 8 / 8..10 |
| 2 + 2n | 2 + 2n | card delay-cmp | comparator line | 2-3 / 2-3 |
| 6 + 4n | 6 + 4n | card delay-torch | torch tower | 3 / 3 |

Even N: a repeater line, N = 2 x the sum of its delays, 2 blocks (repeater + stone) per started 8
gt: blocks = 2 x ceil(N / 8). Comparator lines (2 gt per comparator, 2 blocks each) keep shorter
pulses (min-pulse 2-3) at the cost of blocks. Odd N >= 3: `mctas circuit delay piston-or --n N`
(N = 3 + 2 x tail).

## Why odd N needs a piston, and why the input timing matters

Every redstone component (dust 0, repeater 2/4/6/8, comparator 2, torch 2, observer 2) moves an
edge by an even number of gt. A piston acts in the block-event phase: an input that changes
BETWEEN ticks (the judge's input, a player's lever) makes it act 1 gt later; a change inside a
tick (from a repeater, in the scheduled-tick phase) makes it act in the same tick. A pushed block
leaves its old cell when the piston acts and arrives 2 gt later (the moving block lands in the
block-entity phase). So, from a between-ticks input, a pushed redstone block gives a 3/1 signal at
its new cell (on 3 gt after the rise, off 1 gt after the fall) and a 1/3 signal for "absent from
its old cell"; edges at odd offsets exist only through such a piston. With a repeater in front of
the piston the odd gt disappears (4/2 at the new cell; piston-or with front repeaters measured N
even), so longer odd lines put the repeaters AFTER the odd stage.

Combining: OR takes the earlier rise and the later fall, AND the later rise and the earlier fall.
* N = 3: OR(3/1, 3/1 delayed 2 = 5/3) = 3/3 (delay-piston-or-3). The OR needs the output cell
  next to the block's new cell and a 2 gt copy fed back to the same cell: a repeater behind the
  block's new cell cannot reach a cell next to it in fewer than three hops (stone + two dusts).
* N = 1: AND(1/3, OR(input, 3/1)) = AND(1/3, 0/1) = 1/1. delay-piston-1 (4 blocks) did the AND by
  dust redirection: the dust is powered by the input and bent by the redstone block at rest (the
  block also powers it); when the block leaves (1 gt after the rise) the dust straightens; the
  block's arrival next to the output covers the gt after the fall. The judge accepted it until it
  learned that a circuit must drive something: straightening a dust whose power stays 15 sends no
  block update, so a lamp at the output only lights when the block lands (3 gt): update-less,
  removed by `circuit archive --rejudge` (file in `update-less/`). The update-correct version,
  delay-piston-qc-1 (6 blocks, robust): the input cell sits ABOVE a dust E (a redstone block above a
  dust powers it without a connection), E feeds the output dust D at 14 while the redstone block at
  rest gives D 15, so when the block leaves D's power drops 15 -> 14 (a power change: the output
  cell gets its update); and the piston beside E is powered by quasi-connectivity (the input touches
  the cell above the piston; E's power change updates the piston, so it still acts 1 gt after the
  rise). Update-correct odd delays: N = 1 (6 blocks), 3 (9), 5+ (13 + 2n).

## Things that fail the judge (and why)

* Observer chains + a copper bulb (an edge detector toggles a bulb, a comparator reads it,
  N = 2k + 2): a 1 gt input pulse gives one observer pulse for two edges, the bulb desyncs and
  every later pulse is inverted. The judge's stimulus starts with 1 gt pulses, so any
  edge-reconstruction line fails unless it resyncs on its own.
* Piston lines without care: a sticky piston that loses power before its block lands (pulses of
  1-2 gt) spits the block; the designs above recover on the next 3+ gt pulse (the next retraction
  pulls the spat block back), so the judge measures min-pulse 3-4 instead of failing them.
* Census candidates judged only with long pulses (40 on / 40 off) included pulse generators that
  happened to match; every census hit is now confirmed with the judge's own redsim screen.

## Tools

* `mctas circuit delay check|sim <design>`: the track's own checker (N, one-edge, min-pulse).
* `mctas circuit delay repeaters --delays 4,4,1`, `comparators --n K`, `torches --n K`,
  `observers --n K` (fails the judge, kept as evidence), `piston-or --n N | --tail 4,1`.
* `mctas circuit delay census --dims 4x2x3 --input=x,y,z --palette stone,rblock,dust,rep,cmp
  [--fixed "x,y,z=state;..."] --k 4 --max-n 20`: every filling of the box (plus fixed seed blocks),
  every output cell read at once, minimal designs only, each hit confirmed by the judge's redsim
  screen; the best per N.

## Next

* A smaller N=3 (the census with the N=1 geometry as a seed) and a cheaper odd tail than 13 blocks
  (the OR before the tail costs a dust and a stone).
* Extendable cards (repeater line, piston-or) when the circuit archive supports them.
