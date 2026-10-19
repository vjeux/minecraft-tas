# Clocks: the clock study

vjeux (2026-10-19): "Finding as many clock types for each tick count (or family of ticks). Build
the scaffolding for this and let agents find all sorts of clocks." This page is the one home for
what a clock is, how it is judged, the design format and the commands. The brief and the tracks:
[CLOCKS.md](../designs/clocks/CLOCKS.md) (copied from the coordinator's brief); the game mechanics
the clocks use: [mechanics.md](mechanics.md); the simulator: [internals.md](internals.md).

## What counts as a clock

* A structure that, once placed (and optionally after ONE start kick at a declared `start` cell),
  keeps producing a periodic signal forever with no further input. Self-starting and kicked clocks
  both count; the start is part of the record (`self-start`, `kick: ...`).
* **Period P** (game ticks, gt): the smallest P such that the clock's state repeats exactly every P
  gt after a warmup W. The VISIBLE state decides it: every block state and every comparator's
  output strength (what any redstone component could react to). Hidden state (pending scheduled
  ticks, container contents and hopper cooldowns, sculk sensors' vibrations in flight, the torch
  burnout memory) must repeat too, with a period that is a multiple of P: on redsim the WHOLE
  state is keyed exactly (`redsim::snapshot`: every cell, the pending ticks with their delays,
  priorities and order, block events, moving blocks, every block entity, the torch memory younger
  than 60 gt) and the first exact repeat gives the whole-state warmup and period; the clock's P is
  then the smallest divisor of it over which the visible state repeats, and `clock sim` prints the
  whole-state period when it is larger (e.g. items sloshing in hoppers no comparator reads). The
  real game shows the visible state (block states every tick, comparator strengths from the block
  entities), so the judge asks it to repeat with period P for at least 3 periods after W.
* **Waveform**: per changing cell, on-ticks per period (duty), rising edges per period and pulse
  widths; the same at the `output` port (default: the changing cell with the most rising edges).
  "On" ([`wave::active`](../crates/clock/src/wave.rs)): pistons extended, a piston head or moving
  block present, sculk sensors ACTIVE, hoppers locked, doors/trapdoors/gates open, dispensers,
  droppers and crafters triggered, leaves distance < 7, scaffolding distance > 0, else `lit`, then
  `power > 0`, then `powered`; a plain block: present. An output cell holding a plain block or air
  reads the power it receives (`getBestNeighborSignal > 0`).
* **Family**: the set of block types whose state changes during one period (the active set),
  detected automatically: a cell counts when its block state, a comparator's output strength or a
  container's contents change; plain blocks (stone, slabs, logs, glazed terracotta...) and moving
  blocks are left out (a moved block's piston names itself), piston heads count as their piston,
  wall torches as torches, every waxed copper bulb as `copper_bulb`. Written sorted and joined by
  `+`: `observer`, `comparator+redstone_wire`, `comparator+hopper`,
  `redstone_torch+redstone_wire+repeater`.
* **Type**: the structure up to translation and the 4 horizontal rotations (mirror images noted).
* **Size**: blocks placed (piston heads are part of their piston; a temporary start block is not
  counted) and the SWEPT box: every cell non-air at any tick of the run, plus the design's box and
  the start cell (a piston head poking out of the design counts). Ranking inside a period: fewest
  blocks, then smallest swept volume, then the waveform.
* **Must work in the real game** (`mctas clock verify`, the official judge): exactly periodic at N
  positions x 4 rotations, no entities created (a drop, a popped block), never reaching the edge of
  the watched box (its box + `--margin`, default 3). Builds that disagree on period, output waveform
  or family make the clock `position-dependent` or `rotation-dependent`: recorded, flagged.
* The study's product: for every period P, every family that reaches it, its distinct types and the
  smallest of each, and which periods a family cannot reach (with the reason, or an exhaustive
  census as the proof): `designs/clocks/CATALOG.md` (generated) and the board's
  `minecraft-tas/clocks/coverage.md`.

## Design format

The door design text format ([tools.md](tools.md#design-files), `crates/mctas/src/design.rs`), with
no door and no lever:

```text
name: cal-observer-pair
port output: 0 0 0          # optional: where the signal is read
port start: 1 0 0 last      # optional: the one-time start kick
palette:
  E minecraft:observer[facing=east]
  W minecraft:observer[facing=west]
layer 0:
  EW
```

* No `port start` = **self-starting**: placed exactly like the door judge places a door (raw writes
  in file order, a shape update of every wire, a `self` update of every block; no observer pulse and
  no piston fire from the placement itself), then run with nothing else.
* `port start: x y z <kick> [at N]`, the kick (between ticks, at tick 0 = right after placement and
  before the first tick, or after N ticks with `at N`):
  * `last`: the design's own block in that cell is left out of the silent placement and placed with
    updates (a player placing it). The two-observer clock starts this way.
  * `use [T]`: one right-click on the design block there (lever, button, note block, repeater,
    comparator, dust, oak trapdoor/door); with `T`, a second right-click T gt later (a lever pulse).
  * `<block state> <T>`: a temporary block placed with updates in an AIR cell and removed with
    updates (no drop) T gt later: `minecraft:redstone_block 1` is a 1 gt power pulse,
    `minecraft:stone 1` a shape update for an observer.
  * Parity: every redstone component delays by an even number of gt, so a clock of torches,
    repeaters and comparators started on one parity class keeps it; `at N` with an odd N starts a
    second class (an odd period needs one).
* `placement: player` (header): every block placed with updates in file order instead (flagged in
  the start label).
* `nbt: x y z {snbt}` (header, track clk-item): a block entity's data merged right after the raw
  paste, before the wire shapes and self updates (`data merge block`): container contents
  (`{Items:[{Slot:0b,id:"minecraft:stone",count:1}]}`), a hopper's `TransferCooldown`. redsim models
  Items of its item table and TransferCooldown; anything else is UNMODELED for `clock sim`.
* Extra words on any `port` line are kept: `Design.meta["port <name>"]`, written back on the port
  line.

Put your designs in `designs/clocks/tracks/<track>/`; the coordinator archives verified clocks in
`designs/clocks/archive/` with a caption line in `captions.tsv`.

## Commands

| command | what |
|---|---|
| `mctas clock sim <design>... [--horizon 50000] [--margin 4] [--json] [-q] [--trace N]` | the exact period on redsim in milliseconds: place, kick, run until the whole state repeats (W, P), then replay one period for the waveform per changing cell and at the output, the family, the blocks and the swept box. `CLOCK:` line, or `STOPS` (it goes still, or only hidden state changes), `NO-REPEAT` (horizon), `UNMODELED` (a block or case redsim does not model exactly: judge it with `clock verify`), `INVALID` (a drop, or a write beyond the margin). `--trace N` prints every cell change of the first N ticks |
| `mctas clock verify <design>... [--positions 8] [--no-rotations] [--horizon 4000] [--margin 3] [--workers 8] [-v] [--json]` | THE judge, in the real game: every build in a cleared sandbox at the door judge's positions x 4 rotations (the game turns every block state, and a temporary start block), placed and kicked like `clock sim`, every tick of the watched box recorded (block states and block entities); exact periodicity of the visible state for >= 3 periods after warmup, no entity, never reaching the watched box's edge, the same period/output waveform/family everywhere. Prints `CLOCK: <name> period P gt, on Q/P, edges E, warmup W, family <sig>, <blocks> blocks, <WxHxD>, robust\|position-dependent\|rotation-dependent`, the builds that differ or fail, and the redsim agreement (`redsim AGREES (period, warmup, output waveform, family, box)` or what redsim says); exits 1 unless it is a clock |
| `mctas clock archive <design>... [--caption '...'] [--dir designs/clocks/archive]` | (coordinator) the judge, then a copy in the archive (header: date, source, the judge's line), its row in `verdicts-v1.tsv` (period, warmup, output duty and edges, family, blocks, box, robustness, start, type key, chirality) and its caption in `captions.tsv` (default: the design's first comment block); a clock already archived (same blocks, ports, kick, block entities) is skipped |
| `mctas clock catalog` | (coordinator) writes `designs/clocks/CATALOG.md` from the archive's verdicts (no game): the period x family coverage matrix (fewest blocks, linked), the periods a family cannot reach (`designs/clocks/impossible.tsv`, hand-edited: family, periods, why), and per period every distinct type (the smallest instance of each, the smallest per family in bold) with its box, duty, edges, start, robustness and caption; and `designs/clocks/coverage.md` (the matrix alone, mirrored on the board's `minecraft-tas/clocks/coverage.md`) |
| `mctas clock census --box AxBxC --k N [--kmin 1] [--palette core] [--max-period 500] [--margin 2] [--no-kick] [--workers W] --out DIR` | the exhaustive census on redsim: every filling of the box with kmin..=k blocks of the palette (every facing of every component: shorthands `core` = stone, glass, rblock, dust, torch (standing + 4 wall), repeater (4 facings x 4 delays), comparator (4 x 2 modes), observer (6), piston (6), sticky (6), lamp, bulb, note, target, slime; also sculk, csculk, hopper, dropper, dispenser, trapdoor, itrapdoor, leaves, scaffold, honey, wool, obsidian, amethyst, or block states), one per class up to translation (it touches the box's low faces) and the 4 rotations (smallest encoding among the rotations that fit), only fillings with a delay element (torch, repeater, comparator, observer, piston, lamp, sculk, hopper, dropper...); judged self-starting, then with each block placed last; a found clock gets the full `clock sim` analysis and a MINIMAL flag (no single block can go with its period kept). Writes `DIR/clocks.tsv` (every clock: period, whole-state period, warmup, family, blocks, box, duty, edges, pulses, start, type key, canonical encoding, minimal), `minimal.tsv`, `designs/` (the smallest clock of each (period, family)), `summary.md`. Resumable: `DIR/parts/` holds one file per finished chunk (a cell set and its first cell's state); a rerun skips them |

Coming next (this track): clock checks in `door selftest`, and `clock video` (the coordinator
records).

## Census results (redsim, `core` palette, self-start or one block placed last)

`designs/clocks/census/<box>-k<k>/`: `summary.md`, `minimal.tsv`, `designs/` (the smallest clock of
each (period, family); every one of them verified `robust` in the game by `clock verify`).

| box | k | clocks | minimal types | periods: smallest clock per family |
|---|---|---|---|---|
| 2x1x1 | 2 | 1 | 1 | P6: observer pair (2) |
| 3x1x1 | 3 | 33 | 2 | P6: observer pair (2); P12: observer pair + copper bulb (3: the bulb halves the pair's pulses) |
| 2x2x1 (vertical) | 4 | 3621 | 9 | P6: observer pair (2), sticky piston + slime + redstone block (3: QC power from the block above, dragged away by the slime), + note block / lamp / observer variants (4); P8: 4-observer ring (4); P12: sticky piston + slime + redstone block + copper bulb (4) |
| 2x1x2 (flat) | 4 | 1073 | 5 | P6: observer pair (2), sticky piston + slime + redstone block flat (4); P8: 4-observer ring (4) |

Every other filling of these boxes with that many blocks is not a clock (on redsim, with these
starts): with <= 2 blocks the observer pair is the only clock; with <= 3 blocks in a 3x1x1 row, the
pair alone or with a bulb.

Types: `clock::canon` turns and mirrors block states on their text (facing, axis, the four side
properties, rotation, rail shapes, crafter orientation, hinge) and drops the dynamic properties
(powered, lit, power, extended, open, locked, distance...): the type key is the smallest encoding
over the 4 rotations, and a type is chiral when its mirror image has another key.

## What redsim models exactly

`clock sim` is exact for every block `redsim::Block::parse` accepts ([mechanics.md section
12](mechanics.md#12-redsim-the-exact-model-track-redmodel)): dust, repeaters, comparators, torches,
levers, buttons, observers, lamps, waxed copper bulbs, note blocks, targets, iron/oak trapdoors and
doors, oak fence gates, powered/activator rails, dispensers/droppers (single stack)/crafters (redstone
behavior), hoppers, leaves, scaffolding, pistons and sticky pistons with moving blocks, slime and
honey, gravity blocks, sculk and calibrated sculk sensors, the plain blocks of its table, and the
constant comparator sources (cake, composter, end portal frame, respawn anchor). Anything else
(unwaxed copper, entities, item behaviors of dispensers, multi-stack droppers, jukeboxes, chiseled
bookshelves, daylight detectors...) is `UNMODELED`: the real game (`clock verify`) is the only judge.
The bridge and redsim emit no `block_place` vibration for a placement (a sculk clock cannot start
from a placement sound; kick it with a moving piston or a lever).

## Calibration clocks

`designs/clocks/calibration/`: classic clocks built from first principles, each measured (never
assumed) by `clock sim` and `clock verify` (8 positions x 4 rotations):

| design | how it works | P | output on/P, edges | warmup | family | blocks, box | start |
|---|---|---|---|---|---|---|---|
| [observer-pair](../designs/clocks/calibration/observer-pair.txt) | two observers facing each other, each watching the other | 6 | 2/6, 1 | 0 | observer | 2, 2x1x1 | last (the second observer) |
| [comparator-subtract](../designs/clocks/calibration/comparator-subtract.txt) | subtract comparator fed 15 by a redstone block, its output through 3 dust into its own side (15 -> 2 -> 15) | 4 | 2/4, 1 | 2 | comparator+redstone_wire (the comparator's block state never changes: its output strength, in its block entity, does) | 9, 3x2x2 | self-start |
| [torch-repeater](../designs/clocks/calibration/torch-repeater.txt) | wall torch lights a dust ring into a delay-4 repeater pointing into the torch's block | 20 | 10/20, 1 | 0 (whole state: 51, the torch memory fills) | redstone_torch+redstone_wire+repeater | 14, 3x2x3 | self-start |
| [bulb-observer](../designs/clocks/calibration/bulb-observer.txt) | an observer watches a waxed copper bulb; its pulse runs (block, dust, delay-2 repeater) back into the bulb | 28 | 14/28, 1 | 0 | copper_bulb+observer+redstone_wire+repeater | 13, 3x2x3 | last (the bulb) |
| [observer-piston](../designs/clocks/calibration/observer-piston.txt) | a sticky piston lifts a stone in front of an observer whose pulse powers the piston (block, dust under it); the stone is spit on the short pulse | 8 | 4/8, 2 | 3 (whole state: 4) | observer+redstone_wire+sticky_piston | 8, 3x4x1 | last (the stone) |
| [sculk-piston](../designs/clocks/calibration/sculk-piston.txt) | a sculk sensor hears a piston and powers it through two delay-4 repeaters: the retraction comes after the sensor's cooldown | 49 | 30/49, 1 | 5 (whole state: 53) | repeater+sculk_sensor+sticky_piston | 6, 5x2x2 | redstone block 2 gt |
| [hopper-pair](../designs/clocks/calibration/hopper-pair.txt) | two hoppers facing each other pass one item back and forth (`nbt:` line), a comparator reads one | 14 | 7/14, 1 | 3 | comparator+hopper | 4, 3x2x1 | self-start |
| [burnout-torch](../designs/clocks/calibration/burnout-torch.txt) | (clk-rt) torch + delay-1 repeater ring: the torch burns out at its 8th turn-off and relights 160 gt later | 220 | 32/220, 8 | 0 | redstone_torch+redstone_wire+repeater | 10, 3x2x2 | self-start |

All eight: `robust` in the real game (32 builds each), and redsim agrees on period, warmup, output
waveform, family and box. The warmup in the `CLOCK:` line is the block states' (what the real game
shows); `clock sim` also prints when the whole state repeats (hidden state such as the torch
burnout memory or a sensor's last frequency settles later).

## For explorer crates

A track's generator lives in its own crate, already created as a stub for each explorer:
`crates/clock-obs`, `clock-rt`, `clock-sculk`, `clock-item` (each depends on `clock`; its owner
replaces the stub freely). `mctas clock <obs|rt|sculk|item> <args...>` passes every argument after
the track name to that crate's `pub fn run(args: Vec<String>) -> anyhow::Result<()>`, so a track's
commands never touch the shared crates. The in-process API:

```rust
let c = clock::Clock::load(path)?;            // or Clock::from_design(design)
let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 50_000, margin: 4 });
// r.verdict (Clock | Stops | NoRepeat | Unmodeled | Invalid), r.why, r.period, r.warmup,
// r.analysis.family, r.analysis.cells (pos, states, kinds, on, edges, widths), r.output,
// r.blocks, r.swept, r.summary()
let v = clock::verify::verify(&c, &clock::verify::VerifyOptions::default())?; // the judge
```
