# clk-misc HANDOFF (session da291dc5, devvm47239, branch explore/clk-misc)

Track: clocks made of the blocks no other track covers (coordinator 19:35Z): crafters, rails and minecarts, fluids,
daylight detectors, target blocks, tripwire, doors, fence gates, heads, leaves, scaffolding, boats; redstone-less clocks
(vjeux 22:23Z: water, a boat, wooden pressure plates). Claims agreed with clk-obs (rails, target, doors, gates, heads,
bells, leaves, scaffolding, crafters) and clk-item (crafters, chiseled bookshelf, lectern). Board notes: Space memory
`minecraft-tas/clocks/families/misc.md`. Crate: `crates/clock-misc` (`mctas clock misc ...`).

## Archived (all `mctas clock archive`, robust)
| what | P (gt) | pieces | family |
|---|---|---|---|
| relay rings x8 (extendable, n = 0..47) | 10 + 4n | 6 + 2n (doors 7 + 2n) | crafter / powered_rail / activator_rail / oak_fence_gate / oak_trapdoor / skeleton_skull / iron_door / oak_door + observer |
| crafter census picks | 4 (x3), 6 (x2), 8 (x2) | 3-6 | crafter+observer |
| clk-misc-crafter-loop (extendable, n = 0..20) | 91 + 28n | 7 + 2n | comparator+crafter+hopper+observer |
| clk-misc-daylight-spit | 40 | 5 | daylight_detector+observer+sticky_piston |
| clk-misc-water-channel | 15 | 24 | observer+sticky_piston+water |
| clk-misc-cart-pingpong (extendable, n = 0..14), -short, -bare | 37 + 5n, 33, 40 | 17 + 2n, 15, 15 | detector_rail+minecart (levers, no redstone blocks) |
| clk-misc-cart-loop (extendable, n = 0..16), loop3, loop-long-n0 | 26 + 5n, 476, 75 | 29 + 4n, 19, 27 | detector_rail+minecart (levers) |
| leaf clocks | 9 (x3), 10 (x4), 18, 20 | 4-9 | oak_leaves+observer+sticky_piston |
| scaffolding clocks | 10, 18 | 6, 10 | observer+scaffolding+sticky_piston |
| REDSTONE-LESS boat rings: ring4, ring4c (= 2x2), boat-ring-AxB (A <= B in 2..6) | 380, 412, 476, 488, 494, 506, 509, 516, 522, 572, 586, 600, 608, 622, 646, 1216 | 77-157 | oak_boat+oak_pressure_plate (stone, water, plates, the boat) |
| REDSTONE-LESS vertical boat loops: boat-lift-l{3,4}h{2..6} | 161, 165, 167, 173, 175, 178, 181, 186, 188, 190 | 97-169 | oak_boat+oak_pressure_plate (+ soul sand, wall banners) |
| boat-lift-l{3,4}h2-tripwire | 173, 188 | 103, 111 | oak_boat+oak_pressure_plate+tripwire+tripwire_hook |
| boat-lift-l{3,4}h2-dripleaf | 187, 201 | 99, 107 | big_dripleaf+oak_boat+oak_pressure_plate |
| boat-lift-l{3,4}h2-chest1 | 173, 188 | 97, 105 | hopper+oak_chest_boat+oak_pressure_plate (an item loop lifted by a chest boat; one hopper in the upper floor) |
| item loops with a minecart lift: bookshelf-cart, -b2, hopper-cart | 56, 28, 46 | 20, 20, 17 | chiseled_bookshelf+hopper+hopper_minecart, hopper+hopper_minecart |
| clk-misc-bubble-flip | 16 | 12 | bubble_column+observer+redstone_wire+sticky_piston |
| clk-misc-dripleaf-stand | 4 | 8 (7 + an armor stand) | armor_stand+big_dripleaf+observer+redstone_wire |
| REDSTONE-LESS bouncers: boat-bouncer, -slab, -c3 | 82, 77, 73 | 19, 20, 20 | oak_boat+tripwire+tripwire_hook (a boat launched every 60 gt by a bubble column trips a tripwire; the ceiling height sets P) |
| boat-bouncer-trapdoor | 73 | 20 | oak_boat+oak_trapdoor+tripwire+tripwire_hook (the hook opens a trapdoor into the boat's path) |

Not archived on purpose (same mechanism, another block; available on request): the boat rings with light / heavy
weighted pressure plates (P380 like ring4c, new families light_weighted_pressure_plate+oak_boat and
heavy_weighted_pressure_plate+oak_boat); the 8 other tripwire / dripleaf lift sizes; an oak shelf (1.21.9, a
ListBackedContainer: hoppers fill it, its contents count as its state) in place of the chiseled bookshelf.

## Tools (crates/clock-misc)
- `clock misc ring --relay R --n N [--check K]`: relay rings.
- `clock misc crafter-loop --n N`, `clock misc cart-pingpong --n N [--bare|--short|--track S/P/D/L...]`,
  `clock misc cart-loop --n N [--three|--long-only]`, `clock misc boat-ring --a A --b B`,
  `clock misc boat-lift --l L --h H [--tripwire] [--dripleaf] [--chest N]`: generators (judge with `clock verify`;
  `clock sim` runs them on redsim since the fluids + entities port, crafting excepted).
- `clock misc trace <design> [--ticks N] [--every K] [--only prefixes]`: built like the judge, prints every entity's
  position / motion and the changed cells per tick (how the boat rings were found).
- `clock misc trace-export <archived designs> [--exact] [--ticks N]`: real-game traces for the in-browser viewer
  (webrender), JSON Lines in designs/clocks/traces/ (format: designs/clocks/traces/README.md). Re-export after re-archiving
  an entity / fluid clock, and tell site (6df38b98) and webrender (084de54f) about new traces or new block kinds.
  `--exact` (designs/clocks/traces/exact/) is redsim's gate: re-export those too, then
  `redsim-fuzz trace-compare designs/clocks/traces/exact/*.trace` must say OK for every trace.

## The redsim fluids + entities port (2026-10-19, with clockinfra d650ed4f)
- Gate: all 50 exact traces tick-identical (`redsim-fuzz trace-compare`), plus the test scenes in
  `redsim-tests/` (`--ticks` traces of hand-made scenes: boat fall / slide / ice / slow, a 300-tick dripleaf stand;
  `rt-boat-airx` is UNMODELED: the long-fall raycast). Bit-identity for designs without entities: full selftest, and
  the tracked censuses re-run byte-identical (`minimal.tsv`, `summary.md`: 2x1x1-k2, 2x2x1-k4, 2x2x2-k4, 3x1x2-k4,
  3x1x3-k4, 3x2x1-k4, 3x3x1-k4, 3x3x2-k3, 3x2x2-k4 at 5d3b47467). `clock sim` gives the judge's CLOCK line for 76 of the
  78 archived clk-misc clocks (the other two: crafter items, a daylight detector).
- Mine: `redsim/src/entity/{collide,kinds,boat,minecart,armor_stand}.rs` (Entity.move / collide / step-up / movement
  recording, checkInsideBlocks, baseTick, AbstractBoat, AbstractMinecart + OldMinecartBehavior, LivingEntity.travel for
  a stand), `react.rs` (plates, tripwire, hooks, bubble-column entity effect, big dripleaf), `rails.rs` (RailState for
  every rail kind, plain and detector rails), the containers in `redstone/container.rs` (CRef: block entities or
  container entities; chest boats, hopper minecarts, the chiseled bookshelf), `redsim-fuzz/src/trace_compare.rs`.
  clockinfra's: fluids (`fluid/`), fluid pushing, waterlogging hooks, item entities, entity spawn / clock sim wiring,
  the rail-shape fuzz gate.
- Known gaps (noted in the code): per-box collideX when an entity already overlaps a multi-box shape, the long-fall
  raycast, entities pushing each other, a hopper minecart's item-entity pulls (clockinfra's hook), random container
  picks among several container entities.
