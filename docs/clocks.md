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
* **Period P** (game ticks, gt): the smallest P such that every cell's full state (its block
  state, a comparator's output strength, a container's contents and a hopper's cooldown) repeats
  exactly every P gt after a warmup W. The rest of the hidden state must repeat too: on redsim
  the WHOLE state is keyed exactly (`redsim::snapshot`: every cell, the pending ticks with their
  delays, priorities and order, block events, moving blocks, every block entity, the torch memory
  younger than 60 gt) and the first exact repeat gives the whole-state warmup and period (a
  multiple of P: the order of independent ticks due together can permute), which `clock sim`
  prints apart. The real game shows block states every tick and the block entities' data, so the
  judge asks the same full cell states to repeat with period P for at least 3 periods after W.
* **Output**: `port output`, else the changing cell with the longest own period (then the most
  rising edges).
* **Composite** (coordinator's ruling, 2026-10-19): when the output repeats faster than the whole
  structure AND the changing cells split into independent groups (cells linked when at most 2
  apart, or within a sculk sensor's range), the design is independent sub-clocks side by side
  (their period would be the lcm): `COMPOSITE`, rejected by `clock sim`, `clock verify`, `clock
  archive` and the census. One mechanism whose output fires twice per cycle (an observer watching
  both strokes of a piston) is a normal clock of the whole cycle. Beat clocks (the output combines
  sub-clocks that run alone) pass and carry `# composite` in their design: the catalog lists them
  apart. Dividers (flip-flop stages on a base clock) are normal clocks.
* **Waveform**: per changing cell, on-ticks per period (duty), rising edges per period and pulse
  widths; the same at the output.
  "On" ([`wave::active`](../crates/clock/src/wave.rs)): pistons extended, a piston head or moving
  block present, sculk sensors ACTIVE, hoppers locked, doors/trapdoors/gates open, dispensers,
  droppers and crafters triggered, leaves distance < 7, scaffolding distance > 0, else `lit`, then
  `power > 0`, then `powered`; a plain block: present. An output cell holding a plain block or air
  reads the power it receives (`getBestNeighborSignal > 0`).
* **Family**: the set of block types whose own state changes IN PLACE during one period (the same
  block in the cell before and after: lit, powered, power, open, extended, phase, a comparator's
  strength, a container's contents), plus a redstone block that moves (a moving power source),
  detected automatically. A block that only moves (a lamp a piston pushes, never lit) is listed
  apart (`moved only` in `clock sim`); plain blocks never count; piston heads count as their
  piston, wall torches as torches, every waxed copper bulb as `copper_bulb`. Written sorted and
  joined by `+`: `observer`, `comparator+redstone_wire`, `comparator+hopper`,
  `redstone_block+sticky_piston`, `redstone_torch+redstone_wire+repeater`.
* **Type**: the structure (block states with their facings, container contents) up to
  translation, the 4 horizontal rotations and the mirror image (a mirror pair is one type, noted
  `chiral`), and up to the swaps that leave the run identical (vjeux 2026-10-19, `clock::ident`):
  sticky pistons count as pistons when they never pull a block, any inert full solid (stone,
  wool, planks, metal blocks, obsidian...) as stone when its kind changes nothing (both checked
  per design on redsim: every block state, comparator strength and container content at every
  tick of warmup + 2 periods must match modulo the swap). `clock archive --dedupe` removes such
  twins (aliases.tsv: "built with sticky_piston, without piston"). The keys are cached in
  `designs/clocks/archive/type-keys.tsv` (not committed).
* **Dominance fold** (the gallery only, vjeux 2026-10-19): within a period, a clock is listed as a
  variant under a clock with strictly fewer blocks whose block kinds are a subset of its own (the
  observer pair, 2 blocks, folds the observer + note block clock, 3); folded clocks get no
  picture card, only a line "variants of X: name (3 blocks, +note_block)"; coverage.md counts
  types shown and variants folded (variants are left out of the matrix). The archive keeps them.
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
* `entity: <type> <x> <y> <z> [{snbt}]` (header, the door format's; track clk-misc: minecart
  clocks): a design entity, summoned once the blocks stand (before the kick), turned with the
  design (position like the cells, `Rotation` by the game, `Motion` by the judge). `clock verify`
  then fails on any entity that is NOT the design's (a drop, a popped block) and on a design
  entity lost, duplicated or outside the watched box at the end; the period stays the block
  states' (a detector rail, a comparator reading a hopper minecart). Each entity counts as one
  piece in `blocks`, its kind joins the family (`detector_rail+minecart`) and its cell joins the
  type key. `clock sim` answers UNMODELED (redsim has no entities). Example: a minecart on a 3x3
  rail loop with two powered rails on redstone blocks over a detector rail is a robust P476 clock
  (warmup 556; the detector's 20 gt re-check against the lap time).

### Extendable clocks (`extend` headers, vjeux 2026-10-19)

A family of clocks that grow by a repeated step ("add 2 observers and make the tower bigger") is
ONE base design with these headers:

```text
extend family: clk-rt-fade          # the family's id
extend period: 114 + 56n            # P(n) in gt: linear a + b·n, or geometric a·k^n (+ c): 36 * 2^n
extend blocks: 18 + 4n              # blocks(n), the same forms
extend rule: lengthen both rails by one repeater and one dust each   # how to extend, one line
extend n: 0..12                     # the n range judged in the game
extend base: 0                      # the n of this design (default: the low end)
extend generator: mctas clock rt family fade --n {n} --out {out}     # builds any n
```

The generator is one command (split on spaces, no shell; `{n}` and `{out}` are replaced, `mctas`
is the running executable). `mctas clock extend-check <design> [--n a..b | a,b,c] [--record]`
builds and judges instances with `clock verify` (period and blocks must equal the formulas);
`clock archive` of a family design judges n = low, low+1, low+2, the middle and the high end
first, refuses the family when one fails, and records the instances in
`designs/clocks/archive/extend.tsv` (family, n, period, blocks, type key, verdict). A family's
base is archived even when its instance is already archived (the family card is another entry).
`clock catalog` shows one card per family in an "Extendable clocks" section (picture of the base,
P in gt and in seconds, blocks, the rule, the n range and the judged n) and leaves the recorded
instances out of the per-period gallery, with a line "Also reachable by: <family> (n = k)" under
every period a family reaches.

Put your designs in `designs/clocks/tracks/<track>/`; the coordinator archives verified clocks in
`designs/clocks/archive/` with a caption line in `captions.tsv`.

## Commands

| command | what |
|---|---|
| `mctas clock sim <design>... [--horizon 50000] [--margin 4] [--json] [-q] [--trace N]` | the exact period on redsim in milliseconds: place, kick, run until the whole state repeats (W, P), then replay one period for the waveform per changing cell and at the output, the family, the blocks and the swept box. `CLOCK:` line, or `STOPS` (it goes still, or only hidden state changes), `NO-REPEAT` (horizon), `UNMODELED` (a block or case redsim does not model exactly: judge it with `clock verify`), `INVALID` (a drop, or a write beyond the margin). `--trace N` prints every cell change of the first N ticks |
| `mctas clock verify <design>... [--positions 4] [--no-rotations] [--horizon 4000] [--margin 3] [--workers 8] [-v] [--json]` | THE judge, in the real game: every build in a cleared sandbox at the door judge's positions (4 by default, vjeux 2026-10-19) x 4 rotations = 16 builds (the game turns every block state, and a temporary start block), placed and kicked like `clock sim`, every tick of the watched box recorded (block states and block entities); the horizon grows with redsim's period (warmup + 4 periods, up to 20000 gt); exact periodicity of the full cell state for >= 3 periods after warmup, no entity but the design's own (`entity:` lines: still there, same counts, inside the box), never reaching the watched box's edge, the same period/output waveform/family everywhere. Prints `CLOCK: <name> period P gt (T s), on Q/P, edges E, warmup W, family <sig>, <blocks> blocks, <WxHxD>, robust\|position-dependent\|rotation-dependent`, the builds that differ or fail, and the redsim agreement (`redsim AGREES (period, warmup, output waveform, family, box)` or what redsim says); exits 1 unless it is a clock |
| `mctas clock archive <design>... [--caption '...'] [--dir designs/clocks/archive] [--dedupe]` | (coordinator) the judge, then a copy in the archive (header: date, source, the judge's line), its row in `verdicts-v1.tsv` (period, warmup, output duty and edges, family, blocks, box, robustness, start, type key, chirality) and its caption in `captions.tsv` (default: the design's first comment block); a clock already archived (same blocks, ports, kick, block entities) is skipped, and a design of an already archived TYPE at the same period (a rotation, mirror image or move of it) is refused (`SAME TYPE ... same type as <file>`); a name already taken by another structure is archived as `<name>-<6 hex digits of its type hash>.txt` (the same structure gets the same name on every branch). `--dedupe` (no judging): keep one archived design per type (the earliest archived), remove the others (design, pictures, rows) and record them in `aliases.tsv` (the catalog lists them as "also found as") |
| `mctas clock census-report <census dir>... [--out F]` | the MINIMAL clocks of several census runs merged: per (period, family) the fewest blocks, its box and census, the minimal type count, the periods never found below the largest (`designs/clocks/census/README.md`) |
| `mctas clock extend-check <design> [--n a..b \| a,b,c] [--positions 4] [--record] [--dir designs/clocks/archive]` | judges instances of an extendable family (its `extend` headers, above): builds each n with the generator, `clock verify`, period and blocks against the formulas; `OK`/`FAIL` line per n; `--record` adds the passing ones to `extend.tsv` (default n: low, low+1, low+2, middle, high) |
| `mctas clock catalog` | (coordinator) from the archive's verdicts (no game): `designs/clocks/CATALOG.md` = the INDEX (a one-line intro and legend, the "Extendable clocks" cards: one per row, the period formula as a heading `### P = 8 + 4n gt`, the two smallest members side by side (`img/<base stem>.jpg` and `img/<base stem>.n<k>.jpg`, labelled with n, period and blocks), then the seconds and blocks formulas, the rule and the judged n range; then a table of period ranges with their clock types and folded variants linking to the gallery pages); `designs/clocks/gallery/p<lo>-<hi>.md` = the pictures by period (ascending; consecutive periods while a page stays under 120 KB and 300 pictures, a period never split; stale pages removed), per period a 3-wide grid of every distinct type's picture (linked to its design) with one caption line (`20 gt (1 s) · observer+piston · 6 blocks`, "also found as" aliases; "(picture pending)" without a picture), the variants lines of the dominance fold and "Also reachable by: <family> (n = k)"; `gallery/composite.md` the composite (beat) clocks; `designs/clocks/coverage.md` (the period x family matrix of the types shown, mirrored on the board's `minecraft-tas/clocks/coverage.md`); pictures of designs no longer archived are pruned (a family member's `<stem>.n<k>.jpg` stays with its base). The periods a family cannot reach stay data in `designs/clocks/impossible.tsv` (family, periods, why). `clock shot` of a family base also pictures its two smallest members, built with the family's generator |
| `mctas clock census --box AxBxC --k N [--kmin 1] [--palette core] [--max-period 500] [--margin 2] [--no-kick] [--workers W] --out DIR` | the exhaustive census on redsim: every filling of the box with kmin..=k blocks of the palette (every facing of every component: shorthands `core` = stone, glass, rblock, dust, torch (standing + 4 wall), repeater (4 facings x 4 delays), comparator (4 x 2 modes), observer (6), piston (6), sticky (6), lamp, bulb, note, target, slime; also sculk, csculk, hopper, dropper, dispenser, trapdoor, itrapdoor, leaves, scaffold, honey, wool, obsidian, amethyst, or block states), one per class up to translation (it touches the box's low faces) and the 4 rotations (smallest encoding among the rotations that fit), only fillings with a delay element (torch, repeater, comparator, observer, piston, lamp, sculk, hopper, dropper...); judged self-starting, then with each block placed last; a found clock gets the full `clock sim` analysis and a MINIMAL flag (no single block can go with its period kept). Writes `DIR/clocks.tsv` (every clock: period, whole-state period, warmup, family, blocks, box, duty, edges, pulses, start, type key, canonical encoding, minimal), `minimal.tsv`, `designs/` (the smallest clock of each (period, family)), `summary.md`. Resumable: `DIR/parts/` holds one file per finished chunk (a cell set and its first cell's state); a rerun skips them |

| `mctas clock shot <design>... [--out-dir designs/clocks/img] [--tick T] [--strip N] [--width 800] [--hold-ms 1500] [--redo]` | pictures in the REAL client (uses an X display: vjeux allowed it on clockinfra's dedicated box), one client session for all clocks, ~2 s a clock: each clock alone on a grass floor at noon (two sandboxes used in turn, cleared, never dropped; the camera always enters from the other one so the client loads it fresh), kicked like the judge, at the most active moment of a period after its warmup with nothing in motion (or `--tick`; for clocks redsim does not model, mid-period from the archive's judge line), an orange stained glass block where the output signal is (the output cell when air, else the air cell the output powers or its neighbor most visible; written raw only for the picture), a still camera framing its swept box (center crop; an elongated clock, longest side over 16 blocks and 5x the next, gets a 16-block close-up around its output), saved as `<out-dir>/<design stem>.jpg`. A dark (mean luma < 70) or nearly uniform frame is taken again after a longer wait (`BAD` after 2 retakes). `--strip N` also tiles N moments of one period (`<stem>.strip.jpg`). Existing pictures are kept unless `--redo`. `--grid` (experimental: neighbors show) puts many clocks in one world. `mctas clock shot-compare DIR [--against designs/clocks/img]` prints the SSIM of each picture against a trusted set |
| `mctas clock video <design> --out f.mp4 [--periods 2] [--min-secs 8] [--tps T] [--card] [--relabel] [--dry-run]` | (coordinator: shared X displays) the clock running in the real client: the kick and warmup stepped unseen, then >= 2 periods and >= 8 s filmed (10 tps, faster for long periods: <= 60 s), a still camera framing the swept box, a corner label (period; family, blocks, box; duty, start; name), an optional title card; H.264. The clean clip is kept as `f.clip.mp4` (`--relabel` recuts in seconds); `--dry-run` prints the plan and writes the label image |
| `mctas clock montage <design>... --out-dir DIR [--periods 6,8] [--min-secs 6]` | (videos) per period, every given clock's labeled video (each filmed alone; clips already filmed are reused) after a title card (`P gt, N clocks, smallest first`), concatenated into `DIR/montage-p<P>.mp4`; the per-clock videos stay in `DIR/clips/` |

Clock checks in `door selftest`: `clock:sim-calibration` (quick: the 8 calibration clocks' known
period, warmup, output, family, blocks and box), `clock:sim-negative` (quick: the observer pair
placed silently: STOPS), `clock:verify-calibration` (full suite, the real game: observer pair and
subtract comparator robust, redsim agrees), `clock:verify-negative` (full suite),
`clock:verify-entities` (full suite: the observer pair beside a minecart on a rail is robust; with
the cart in the air, falling away, it is rejected).

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
pair alone or with a bulb. The bigger boxes (3x2x1, 3x1x2, 2x2x2, 3x1x3, 3x3x1, 3x2x2 with k 4,
3x3x2 with k 3) are merged by `clock census-report` into `designs/clocks/census/README.md`.

### Periods 2 and 3 (vjeux's question; period 1 is no signal by definition)

`clock census --palette full --max-period 3 --warmup 60` (every modeled component that needs no
item or entity: the `core` palette plus honey, obsidian, oak and iron trapdoors, powered and
activator rails, fence gates, droppers, leaves; ~100 states; self-start or one block placed last;
a candidate whose whole state does not repeat within 12 gt is dropped early) finds NO clock of
period 2 or 3 in: 2x1x1 (k 2), 3x1x1 (k 3), 2x2x1, 2x1x2, 3x2x1, 3x1x2, 2x2x2, 3x3x1, 3x2x2, 3x1x3
(k 4), 3x3x2 (k 3). The same runs with `--max-period 4` find the P4 clocks (24 observer types and 2
observer + piston types in 3x2x1), so the pruning keeps real clocks. The game-source arguments for
the families that cannot reach P2/P3 at any size are in `designs/clocks/impossible.tsv` (clk-rt,
clk-obs).

Types: `clock::canon` turns and mirrors block states on their text (facing, axis, the four side
properties, rotation, rail shapes, crafter orientation, hinge) and drops the dynamic properties
(powered, lit, power, extended, open, locked, distance...): the type key is the smallest encoding
over the 4 rotations and their mirror images, with each cell's `nbt:` data attached (a mirror pair
is ONE type; it is `chiral` when the mirror image is not a rotation of it). The census, `clock
archive` (refuses a design of an archived type: `SAME TYPE`) and `clock catalog` (each type once,
gallery included, the other designs of the type listed as "also found as") all use it.

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
assumed) by `clock sim` and `clock verify` (8 positions x 4 rotations, before the default became 4):

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
