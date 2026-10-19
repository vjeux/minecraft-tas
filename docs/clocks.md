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
* **Period P** (game ticks, gt): the smallest P such that the whole state repeats exactly every P gt
  after a warmup W. On redsim the whole state is every cell, the pending scheduled ticks (delay
  relative to now, priority, order among ticks due together), pending block events, moving blocks
  (moved state, direction, progress, ticker order), block entities (comparator outputs, container
  contents and hopper cooldowns, sculk sensors' vibrations in flight and selector candidates) and
  the torch burnout memory younger than 60 gt (`redsim::snapshot`): the first exact repeat gives W
  and P. The real game exposes block states, so the judge asks every cell's block state to repeat
  with period P for at least 3 periods after W.
* **Waveform**: per changing cell, on-ticks per period (duty), rising edges per period and pulse
  widths; the same at the `output` port (default: the changing cell with the most rising edges).
  "On" ([`wave::active`](../crates/clock/src/wave.rs)): pistons extended, a piston head or moving
  block present, sculk sensors ACTIVE, hoppers locked, doors/trapdoors/gates open, dispensers,
  droppers and crafters triggered, leaves distance < 7, scaffolding distance > 0, else `lit`, then
  `power > 0`, then `powered`; a plain block: present. An output cell holding a plain block or air
  reads the power it receives (`getBestNeighborSignal > 0`).
* **Family**: the set of block types whose state changes during one period (the active set),
  detected automatically: plain blocks (stone, slabs, logs, glazed terracotta...) and moving blocks
  are left out (a moved block's piston names itself), piston heads count as their piston, wall
  torches as torches, every waxed copper bulb as `copper_bulb`. Written sorted and joined by `+`:
  `observer`, `observer+redstone_wire+sticky_piston`, `redstone_torch+redstone_wire+repeater`.
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
* Extra words on any `port` line are kept: `Design.meta["port <name>"]`, written back on the port
  line.

Put your designs in `designs/clocks/tracks/<track>/`; the coordinator archives verified clocks in
`designs/clocks/archive/` with a caption line in `captions.tsv`.

## Commands

| command | what |
|---|---|
| `mctas clock sim <design>... [--horizon 50000] [--margin 4] [--json] [-q] [--trace N]` | the exact period on redsim in milliseconds: place, kick, run until the whole state repeats (W, P), then replay one period for the waveform per changing cell and at the output, the family, the blocks and the swept box. `CLOCK:` line, or `STOPS` (it goes still, or only hidden state changes), `NO-REPEAT` (horizon), `UNMODELED` (a block or case redsim does not model exactly: judge it with `clock verify`), `INVALID` (a drop, or a write beyond the margin). `--trace N` prints every cell change of the first N ticks |
| `mctas clock verify <design>... [--positions 8] [--no-rotations] [--horizon 4000] [--margin 3] [--workers 8] [-v] [--json]` | THE judge, in the real game: every build in a cleared sandbox at the door judge's positions x 4 rotations (the game turns every block state, and a temporary start block), placed and kicked like `clock sim`, every tick of the watched box recorded; exact periodicity for >= 3 periods after warmup, no entity, never reaching the watched box's edge, the same period/output waveform/family everywhere. Prints `CLOCK: <name> period P gt, on Q/P, edges E, warmup W, family <sig>, <blocks> blocks, <WxHxD>, robust\|position-dependent\|rotation-dependent`, the builds that differ or fail, and the redsim agreement (`redsim AGREES (period, warmup, output waveform, family, box)` or what redsim says); exits 1 unless it is a clock |

Coming next (this track): `clock census` (every filling of a box with <= k blocks of a palette, up
to translation and rotation, judged by `clock sim`: the ground truth of the small clocks), `clock
archive` and `clock catalog` (designs/clocks/CATALOG.md: per period the types, families and the
smallest of each, and the period x family coverage matrix), clock checks in `door selftest`, and
`clock video` (the coordinator records).

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
| [comparator-subtract](../designs/clocks/calibration/comparator-subtract.txt) | subtract comparator fed 15 by a redstone block, its output through 3 dust into its own side (15 -> 2 -> 15) | 4 | 2/4, 1 | 2 | redstone_wire (the comparator's block state never changes: its output strength lives in its block entity) | 9, 3x2x2 | self-start |
| [torch-repeater](../designs/clocks/calibration/torch-repeater.txt) | wall torch lights a dust ring into a delay-4 repeater pointing into the torch's block | 20 | 10/20, 1 | 0 (whole state: 51, the torch memory fills) | redstone_torch+redstone_wire+repeater | 14, 3x2x3 | self-start |
| [bulb-observer](../designs/clocks/calibration/bulb-observer.txt) | an observer watches a waxed copper bulb; its pulse runs (block, dust, delay-2 repeater) back into the bulb | 28 | 14/28, 1 | 0 | copper_bulb+observer+redstone_wire+repeater | 13, 3x2x3 | last (the bulb) |
| [observer-piston](../designs/clocks/calibration/observer-piston.txt) | a sticky piston lifts a stone in front of an observer whose pulse powers the piston (block, dust under it); the stone is spit on the short pulse | 8 | 4/8, 2 | 3 (whole state: 4) | observer+redstone_wire+sticky_piston | 8, 3x4x1 | last (the stone) |
| [sculk-piston](../designs/clocks/calibration/sculk-piston.txt) | a sculk sensor hears a piston and powers it through two delay-4 repeaters: the retraction comes after the sensor's cooldown | 49 | 30/49, 1 | 5 (whole state: 53) | repeater+sculk_sensor+sticky_piston | 6, 5x2x2 | redstone block 2 gt |

All six: `robust` in the real game (32 builds each), and redsim agrees on period, warmup, output
waveform, family and box. The warmup in the `CLOCK:` line is the block states' (what the real game
shows); `clock sim` also prints when the whole state repeats (hidden state such as the torch
burnout memory or a sensor's last frequency settles later).

## For explorer crates

A track's generator lives in its own crate (`crates/clock-<track>`, depending on `clock`); ask
clockinfra to register its subcommand under `mctas clock`. The in-process API:

```rust
let c = clock::Clock::load(path)?;            // or Clock::from_design(design)
let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 50_000, margin: 4 });
// r.verdict (Clock | Stops | NoRepeat | Unmodeled | Invalid), r.why, r.period, r.warmup,
// r.analysis.family, r.analysis.cells (pos, states, kinds, on, edges, widths), r.output,
// r.blocks, r.swept, r.summary()
let v = clock::verify::verify(&c, &clock::verify::VerifyOptions::default())?; // the judge
```
