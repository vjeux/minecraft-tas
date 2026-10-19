# Contraption studies (`mctas circuit`)

Seven studies in the clock study's spirit ([clocks.md](clocks.md)): for each kind of contraption,
find as many structurally different designs as possible for each value of its main number,
smallest first, every one judged in the REAL game, archived and cataloged. The brief is vjeux's
(2026-10-19, "Let's do 1-7"); clockinfra owns the judge (crates/circuit, crates/circuit-cli, this
page), each track owns its crate (`crates/circuit-<track>`, `mctas circuit <track> ...`).

| study | track | kinds | axis (the gallery's main number) | metrics |
|---|---|---|---|---|
| 1 pulse circuits | pulse | rising-edge, falling-edge, dual-edge, pulse-limiter, pulse-extender | output pulse length L | delay, min-pulse |
| 2 T flip-flops | tff | tff | mechanism family | latency, edge, min-pulse, min-period |
| 3 delay lines | delay | delay | N | min-pulse, min-gap (one-edge delays flagged) |
| 4 logic gates | gates | gate [not, and, or, xor, xnor, nand, nor, imply, nimply] | the function | latency, glitches, instant class |
| 5 memory | memory | rs-latch, d-latch, counter N | the function (N) | latency, enable polarity, output style |
| 6 piston extenders | pext | piston-extender N | reach N | extend, retract |
| 7 flying machines | fly | flying-machine | kind + speed (gt per block) | latency, period |

No pictures: the website renders every build with redsim (vjeux 2026-10-19: "we don't need images
anymore, we should render them"); the input and output cells of a design are its markers.

## Design format

The door/clock design text format (`crates/mctas/src/design.rs`: `name`, `origin`, `palette`,
`layer Y:` rows = z, columns = x) plus:

```text
name: sub-rising-4
circuit: rising-edge             # the kind (+ its argument: `gate xor`, `counter 4`, `piston-extender 3`)
port input: 0 1 0                # one input; several: port input.<name>: (input.a / input.b, input.s / input.r, input.d / input.e)
port output: 3 1 0               # one output; several: port output.<name>: (output.q / output.qbar)
stimulus: in 10+ 14- 40+ 41-     # optional, for exploring (the kind's checker makes its own)
```

* **Inputs** (`port input[.<name>]: x y z [args]`): the args say how the judge drives the cell.
  None: the cell is AIR in the design; the judge writes `minecraft:redstone_block` there with
  updates (on) and air with updates (off). `<on-state> [<off-state>]`: custom states written with
  updates (the design holds the off-state, default air; custom states are not turned with the
  design). `lever`: the design's lever there (off at rest), right-clicked at every edge. `button`:
  the design's button there, right-clicked at every rising edge (its own pulse length). Inputs
  change between ticks: on at frames t..t+L-1 is an L gt pulse. Latch inputs may be spelled
  `set`/`reset`/`data`/`enable` (= s, r, d, e).
* **Outputs** (`port output[.<name>]: x y z`), read every gt: redstone dust gives its power; a
  repeater, torch or observer its own output (15 when on); a comparator its strength; any other
  cell (air, a solid block, a lamp, a piston, a bulb) the best signal INTO it (what a lamp there
  would see). On = level > 0. Put the output where a lamp would go, not on a lamp (a lamp's lit
  state lags 4 gt when it turns off). The main output is `q` when there is one, else the first.
* **Machines**: `port start:` (the clock kick: `last`, `use [T]`, `<state> T`), `travel: dx dy dz`
  (the push or travel direction, a unit vector), `distance: k` (how far a flying machine must
  go), `port payload[.<name>]: x y z` (tracked blocks: the pushed block, the payload).
* **Block count**: every design block except piston heads (part of their piston) and the blocks in
  input cells (a lever or button there is the stimulus device), plus design entities. A block in
  the output cell counts: leave it air when it is only a marker.
* **Stimulus** (explicit, for `--stimulus` or a design without `circuit:`): `;`-separated parts
  `[<input>] <edges>` with edges `10+ 14-` (on at frame 10, off at 14), or `pulse L every T x K
  [from S]` (from 40 by default). The input name may be left out with one input.

## The judge

`mctas circuit verify` builds the design at 4 positions x 4 rotations (the clock judge's offsets;
the game turns every block state, the ports and `travel` turn with it), each in its own sandbox,
placed exactly like `clock verify` places a clock (raw writes, a shape update of every wire, a
`self` update of every block), runs the kind's stimulus and records every gt: each input's state
read from its cell, each output's level, every non-air cell of the watched box (the circuit's box
plus a 3-cell margin; swept along `travel` for machines). A build fails when a design block cannot
stand where it is (the placement removes a floating dust; a floating lever, torch or repeater
fails `canSurvive`: a player could not build it), when anything reaches the
outer layer of the watched box, when an entity appears (a drop, a popped block), or when the
kind's checker says so. All builds passing with the same metrics: `robust`; passing with
different metrics: `position-dependent` / `rotation-dependent` (allowed, flagged). One line:

`CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, robust` or `FAIL: <name> <kind>: <why>`.

`mctas circuit sim` runs the same stimulus on redsim (exact for the blocks it models, in
milliseconds) and prints the same line ending in `redsim`. Both size the stimulus from redsim's
recovery time: how long the circuit takes to come back to rest after every input turns on for 40 gt
and after they turn off, plus 10 gt (at least 20; 100 when redsim does not model the design or it
never rests; `--gap N` overrides). The circuit stands 40 gt at rest after placement (and after its
kick) before the first input.

### What each kind is judged on

* **Pulse circuits**: one input pulse of each length 1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40 gt
  (a button: 3 presses), each followed by the gap. The output must be off at rest and back off
  at the end of every gap. Per input pulse, the output pulses (delay from the input's edge,
  length):
  * `rising-edge`: exactly one pulse at the same delay after every rising edge, the same length L,
    nothing on the falling edge; `falling-edge`: the same after the falling edge; `dual-edge`: one
    pulse after each edge (L and delay per edge when they differ);
  * `pulse-limiter`: one pulse from the rising edge, length L for every input >= L (shorter inputs
    may pass shorter); L must be shorter than the longest input;
  * `pulse-extender`: one pulse from the rising edge, length L for every input shorter than L and
    never shorter than L; at least one input shorter than L must be handled.
  * `min-pulse`: the shortest input from which every longer one is handled (at least the 3 longest
    must be).
* **tff**: 8 pulses of 4 gt (A), then two each of 10, 3, 2 and 1 gt (B), then trains of 6 pulses
  (half on, half off) at periods 30, 20, 16, 12, 10, 8, 6, 5, 4, 3, 2 gt (C). Every A pulse must
  change the output exactly once (steady until the next), and so must every longer pulse (a lever
  held down). `latency`: input edge to output change;
  `edge`: rising when the change keeps its offset as the pulse lengthens, falling when it moves
  with the falling edge; `min-pulse`: the shortest pulse from which every longer one toggles;
  `min-period`: the fastest train period from which every slower one gives exactly 6 changes and a
  steady output.
* **delay**: pulses of 1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30 gt, then pairs of 20 gt pulses with
  gaps of 1, 2, 3, 4, 5, 6, 8, 10 gt. `N`: the shift of the longest pulse's rising edge; a pulse
  (or pair) is handled when the output is exactly the input shifted, both edges. `min-pulse` /
  `min-gap`: the shortest pulse / gap from which every longer one is handled. When the falling edge
  moves by another amount than the rising edge, the line says `one-edge delay` with both shifts.
* **gate**: an Euler walk through all input combinations (2 inputs: 13 states, every one of the 12
  ordered transitions once, both inputs changing in the same tick included), each held for the
  gap, from 00 at rest. The output must be steady in the second half of every hold and the same
  for every visit of a combination; the truth table names the function (inputs in name order: a,
  b; `imply` = a -> b, `imply-ba` = b -> a), a claimed function must match, a constant or
  single-input output fails. `latency`: the worst gt from an input change to the output's final
  value; `glitches`: extra output changes; latency 0 = the `instant` class.
* **rs-latch** (inputs r, s): pulses s, s, r, r, s, r of 4 gt and s, r of 20 gt; q must be set by
  s and cleared by r and stay; a declared `qbar` must be !q. **d-latch** (inputs d, e): 12 steps of
  (d, e) held for the gap; q must follow d while enabled and hold otherwise; both polarities pass
  (`enable high`: transparent while e is on; `enable low`: while it is off, a repeater lock).
  `latency`: the worst change time. **counter N**: 3N + 1 pulses of 4 gt; the output must change
  once every N pulses (`toggles`), pulse once every N pulses (`pulses`), or its level at the end of
  each pulse must repeat with period exactly N (`level`, comparator counters).
* **piston-extender N** (payload, travel): 3 cycles of the input on for the gap then off for it
  (a button: one press, then 2 gaps). The payload's state must be unique in the design. Per
  cycle: from some frame after the on edge to the off edge (a button: at some frame) the payload
  stands at p0 + N x travel, nothing moving; after the off edge the whole rest state (every cell)
  comes back and stays. `extend`: on edge to the first frame of the held extension; `retract`: off
  edge (a button: the press) to the first frame of the final rest; every cycle must give the same
  numbers (track pext's own measure).
* **flying-machine** (kick, travel, distance): one flight of 200 + 60 x distance gt in a corridor
  swept `distance` + 3 cells along travel (a machine reaching its far end ends the run there; any
  other face fails). Track fly's tracker ([`crate::fly`], from crates/circuit-fly): per frame the
  machine's translation (structural states, piston heads part of their piston, a moving block = in
  motion) and a canonical frame; the steady state is frame t + P = frame t shifted by d cells for
  at least 2 periods. It must fly along `travel` and stay intact over `distance` cells (or to the
  corridor's end); a steady shuttle in place over 2+ positions passes as two-way. `speed`: P / d gt
  per block; `latency`: the kick to the first change; the box is the design's own.

## Archive and catalog

`mctas circuit archive <design>... [--caption '...'] [--root designs/circuits]` judges each design
(`circuit verify`) and keeps a passing one in its study's archive,
`designs/circuits/<study>/archive/`: the design with two header lines (`# archived <date> from
<path> by mctas circuit archive`, `# CIRCUIT: ...`), its row in `verdicts.tsv` (kind, the study's
axis value, metrics, blocks, box, family = the kinds of the blocks whose state changes,
robustness, block kinds, type key) and its caption in `captions.tsv` (default: the design's first
comment block). Refused: a failing design, or a structure already archived as the same kind (the
TYPE: up to translation, the 4 rotations and the mirror image, input cells marked with their
device (a lever, a stone or wooden button, the judge's redstone block: their timings differ; not
their facing), output and payload cells marked, after the swaps that keep the run identical on redsim under
the kind's stimulus, as for clocks: sticky pistons that never pull = pistons, inert solids =
stone: `same type as <file>`). `--dedupe` (no game) keeps one design per kind and type in every
study (the earliest archived) and lists the removed twins in `aliases.tsv` ("built with oak_planks,
without stone"); the catalog shows them as "also found as". A name taken by another structure
gets `-<6 hex digits of its type hash>`. The axis value: pulse kinds `L = <L>`, delay `N = <N>`,
gates the function, T flip-flops their family, latches the kind, `counter N`, extenders `N = <N>`,
flying machines their speed (or `two-way`).

`mctas circuit catalog [--root designs/circuits]` (no game) writes, per study,
`designs/circuits/<study>/CATALOG.md` (by axis value, numbers ascending, the smallest design first:
design link, kind, metrics, blocks, box, robustness, caption) and `variants.md` (the dominance fold,
as for clocks: within a kind and axis value, a design with more blocks that uses every block kind of
a smaller one, plus others, is its variant, listed under it with the kinds it adds); the index
`designs/circuits/CATALOG.md`; and `designs/circuits/catalog.tsv`, one row per archived design
(study, kind, axis, name, path, metrics, blocks, box, family, robustness, variant_of, caption): the
data the website renders.

Extendable families (`extend` headers, cards with groups, as for clocks) come next.

## Commands

| command | what |
|---|---|
| `mctas circuit info <design>...` | parse: kind (study), inputs and how they are driven, outputs, payloads, block count |
| `mctas circuit sim <design>... [--table] [--wave] [--stimulus S] [--gap N]` | the redsim screen: the kind's stimulus, the checker's table (`--table`), the waveform (`--wave`: `#` on, `.` off, an output's level as a hex digit), the CIRCUIT line |
| `mctas circuit verify <design>... [--positions 4] [--no-rotations] [--workers 8] [--table] [--stimulus S] [--gap N]` | the judge in the real game (exit 1 unless every design passes) |
| `mctas circuit archive <design>... [--caption C] [--root designs/circuits]` | (coordinator) judge and keep in `designs/circuits/<study>/archive/` |
| `mctas circuit catalog [--root designs/circuits]` | (coordinator) every study's CATALOG.md + variants.md, the index, catalog.tsv |
| `mctas circuit <track> <args...>` | track commands: `pulse`, `tff`, `memory`, `delay`, `gates`, `pext`, `fly` -> `circuit_<track>::run(args)` in `crates/circuit-<track>` |

Tracks: put your designs in `designs/circuits/<study>/tracks/<track>/`; the coordinator archives
the verified ones.
