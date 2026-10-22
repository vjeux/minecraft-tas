# A 7-segment display that reads as digits

clk-obs, 2026-10-21. Gate G1 draft (docs/process.md). Trigger: vjeux 21:36 on rings12, "This doesn't look
like a 7 display segment": one lamp per segment with dark corners reads as a dot grid.

## Use

Show a number on a wall, read at a glance like a real 7-segment display. One digit shows 0–F for a 0–15
signal (a chest's fill level, an item counter, a composter). Digits sit side by side for bigger numbers.

## The number

BACKLOG row 24 asks for "reads as a digit from 10 blocks; blocks". Readability is a pass/fail
must-have: the 13-lamp bitmap, judged per value. After that, **blocks** are the number (G1 review):
latency is reported only. No block is spent to get under about 1 s (20 gt), because no player compares a
6 gt display with a 20 gt one.

1. **Blocks** for one digit. The judge's 13 lamps are not counted.
2. **Latency** (reported): game ticks from the input changing to the whole face showing the new digit,
   worst case over every change of value. Ceiling 6 gt (see below), kept as information, not as a target.
3. **Visible on-blips** (reported apart from glitches): a lamp that lights wrongly during a change shows
   for at least 4 gt. Prefer 0 when it costs no more than ~10% more blocks.

**Why 0–F and not 0–9:** a 0–15 signal has 16 values and every one should read as something. A player
reading a chest level accepts "F". Counters and scores are decimal, though: a 0–9 variant (values 10–15
as don't-cares) is reported too wherever it is clearly cheaper, and the storage counter is the 2-digit
decimal use.

**The face:** 3×5 with lit corners is the smallest face that reads as a digit (one lamp per segment does
not). The decoded segment lines can drive a bigger face (4×7, segments two lamps long) with face blocks
only: each segment stone feeds one more dust cross per extra lamp, and each corner dust reads the same
stones.

## Input: one 0–15 signal strength (decision)

- Every block a player measures with gives a signal strength through a comparator: chests, barrels,
  hoppers, composters, cauldrons, lecterns, cake, item frames, jukeboxes. The storage counter reads a chest.
  Four binary lines come only from binary counters and redstone computers.
- One wire to bring, not four.
- Cost: strength decays on dust (−1 per block). The display reads the container itself, or a comparator
  chain carries the level losslessly (2 gt per comparator).
- In the judge, the input is a chest the judge fills: `port input.a: x y z strength`
  (compute::strength_items). A player puts any container there, or feeds that chest with a hopper.

## Mechanics, from the game code (26.3, `mctas src show <Class>`)

- **Lamp** (`RedstoneLampBlock.neighborChanged`, `tick`):
  - It lights in the same tick that `hasNeighborSignal` becomes true.
  - When power leaves, it calls `scheduleTick(pos, this, 4)`, and `tick` turns it off only if it is still
    unpowered. So a lamp goes off 4 gt late, and an off-dip shorter than 4 gt never shows.
  - Any on-blip lights it for at least 4 gt: that is the visible flicker.
  - `setBlock(…, 2)`: a lamp changing sends no neighbour updates.
- **A lamp is a conductor** (`SignalGetter.getSignal`: a conductor's signal is
  `max(own, getDirectSignalTo(pos))`):
  - A lamp that dust or a diode points *into* becomes a powered block, and it lights the lamps beside it.
  - In a 3×5 face, a segment's neighbours are its own corners, so the picture stays right. But the judge's
    lamps then carry signal, and `verify::lamp_part` refuses the design unless every corner is also wired
    directly (found on rings13, 21:54).
  - A corner lamp must be lit by a powered block behind it, never pointed into: its neighbours are
    segments.
- **Comparator** (`ComparatorBlock`):
  - `getDelay` = 2.
  - `calculateOutputSignal`: rear 0 gives 0; side > rear gives 0; otherwise SUBTRACT gives rear − side and
    COMPARE gives rear.
  - `getInputSignal` reads a container's analog signal directly or through one conductor block behind it.
    This is the only way to read the chest, and at most 4 comparators can (one per horizontal face).
  - `checkTickOnNeighbor` schedules one tick 2 gt after the first change, and only if the output would
    change. `refreshOutputState` re-reads the inputs at that tick, so the comparator samples: changes inside
    the window merge, and a blip that reverts in time is never seen.
  - Tick priority is `HIGH` when `shouldPrioritize`, otherwise `NORMAL`.
- **Side inputs** (`DiodeBlock.getAlternateSignal` → `SignalGetter.getControlInputSignal`):
  - A comparator's side reads dust power, a redstone block (15) and a diode's direct output.
  - It does not read a powered block, and not a torch: `RedstoneTorchBlock.getDirectSignal` gives power
    only `DOWN`.
  - A repeater's side reads diodes only, which is how locking works (`RepeaterBlock.isLocked`).
- **Repeater** (`DiodeBlock.tick`, `checkTickOnNeighbor`):
  - 2–8 gt delay.
  - It stretches a pulse to at least its delay (when it turns on with the input already gone, it schedules
    the off).
  - Priority is `EXTREMELY_HIGH` when it faces another diode, `VERY_HIGH` when turning off, `HIGH`
    otherwise.
  - It restores a line to 15, so it destroys a strength value.
- **Torch** (`RedstoneTorchBlock`):
  - It switches 2 gt after its base changes (`scheduleTick 2`).
  - It burns out after 8 toggles within 60 gt (`MAX_RECENT_TOGGLES`, `RECENT_TOGGLE_TIMER`) and relights
    after 160 gt (`RESTART_DELAY`). A torch that toggles on every input change limits the input to fewer
    than 8 changes per 3 s. That is why the judge holds each value at least 10 gt (`compute::stimulus`).
  - It strongly powers only the block above it.
- **Dust** (`RedstoneWireBlock`, `DefaultRedstoneWireEvaluator`):
  - Power is the max of the block signal and the neighbouring wire minus 1 (`calculateTargetStrength`).
  - `getSignal`/`getDirectSignal`: dust powers the block under it and the sides its shape points to.
  - Pasted as a dot (every side `none`), it points nowhere, and `getConnectionState` keeps a dot a dot.
    A cross points all four ways.
  - `getBlockSignal` clears `shouldSignal`, so dust ignores power that other dust put into a block.
    Dust-powered blocks light lamps but do not feed dust.
  - **Zero-delay comparison:** a strength-s line run through k dust is left with s − k, so "s > k" costs
    no tick.
- **Update order:**
  - `DefaultRedstoneWireEvaluator.updatePowerStrength` updates the wire and its 6 neighbours through a
    `HashSet<BlockPos>`, so the order depends on coordinates.
  - Scheduled ticks run by time, then `TickPriority`, then scheduling order.
  - So two equal-delay paths to one lamp can finish in either order by position and rotation. The judge
    builds 16 times and reports the worst glitch count.
  - Unequal path delays make a lamp blink during a change. A blink is visible only when it is an on-blip,
    or an off-dip of 4 gt or more.

## Trade-offs

- **Blocks vs latency:** shared thresholds and chains are fewer blocks but more comparator stages
  (2 gt each). Flat, duplicated decoding is more blocks and fewer stages.
- **Input type:** strength is one line, cheap comparisons (dust distance, comparator sides), and sources
  everywhere, but it decays on dust. Binary is 4 lines, every non-monotone segment needs inverters
  (torch, subtract comparator), and the sources are rare.
- **Corners:**
  - Free at the face: a cross dust in each corner cell reads the adjacent segment stones' strong power,
    which is the OR.
  - Explicit: lanes per corner.
  - Decoded per lamp: 13 functions instead of 7, so more windows.
- **Flicker:** equalising path delays costs repeaters. Accepting transient blips is free, and only
  on-blips show.
- **Tiling:** logic inside the digit's 4 columns (face plus a 1-column gap) is deep and narrow, with
  longer lanes. Logic spread sideways is shallower but does not tile.
- **Constants:** containers with items are survival-friendly and compact. Dust decay from a redstone
  block needs more dust.

## Five layouts

The 7 segments light on 3–5 runs of values each, 28 runs in all, 22 of them distinct:
- a: [0] [2,3] [5,10] [12] [14,15]
- b: [0,4] [7,10] [13]
- c: [0,1] [3,11] [13]
- d: [0] [2,3] [5,6] [8,9] [11,14]
- e: [0] [2] [6] [8] [10,15]
- f: [0] [4,6] [8,12] [14,15]
- g: [2,6] [8,11] [13,15]

The corners are cheap as their own functions: cd is off only at 15, ab only at 11, bcg only at 12.

The face module is shared by every layout, about 31 blocks and 0 gt. Behind the lamps, one cell back, are
a dust cross in front of each segment's strongly powered stone and a stone behind each corner lamp. Two
cells back are the segment stones, with a cross dust in each corner cell; it reads the adjacent segment
stones, which gives the OR for that corner.

| # | Layout | Est. blocks | Est. latency (% of 6 gt) | Constraints |
|---|---|---|---|---|
| 1 | **Obvious**: thermometer (B units: subtract comparator = container constant k − level), one-hot `E_v = T_v ∧ ¬T_{v+1}` per value, then per segment an OR of the E_v it shows (a 78-tap grid), corners at the face | ~550 est.; **2761 measured** | ~10 gt est.; **150 gt measured (4%)** | 16 decoders side by side on a comparator chain; 107 wide, does not tile |
| 2 | **Shared run windows**: 15 thresholds, one window per distinct run (22), OR by dust at the segment stone | ~300 | 6 gt (100%) | windows behind the face; tiles if stacked deep |
| 3 | **Per-lamp ON/OFF**: each of the 13 lamps decoded alone, from its on-runs or the complement of its off-runs (whichever is cheaper); no corner OR | ~280 | 6–8 gt | cd/ab/bcg are one window each; e.g. a has 4 off-runs vs 5 on-runs |
| 4 | **Dust-ladder thresholds**: one dust line from the reading comparator holds s − k at its k-th block; repeater taps give "s > k" (15 thresholds for ~30 blocks), feeding layout 2's windows | ~220 | 6 gt (100%) | taps alternate sides of one straight run of 15 |
| 5 | **Strength → binary → Boolean**: a 4-stage ADC (b3 = s ≥ 8, subtract 8, …), then a minimised sum-of-products per lamp with torch NOR gates | ~250 | ~14 gt (43%) | slow; the right layout if the input were 4 binary lines (~200 blocks, ~6 gt without the ADC) |

## Decision

1. Build layout 1 first as the bar (G2).
2. Then search from **4 + 2**: dust-ladder thresholds (the cheapest comparison in the game) feeding shared
   run windows, ORed by dust at the segment stones, corners at the face.
3. Per-lamp ON/OFF choice (layout 3) is a search dimension, not a separate design.

Why: it has the fewest blocks per comparison, it reaches the 6 gt ceiling, and it can stay within 4 columns
by stacking windows behind the face.

## Must-haves

- The 3×5 face of 13 lamps: each corner lit iff a segment it touches is, the interior cells (1,1) and (1,3)
  dark. The judge checks every value's whole bitmap (compute::CORNERS, digit_face; pushed on
  explore/clk-obs dd045bbaa0).
- One strength input, read from a container on an outer face where a player can place their chest or a
  hopper.
- No flicker on a settled value: after the latency the face holds until the input changes. The judge
  already requires this.
- Robust in all 16 builds (offsets and rotations). Survival materials only.
- Tileable: every block within the digit's 4-column band, so digits tile at pitch 4, each with its own
  input.
- Nothing in front of the face.

## Not goals

- Zero transient blips. They are counted and reported; a player sees only on-blips.
- Decimal 0–9 or two digits: that is the storage counter's brief.
- Brightness and colour.

## Judge changes this needs (done on explore/clk-obs)

- `seven-segment 4 hex`: one strength input `a` (today's ports use 4 binary lines).
- The walk visits every ordered pair of values: 240 changes, an Euler tour of the complete digraph on 16
  values. Each value is held at least 10 gt.
- `verify::sizing` now drives a strength input to 15 and back. It also counts the walk's level changes,
  which it used to miss: every strength-input kind was held only 10 gt per value. For the obvious design it
  now holds 160 gt.
- Latency on lamp power as today (`trace::level`). The lamp's own 4 gt off-delay is the same for every
  design.

## Ceiling (latency): 6 gt

- **Lower bound 4 gt.** Every lamp is lit at some value and off at a larger one (a: 0/1, b: 4/5, c: 1/2,
  …, cd: 14/15), so some *increasing* change must turn it off.
  - At 2 gt only the chest's readers have switched.
  - A comparator with a constant side, dust and blocks are all non-decreasing in s, so power can only rise
    by then.
  - At least one more stage is needed: 4 gt.
- **4 gt is unreachable with one chest.**
  - At most 4 comparators read it.
  - A stage-2 window `[lo, hi]` needs a full-strength side that is on exactly from hi+1, and from 4
    readers (with dust distance) that exists for at most 4 values of hi+1.
  - The 13 lamps' runs end at 13 distinct places: hi+1 ∈ {1,2,3,4,5,7,9,10,11,12,13,14,15}.
- **6 gt is reachable:** read (2) → threshold comparators at any k (4) → subtract windows whose rear is a
  reader through dust (6) → dust OR, corners at the face (0).
- In game, a lamp turning off shows 4 gt later than that, for every design alike.
- Blocks have no useful lower bound (about 60 by a loose count: 1 reader, about 20 windows, 13 drivers).

## The 0–9 display, built (2026-10-22 01:30)

`designs/circuits/tracks/display/decimal/decimal7-v2.txt` (the pick; v1 beside it: 506), generated by
`mctas circuit display decimal7` (`crates/circuit-display/src/decimal7.rs`; 513 blocks), then `display
settle` and `display trim --all` (16 blocks: face walls and unused last ladder cells). Judge:
`seven-segment 4 hex decimal`, all 90 ordered pairs of 0–9, every face bitmap. **Real game (`circuit
verify`): robust, 496 blocks**, latency 32 gt (1.6 s), glitches 25, **on-blips 0**, box 13×10×22.
The reviewer rendered all ten digits from the front (devvm55865): every one reads as its digit.

**Side by side: not at pitch 4.** The build is 13 wide (the face is 3 of it, the decoder runs along the
width), so two digits side by side need pitch 13. The brief's tileable-at-pitch-4 must-have is not met.
Two digits fit at pitch 4 only with one decoder behind or above the other and longer lanes, which was
not built.

How it works:

- **Decode.** One chest reader feeds three dust ladders, so ladder cell d holds s − d. The second and
  third ladders are stacked above the first. They are reached by a lossless climb: a compare
  comparator powers a block, and the dust on top of that block reads its strong power, s, at full
  strength.
- **One track per segment.** Each segment's runs sit on its own side of a ladder. Every stage ends in a
  diode pointing into the segment's dust bus, which is the OR. The stages:
  - a bottom run [0, hi] is a torch on cell hi's support;
  - a top run [lo, 15] is a tap reading cell lo − 1;
  - an inner run [lo, hi] is a subtract comparator, rear from the tap at lo − 1 and side from the tap
    at hi. For one value the side is a single dust. Otherwise it is a dust line with one repeater
    into the side, which arrives within the comparator's own 2 gt, so the run never flashes on.
  - e's {6, 8} is a cascade: [6]'s comparator points into [8]'s rear block.
- **Segment runs.** a: [0] [2,3] [5,15]; b: [0,4] [7,15]; c: [0,1] [3,15]; d: [0] [2,3] [5,6] [8,15];
  e: [0] [2] {6,8}; f: [0] [4,6] [8,15]; g: [2,6] [8,15]. b and c share a track on separate buses.
- **Face.** Seven routed lanes run from the bus ends to the face module of the obvious design (corner
  dusts OR their segments).

How it got here:

| layout | blocks | lanes | note |
|---|---|---|---|
| star: ladders E, W, S of the reader | 634–667 | 239–272 | sources point six ways |
| three parallel ladders north, comparator copies | 541 | 161 | on-blips 4 (x → 7: f, g) |
| same, dust side lines (no repeater chains) | 549 | 161 | on-blips 0 (+8 blocks, 1.5%) |
| two stacked + one copied west | 527 | 146 | |
| three stacked (two climbs) | 521 | 142 | trimmed: 506, game-verified (v1) |
| same, segments g d e f a cb on the six tracks | 513 | 133 | trimmed: **496**, game-verified (v2) |

**The bar, built (G2 for 0–9):** `mctas circuit display obvious --decimal` with the face moved beside the
ROM (`OBVIOUS_FACE=-8,3,12`), then `display chests` and `display settle`: one-hot E0..E9 (the comparator
chain with constant hoppers), a 49-tap ROM, seven collectors and the same face. Redsim: 1668 blocks,
latency 98 gt, glitches 59, on-blips 14, box 68×10×29. Trim and game: see below. The decoder column is 6
wide: two constant comparators with their hoppers, the window and its side path over the chain take 5
columns, and the chain's comparator/stone alternation makes the pitch even. So the ROM's E lines sit 6
apart, and the seven collectors cross about 50 columns. decimal7 is about **3.4× smaller**, with no
on-blips against 14.

Picture check: the reviewer's renders (above). This box has no Vulkan adapter. `mctas circuit display
faces <design> --out <dir>` writes the design once per digit, with its chest replaced by a hopper at that
level, for `mctas web shot <dir>/*.txt -o <shots> --yaw 180` on a box with lavapipe.

## The search, the first hours (2026-10-22 00:35)

Approved at G1 (reviewer and coordinator): search from 4 + 2, with blocks first. What the first hours found
(counts from `crates/circuit-display/src/compact7.rs`'s tests, geometry checked against the mechanics
above):

- **Term counts.**
  - Hex: the 7 segments light on 28 runs, 22 of them inner windows.
  - Best-polarity sum-of-products is 25 terms and 86 literals for the segments, plus 10 terms and
    40 literals for the corners as lamps.
  - 0–9 (10–15 don't-care): 8 inner windows, 6 taps and 6 torches. With ON/OFF chosen per segment it is
    7 single-value windows (E1 E2 E3 E4 E6 E7 E8), 3 torches and 1 tap.
- **What is local on a dust ladder.**
  - A single-value window [v,v] is local. A tap at cell v−1 powers the comparator's rear block, and the
    tap at cell v points into its side.
  - Two singles two values apart on one side of the ladder share a cell. The first one's output is the
    second one's rear block, so the second outputs the union (E6 → E8 gives e's {6, 8}).
  - The off-values 1, 4, 7 (E1, E4, E7) fit exactly on one side, and E2 and E6 on the other.
  - A longer run is not local: its side input must read cell hi, and a repeater at the comparator's side
    can only read the cell beside it.
- **Collecting terms per segment is the cost.** The face module takes one strong input per segment stone,
  and side segments can take up to three. A segment with 3–4 terms needs routed lanes (about 7–8 blocks
  each with supports) or an OR row of target stones under a dust line.
  - The router now has a strict mode: a lane ends with a repeater pointing into its target, for targets
    that comparators and repeaters read.
  - Every complete layout counted that way lands at **300–400 blocks for 0–9** and **450–600 for hex**.
    The obvious design laid out compactly is about 400–500 (0–9) and about 550 (hex).
- **Layout 5 is no cheaper.** A torch PLA costs about 3 blocks per literal touch (a dust stub into the
  term's base), about 600 for hex with the ADC.

Conclusion so far: with these primitives, a compact design sits near the obvious design's size, not
clearly below it. A clear win needs a placement search, an annealer over voxel layouts judged on redsim
(like `circuit compact`, but free to restructure), or a decode idea that shares terms across segments
better than runs and products do.

## The obvious design, measured

**The G3 bar** (review): the cheaper of a compact re-lay of the obvious design (~550 blocks by hand
estimate) and rings12 plus the corner OR at the face (587 + about 31). It is never the routed build below,
whose size is the router's artifact ("routed obvious").

`designs/circuits/tracks/display/obvious/obvious-v1.txt`, the routed obvious design, generated by
`mctas circuit display obvious`, then `display chests` and `display settle`.

**Redsim judge: passes.** 2761 blocks, latency 150 gt (worst change 0 → 15), 167 glitches, box 107×10×38.
- That is 4% of the 6 gt latency ceiling, and 27 times the 107-column width a tileable digit allows (4).
- The game judge (16 builds) runs next.

How it is built:
- **Level chain:** the chest's level travels along a lossless comparator chain (a comparator every 2
  blocks).
- **Decoders:** 16 decoder columns at a pitch of 6. Each has a north B_{v+1} and a south B_v (a subtract
  comparator whose rear is a container constant k and whose side is the level: k − L > 0 iff L < k), and a
  subtract comparator `E_v = O_{v+1}` gated by `O_v`.
- **ROM:** each E_v line crosses 7 segment collectors at a pitch of 3. Where the segment shows v, a repeater
  in the line powers a stone that the collector's dust steps down onto (78 taps). Elsewhere the collector
  bridges over the line on a stone.
- **Face:** 7 routed lanes go to the face module. Each ends in a walled dust socket and a repeater that
  strongly powers the segment's stone; a dust cross in front of it lights the lamp, and a dust cross in each
  corner cell gives the OR of its segments.

Where the numbers go:
- **Latency:** the chain's 48 comparators (about 96 gt to the far column) plus 8 collector repeaters
  (16 gt).
- **Blocks:** the collectors (7 × 107 cells of dust on stone) and the 16 decoder columns.

Mechanics this build ran into (each is now handled in the generator):
- **Diagonal dust steps.** A dust steps down onto the dust diagonally below it whenever the cell above that
  dust is not a conductor (`RedstoneWireBlock.getConnectingSide`). A collector standing on a tap stone
  joined the line beyond it and drove that value's line, so every crossing now has a stone cap over the
  dust after a tap.
- **Weak power in a block.** A repeater reads the weak power that a dust puts into the block behind it
  (`DiodeBlock.getInputSignal` → `SignalGetter.getSignal`, which counts a wire's `getDirectSignal`). Dust
  ignores that power (`getBlockSignal` clears `shouldSignal`). So a dust, never a repeater, comes right
  after a stone that a collector stands on.
- **Pasted dust shape.** Dust pasted with no side properties is a dot and stays a dot. The settle tool now
  keeps crosses.
- **The router's target model.** A routed lane "lights" its target by powering a block beside it, as for a
  lamp. A repeater behind a stone does not see that, so the lanes end in a dust socket, walled so that no
  other lane runs beside it.
