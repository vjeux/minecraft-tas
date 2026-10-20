# Track gates (study 4: logic gates) — notes

Owner: session 7c07d5cb, box devvm55879, branch explore/delay (shared with track delay). Judge:
`mctas circuit sim` / `verify` / `archive` (docs/circuits.md: an Euler walk over every transition,
latency, glitches, the instant class). Track commands: `mctas circuit gates ...`
(crates/circuit-gates).

## A circuit must drive something (2026-10-19)

The first census found "instant" gates for almost every function by DUST REDIRECTION: an input
block next to a powered dust adds a connection, the dust changes shape and stops pointing into the
output cell. The judge read outputs by query (the best signal into the cell each gt) and accepted
them, but a redirected dust keeps its power and sends NO block update: a lamp, a torch on a block
or a repeater at the output never reacts (real-game tests: a lamp in the instant NOR's output cell
never changes, watched by an observer; instant NAND + a torch on its output stone: the torch never
relights). The judge now puts a redstone lamp in every air output cell and requires it to follow
the output, and refuses a passive block (a stone) in an output cell (crates/circuit:
`check::update_less`, `Circuit::passive_output`; docs/circuits.md "A circuit must drive
something"). `circuit archive --rejudge` removed the 8 redirection gates (their files:
`update-less/`).

What still works at 0 gt: a dust whose power CHANGES sends the updates. Two ways:
* the inputs power the dust (OR: a dust between the inputs, the output under its stone);
* the power bump: the redirected dust runs at 14 (fed by a dust on a redstone block, or by the
  other input through one dust) and the input that bends it is a 15 source, so bending it also
  raises its power: NOT, NOR, NIMPLY, IMPLY in 4 blocks, NAND in 8 (two bump NOTs into one cell).
A single dust cannot make XOR update-correct (the 10 -> 11 and 01 -> 11 transitions would both
need a power change, but max(p_a, p_b) equals one of them), and no dust shape can make AND or XNOR
(adding connections only removes pointing into free cells).

## Results (archived, robust)

| function | fewest blocks | fastest | others |
|---|---|---|---|
| OR | 1 (levers on one stone; 0 gt) | 0 gt: 2 (dust on a stone, output under it) | |
| NOT | 2 (lever + torch, 2 gt) | 0 gt: gate-not-bump-4b | gate-not-torch-3b (block input, 2 gt) |
| NOR | 2 (levers + torch, 2 gt) | 0 gt: gate-nor-bump-4b | gate-nor-torch-3b (2 gt) |
| NIMPLY / NIMPLY-BA | 2 (subtract comparator, 2 gt) | 0 gt: gate-nimply(-ba)-bump-4b | |
| IMPLY / IMPLY-BA | 3 (dust + torch, input touching the output; 2 gt, 2 glitches) | 0 gt: gate-imply(-ba)-bump-4b | |
| NAND | 6 (two torch NOTs into one cell, 2 gt) | 0 gt: gate-nand-bump-8b | |
| AND | 5 (two subtract comparators: a - (15 - b), 4 gt) | 4 gt | |
| XOR | not found (none up to 5 blocks in 3x2x3 with every component; up to 6 without comparators or torches) | | the one-dust XOR is update-less |
| XNOR | not found (none up to 6 blocks) | | |

## Tools

* `mctas circuit gates check <designs> [--hold 30]`: the track's own checker (query only).
* `mctas circuit gates census --dims 3x2x3 --inputs "x,y,z;x,y,z|..." [--palette
  stone,rblock,dust,torch,rep,cmp] [--k 5] [--threads N] [--out-dir D]`: every filling of the box
  (up to k blocks) around fixed input cells, every candidate output cell read at once; a hit must
  pass the judge's own redsim screen (the lamp included) and be minimal for the judge (without any
  one of its blocks the judge refuses the claim); keeps the fewest blocks and the fastest per
  function. 1 input: `--inputs "0,1,1|..."` (NOT).

## Next

* XOR and XNOR: by hand, the comparator versions need a dust link per input (each input feeds one
  comparator's back and the other's side, which never share a cell when both fronts meet): about 8
  blocks for XOR (2 gt) and 10 for XNOR (4 gt: 15 - max(a - b, b - a) on a third comparator).
  A census needs a bigger box or k = 7-8 with a narrow palette.
