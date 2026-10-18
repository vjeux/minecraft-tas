# Pistons and moving blocks: inventory from the 26.3 source

Track pistmodel. Everything here is read in the decompiled game (`mctas src show <Class>`; line numbers
are that listing's) and is what `crates/redsim` (the exact Rust model) implements and the differential
fuzzer checks against the bridge (real game code). Ticks are game ticks. "t" = the tick of the block
event that starts an action.

## 1. Where piston things run in a tick (`ServerLevel.tick` 358-460)

| phase | what pistons do there |
|---|---|
| scheduled block ticks (`blockTicks.tick`) | observers/repeaters/torches change power: pistons only QUEUE block events (`checkIfExtend`); falling blocks start falling (`FallingBlock.tick`) |
| block events (`runBlockEvents` 1267) | `PistonBaseBlock.triggerEvent`: every extend/retract happens here. FIFO `removeFirst` until empty; events queued meanwhile run in the same phase; `ObjectLinkedOpenHashSet` (215): an identical pending (pos, block, b0, b1) is dropped; `doBlockEvent` 1294 runs it only if the cell still holds that block TYPE (piston vs sticky piston) |
| entities | `FallingBlockEntity.tick` (gravity, landing, breaking into an item); item entities (drops) |
| block entities (`tickBlockEntities`, Level 532) | `PistonMovingBlockEntity.tick`: progress 0 -> 0.5 -> 1.0 -> placed. Order = creation order of the tickers (`blockEntityTickers`, a list; `LevelChunk.updateBlockEntityTicker` rebinds an existing wrapper when a new BE replaces one at the same pos without removal, so it keeps the old slot) |

`isHandlingTick()` is true from the start of the tick through the block-event phase, false in the
entity and block-entity phases and between ticks (player actions).

## 2. Piston base (`PistonBaseBlock`)

* **Power** `getNeighborSignal` (116-136): `hasSignal(pos+d, d)` for the 5 non-front sides, then
  `hasSignal(pos, DOWN)` (dead for pistons: not a conductor, not a source), then
  **quasi-connectivity**: `hasSignal(above+d, d)` for the 6 directions except DOWN (the piston itself).
* **When it checks:** `neighborChanged` (71), `onPlace` (80: only if the block type changed and there is
  no block entity at pos, so a landing piston checks), `setPlacedBy` (player placement). Nothing else:
  **BUD** (a QC- or otherwise powered piston that gets no update never acts) and **no retry** (a check
  whose push does not resolve queues nothing, ever).
* **checkIfExtend** (93-114): powered + retracted -> queue event 0 only if `PistonStructureResolver`
  resolves NOW; unpowered + extended -> queue event 1 (retract and pull), or event 2 (retract and DROP)
  when the cell 2 ahead is a `moving_piston` facing the same way whose entity is extending and
  (`getProgress(0)` < 0.5 (= progressO, i.e. its first block-entity tick has not passed the
  second one yet) or `gameTime == lastTicked` (it already ticked this tick) or `isHandlingTick()`).
* **triggerEvent** (139-213): re-checks power first: retract events (1, 2) with power back -> just
  `setBlock(extended=true, 2)`, return; extend (0) without power -> return.
  * **Extend (0):** `moveBlocks(extending)`; if it fails nothing happens; else
    `setBlock(pos, extended=true, 67)` (neighbor + shape updates, movedByPiston), game event
    BLOCK_ACTIVATE.
  * **Retract (1, 2):** (a) if the head cell holds a moving-piston entity (the head still extending)
    it is `finalTick`ed (source -> air); (b) base -> `moving_piston[type=sticky|normal]` with
    `setBlock(.., 276)` (no updates, no BE side effects, movedByPiston) plus a source entity
    (movedState = the RETRACTED base, extending=false); (c) `updateNeighborsAt(pos)` +
    `updateNeighbourShapes(pos, 2)`; (d) sticky: if the cell 2 ahead is a moving piston extending in
    the same direction, `finalTick` it (**placed now, never pulled**: the 0-tick / spit rule); else if
    b0 != 1, or air, or not pushable (pull direction), or its push reaction is not PUSH_PULL and it is
    not a piston -> `removeBlock(head cell)` (flags 3); else `moveBlocks(retracting)` (the head is
    removed inside with flags 276 first: if the pull then fails to resolve, the piston retracts alone
    and the head vanished without updates). Normal pistons: `removeBlock(head cell)`.
    Game event BLOCK_DEACTIVATE.
* **isPushable** (215-257): air yes; pistons: only retracted (and no block entity); else
  `getDestroySpeed == -1` no (bedrock, barrier, command/structure/jigsaw blocks, end portal frame...);
  push reaction IMMOVEABLE no (obsidian, crying obsidian, reinforced deepslate, respawn anchor,
  lodestone, anvils, grindstone, piston heads, moving pistons, portals, end gateway...); POPPED only
  when destroyables are allowed; PUSH (glazed terracotta) only along the connection direction (pushed
  in line, never pulled, never dragged); finally any block entity no (chests, furnaces, hoppers,
  droppers, dispensers, sculk sensors, lecterns, ...). POPPED is checked BEFORE the block entity, so a
  POPPED block with a block entity is destroyed (beds, banners, decorated pots).
* **moveBlocks** (259-359), in order: resolve; remember every moved state (`deleteAfterMove`, a
  `HashMap<BlockPos,BlockState>`: its ITERATION ORDER = Java HashMap order, bucket
  `(h ^ h>>>16) & 15` with `h = BlockPos.hashCode()`, then insertion order: it orders the air
  placements and shape updates below); destroy every `toDestroy` (reverse order): `dropResources`
  (items: the judge fails), `setBlock(air, 18)`, game event BLOCK_DESTROY; every moved block (reverse
  list order) becomes `moving_piston[facing]` at its DESTINATION with `setBlock(.., 324)` (no updates)
  + an entity (movedState, direction, extending, source=false); extending: the arm cell becomes the
  moving head (source=true, movedState = `piston_head[type]`); every vacated cell -> `setBlock(air, 82)`
  (no neighbor/shape updates), then for each vacated cell: old state's indirect shape updates (dust),
  air's shape updates to the 6 neighbors, air's indirect shape updates; then neighbor updates:
  destroyed cells (`affectNeighborsAfterRemoval`, indirect shapes, `updateNeighborsAt`), moved cells
  (`updateNeighborsAt(old pos, moved block)`, reverse order), finally the arm cell
  (`updateNeighborsAt(arm, PISTON_HEAD)`).

## 3. Structure resolver (`PistonStructureResolver`)

* Start = front cell (extend) or the cell 2 ahead (retract, push direction = opposite). Start not
  pushable: extending into a POPPED block destroys just it; else fail.
* `addBlockLine`: a line of sticky blocks is collected BACKWARDS first (slime/honey chain, stops at
  air, at non-sticking pairs, at unpushable blocks (glazed terracotta never: connection = opposite),
  at the piston), then forward: each next cell air -> done; unpushable or the piston itself -> FAIL;
  POPPED -> destroyed, done; already in the list -> reorder at the collision, branch its sticky
  blocks, done; > 12 blocks -> FAIL.
* Branching: every sticky block in the list drags its neighbors on the 4 sides perpendicular to the
  push axis (Direction order D, U, N, S, W, E) through `addBlockLine(neighbor, side)`: a perpendicular
  unpushable neighbor (obsidian, extended piston, block entity, glazed terracotta) is simply skipped;
  POPPED neighbors (dust, torches...) are left in place (only a POPPED block in a forward line
  breaks).
* Stickiness: slime-slime, honey-honey, slime/honey with any other block; slime-honey never.
* Push limit: 12 blocks (moved list; destroyed blocks do not count).

## 4. Head, moving piston, moving block entity

* `PistonHeadBlock`: survives only in front of a fitting EXTENDED base (same type and facing) or a
  `moving_piston` with the same facing; `updateShape` turns it to air when the base-side neighbor
  changes and it cannot survive; `neighborChanged` forwards every neighbor update to its base (an
  extended piston re-checks when a cell next to its head changes); `affectNeighborsAfterRemoval`
  destroys a fitting base WITH drops.
* `MovingPistonBlock`: no own block entity (pistons attach `PistonMovingBlockEntity`), immovable,
  empty shape, collision shape from the entity, `destroy()` removes an extended base behind it.
* `PistonMovingBlockEntity.tick` (294-327): `lastTicked = gameTime`, `progressO = progress`; if
  progressO >= 1: remove the entity and, if the cell is still a moving piston, compute
  `updateFromNeighbourShapes(movedState)` (each `updateShape` of the landing block in UPDATE_SHAPE_ORDER:
  dust connections, note-block instrument, an OBSERVER facing anything schedules its 2 gt tick = moved
  observers fire, falling blocks schedule their fall tick); air -> `setBlock(movedState, 340)` +
  `updateOrDestroy` (destroyed with drops); else `setBlock(newState, 67)` + `neighborChanged(pos)` on
  itself (a landed piston checks power). Else progress += 0.5 (entities pushed: not modeled).
  So an action started at t (block-event phase) has progress 0.5 after t's block-entity phase, 1.0
  after t+1's, and lands in t+2's block-entity phase.
* `finalTick` (262-283): if progressO < 1: remove the entity; if the cell is a moving piston:
  source -> air, else `updateFromNeighbourShapes(movedState)`; `setBlockAndUpdate` (flags 3) +
  `neighborChanged(pos)`. Called by a retracting piston on its head cell and on the cell 2 ahead
  (sticky), and by `preRemoveSideEffects` when the cell is overwritten without flag 256 (never by a
  piston action).

## 5. Timing consequences (all derived from 1-4; gadgets in docs/mechanics.md)

* Pushed block: moving piston at t (old cell air at once), lands t+2 in the block-entity phase: pistons
  it powers act at t+3, components schedule from t+2.
* Retracting base is a moving piston for 2 gt (immovable, cannot act); lands t+2, a powered one
  re-extends at t+3 at the earliest.
* **Spit / drop:** sticky retract while the block it pushed is still moving (event 2, or the
  finalTick in (d)) places that block at once at its destination (block-event phase) and the piston
  retracts alone. A 2 gt observer pulse = extend at t, retract at t+2 in the scheduled/block-event
  phase -> spit.
* **0-tick:** extend and retract in the same block-event phase: the pushed block is placed in that
  phase (sticky), or keeps moving and lands at t+2 (normal piston); a 0-ticked sticky piston with an
  empty front pulls the block 2 cells away (reach-through), landing at t+2.
* **Killer pistons** (a block event of piston B removing A's source or front) = ordinary FIFO block
  events in one phase; `doBlockEvent` skips an event whose cell no longer holds that piston type.
* Pistons pushing/pulling pistons: retracted pistons move like blocks (pull: event 1 pulls a
  retracted piston too); a landing piston runs `onPlace` -> checkIfExtend (extends next tick if
  powered there, or in the same phase when placed by a finalTick).
* Moving power sources: a redstone block that starts moving is a moving piston at once (power lost
  in the same phase: instant-off); it powers again when it lands (t+2, block-entity phase) or when it
  is placed by a finalTick (block-event phase).
* Moving observers: fire 2 gt after landing (`updateShape` on landing); a powered observer that is
  moved updates its old front as unpowered (`affectNeighborsAfterRemoval`) and lands unpowered.
* Falling blocks (sand, red sand, gravel, concrete powder): `onPlace`/`updateShape` schedule a tick
  2 gt later; it falls if the cell below is air/fire/liquid/replaceable (a moving piston is support);
  the entity falls with gravity 0.04/tick, drag 0.98, lands where it touches ground: a moving piston
  in its cell -> waits (bounces), placeable + survivable -> `setBlockAndUpdate`, else breaks into an
  item. A pushed sand block that lands in mid-air falls 2 gt after landing.

## 6. Blocks destroyed or refused by pistons (palette of the doors)

* Moved (PUSH_PULL, the default): stone, smooth stone, glass, tinted glass, snow block, logs, redstone
  block, redstone lamp, note block, target, observer, copper bulb/grate, slime, honey, sand, iron
  trapdoor, rails, retracted pistons (pistons are IMMOVEABLE by property but special-cased).
* Pushed only (PUSH): glazed terracotta.
* Destroyed (POPPED; drops = the loot table with no tool): redstone dust, repeater, comparator,
  torches, levers, buttons, pressure plates, doors, scaffolding, skulls, dragon egg, carpets, plants,
  LEAVES (random sapling/stick drops), snow layer (no drop without a shovel).
* Immovable: obsidian, crying obsidian, reinforced deepslate, bedrock, extended pistons, piston heads,
  moving pistons, every block entity (sculk sensors, hoppers, droppers, dispensers, crafters,
  chests, furnaces...).
* The exact table for every block comes from the game itself (`redsim` palette check against the
  bridge), not from this list.

## 7. Exclusions (and why)

* Entities moved by pistons (`moveCollidedEntities`, honey sticking, slime bounce): only the
  entity category and players; the model tracks no entities except falling blocks and drop events.
* Fluids and waterlogging, concrete powder hardening: no water in the doors' palette.
* Random ticks: the judge rejects randomly ticking blocks.
* Block entities other than moving pistons: immovable obstacles (their own logic is redstone's,
  sculk vibrations excluded for now: game events are recorded for a later sculk model).
* Experimental redstone (orientations): off in the judge's worlds.
* World border and build height: never reached by a door.
