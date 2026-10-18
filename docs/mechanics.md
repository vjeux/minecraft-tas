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
| `landing-fire` | piston moved onto a powered spot | a piston pushed onto a powered cell extends as it lands (onPlace), no neighbor update needed |
| `no-retry` | piston, moving block | a powered piston whose push line holds a moving block queues nothing and never retries when that block lands two cells away: it sits powered and retracted until a neighbor update (speed track's gotcha) |

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
