# Study 1, pulse circuits: world-record targets

vjeux (2026-10-20 12:45Z): "world record kind of implementations for every single one ... quality over
quantity". This page sets, per variation, the record target (derived from the 26.3 mechanics and the
census proofs), the holder and the proof obligation. A design below the bar folds or hides once a
record holds its variation. Judge: `mctas circuit sim|verify` (docs/circuits.md, "Pulse circuits");
the record files live in [wr/](wr/) until archived.

## Categories (every kind, every L)
- **smallest**: the fewest blocks (the judge's count: every block but the input cells; since R7 a
  block the design keeps IN its input cell counts).
- **flat**: one layer high (dims y = 1). A design with a floor under its dust, repeaters or
  comparators is 2 high; "flat (2 high)" is listed when no 1-high design exists.
- **1-wide tileable**: one block wide, with copies side by side at pitch 1 each driven by its own
  input and never by a neighbor's (the judge's `tile:` header, open to pulse kinds since 44b8f83c7).
- **fastest**: the smallest delay (input edge -> output rise); `instant` = 0 gt.
- **min-pulse 2**: every input from 2 gt up is handled. Since R7 a real input makes no pulse under
  2 gt (the judge's repeater), so 2 gt is the floor ("the shortest a real input makes"); the old
  "min-pulse 1" rows read min-pulse 2.
- **long L**: one design covers any L (a constant-size timer tuned by a count or a delay), not a
  chain that grows with L. The bar is the critic's C34: about 5-8 blocks for every L.

## R7: real input devices (judge since 2026-10-20)
The judge drives a plain input like a real build: its smooth stone in the input cell (or the
design's own receiving block there, counted), powered by a delay-1 repeater from the first side
that leads out of the build, the switch placed 2 gt early, so the stone's power changes DURING the
edge's tick (docs/circuits.md, "Inputs"). What it changes for this study:
- An observer can't watch the input cell: the stone never changes state. Every observer edge
  circuit needs a block that does (a lamp, note block, copper bulb, dropper, trapdoor or piston
  beside or in the input cell): the dual-edge record is 2 blocks (observer + lamp: delay 2 / 6 gt,
  min-pulse 2; observer + note block or copper bulb: delay 2 gt both edges, min-pulse 5).
- No input under 2 gt: min-pulse floors at 2, and a 2 gt extender has nothing to extend.
- A piston reacts in the input's own tick, as from any redstone line: the old odd-length trick (a
  piston one tick behind the input, which the block placement between ticks made) is gone with
  it. Odd lengths now come from the hopper item timer (L = 8n - 1).
- A design must not move its input cell's block (the census pistons that pushed the input's
  redstone block are out).
`mctas circuit r7` lists what changes; it skips tiled designs, so `mctas circuit sim` over the
archive is the full check (43 of 185 pulse designs failed it at the merge, 2026-10-20 16:00Z).

## Mechanics that set the bounds
- One block can't separate rising from falling. Before R7 the only 1-block edge circuit was the
  observer watching the input (dual-edge, L 2 gt); since R7 it needs a block that changes state
  beside it, so every edge circuit needs at least 2 blocks.
