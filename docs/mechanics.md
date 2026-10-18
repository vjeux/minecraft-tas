# Mechanics notes (read from the 26.3 source)

Everything here was read in the decompiled game (`mctas src show <Class>`) and checked in the
simulator. Class names are 26.3's (unobfuscated). Ticks are game ticks (gt, 1/20 s).

## Order inside one tick (`ServerLevel.tick`)

1. world border, weather; 2. sleep check; 3. game time + 1 (`tickTime`);
4. **scheduled block ticks** (`blockTicks.tick`: repeaters, comparators, observers, torches,
   buttons...) then fluid ticks; 5. raids; 6. chunks (random ticks, spawning, broadcast);
7. **block events** (`runBlockEvents`: pistons, note blocks...), FIFO until empty;
8. entities; 9. **block entities** (`PistonMovingBlockEntity.tick`); 10. entity management.

Neighbor updates go out in the fixed order `WEST, EAST, DOWN, UP, NORTH, SOUTH`
(`NeighborUpdater.UPDATE_ORDER`), through `CollectingNeighborUpdater`.

## Pistons (`PistonBaseBlock`, `PistonStructureResolver`, `PistonMovingBlockEntity`)

* **Power** (`getNeighborSignal`): any neighbor except the front gives signal into the piston,
  OR any neighbor of the cell *above* the piston (except the piston itself) gives signal into
  that cell — **quasi-connectivity**. The piston only re-checks when it gets an update, so a
  QC-powered piston can sit unextended until something updates it.
* A power change queues a **block event**; the event re-checks power when it runs (so a pulse
  that ends before the block-event phase is cancelled).
* **Extend:** the resolver pushes the line in front (max **12** blocks; slime/honey drag
  neighbors; `POPPED` blocks like dust/torches are destroyed and drop items). Moved blocks become
  `moving_piston` with a block entity at progress 0; the block entity phase of that tick
  brings it to 0.5, the next tick to 1.0, and the tick after places the real block. A piston
  action started at tick t is finished in the block-entity phase of tick t+2.
* **Retract:** the base itself becomes a `moving_piston` for those 2 ticks (so a retracting
  piston cannot be pushed or pulled until it is done). A sticky piston pulls only the block
  in front of its head (plus whatever slime/honey drag along), and only with event type 1.
* **Short pulses (spitting):** if power drops while the block 2 in front is still a
  `moving_piston` being pushed by this piston (progress < 0.5, same tick, or during the tick
  phase), the retract event is type 2: the moving block is finished in place and **not pulled**.
* **Immovable:** extended pistons, obsidian/bedrock (`getDestroySpeed == -1`), blocks with
  block entities, `PushReaction.BLOCK` (piston heads, moving pistons). `PUSH_ONLY` blocks
  (glazed terracotta) can be pushed but not pulled and are never dragged by slime.
* **Slime/honey:** a moved slime or honey block drags every adjacent movable block, except that
  slime and honey never stick to each other. Non-sticky moved blocks drag nothing: moving a
  piston that sits under a slime block leaves the slime behind.
* A piston that lands somewhere (moved by another piston) runs `onPlace` and extends at once
  if it is powered there.

## Redstone dust (`RedstoneWireBlock`, `DefaultRedstoneWireEvaluator`)

* Vanilla still uses the default evaluator (redstone experiments are off): when a wire's power
  changes it updates itself and its 6 neighbors in **`HashSet<BlockPos>` order — position
  dependent**. The door verifier therefore tests every design at several world positions.
* Dust powers the block below it and the blocks it points into. While a wire computes its own
  input it turns off every wire's signal (`shouldSignal=false`), which is why dust does not feed
  dust through a solid block ("weak power").

## Others

* **Observer:** reacts to *shape updates* of the block it faces (not to neighbor updates),
  schedules a 2 gt tick, turns on, schedules 2 gt more, turns off: a 2 gt pulse starting 2 gt
  after the change, output from its back.
* **Lever:** strongly powers the block it is attached to, weakly every neighbor.
* **Torch:** 2 gt delay; powers every neighbor except its attachment, strongly the block above;
  a lit/unlit change also updates the neighbors of its neighbors.

## More facts read in the source (mechanics track)

* **Dust shape:** the default `redstone_wire` state is a dot, and a dot stays a dot
  (`getConnectionState`: `wasDot && isDot`), powering only the block below. Write an isolated
  dust as a cross (`east=side,north=side,south=side,west=side`) when it must point sideways; a
  dust with one connection becomes a line through it and points both ways.
