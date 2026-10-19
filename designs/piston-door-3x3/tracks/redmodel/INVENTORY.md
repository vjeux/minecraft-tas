# redmodel inventory: what redsim models, from the 26.3 source

Per-component source lines and model notes: docs/mechanics.md section 12. Pistons and moving
blocks: [../pistmodel/INVENTORY.md](../pistmodel/INVENTORY.md).

## In (deterministic, exact, fuzzed against the game)

* Engine: `Level.setBlock` / `LevelChunk.setBlockState` side effects and flags (incl. `& -34`),
  `CollectingNeighborUpdater` (W,E,D,U,N,S, nested FIFO), shape updates (`updateNeighbourShapes`,
  indirect shapes), `LevelTicks` (priorities, per-chunk merge, `willTickThisTick`), block events
  (FIFO, dedup), tick phases, block-entity tickers (hoppers, moving pistons),
  `updateNeighbourForOutputSignal`, item drops (loot or none), the judge's placement.
* Signals: `getSignal` / `getDirectSignal`, conductors, `hasNeighborSignal`,
  `getBestNeighborSignal`, quasi-connectivity reads (dispensers, droppers, crafters, pistons).
* Components: redstone dust (all shapes, steps, levels 0-15, hash-set update order), redstone
  block, torch and wall torch (burnout), lever, stone/wood buttons, repeater (delays 1-4, locking,
  priorities), comparator (compare/subtract, side inputs, analog through one conductor), observer,
  redstone lamp, waxed copper bulbs (4 oxidation levels, all waxed), note block (instruments, mob
  heads), target block, iron/oak trapdoors, iron/oak doors, fence gates (redstone only), powered
  and activator rails (incl. `RailState` reshaping), dispenser, dropper, crafter (empty), hopper
  (locking and items), persistent leaves + logs, cake, composter (7 -> 8 tick), end portal frame,
  respawn anchor, skulls / wall skulls, slabs, glazed terracotta, 60 plain blocks (stones,
  concretes, wools, planks, glass, obsidian, slime, honey, pumpkin, soul sand, mud...).
* Containers: dropper -> facing container, hopper push / pull / cooldown / lock, comparator fill
  level; contents set with `data merge block` in the bridge.
* Uses (`World::use_block`, as the bridge's `use`): lever, buttons, note block tuning, repeater
  delay, comparator mode, dust dot/cross, oak trapdoor, oak door.

## Out, and why

* Random ticks (unwaxed copper oxidation, crops, leaf decay), weather, time of day (daylight
  detector): not deterministic in a door's run, and the judge rejects them.
* Entities and players: pressure plates, tripwire, item frames, lecterns (page turns), item
  entities (a dropper spitting into the world, hoppers catching drops; flagged when reached),
  falling-block entities beyond pistmodel's model.
* Sculk sensors (their own category; vibrations need the game-event system).
* Fluids and waterlogging (rejected at parse), cauldrons, beehives, jukeboxes, chiseled bookshelves
  (analog sources that need items or entities to change).
* Fence-gate `use` (needs the player's facing), dispensers' item behaviors (entities, block
  placement by items), a dropper holding 2+ stacks (random slot).
* Unknown blocks: rejected loudly (`Block::parse`), never guessed.
