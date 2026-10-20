# redsim fluids + entities (clockinfra, started 2026-10-19 17:00 PT; coordinator's GO 16:52)

Goal (vjeux): every contraption runs live in the browser with redsim (WASM), so redsim must model
the real-game-only clocks: water, boats, minecarts, pressure plates, detector rails, item entities.
Port the game's own logic (26.3 decompiled: `mctas src find <Class>`), exact gt order, validated
tick by tick against real-game traces (clk-misc exports them; our bridge can record any design).

## Milestones and gates (estimate sent to the coordinator 16:43)
- M1 fluids (1.5-2 d): water source/flow/falling (FlowingFluid spread, getSpread slope search,
  source creation), a separate fluid tick list ticked right after block ticks (ServerLevel), the
  `waterlogged` property on redsim's waterloggable blocks (Slab, Rail, Trapdoor, Scaffolding,
  Leaves, Sculk), what holds/breaks water (tags WASHED_AWAY_BY_FLUIDS, BLOCKS_FLUID_FLOW),
  flow vectors (getFlow), bubble columns (soul sand up, magma down). Gate: block states
  tick-identical to the game on the 31 water designs of designs/clocks/archive.
- M2 entity core (2 d): entity list in insertion order ticked where gravity::tick_entities runs
  (after block events, before block entities); exact Vec3/AABB doubles; Mth sin/cos tables;
  collision shapes (behavior::collision_boxes exists: Box16); Shapes.collide in the game's axis
  order; move, checkInsideBlocks, fluid pushing, entity push; pressure plates (wood/stone/
  weighted), tripwire + hooks. Then say whether a 2nd agent should take M4.
- M3 boats (1-1.5 d): AbstractBoat status/float/friction/bubble columns. Gate: 30 boat clocks.
- M4 minecarts (1.5 d): AbstractMinecart + OldMinecartBehavior, rails (plain/detector rails
  are NOT in redsim yet: only Rail{activator} = powered/activator), hopper minecart. Gate: 9 carts.
- M5 integration (1 d): entities in clock/circuit sim, entities + fluid ticks in the state key,
  `redsim trace-compare`; then item entities (+0.5 d).
- Bit-identity for designs without water/entities: full selftest, census rerun identical TSVs,
  redsim-fuzz. New code must never run for them (empty lists, same to_string output).

## redsim map (what to touch)
- block.rs: `Block` (Copy enum), `Kind`, parse/to_string (game state strings must round-trip).
- world.rs: World { cells, time, sub_tick (shared counter, like Level.subTickCount), ticks:
  LevelTicks (block ticks, key (pos, Kind)), events, ... }; tick(): block ticks -> events ->
  gravity::tick_entities -> hoppers -> sculk -> piston block entities. set_block(_limit),
  neighbor_changed, neighbor_shape_changed, update_or_destroy, schedule_tick(pos, kind, delay, pr).
- behavior.rs: dispatch (tick, update_shape, neighbor_changed, on_place, can_survive,
  push_reaction, collision_boxes, sturdy...).
- ticks.rs: LevelTicks (per-chunk containers, DRAIN order) -> a second instance for fluids.
- snapshot.rs: the exact state key (must include fluid ticks + entities later).

## Java to port (26.3 line counts)
FlowingFluid 526, WaterFluid 160, LiquidBlock 243, BubbleColumnBlock 212, Fluid 127, FluidState
142, SimpleWaterloggedBlock 56; AbstractBoat 844; AbstractMinecart 619 + OldMinecartBehavior
408; RailState 352, BaseRailBlock 296, PoweredRailBlock 158, DetectorRailBlock 188; pressure
plates ~250; TripWire(Hook) ~400; Entity (4330: move/collide/fluid pushing/checkInsideBlocks
~700); Shapes/VoxelShape ~800.

## Progress log
- 17:00 notes written; reading FlowingFluid (getFlow, spread, getNewLiquid, getSpread,
  getSlopeDistance (SpreadContext caches: holes/states keyed by relative xz), tick).
- 17:25 M2 API skeleton pushed (56289d28c), agreed with clk-misc (who then took M3 boats, M4
  minecarts/rails/containers AND filled the M2 core: collide.rs, kinds.rs, base_tick...).
- 17:45 M1 water + bubble columns (2be1b9201): redsim-fuzz --gen basin, 6000 cases = game.
- 17:49 fluid pushing (fluid_push.rs, EntityFluidInteraction) (751add30c): 0 delta divergence.
- 17:55 waterlogging hooks (c93a8cda6): fluid::waterlogged / set_waterlogged for variants that
  carry the property (clk-misc's dripleaf, lantern, stone slab); redsim's older waterloggables
  stay dry (model note).
- 18:20 M5: clock sim spawns entities (redsim::entity::spawn, also trace-compare's), the state key
  hashes entities, and entity clocks' period comes from the BLOCKS by the judge's rule
  (bde472e1a). Item entities (item.rs) + hoppers pulling them (6b3485076).
- 18:36 rail-shape fuzz gate (palette plainrail / detrail): powered rails onto rails::RailState,
  LevelChunk's BaseRailBlock condition (19bc712a6): 314/2000 -> 0 over 13000.
- 18:48 GATE MET: 50/50 traced entity/fluid clocks give redsim CLOCK lines identical to the game's;
  59/60 exact traces tick-identical (the 60th: clk-misc's long-fall raycast scene).
- Open: falling blocks still in gravity.rs's own list (order vs other entities only matters for a
  design mixing both); item drops and items stuck in blocks are random in the game itself.
