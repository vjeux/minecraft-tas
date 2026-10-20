# Contraption studies (`mctas circuit`)

Seven studies in the clock study's spirit ([clocks.md](clocks.md)): for each kind of contraption,
find as many structurally different designs as possible for each value of its main number,
smallest first, every one judged in the REAL game, archived and cataloged. The brief is vjeux's
(2026-10-19, "Let's do 1-7"; study 9, TNT dupers, 2026-10-20); clockinfra owns the judge
(crates/circuit, crates/circuit-cli, this page), each track owns its crate
(`crates/circuit-<track>`, `mctas circuit <track> ...`).

| study | track | kinds | axis (the gallery's main number) | metrics |
|---|---|---|---|---|
| 1 pulse circuits | pulse | rising-edge, falling-edge, dual-edge, pulse-limiter, pulse-extender | output pulse length L | delay, min-pulse |
| 2 T flip-flops | tff | tff | mechanism family | latency, edge, min-pulse, min-period |
| 3 delay lines | delay | delay | N | min-pulse, min-gap (one-edge delays flagged) |
| 4 logic gates | gates | gate [not, and, or, xor, xnor, nand, nor, imply, nimply] | the function | latency, glitches, instant class |
| 5 memory | memory | rs-latch, d-latch, counter N, tff-reset, dff, dff-enable, shift N | the function (N) | latency, enable polarity, output style, reset, edge, setup, hold |
| 6 piston extenders | pext | piston-extender N | reach N | extend, retract |
| 7 flying machines | fly | flying-machine | kind + speed (gt per block) | latency, period |
| 8 shulker loaders and unloaders | shulker | shulker-loader [stack], shulker-unloader [stack] | load or unload, item stack size | boxes/min, items/s, cycle, first box |
| 9 TNT dupers | tnt | tnt-duper, tnt-duper-triggered | rate (TNT per minute) / TNT per pulse | period, latency, gap ([variations](../designs/circuits/tnt/VARIATIONS.md)) |
| 10 BUD switches | clk-obs (bud) | bud | the update classes it detects | latency, pulse, re-arm, jams |

No pictures: the website renders every build with redsim (vjeux 2026-10-19: "we don't need images
anymore, we should render them"); the input and output cells of a design are its markers.

## Design format

The door/clock design text format (`crates/mctas/src/design.rs`: `name`, `origin`, `palette`,
`layer Y:` rows = z, columns = x) plus:

```text
name: sub-rising-4
circuit: rising-edge             # the kind (+ its argument: `gate xor`, `counter 4`, `piston-extender 3`)
port input: 0 1 0                # one input; several: port input.<name>: (input.a / input.b, input.s / input.r, input.d / input.e, input.t / input.r, input.clk / input.d [/ input.e])
port output: 3 1 0               # one output; several: port output.<name>: (output.q / output.qbar)
port target: 0 1 2               # bud only: a cell (air) where the judge performs its block updates
stimulus: in 10+ 14- 40+ 41-     # optional, for exploring (the kind's checker makes its own)
```

* **Inputs** (`port input[.<name>]: x y z [args]`): the args say how the judge drives the cell.
  None: the cell is AIR in the design; the judge writes `minecraft:redstone_block` there with
  updates (on) and air with updates (off). `<on-state> [<off-state>]`: custom states written with
  updates (the design holds the off-state, default air; custom states are not turned with the
  design). `lever`: the design's lever there (off at rest), right-clicked at every edge. `button`:
  the design's button there, right-clicked at every rising edge (its own pulse length). Inputs
  change between ticks: on at frames t..t+L-1 is an L gt pulse. Latch inputs may be spelled
  `set`/`reset`/`data`/`enable`/`clock`/`toggle` (= s, r, d, e, clk, t).
* **Outputs** (`port output[.<name>]: x y z`), read every gt: redstone dust gives its power; a
  repeater, torch or observer its own output (15 when on); a comparator its strength; any other
  cell (air, a solid block, a lamp, a piston, a bulb) the best signal INTO it (what a lamp there
  would see). On = level > 0. Put the output where a lamp would go, not on a lamp (a lamp's lit
  state lags 4 gt when it turns off). The main output is `q` when there is one, else the first.
* **A circuit must drive something** (vjeux/coordinator 2026-10-19): the judge puts a redstone
  lamp in every AIR output cell (not a design block: not counted, not part of the type; none for
  kinds read through payloads, piston extenders and flying machines, nor where a piston can move a
  block in: its head cell or the cell after the row of blocks it pushes, any cell within 13 in
  front of it when the design has slime or honey) and the lamp must follow the output: lit
  whenever the output is on, unlit within 5 gt after it turns off (a lamp turns off 4 gt after the
  update that tells it, so shorter gaps stay lit; a later 4 gt flash from a state inside one tick,
  like two inputs placed in the same tick, is a glitch, not a failure), from the first frame the
  circuit may be driven. The metrics still come from the level above, so they do not change. An
  output that changes without a block update fails (`update-less output: ... it drives
  nothing`): a dust that is only REDIRECTED (it changes shape, its power stays the same) updates
  nothing, so a lamp, a torch on a block or a repeater there never reacts (instant
  dust-redirection gates, track delay's 1 gt line). An output cell is therefore air or holds a
  redstone component or consumer (dust, repeater, comparator, torch, observer, lamp, piston, copper
  bulb, note block, door, trapdoor, fence gate, dispenser, dropper, crafter, rail, TNT, bell,
  hopper; a lamp the design puts there is watched like the judge's): a passive block there (a
  stone...) drives nothing by itself and is refused. A dust redirected WITH a power change (the
  input stronger than the power it had: `gate-not-bump-4b`) does update, and passes.
  `mctas circuit archive --rejudge` re-judges an archive after a judge change.
* **Machines**: `port start:` (the clock kick: `last`, `use [T]`, `<state> T`), `travel: dx dy dz`
  (the push or travel direction, a unit vector), `distance: k` (how far a flying machine must
  go), `port payload[.<name>]: x y z` (tracked blocks: the pushed block, the payload),
  `port station[.<name>]: x y z` (a two-way flying machine's return-station block: a design block
  that counts, left out of the machine's tracked shape; calibration: `cal-two-way-machine`).
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
kind's checker says so. All builds passing with the same metrics: `robust`; builds that
disagree, with other metrics or failing (as for clocks): `position-dependent` /
`rotation-dependent` (allowed, flagged; the archive row notes how many builds fail and why, e.g. a
flying machine that flies east-west only). No build passing: `FAIL`. One line:

`CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, robust` or `FAIL: <name> <kind>: <why>`.

`mctas circuit sim` runs the same stimulus on redsim (exact for the blocks it models, in
milliseconds) and prints the same line ending in `redsim`. Both size the stimulus from redsim's
recovery time: how long the circuit takes to come back to rest after every input turns on for 40 gt
and after they turn off, plus 10 gt (at least 20; 100 when redsim does not model the design or it
never rests; `--gap N` overrides). When an event of the kind's own stimulus (run on redsim spaced by
1000 gt) takes longer to settle than that, the gap is that settling plus 10 gt instead (a slow path
the all-on / all-off pair never takes: a crafter latch's nine ingots coming back through a hopper,
79 gt). The circuit stands 40 gt at rest after placement (and after its
kick) before the first input.

### What each kind is judged on

* **Pulse circuits**: one input pulse of each length 1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40 gt,
  then 60, 80, 120, 160, 240, 320, 480... gt up to twice the gap (the gap is the recovery time, so
  a long limiter's or extender's L is exceeded by the longest input)
  (a button: 3 presses), each followed by the gap. The output must be off at rest and back off
  at the end of every gap. Per input pulse, the output pulses (delay from the input's edge,
  length):
  * `rising-edge`: exactly one pulse at the same delay after every rising edge, the same length L,
    nothing on the falling edge; `falling-edge`: the same after the falling edge; `dual-edge`: one
    pulse after each edge (L and delay per edge when they differ);
  * `pulse-limiter`: one pulse from the rising edge, length L for every input >= L (shorter inputs
    may pass shorter); L must be shorter than the longest input, and at least two inputs longer
    than L must come out exactly L (one would not tell a limiter from a delay line that shortens
    every pulse by a fixed amount; a longer gap or a `stimulus:` hint adds longer inputs);
  * `pulse-extender`: one pulse from the rising edge, length L for every input shorter than L and
    never shorter than L; at least two inputs shorter than L (every tested one when fewer exist,
    as for L 2) must come out exactly L inside the handled inputs (from the min-pulse up), so a
    stretcher that adds a fixed amount to every pulse fails.
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
* **tff-reset** (inputs r, t; [crate::seq](../crates/circuit/src/seq.rs)): pulses t, t, t, r, r,
  t, r, t, t, r of 4 gt, then t, r, t, r of 20 gt, each followed by the gap: every t pulse must
  change q exactly once, every r pulse must leave q off with one change from q = 1 and none from
  q = 0 (so a reset from 0 and a repeated reset change nothing). Then t pulses of 3, 2, 1 gt (two
  each) and an r pulse, for the metric only. `latency`: the worst toggle time of the 4 gt t
  pulses; `reset`: the worst clear time from q = 1; `min-pulse`: the shortest t pulse from which
  every longer one toggles.
* **dff** (inputs clk, d): an edge-triggered D flip-flop (a 1-bit register). 8 clock cycles, each
  4 steps held for the gap: d := a with the clock low, the clock rises, d := b, the clock falls,
  with (a, b) = 10, 01, 11, 00, 10, 00, 01, 11. The run is compared with an ideal register run on
  the same schedule (q takes d at each clock edge of its type, holds otherwise), for rising edges,
  then falling edges: at the end of every step q must equal the ideal's q, steady over the second
  half of the step, with exactly one change in a step where the ideal changes and none elsewhere
  (a level latch fails: it follows d while transparent). The type that passes is reported
  (`edge`). Then setup and hold sweeps, each test a prime clock pulse with d = !v (both types end
  with q = !v) and then: `setup-r x` d := v x gt before a rising edge (x = 8, 4, 2, 1, 0: 0 = in
  the edge's own frame), `setup-f x` the same before a falling edge, `hold-r x` / `hold-f x` d := v
  well before the edge and back to !v x gt after it (x = 8, 4, 2, 1). The other type's tests must
  match the ideal (d is steady around that type's edges); the own type's give `setup` / `hold`:
  the smallest tested offset from which every larger one matches (`over 8 gt` when none).
  `latency`: the worst edge-to-q time. **dff-enable** (inputs clk, d, e): the same main sequence
  with e set at the start of each cycle, 10 cycles (a, b, e) = 101, 011, 110, 001, 100, 111, 000,
  011, 110, 101: the ideal loads only when e is on at its edge; metrics `latency`, `edge`.
* **shift N** (inputs clk, d): a shift register of N cells, q = d as sampled N clock edges ago. N +
  12 clock cycles as for dff, with (a, b) taken from two offsets of a 31-bit pseudo-random
  pattern (so each edge type shifts in a different sequence, and a register that samples on one
  edge and shifts on the other fails); compared with an ideal N-cell register of each type (q
  unknown, and free to take one value, until N edges have filled its cells). Metrics `N`,
  `latency`, `edge`. A shift-register cell is judged as a chain of 2 (`shift 2`), so cells that
  race through each other fail.
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
  motion; station cells holding their own block left out of the shape, never out of the frame) and
  a canonical frame; the steady state is frame t + P = frame t shifted by d cells for at least 2
  periods, an in-place run (d = 0, frames changing) winning over a moving run it outlasts. It must
  fly along `travel` and stay intact over `distance` cells (or to the corridor's end); a steady
  shuttle in place over 2+ positions (a two-way machine between its return stations: the round
  trip repeats in place) passes as two-way. `speed`: P / d gt per block; `two-way`: the round
  trip's period and the positions it stands intact at; `latency`: the kick to the first change;
  the box is the design's own.
* **tnt-duper** (study 9, [`crate::tnt`]; no input, an optional kick): the runners record every
  primed TNT (redsim: a prime record; the game: each new TNT entity, read one tick after it spawns,
  is killed by its UUID before it can explode, so explosions never happen). 600 gt after the kick
  the frames must repeat with a period P through the run's second half for at least 8 periods,
  every period must spawn the same n >= 1 primed TNT, and every TNT block of the design must stand
  again in every period (a moved TNT is in flight for 2 gt: the most seen in the period counts).
  `rate` (the gallery's axis): 1200 n / P TNT per minute; `period`, `per period`, `latency` (the
  kick to the first spawn). The spread of the primed TNT's position and motion (precision: the
  game's random 0.02 kick) is in the table, not in the metrics the builds are compared on. In the
  real game each TNT is followed to fuse 1 (removed there): when every TNT of the periodic region
  that reached fuse 1 is at the same point then, bit for bit (8 or more), the line ends with
  `(precise: N TNT at one point at fuse 1)` (the PRECISE variation; it keeps its own catalog card).
* **tnt-duper-triggered** (one input): 6 pulses of 4 gt spaced by the gap; nothing may spawn before
  the first pulse, each pulse's window must spawn the same k >= 1 primed TNT and end with every TNT
  block standing. `per pulse`, `latency` (the pulse's start to its first spawn), `gap`.
* **tnt-duper with `port input:`** (the lever on/off variation): windows of 300 gt with the input
  off, on, off, on; it must dupe periodically (the same period in both run windows, TNT intact) in
  exactly one input state and spawn nothing in the other from 40 gt after the switch. `runs input
  on|off`, `stop` (the switch to the last TNT), `start` (the switch to the first); the axis reads
  `<rate> switchable`. In the game run, a burning TNT touching a pressure plate, tripwire or
  detector rail fails the run (it would react in the game, not on redsim), and a TNT whose fuse is
  missing from its saved data is a judge error.
* **shulker-loader [64|16|1]** and **shulker-unloader [64|16|1]** (study 8, track shulker,
  crates/circuit/src/shulker.rs; the variations and how each is measured:
  designs/circuits/shulker/VARIATIONS.md). The number is the scenario item's max stack:
  cobblestone (the default), snowballs, iron shovels. These kinds are not judged by the waveform.
  The design declares air cells `port items[.k]:`, `port boxes[.k]:` and `port output[.k]:`, and
  the judge puts a single chest on each (not counted, not part of the type). A loader's `items`
  chest is a source the judge keeps topped up between ticks until 3 boxes' worth is supplied; its
  `boxes` chest holds 4 empty boxes. An unloader's `boxes` chest holds 3 full boxes, and its
  `items` chest is a sink the judge empties and counts. The judge empties the `output` chests and
  records every box that arrives (the gt, colour, name, contents) from per-gt block-entity frames.
  An optional `port input: x y z lever` is turned on once at the start; without it the design is
  always on. The design is placed and left 40 gt, then the chests are filled. Pass:
  * a loader delivers 3 boxes, each holding 27 full stacks of the item;
  * an unloader delivers 3 empty boxes, and every item reaches `items`;
  * nothing else reaches an output, no item or box is left in the machine, no entity is alive at
    the end, and the design stands still for 400 gt (it gets up to 5 windows to settle).
  The run is capped at one hopper's pace x 1.25 + 2000 gt. Metrics: `items/s` is, for a loader,
  the items in its delivered boxes over the gt from the start to the last box (hoppers and
  droppers buffer what leaves the source, so the source's outflow would overstate it; a serial
  loader's first box takes about one cycle, so this is its cycle rate, and boxes filled in
  parallel count together); for an unloader, the steady flow into its items chests (per-gt
  block-entity frames) between 10% and 90% of the total, over the gt between those two moments;
  `boxes/min` is that rate over the items per box; `cycle` is the first to the
  last box over the boxes between; `first box` is the gt from the start. A piston drops the box at
  a random offset with a random drift, which can shift a pickup by a gt. Builds within 4 gt (and
  3% of the flow) of the base build are the same circuit (`robust`), and the reported metrics are
  the builds' medians. `circuit sim` reports UNMODELED until redsim models shulker boxes and chests.
  A design that passes its BASE also gets the line `badges`: the variations below it handles, each
  judged at 4 rotations with unstackables (27 items per box: fast runs), unless noted:
  * `always-on`: no start lever;
  * `16-stack` and `unstackable` (64 kinds): the same run with snowballs or iron shovels;
  * `colours`: the boxes are red, blue, a plain one named "Box A", then plain, and each delivered
    box is one the judge supplied, with its colour and name;
  * `restart` (loaders): the source stops for 3000 gt once 1.5 boxes' worth was supplied;
  * `top-up` (loaders): the 3 supplied boxes already hold 10 stacks and a half, 1 item and 26
    stacks (no spare: the dispenser places a random slot), and must leave full;
  * `flush` (loaders): 2.5 boxes' worth: the half box must arrive within 6000 gt after the source
    runs dry for good (longer than `restart`'s pause: a design that waits 3000 to 6000 gt gets
    both; one that flushes at once cannot have `restart`);
  * `mixed` (loaders): 3 iron ingots once, then 16 rounds of cobblestone 64, snowballs 16, dirt
    37, cobblestone 20, snowballs 5, dirt 64 (6 empty boxes): every item must end in a delivered
    box, every box but the last with 27 occupied slots, the last within 6000 gt of the source
    running dry. The rare item's slot stays partial for good, so a design that waits for a box to
    read full jams;
  * `filter` (loaders with a `port reject:` chest): one box's worth of the item in full stacks,
    8 dirt or 2 sticks after each: the box must leave full of the item alone, and every other
    item must reach the reject chest. What the design's own containers hold at the start (a
    sorter's filter hopper) is theirs: it is left out of the end inventory;
  * `box-starved`: 2 boxes at the start (empty or full); the judge adds the rest 3000 gt after the
    2nd box arrives;
  * `drop-free`: entities polled every gt: every item entity is picked up within 8 gt and never
    touches the ground;
  * `tileable(N)` (a `tile: dx dy dz [copies]` header, the step between copies, N its length):
    the judge builds the copies (3 by default) side by side, each copy's ports renamed
    `<port>.t<k>`, and each copy must deliver its own 3 boxes from its own chests (no block or
    port may land on another copy);
  * `combo` (a `port input.mode: x y z lever`): the other job on the same chests with the mode
    lever turned on at the start (a loader judged as an unloader: its `boxes` chest gets full
    boxes, `items` becomes the sink, `output` gets the empties; or the other way). The base run
    leaves the mode lever off.
  After the builds pass, the DROP STRESS: 64 more runs at the rotations in turn, each with other
  drop draws (unstackables, or the design's own item when it sorts; every other run a burst:
  boxes that already hold all but one stack, so they break right after they are placed and the
  drops follow each other within the collection hopper's cooldown). Two failing runs fail the
  design (`fragile`): a drop drifts about half a block before a hopper takes it, longer when that
  hopper is busy, so a missing wall loses one every few runs; one failure alone is tolerated (a
  drop pushed into a block gets out with the entity's own, unseeded, random: a sound design can
  lose one in a few hundred bursts). `MCTAS_SHULKER_STRESS=N` runs N; `MCTAS_SHULKER_BADGE=burst`
  (with `MCTAS_SHULKER_SALT=k`) judges a burst run in place of the base, with its tables.
  The `bill` line is the survival cost of the blocks: iron, quartz, redstone dust, slime balls
  (hopper 5 iron; piston 1 iron + 1 dust, sticky + 1 slime; observer 1 quartz + 2 dust;
  comparator 1 quartz + 3 dust; repeater 3 dust; redstone block 9 dust; dispenser, dropper, torch
  and wire 1 dust). `hoppers awake` (lag) counts the design's hoppers left unlocked at rest: each
  looks for items every gt. What a design's own container entities (a hopper minecart) hold at
  the end counts as left in the machine. The table adds `badge X: yes` or `no (why)` lines.
  `MCTAS_SHULKER_DEBUG=N` (snapshots of the first N gt) and `MCTAS_SHULKER_WATCH=a-b` (1 gt steps,
  snapshots of a..b) add what changed to `--table`: a design aid. `MCTAS_SHULKER_BADGE=<badge>`
  judges that badge's scenario in place of the base (with its tables), the way to see why a badge
  fails. Each build seeds the level's random from where it stands, so the 16 builds and the badge
  runs sample different drops (a drop's offset and drift are random): a design that loses a drop
  now and then shows up as a failing build or a missing badge.
  `mctas circuit shulker shrink <design> [--badges a,b]` (crates/circuit-shulker) removes blocks
  greedily, each candidate judged in the game on the plain scenario at 16 builds and every kept
  badge at 4 rotations, then verifies the result. `circuit archive --remove <study>/<file>` takes a
  design another one dominates out of the archive. `mctas circuit shulker trace <designs>
  [--exact] [--stack N]` records real-game traces for the viewer
  (designs/circuits/shulker/traces/README.md).
* **bud** (study 10, [crate::bud](../crates/circuit/src/bud.rs), the variations and why:
  [designs/circuits/bud/VARIATIONS.md](../designs/circuits/bud/VARIATIONS.md)): no input; one or
  more `port target[.<name>]: x y z [<state-a> <state-b>]` cells, AIR in the design (default states
  `oak_log[axis=y]` / `[axis=x]`), and one output; optional claims `bud: place remove state shape
  update`. The judge performs five classes of block update at every target, 3 times each with a
  gap between (the recovery time, 30..200 gt): `place` (air -> state-a, flags 3 like a player),
  `remove` (-> air, flags 3), `state` (state-a <-> state-b, flags 3), `shape` (the same change with
  flags 2: shape updates only, no block update, how crops grow) and `update` (a pure block update
  around the cell: `updateNeighborsAt`, nothing changes), with idle spans before and after. A class
  is DETECTED when every one of its events gives exactly one output pulse with one latency and one
  length (some but not all: `unreliable`; changing: `unsteady`), not detected when none does; at
  least one class must be detected, every claimed class too; idle gives nothing (`false trigger`)
  and the output is off at the end of every gap (`latches`). Then pairs of events 30, 20, 15, 10,
  8, 6, 4, 2 gt apart (per family: update, state, shape, place/remove) give the `re-arm` time (the
  shortest gap down to which both events give their own pulse); a pair that leaves the output on
  JAMS the design (`jams: <family> pair N gt apart`: reported, and nothing after it is measured).
  Before the pairs, the FOOTPRINT: a place, a removal and a pure update at every air cell next to
  the design that is no port of it (`Circuit::probe_cells`); `footprint` counts the targets plus
  the probe cells where any of them gives a pulse (`>=` when a probe jams it). The box leaves out
  the target cells and the probe block in the probe cells.
  Axis: `detects <classes>`; metrics: latency, pulse, re-arm (per class when they differ:
  `latency 2 gt place / 6 gt remove`), jams, footprint.
  The judge's actions in the game: the bridge's `set` (modes `update` and `shape`) and `update`
  (`neighbors`) ops; on redsim `World::set_block` (flags 3 / 2) and `update_neighbors_at`.

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
gates the function, T flip-flops their family, latches and registers the kind, `counter N`,
`shift N`, extenders `N = <N>`,
flying machines their speed (or `two-way`).

`mctas circuit catalog [--root designs/circuits]` (no game) writes, per study,
`designs/circuits/<study>/CATALOG.md` (by axis value, numbers ascending, the smallest design first:
design link, kind, metrics, blocks, box, robustness, caption) and `variants.md` (the dominance fold,
as for clocks: within a kind and axis value, a design with more blocks that uses every block kind of
a smaller one, plus others, is its variant, listed under it with the kinds it adds); the index
`designs/circuits/CATALOG.md`; and `designs/circuits/catalog.tsv`, one row per archived design
(study, kind, axis, name, path, metrics, blocks, box, family, robustness, variant_of, caption): the
data the website renders.

**Extendable families** use the clock `extend` headers ([clocks.md](clocks.md#extendable-clocks-extend-headers-vjeux-2026-10-19)) with
`extend axis:` for the formula of the study's axis value in n (L, N, the reach; `extend period:`
is read the same), e.g. track pulse's `extend family: limiter-u`, `extend axis: 4 + 4n`, `extend
blocks: 10 + 4n`, `extend generator: mctas circuit pulse family limiter-u --n {n} --out {out}`,
and the same `extend group` headers. `circuit archive` of a family base judges n = low, low+1,
low+2, the middle and the high end in the game (built with the generator; each must pass with the
formulas' axis value and block count) and records them in `extend.tsv`; the catalog shows one
card per family (or group) at the top of the study page (the formula as the heading, the two
smallest members' value and size, the rule, the n range and the judged n), its members leave the
value tables, and `designs/circuits/families.tsv` lists the families for the website.
`mctas circuit gen repeater-line --n N --out F` is the judge's own calibration family (n + 1
delay-1 repeaters: N = 2 + 2n gt).

**Tunable groups** (vjeux 2026-10-19: "we don't need 20 variations of the same design"): designs
that are ONE mechanism with other repeater delays, comparator modes or a longer repeater line are
one card. A design joins a group with a `group: <name>` header (optional `group title:` and `group
note:`, read from the first member carrying them; track generators stamp them, e.g. `mctas circuit
pulse stamp-group`), or automatically when its TUNABLE key (the type key with every repeater's
delay and every comparator's mode masked, `archive::tunable_key`) and kind equal another
design's. A group of two or more designs gets a card under "Tunable groups" (the value range as
the heading, the two smallest members, a table value -> blocks -> design, and the extendable
families carrying the same `group:`); its members leave the value tables. **Equivalent layouts**:
designs with the same kind, value, block count and block kinds as a shown design (the one with the
smallest box, then archived first) are listed on its row as "also N other layouts". `catalog.tsv`
has a `group` column (the member's group) and its `variant_of` names a layout's shown design too.
`mctas circuit catalog` prints, per study, the card count before and after the folds.

## Commands

| command | what |
|---|---|
| `mctas circuit info <design>...` | parse: kind (study), inputs and how they are driven, outputs, payloads, block count |
| `mctas circuit sim <design>... [--table] [--wave] [--stimulus S] [--gap N]` | the redsim screen: the kind's stimulus, the checker's table (`--table`), the waveform (`--wave`: `#` on, `.` off, an output's level as a hex digit), the CIRCUIT line |
| `mctas circuit verify <design>... [--positions 4] [--no-rotations] [--workers 8] [--table] [--stimulus S] [--gap N]` | the judge in the real game (exit 1 unless every design passes) |
| `mctas circuit archive <design>... [--caption C] [--root designs/circuits]` | (coordinator) judge and keep in `designs/circuits/<study>/archive/` |
| `mctas circuit archive --rejudge [--study S]...` | judge every archived design again (the game; family bases with their sampled members): passing ones get their new metrics and `# CIRCUIT:` line, failing ones leave the archive and go back next to their track file (`update-less/` or `rejudged/`, first line: why) |
| `mctas circuit catalog [--root designs/circuits]` | (coordinator) every study's CATALOG.md + variants.md, the index, catalog.tsv |
| `mctas circuit <track> <args...>` | track commands: `pulse`, `tff`, `memory`, `delay`, `gates`, `pext`, `fly` -> `circuit_<track>::run(args)` in `crates/circuit-<track>` |

Tracks: put your designs in `designs/circuits/<study>/tracks/<track>/`; the coordinator archives
the verified ones.