- `clock misc unarchive FILES`: design + verdict / caption rows + the family's extend rows out of the archive, to
  re-archive a rebuilt clock under its name. `clock misc rename --prefix P FILES`: rename other tracks' picks.
- Searches: `clock obs census --k K --palette "obs,sticky,stone,<raw states>" --require X --out-dir D` (clk-obs) and
  `clock obs pick --top 3 --mechanisms`. A k = 6 census with a 15-state palette does not finish in 2 h (k = 5: 5 min).

## Facts (also in designs/clocks/impossible.tsv)
- target: power only from projectile hits (a declared projectile cannot hit again and again); daylight detector:
  changes only at game time % 20 == 0 -> P a multiple of 20, >= 40; lectern: players only; items: despawn after 6000 gt.
- Lava: random spread delay when a non-empty lava cell rises -> only appear-from-empty / decrease cycles can be exact.
- Judge world: time frozen at dawn: a normal daylight detector reads 7 under open sky and under a 1-block cover;
  inverted reads 0 / 1; piston heads do not block sky light.
- Crafter: rising edge of hasNeighborSignal (no QC), craft 4 gt later, crafting=true for 6 gt; comparator = filled or
  disabled slots.
- Minecart (old behavior): empty carts slow x0.96 per tick on rails; powered rails +0.06 b/t per tick (cap 0.4); a resting
  cart on a powered rail with a conductor behind it is launched at 0.02 b/t; bounces clamp the cart. Detector rails stay on
  until a 20 gt check finds them empty. A lever on a rail's support powers a stretch of up to 17 powered rails.