* **Lever as a source:** a lever is a signal source for adjacent *components* (dust connects to
  it, a repeater whose back faces it reads it), but an adjacent solid block other than its
  attachment is not powered. A repeater reading the lever is a one-way tap.
* **Strong power back-feeds dust:** a block strongly powered by a repeater (or a lever, an
  observer, a torch below it) powers every dust touching it, including dust *on top* of it. A
  dust only *weakly* powers the blocks it points into or sits on (they power components, never
  other dust).
* **Repeaters update quietly:** a repeater switching a solid block in front of it updates that
  block's neighbors only (`updateNeighborsInFront`), not their neighbors. A piston powered
  through that block by quasi-connectivity is not updated: it neither extends nor retracts
  until something next to it changes (silent hold, rb5).
* **Piston heads forward updates:** `PistonHeadBlock.neighborChanged` passes every neighbor
  update to its base, so an extended piston also re-checks when a cell next to its head changes.
* **Slime ignores fragile blocks:** a dragged neighbor with push reaction DESTROY (dust,
  torches, repeaters) is skipped, neither moved nor broken, unless it is in the push line; a
  PUSH_ONLY (glazed terracotta) or immovable (obsidian) neighbor stays too. Floors and wiring can
  hug a slime backbone if they are glazed terracotta.
* **Lamp, copper bulb:** a redstone lamp lights in the tick it is powered and goes dark 4 gt
  after (asymmetric delay, visible to observers). A copper bulb toggles `lit` on every rising
  edge of its power in the same tick (`checkAndFlip`) and a comparator reads it as 15/0.
* **Note block, target:** a note block is a plain conductor whose `powered` state follows
  `hasNeighborSignal` in the same tick (`NoteBlock.neighborChanged`, `setBlockAndUpdate`: it also
  updates its neighbors), so an observer watching it pulses on both edges of its power, with no
  lamp-style fall delay; its `instrument` follows the block below (an observer sees that change
  too). A target is a conductor and a signal source (power 0 unless hit), so dust always connects
  to it (gadget `target-tap`).
* **Walkway rule:** the judge asks the walkway for *no collision*, so collision-free blocks
  (dust, torches, levers, buttons, rails) may sit in it (dust at the door's bottom level on
  floors one lower). The router still keeps out of it.

## Gadget library (`designs/gadgets/`)

Each gadget is a small design plus a drive script whose `expect`s prove the behavior:
`mctas door drive --quiet designs/gadgets/*.drive` prints PASS/FAIL per gadget.

