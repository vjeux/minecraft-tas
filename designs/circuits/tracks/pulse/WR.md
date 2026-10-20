# Study 1, pulse circuits: world-record targets

vjeux (2026-10-20 12:45Z): "world record kind of implementations for every single one ... quality over
quantity". This page sets, per variation, the record target (derived from the 26.3 mechanics and the
census proofs), the holder and the proof obligation. A design below the bar folds or hides once a
record holds its variation. Judge: `mctas circuit sim|verify` (docs/circuits.md, "Pulse circuits");
the record files live in [wr/](wr/) until archived.

## Categories (every kind, every L)
- **smallest**: the fewest blocks (the judge's count: every block but the input cells).
- **flat**: one layer high (dims y = 1). A design with a floor under its dust, repeaters or
  comparators is 2 high; "flat (2 high)" is listed when no 1-high design exists.
- **1-wide tileable**: one block wide, with copies side by side at pitch 1 each driven by its own
  input and never by a neighbor's. This needs judge support for pulse kinds (a `tile:` header, as in
  the BUD and shulker studies).
- **fastest**: the smallest delay (input edge -> output rise); `instant` = 0 gt.
- **min-pulse 1**: every input from 1 gt up is handled.
- **long L**: one design covers any L (a constant-size timer tuned by a count or a delay), not a
  chain that grows with L. The bar is the critic's C34: about 5-8 blocks for every L. Today's
  families cost 4 blocks per 16 gt and only reach every 16th L.

## Mechanics that set the bounds
- One block can't separate rising from falling: the only 1-block edge circuit is the observer
  (dual-edge, L 2 gt; 4-block census, docs below). So every rising, falling, limiter and extender
  record needs at least 2 blocks.
- The judge's input is a redstone block written with `setBlock`: no game event. A sculk sensor
  hears only what the design emits:
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
  runs in the tick after the input, a pushed block lands in the block-entity phase, which is what
  makes odd lengths possible.

## Record board

| kind | variation | target | holder | status |
|---|---|---|---|---|
| rising-edge | L 30 | 2 blocks (bound), min-pulse 1, flat | `pulse-rise-sculk-L30`: note block + sculk sensor, delay 1 | **record** (game) |
| rising-edge | L 10 | 2 blocks, min-pulse 1, flat | `pulse-rise-sculk-cal-L10`: note block + calibrated sensor, delay 1 | **record** (game) |
| rising-edge | L 2 | 2 blocks | `pulse-rising-edge-piston-observer-L2` (min-pulse 4, delay 5) | holds; target min-pulse 1 in <= 3 blocks |
| rising-edge | L 4..8 even | 4 blocks (census: none in 3) | repeater designs (min-pulse 4) | target min-pulse 1 at 4..6 blocks |
| rising-edge | odd L | <= 4 blocks | overlap designs, 12 blocks | target: two sensors adjacent to one output give 10 + d / 30 + d (d = their travel-time difference) |
| rising-edge | any L (long) | constant size, <= 8 blocks | chains of 17-25 blocks | target: an item timer (hopper: 8 gt per item) or a comparator fade, tuned by a count |
| rising-edge | instant | delay 0 | none (best delay 1: sculk) | target: an input -> dust -> output path that a piston cuts after L; or prove impossible |
| rising-edge | 1-wide tileable | pitch 1 | none (no judge support) | judge `tile:` for pulse kinds, then the piston/observer column L2 and repeater lines |
| falling-edge | L 10 | 2 blocks, min-pulse 1 | `pulse-fall-trapdoor-cal-L10`: trapdoor + calibrated sensor deaf while the input is on, delay 1 | **record** (game); first min-pulse 1 falling edge |
| falling-edge | L 30 | <= 4 blocks | none (21 blocks) | target: deaf calibrated sensor -> note block -> sculk sensor out of the trapdoor's range |
| falling-edge | L 1, 2, 4..8 | census bounds | 4 / 3 / 4 blocks | target min-pulse 1 |
| dual-edge | L 2 | 1 block (bound) | observer | **record** |
| dual-edge | L 10 / L 30 | 2 blocks | none (L 30: 32 blocks) | target: trapdoor + sensor (min-pulse 20 / 40); 3 blocks with a deaf second sensor: min-pulse about 11 |
| pulse-limiter | L 10 / L 30 | 2 blocks, min-pulse 1 | `pulse-limiter-sculk-cal-L10`, `pulse-limiter-sculk-L30` | **record** (game) |
| pulse-limiter | any L | constant size | limiter-u4 (10 + 4n blocks) | as rising-edge, long L |
| pulse-extender | L 2..8 | 2 blocks (bound) | one repeater | **record** |
| pulse-extender | L 10 / L 30 | 2 blocks, min-pulse 1 | `pulse-extender-sculk-cal-L10`, `pulse-extender-sculk-L30` | **record** (game) |
| pulse-extender | any L | constant size | one-shots, 17-25 blocks | as rising-edge, long L |

Every record also needs its card headline: why a builder picks it (smallest, min-pulse 1, flat,
tileable, fastest).

## Proof obligations
- The pulse census (`mctas circuit pulse census`, k <= 4, full palette, 1.96 G sims) covers
  face-connected designs. Sculk designs act at a distance, so "no 3-block design" claims must
  also cover sensors placed apart (listener radius) before they count as proofs.
- Each "impossible" in this table needs a census or a mechanics argument written here.