- Boats: pressure plates are NOT washed away and block water (tag washed_away_by_fluids), boats press wooden plates and
  pass over them. A boat summoned before the water arrives sinks: it rides 1-deep currents submerged (friction 0.9, push
  0.014 -> 0.126 b/t), crawls next to sources (under-water status, friction 0.45 -> 0.0115 b/t); under source water it
  sinks (no buoyancy), so a water column does not lift it (only an upward bubble column does, then launches it after 60
  gt above the column). A straight ping-pong between two opposite currents dies out (1D: conservative); a ring needs
  one plate-cut current per corner (after a turn the outer column's water is one level weaker and cancels the next turn).
  Ice is not needed. A submerged boat in an upward bubble column climbs (+0.06 b/t per tick up to 0.7) and flies out (a
  ceiling 2 above the water keeps it in the watched box); wall banners are water curtains (no collision, water cannot
  enter them); a boat drop needs a 2-cell gap (1.375-wide boat) and a plate row at the landing so the falling water's
  spread pushes the boat the right way. Minecarts (0.98 wide) cannot cross a 1-cell plate row (no water overlap).
- Big dripleaf: an entity on top (onGround, y > 0.6875) tilts it: unstable 10 gt, partial 10 gt, full (no collision)
  100 gt, then back; redstone power resets it. Bubble columns update 5 gt after the block below changes (26.3:
  BubbleColumnBlock.updateShape), not 20. A boat floating above an upward column is launched after 60 gt (vy 0.6, about
  2.8 blocks up): a perpetual vertical bounce; a floating boat sits 0.52 above its water cell's bottom, so a tripwire
  must be 2 cells above the water (1 above would be pressed for ever).
- Chiseled bookshelf: a hopper under it can pull a book out in the very tick it arrived (block entity order differs by
  rotation: rotation-dependent); let the hopper minecart under it empty it instead.
- Leaves / scaffolding: distance updates are 1 gt scheduled ticks; persistent leaves never decay; scaffolding falls at 7.

## Next ideas
- Faster crafter item loops (two crafters facing each other exchanging 1 block <-> 9 items: an observer can never watch one
  crafter and power the other, so each direction needs a relay).
- Daylight variants (P is a multiple of 20; 40 is the minimum, longer ones only add delay).
- Kelp / seagrass / coral / dripstone / composters: random ticks, no exact clock.
- Deterministic lava; leaf chains of 2..4 (P12-P16) with other geometries.