| gadget | blocks | what it proves |
|---|---|---|
| `dual-edge` | observer | an observer watching a lever pulses 2 gt, 2 gt after *both* flips |
| `pulse-stretch` | repeater | a pulse shorter than a repeater's delay comes out exactly that long (2 gt -> 8 gt); a second pulse while it is on is swallowed |
| `spit` | observer + sticky piston | a 2 gt pulse makes a sticky piston push its block and leave it (retract event type 2) |
| `qc-bud` | repeater, piston | a piston is powered by anything powering the cell above it, but only acts when updated (BUD) |
| `lamp-fall-delay` | lamp + observer | lamp: rise 0 gt, fall 4 gt |
| `bulb-toggle` | copper bulb + comparator | T flip-flop: toggles on rising edges only, no delay |
| `slime-nonstick` | slime, glazed terracotta, obsidian | slime drags stone but not glazed terracotta or obsidian |
| `silent-hold` | block + repeater | a quasi-powered piston ignores its source switching on and off until a neighbor update |
| `pulse-bridge` | block, cross dust, block | an observer pulse moved one cell sideways into a repeater's back with 4 blocks (mech-50) |
| `lever-tap` | repeater | a repeater reading the lever feeds a net that the lever's block cannot back-feed (mech-89) |
| `pulse-toggle` | observer + retracted sticky piston | one 2 gt pulse per lever edge drives a piston that is retracted at rest: the on pulse pulls its block (still: normal pull), the off pulse pushes it back and spits it (no torch, no steady line; logic-min) |
| `lever-observer` | observer under the lever | the lever's support is an observer facing up that watches it and fires out of its back (logic-min) |
| `stack-feed` | cross dust + floor + observer | one pulse fires a stack of 3 retracted sticky pistons: a cross dust behind the stack at the top level points into the top piston, its floor powers the middle one and quasi-powers the bottom one (observer-seq s6: a whole slime-free side column per dust) |
| `sculk-hold` | sculk sensor + repeater | a sculk sensor next to a piston holds it 30 gt after a lever flip (each flip is a vibration), then cools down 10 gt; the repeater bridges the sensor's 2 gt latency (category `sculk`; center-mech) |
| `head-forward` | piston head | an extended piston held unpowered (silent hold) retracts when a block is placed next to its HEAD: heads forward neighbor updates to the base (the rb doors' Q release) |
| `dust-reach` | dust | a dust changing power updates the neighbors of its neighbors: a held piston two cells away (dust, block, piston) re-checks and retracts |
| `observer-delay` | 3 observers | a chain of observers watching each other delays a pulse 2 gt per observer, with no floors and around corners |
| `repeater-lock` | 2 repeaters | a latch: a repeater locked by another repeater pointing into its side keeps its output whatever its input does, and follows the input again once unlocked |
| `honey-slime` | slime + honey | slime and honey never stick to each other: two touching structures move independently |
| `rider-observer` | observer on a slime structure | an observer carried by a structure fires 2 gt after EVERY landing, out of its back wherever it landed: a structure's landings become pulses (riders r3: the left structure re-fires the shuttle piston) |
| `rider-hold` | redstone block on a slime structure | a redstone block carried by a structure powers a piston only in one position: the piston ignores every update (its own source going away included) until the structure comes back (riders r2: the left structure holds Q) |
| `landing-fire` | piston moved onto a powered spot | a piston pushed onto a powered cell extends as it lands (onPlace), no neighbor update needed |
| `no-retry` | piston, moving block | a powered piston whose push line holds a moving block queues nothing and never retries when that block lands two cells away: it sits powered and retracted until a neighbor update (speed track's gotcha) |
| `blocked-push` | 2 sticky pistons | a piston whose push line starts with an extended piston does nothing; it pushes that piston once it has retracted (fresh-c: the B window fires on both edges and is a no-op on close) |
| `dust-below` | observer + stone + dust | a dust directly UNDER a block strongly powered by an observer lights (dust reads all six neighbors): a one-way tap from an observer chain into a dust net (flat track: the top chain hands its pulse to R_B's input dust) |
| `odd-pulse` | piston + redstone block + repeater + note block + observer | an ODD-tick pulse with no odd source: a repeater reading a redstone block that a lever-fired piston moves at k1 changes at k1+2d; a note block next to its output turns that edge into a state change an observer sees: pulse k5..k7 on the on flip, k7..k9 on the off flip (speed-2 a1: the first solid door opening in 10) |
| `sculk-delay-line` | calibrated sculk sensor + repeater | (category sculk) a calibrated sensor d blocks from the lever wakes floor(d) gt after each flip, active 10 gt (radius 16, no output into its back): a one-block wireless delay line; a repeater after it gives a 10 gt window whose observer fires exactly twice, while an observer on the sensor fires three times (wake, cooldown, inactive) (sculk-2 s1) |
| `sculk-wool-occlusion` | wool + sculk sensor | (category sculk) a wool block on the line from the lever hides the click entirely; a piston whose line misses the wool still wakes the sensor; with the wool gone the click wakes it floor(d) gt later. Wool is a plain full block, so wool door blocks or supports could make a sensor hear one lever edge only (when a moving wool block is in the way) or a door piston instead of the lever (sculk-2) |

| `sculk-hold` | sculk sensor | (category sculk) a lever click or piston move within 8 blocks wakes it floor(distance) gt later; active 30 gt, then 10 gt deaf |
| `flag-observer` | stone + observer | motion trigger (fresh-b mp6..mp8): a plain block glued to a moving slime structure, watched by an observer that touches no slime (so it is never dragged), gives one 2 gt pulse after each move, out and back; watching the cell the flag moves INTO works as well as the one it leaves |
| `slime-rider` | retracted sticky piston | a retracted sticky piston right under a slime row rides it: pushed down when another piston pulls the row down, dragged back up when the row is pushed back, so it can pump the row from either spot (fresh-b mp6..mp8) |
| `bulb-pulse` | waxed copper bulb + observer | exactly ONE pulse per input pulse/rising edge: a dust standing on a copper bulb toggles it on each rising edge, an observer under the bulb fires once (a lamp would fire twice: lit, then dark 4 gt later). fresh-d s4: fires the bottom-right puller off the P_T pulse without double-firing the middle puller |
| `pump` | observer + repeater + sticky piston | a 3+ gt pulse (repeater delay 2 = 4 gt) makes a sticky piston push a 2-block line one cell and pull back only the first block: the far block advances one cell with no piston touching it (a 2 gt pulse spits the whole line instead; fresh-b mp6..mp8 move C this way) |
| `zero-tick` | 2 pistons (ideal inputs) | (tick-study) a sticky piston that extends and retracts inside ONE block-event phase places the block it pushed in that tick (`PistonBaseBlock.triggerEvent` 180-190: the extending moving block two ahead is finalTick'd); the 0-tick comes from a silently armed partner piston updated by the push that shoves the redstone block powering it (FIFO block events, `ServerLevel.runBlockEvents` 1267) |
| `zero-tick-normal` | 2 pistons | (tick-study) the same with a normal piston: it retracts in tick 1 too, but only sticky pistons finalTick the block two ahead (`triggerEvent` 204-205 just removes the head): the block lands at tick 3 |
| `zero-tick-reach` | 2 pistons | (tick-study) reach-through: a 0-ticked sticky piston with an EMPTY front cell pulls the block two cells away (retract type 1 since nothing it pushed is moving, `checkIfExtend` 101-110; head finalTick'd to air, `moveBlocks(false)` starts at pos+2); the pulled block lands 2 gt later (pulls are never finalized early) |
| `zero-tick-onplace` | 3 pistons | (tick-study) a piston carried into place by a 0-tick extends in the SAME phase (`onPlace` 80-86 after `finalTick`'s `setBlockAndUpdate`): chains of placed pistons all act in one tick |
| `zero-tick-observer` | zero-tick + observer | (tick-study) an observer watching a 0-ticked piston's head cell fires once, 2 gt later (`ObserverBlock.updateShape` 67: any update from its face while unpowered), though no tick boundary shows the head |
| `zero-tick-pulse` | zero-tick + a second partner + repeater, comparator, lamp | (tick-study) a 0-tick POWER pulse (a redstone block placed and shoved away inside one block-event phase): the lamp lights at once and goes dark 4 gt later, the repeater turns on 2 gt later for its full delay (`DiodeBlock.tick` 50-63: it turns on, then schedules its own turn-off), the comparator never changes (it recomputes in its tick, the input is gone) |
| `instant-off` | lever, piston, redstone block, dust, sticky piston | (tick-study) power LOSS is instant: a moved redstone block is a moving piston at once, its dust line goes dark in the same phase and the extended piston at the far end retracts in tick 1 (a torch inverter takes 2 gt) |
| `instant-on` | lever, piston, conductor on a dust step | (tick-study) power GAIN without a moved source: pushing a conductor off a dust step (`RedstoneWireBlock` 150: a wire connects up a step only if the block above it is not a conductor) lights the upper line in the same phase; the piston it feeds extends in tick 1 |
| `landing-next-tick` | lever, piston, redstone block, piston, repeater | (tick-study) a pushed block lands in the block-entity phase of t+2 (`PistonMovingBlockEntity.tick` 294-327), which runs after the block-event phase (`ServerLevel.tick` 409 vs 459): a piston it powers acts at t+3 (k4 for a lever piston), a repeater reading it turns on at t+2+2d (k5) |
| `spit-saves-tick` | observer, sticky piston, redstone block, piston | (tick-study) a block spit by a 2 gt pulse (retract type 2: `isHandlingTick` in the scheduled phase) is placed in the block-event phase of the retract tick, so the piston it powers extends in that very tick (k4 instead of k5) |
| `leaves-delay` | lever, piston, log, persistent leaves, observer | (tick-study) a 1 gt delay element: leaves recompute `distance` in a scheduled tick 1 gt after a neighbor change (`LeavesBlock` 102); every redstone component delays by an even count, so this flips a chain's parity (piston k1 -> leaves k2 -> observer k4) |
| `odd-observer` | lever, piston, 2 observers | (tick-study) an observer watching a cell a lever-fired piston changes fires at k3 (odd), a chained one at k5; the head landing 2 gt later does not re-trigger it (it is powered then) |
| `comparator-odd` | lever, sticky piston, 2 redstone blocks, comparator | (tick-study) comparator subtraction on odd ticks: rear = a redstone block, side = a redstone block a lever piston moves; the output follows 2 gt after each input change: off at k5 (side block landed k3), back on at k3 after the off flip (pulled away at k1) |
| `center-exit` | 4 pistons + 2 partners (ideal inputs) | (tick-study) the center's two-move exit at the floor: A' (above the empty edge cell) 0-tick reach-pulls C into it at t1 (lands t3); at t4 A' is pulled aside and F, silently armed, extends into the freed cell, is 0-ticked by its partner X_F and reach-pulls C out: C out of the opening at t4 (the open floor is 4) |
| `double-move` | 2 sticky pistons + 2 partners (ideal inputs) | (tick-study) one block, two moves, both placed, in ONE tick: the block finalTick'd by the first 0-tick updates the second (silently armed) piston, which is 0-ticked in turn (what a close in 1 gt needs for the center) |
| `event-order` | 2 pistons | (tick-study) block events are FIFO in update order (one update reaches W, E, D, U, N, S in that order, `NeighborUpdater.UPDATE_ORDER`): of two pistons powered by one redstone block that both want the same stone, the west one moves it and the north one, finding a moving head in front, does nothing |
| `landing-race` | 2 sticky pistons, redstone block, sculk sensor | (sculk-ew) `no-retry` + `event-order`: a powered piston Q whose push is blocked by a moving block queues nothing until that block lands; the landing block's update (W, E, D, U, N, S) then queues Q and any piston A the block powers in that order. Q west of the landing cell takes the redstone block away before A acts; turned 90 degrees (`landing-race-turned`) A fires first and is 0-ticked, spitting its block. Why sculk-2's s4/s1 worked facing north/south only (tracks/sculk-ew/NOTES.md) |
| `button-pulse` | stone button + sticky piston | (tick-study) a button pressed between ticks powers at once and turns off in the scheduled phase 20 gt later (wood 30, `ButtonBlock` 97): the piston extends in tick 1 and retracts in tick 20 |
| `torch-delay` | wall lever, block, wall torch, sticky piston | (tick-study) a torch inverts with 2 gt (`RedstoneTorchBlock` 87): off at tick 2 after the lever, the piston it holds retracts in tick 2; relit and extending at tick 2 after the lever goes off |
| `open-4` | 9 door blocks + 13 pistons + 4 partners (ideal inputs) | (tick-study) a whole 3x3 opening clear at tick 4, the open floor (docs/ticks.md section 6): 7 plain pullers at k1; T reach-pulled into its corner cell and C into T's cell at k1 by 0-ticked pullers; at k4 the pullers in the exit cells are pulled aside and two 0-ticked reach-through pullers take T and C out. `open-4-cycle.drive` closes the same machine in 4 (k1 C and T into the top-row cells, k2 C home via A' carried back into E, k3 T home via P_T carried back, k4 TL; one carried-piston chain per tick, two started together interleave in the FIFO queue). `open-4-close3.drive` closes it in 3: T's chain padded by one piston so it runs after C's inside tick 2 |
| `center-return` | 4 pistons + 3 partners (ideal inputs) | (tick-study) the center's two-move return with both moves placed: C 0-ticked from E into the edge cell at k1; at k2 a parked sticky piston is 0-ticked into E, fires the moment it is placed (onPlace) and 0-ticks C into the center: C placed at k2 (the close side's carried-piston step) |
| `corner-return` | 4 pistons + 4 partners (ideal inputs) | (tick-study) the edge block's return through its corner, every move placed: T 0-ticked from the side cell into the corner (k1), a piston 0-ticked into the side cell fires and 0-ticks T into its cell (k2), the corner block 0-ticked in from above (k3) |
| `target-tap` | target block | a target beside a straight dust line is powered by it, a plain block there is not: a target is a signal source, so the dust bends into it and powers it; a torch on the target then reads the line (lamp-sweep: `door resynth` put a torch on a target in place of stone + dust + floor, twice in the speed doors o6 and a1) |
| `log-leaves` | oak log on a moving structure + persistent leaves + observer | asymmetric ODD delay (pareto-mid): persistent leaves (no random ticks) re-read their distance 1 gt after a neighbor changes; a log that LEAVES their cell's neighbor at tick m changes them at m+1, a log that ARRIVES is a moving block (distance 7) until it lands at m+2, so they change at m+3. Leaves at the log's rest cell: observer pulse m+3 on the move out, m+5 on the move back; at the cell it moves into: m+5 out, m+3 back. One leaves block per log (two leaves re-read each other and ripple 1 -> 3 -> 5 -> 7). mp8-leafU (43, 15/17), mp7-leafC (46, 15/13) |
| `rb-lamp` | redstone block on a moving structure + lamp + observer | asymmetric EVEN delay (pareto-mid): a lamp next to the cell a redstone block moves INTO lights when the block lands (m+2) and goes dark 4 gt after it leaves (m+4): observer pulse m+4 on the move out, m+6 on the move back (lamp at the rest cell: the other way round). mp8-lampU (44, 14/16) |
| `rb-dust` (no gadget yet) | redstone block on a moving structure + dust + observer | asymmetric EVEN delay the other way (pareto-mid): dust next to the cell a redstone block moves INTO lights when the block lands (m+2) and goes dark the moment it leaves (m, instant-off): observer pulse m+4 on the move out, m+2 on the move back. mp7-dustU (47, 14/13) |
| `shuttle` (no gadget yet) | lever + delay-1 repeater -> sticky piston -> redstone block -> repeater | a lever level whose falling edge comes 2 gt early (pareto-mid): the piston pushes the block next to the repeater at t2, it lands at t4; closing, it pulls it away at t2 and the repeater's input drops at once. Delay d after it: on at 4 + 2d, off at 2 + 2d. mp7-repU-pgF (50, 14/12), mp7-dustU-pgF (51, 14/11) |
* **Sculk hold (gadget `designs/gadgets/sculk-hold`, top-control):** one sculk sensor next to a
  piston holds it 30 gt after a lever click (weak power on every side, strong power into the
  block below). It wakes in the block-entity phase floor(distance) ticks after the click, so a
  piston whose source is the lever directly retracts at tick 1 before the sensor can hold it:
  cover the first ticks with a 4 gt repeater from the lever. The door's own late pistons move
  inside the 10 gt cooldown, so they do not re-trigger it (rb2s2 50, m51s 48). Doors using it
  rank in the `sculk` category.
* **Silent hold (gadget `designs/gadgets/silent-hold`):** a piston kept extended by a quasi
  source that later turns off WITHOUT updating it stays extended, unpowered, until any later
  neighbor update (a piston head forwards neighbor updates to its base, so a block landing next
  to the head counts). A repeater switching a solid block only updates that block's neighbors:
  feed the piston's quasi cell through such a block to drop the hold silently.
* **Slime drags:** blocks with PushReaction DESTROY (dust, repeaters, levers, torches) next to a
  moving slime block are left in place, not dragged or broken (only a DESTROY block in the
  movement path breaks); their floors are normal blocks and do get dragged (glazed terracotta
  never is).
* **Redstone block shuttle:** a sticky piston pushing a redstone block from next to piston A to
  next to piston B switches A off at once (the block becomes a moving piston that tick) and B
  on 2 gt later (when it lands): one actuator, ordered "A then B"; pulled back it gives "B off
  then A on".
* **Redstone block riding a structure (center-mech):** a redstone block carried by a slime
  structure powers a piston at rest and lets it go when the structure moves. It must touch the
  piston itself: in a quasi-only position its return never updates the piston, which then stays
  retracted. Two pushers can share one powered block between them (v2u2: both side structures
  pushed out from under the walkway off one obsidian block).
* **Sculk sensor (center-mech, gadget `sculk-hold`):** a lever flip is a vibration
  (BLOCK_ACTIVATE/DEACTIVATE); a sensor within 8 blocks goes ACTIVE after `floor(distance)` gt
  plus its block-entity tick (after the block-event phase), stays active 30 gt (power by
  distance, all directions, strongly into the block below it), then COOLDOWN 10 gt during which
  it ignores the door's own piston vibrations. One block = a 30 gt monostable on both lever
  edges; a piston that would see the lever's fall first needs a short bridge.
* **Dust does not pick up a block powered only by other dust** (wire signals are off while a
  wire computes its input): to tap a dust net one-way into another, put a repeater (or a block
  strongly powered by a repeater) between them.
## Gravity blocks (`FallingBlock`, `FallingBlockEntity`) — track gravity

* Sand/gravel/concrete powder schedule a tick 2 gt after being placed (pistons place them at the
  end of a move) or shape-updated; the tick makes them fall if the block below is air, fire,
  liquid or replaceable. A `moving_piston` below counts as support. Slime next to sand does not
  hold it up.
* A falling block that lands on a moving piston (or anywhere it cannot be placed) breaks into an
  item. `Sandbox.idle()` waits for falling blocks to land, so the judge sees that item.
* Gadget `designs/gadgets/sand-drop-tower`: a 3-piston tower lowers/lifts a sand column 3 cells
  (the slime chain lets the bottom piston pull both upper pistons down). Used by door g4.
* An isolated redstone dust is a dot and powers nothing sideways; written as a cross
  (`redstone_wire[north=side,east=side,south=side,west=side]`) it stays a cross after the shape
  update and powers all four neighbors.
* **Pulse-driven pistons (logic-min, gadget `pulse-toggle`):** any door piston that only has to
  move its block once per lever edge can be retracted at rest and fired by a 2 gt observer pulse
  (pull on open, push + spit on close): v2's six pistons all work that way except P_T. The
  measured windows (`door ideal`): side/bottom pistons accept the pulse at any tick on both
  edges; A needs one pulse 0..4 gt after the flip plus the two B-edge pulses.
* **Observers on pistons fire twice:** a piston (or its head cell) changes state at the event,
  2 gt later and again 2 gt after that, so an observer watching it pulses twice per motion (a
  pulse-toggled column then moves out and back). Watch a block that moves once, the lever, or
  another observer instead.
* **OR of two repeaters in one dust:** a dust sandwiched between the output block of one
  repeater (above it) and of another (below it, its floor) is lit by either, and a cross dust
  powers the block it points into: a 2-input OR feeding a torch with 1 dust (logic-min P_T).
* **Sticky retract during a push finalizes, never pulls (seen in rb9):** if a sticky piston
  retracts while the block two in front is still a moving piston it pushed (any progress, as long
  as it has not landed), that block is placed at its destination at once and the piston retracts
  alone. rb9's B pushes A down at t4 and retracts at t6: A stays down, the dance goes on.
* **Early window (gadget `designs/gadgets/early-window`):** a delay-1 repeater reading a block
  fed by an observer pulse and by a delay-2 repeater from a SECOND observer gives a window at
  t+4..6 and t+8..12 (a 2gt hole: a repeater's turn-off tick outranks another's turn-on tick),
  vs t+10..18 for the plain pulse stretcher.
* **Symmetric door control (fresh-c fc1/fc2):** with an A-on-B elevator, T dragged out by the
  right structure and one shuttled redstone block (A off then P_R on / P_R off then A on), no
  hold is needed if (1) A gets one pulse BEFORE the shuttle moves, on both edges (open: A is
  still powered, the pulse is swallowed; close: A spits C down one cell, freeing T's cell),
  (2) the shuttle piston is the lever through one repeater (>= 2 gt after that pulse), and
  (3) the B window comes at the same delay on both edges: on close A is extended again and
  B's push fails (gadget `blocked-push`). Close = spit C, T back, A pushes T and C home.
* **Pulse-driven shuttle breaks the drag:** a 2gt pulse on the shuttle piston spits its redstone
  block, which is then placed in the block-event phase; the piston it powers fires in that same
  phase, before T (lifted 2 ticks earlier) has landed, so T is not dragged. Keep it steady.
* **Pull-type pulse piston:** a sticky piston two cells from a structure (one-cell gap) pulls it
  on the first 2 gt pulse and pushes+spits it back on the second, so a side column can be
  pulled open from OUTSIDE its backbone (fc2's P_L at (-3,2,0): 3 backbone slimes, no pusher
  under the walkway).
* **Calibrated sculk sensor (sculk-2, gadget `sculk-delay-line`):** `CalibratedSculkSensorBlock`
  is active 10 gt (not 30), hears within 16 blocks, outputs on every side except its back, and a
  non-zero signal into its back makes it hear only that frequency (lever on / piston extend =
  10, lever off / piston retract = 9). With nothing behind it, it is a one-block wireless delay
  line: activation floor(d) gt after each flip, d up to 16. Echo rule for every sensor: once it
  is INACTIVE again (wake + 20 calibrated, + 40 normal) any piston move within its radius wakes
  it, so whatever it drives must stop moving before then, or the re-run must be a no-op.