- The judge's switch and repeater make no game event. A sculk sensor hears only what the design
  emits (and, in the vibration test, the input's lever and the ambient events):
  - a note block plays (`note_block_play`, frequency 10) only when it becomes powered;
  - a trapdoor or door emits `block_open` (10) / `block_close` (9) on each edge;
  - a piston emits `block_activate` (10) / `block_deactivate` (9).
  A sensor is ACTIVE 30 gt (calibrated: 10 gt) whatever the input does, then COOLDOWN 10 gt
  (`SculkSensorBlock.ACTIVE_TICKS`, `CalibratedSculkSensorBlock.getActiveTicks`). A calibrated
  sensor with a back signal accepts only that frequency (`canReceiveVibration`, checked when the
  event is posted). Its back on the input makes it DEAF while the input is on and lets it hear
  as soon as the input falls.
- The vibration is received in the block-entity phase of a later tick (travel floor(distance)),
  so a sculk design's delay is at least 1 gt. Listener radius 8 (calibrated 16): sculk designs
  can't tile side by side.
- Repeater 2..8 gt (it stretches a shorter pulse to its delay), comparator 2 gt, torch 2 gt,
  observer 2 gt pulse after 2 gt, lamp off 4 gt after power, dust 0 gt. Pistons: the block event
  runs at the end of the tick the piston is powered in, and a pushed block lands in the
  block-entity phase. A hopper passes one item per 8 gt; one that receives an item into an empty
  inventory from a hopper that ticked first waits 7 gt (`HopperBlockEntity.tryMoveInItem`: cooldown
  8 - 1), the odd step in the item timer's 8n - 1.

## Record board (R7, re-judged in the game 2026-10-20 16:50Z)

| kind | variation | target | holder | status |
|---|---|---|---|---|
| rising-edge | L 2 | 2 blocks | `pulse-rising-edge-piston-observer-L2` (a sticky piston pushes an observer: 2 blocks, delay 4, min-pulse 4, tiles every 1); min-pulse 2: `pulse-rise-bulb-L2` (4 blocks, tiles every 1) | **record** both |
| rising-edge | L 4..8 even | 4 blocks | `pulse-rise-repeater-L4/6/8` (4 blocks, min-pulse 4); min-pulse 2: `pulse-rise-bulb-repeater-L4/6/8` (6 blocks, tiles every 1) | holds; target min-pulse 2 at 4-5 blocks |
| rising-edge | L 10, vibration-proof | 5 blocks, min-pulse 2 | `pulse-rise-sculk-filter-L10`: note block + calibrated sensor tuned to 10 by a comparator reading a chest, delay 1 | **record** (game, vibration test) |
| rising-edge | L 10 / L 30, vibration-sensitive (quiet builds, input from afar) | 2 blocks (bound), min-pulse 2, 1 high | `pulse-rise-sculk-cal-L10`, `pulse-rise-sculk-L30` | **record** (repo only: the site shows vibration-proof sculk designs) |
| rising-edge | L 30, vibration-proof | <= 8 blocks | none: a plain sensor has no filter | target: an L 10 filtered stage that re-triggers itself, or prove 30 gt needs isolation |
| rising-edge | odd L | any | none since R7 (the overlap designs now give even lengths and folded) | target: an odd step in a timer (the hoppers' 7 gt), as the limiter's 8n - 1 |
| rising-edge | any L (long) | constant size, <= 8 blocks | `pulse-rise-hopper-oneshot-L*` (group): 13 blocks for every L = 8n (8 .. 480 gt), min-pulse 4, delay 6, robust at L 8, 16, 24, 32, 40, 64, 128, 256. A copper bulb holds a two-hopper item timer open; the first hopper's comparator, inverted by a torch under the bulb, toggles it off when that hopper empties | **constant size reached** (was 17 + 4n); <= 8 open: the bulb latch (2), two locks (2: a block and a torch), the end detector (3: comparator, block, torch), two comparator supports and the edge detector (2) |
| rising-edge | instant | delay 0 | `pulse-rise-instant-L2/4/6/8` (group): 8 blocks, delay 0, min-pulse 4 / 4 / 6 / 8. The input lights the output at once through two dusts and a stone beside the output; a repeater passes the input to a sticky piston that pushes the stone away after L (a moving block conducts nothing) | **record** (was delay 1, sculk) |
| falling-edge | L 2 | 2 blocks | `pulse-fall-piston-observer-L2` (the R7 census, k = 2: a sticky piston pushes an observer away and pulls it back on the falling edge; delay 4, min-pulse 6, tiles every 2); fastest and min-pulse 3: `pulse-fall-torch-piston-observer-L2` (delay 2, 4 blocks: a torch on the input block) | **record** (smallest, R7: no 1-block edge circuit); the 3-block `-mp2` turned rotation-dependent under R7 (min-pulse 6 in one rotation) and was removed |
| falling-edge | L 4..8 even | 4 blocks | `pulse-fall-repeater-L4/6/8` (min-pulse 6) | holds |
| falling-edge | L 10, vibration-proof | 5 blocks, min-pulse 2 | `pulse-fall-sculk-filter-L10`: trapdoor + calibrated sensor tuned to 9 (closing only), delay 1 | **record** (game, vibration test) |
| falling-edge | L 10 / L 30, vibration-sensitive | 2 / 4 blocks, min-pulse 2 | `pulse-fall-trapdoor-cal-L10` (the sensor's back on the input: deaf while it is on), `pulse-fall-sculk-L30` | **record** (repo only) |
| falling-edge | L 1 | 4 blocks | `pulse-fall-note-qc-L1` left the archive at the R7 re-judge; after clk-rt's driver fix it still fails ("pulse 2: the input never turned on": its pistons disturb the judge's input driver) | folded; target: a 1 gt falling edge with a clean input side |
| falling-edge | any L | constant size | `pulse-fall-hopper-oneshot-L*` (group): 13 blocks for every L = 8n, min-pulse 6 (the piston lifts the observer away; pulled back, it fires into the bulb) | **constant size reached** (was 17 + 4n) |
| falling-edge | instant | delay 0 | none: a sticky piston pulling a stone out of a dust step opens it in the falling tick (the instant dual edge does), but its push on the rising edge opens the same steps | target: keep the line dead while the rising-edge push moves the stone |
| dual-edge | L 2 | 2 blocks (since R7: an observer needs a block that changes state beside the input) | `pulse-dual-edge-observer-lamp-L2`: the observer watches a lamp the input lights (delay 2 / 6, min-pulse 2, tiles every 1) | **record**; with a note block or copper bulb instead of the lamp the delays are equal (2 / 2) but min-pulse is 5 |
| dual-edge | L 2, instant | delay 0 | `pulse-dual-instant-L2`: 11 blocks, delay 0 on both edges, min-pulse 4. A redstone block feeds a dust line down a step and up another; a stone cuts one step; the input's sticky piston moves it to the other step and back, and while it moves both steps are open | **record** (fastest; was delay 2) |
| dual-edge | L 4..8 even | 4 blocks | `pulse-dual-lamp-repeater-L4/6/8` (min-pulse 4 / 8 / 15, tiles every 1) | holds |
| dual-edge | L 2 rising / 1 falling | 4 blocks | `pulse-dual-observers-qc-L2-1` | the only 1 gt output under R7 |
| dual-edge | L 10, vibration-proof | <= 9 blocks | `pulse-dual-sculk-filter-L10`: 8 blocks with its input block. A note block plays when the input turns on, and a torch on the input block opens a trapdoor when it turns off; both are frequency 10, so ONE calibrated sensor tuned to 10 hears both, and the trapdoor's closing (9) is filtered out. Delay 1 / 5, min-pulse 20 | **record** (game, vibration test) |
| dual-edge | L 10, vibration-sensitive | 2 blocks (min-pulse 30), 3 blocks (min-pulse 15) | `pulse-dual-sculk-cal-L10`, `pulse-dual-sculk-deaf-L10` | **record** (repo only) |
| dual-edge | L 18, 34, 30 + 16n | chains | `pulse-dual-lock-L18/L34` (24 / 28 blocks), `pulse-dual-edge-reset` (33 + 4n): since R7 a copper bulb in the input cell shows the observer both edges; they answer inputs from 40 ticks | hold for min-pulse |
| dual-edge | any L (long) | constant size | `pulse-dual-hopper-oneshot-L*` (group): 13 blocks for every L = 8n from 16, min-pulse about 2 L (an edge during the run cuts it short), robust at L 16, 24, 32, 40; L 64+ needs inputs longer than the judge's longest test | **constant size reached** (the chains cost 24-33+ blocks) |
| pulse-limiter | L 2 | 2 blocks | `pulse-limiter-piston-observer-L2` (min-pulse 4, tiles every 1) | holds |
| pulse-limiter | L 4..8 even, instant | delay 0 | `pulse-limiter-instant-L4/6/8` (group, the instant rising edge judged as a limiter): 8 blocks, delay 0, min-pulse 2 | **record**: folds `pulse-limiter-u-L4/6/8` (10 blocks, delay 2, min-pulse 3) |
| pulse-limiter | L 10..38 even | 10 + 4 per 16 gt | `pulse-limiter-u-L*` / `pulse-limiter-u4` (min-pulse 3, delay 2); an instant one past L 8 needs a second repeater and a longer dust line (12 blocks) | holds for L >= 10 |
| pulse-limiter | L 10 | 5 blocks proof / 2 blocks sensitive, min-pulse 2 | `pulse-limiter-sculk-filter-L10` / `-cal-L10`, `-L30` | **record** (game) |
| pulse-limiter | odd L = 8n - 1 | constant size | `pulse-limiter-hopper-timer-L*` (group `hopper item timer`): 8 blocks with its input block for every L = 8n - 1 (7 .. 2559 gt), min-pulse 3, robust at L 7, 15, 23, 31, 127, 511 | **record**; the only odd lengths since R7 |
| pulse-extender | L 4..8 | 2 blocks (bound) | one repeater (min-pulse 2, tiles every 1 reading the repeater) | **record**; L 2 has nothing to extend since R7 |
| pulse-extender | L 10 | 5 blocks proof / 2 blocks sensitive, min-pulse 2 | `pulse-extender-sculk-filter-L10` / `-cal-L10`, `-L30` | **record** (game) |
| pulse-extender | any L | constant size | `pulse-extender-hopper-oneshot-L*` (group): 13 blocks for every L = 8n, min-pulse 4 (the rising-edge one-shot judged as an extender) | **constant size reached** (was 17 + 4n) |
| pulse-extender | instant | delay 0 | none: the instant dust path ends with the input, so the rest must come from a hold that starts within 2 ticks and ends L after the input STARTS; a repeater's stretched copy ends a fixed time after the input ENDS, and an observer's pulse fires again on the falling edge | target |

Folded at the R7 re-judge (in `designs/circuits/pulse/tracks/rejudged/`, the reason in each file's
first line): the 1-block observer dual-edge and observer+repeater L4/6/8 (the observer watched the
input cell), extender-repeater-L2 (nothing shorter to extend), the census piston pairs L2 (they
pushed the input's redstone block), every odd-length family (psub, odd2: the piston's tick behind
the input; fall-odd and dual-odd now give even lengths and were removed), dual-three-observers-L4-2
and fall-note-qc-L1 (see above). Dual-lock L18/L34 and dual-edge-reset came back with a copper
bulb in the input cell.

The vibration test (docs/circuits.md, "Vibrations"): a sculk design must ignore its own input's lever
(a hard rule, or it says `input-device: far`), and `vibration-proof` says whether footsteps, items and
blocks around it leave it alone; a design that is not proof is its own class (axis `...,
vibration-sensitive`).

Every record also needs its card headline: why a builder picks it (smallest, min-pulse 1, flat,
tileable, fastest).

## Proof obligations
- The pulse census (`mctas circuit pulse census`, k <= 4, full palette, 1.96 G sims) covers
  face-connected designs. It ran under the old input placement; since 2026-10-20 the census drives
  its input like R7 (the judge's stone and repeater from a free side, no 1 gt input), and its bounds
  stand only once re-run. Sculk designs act at a distance, so "no 3-block design" claims must also
  cover sensors placed apart (listener radius) before they count as proofs.
- Each "impossible" in this table needs a census or a mechanics argument written here.
