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
