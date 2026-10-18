# Track `entities`: a separate category for doors that use entities

vjeux (2026-10-18): a SEPARATE category that allows entities (minecarts, armor stands, boats...) in
a door; every existing board keeps banning them. This track built the category (format, bridge,
judge, boards) and searched it.

## The category (commit 147ba29 + follow-ups on explore/entities)
* **Format** (`crates/mctas/src/design.rs`): header lines `entity: <type> <x> <y> <z> [snbt]`, world
  coordinates as decimals (not shifted by `origin`), optional SNBT (`{Rotation:[90f,0f]}`). Parsed,
  saved, translated; `Design::content_key` (dedup/caches) is unchanged for designs without entities.
* **Bridge**: `summon` (deterministic: UUID from the index, the entity's own random reseeded -
  `Entity.random` is `RandomSource.create()`, clock-seeded; tag `mctas_design`; mobs refused);
  `entities` returns motion, yaw/pitch, design tag, `still`, `collides`, `embedded`, box and a key
  state (saved data minus position/motion/volatile fields, keys sorted, plus a minecart's rail);
  `idle` also waits for design entities to be at rest, and does not count a sensor held by a design
  entity re-checking (a cart on a powered detector rail, a pressed plate: they only reschedule);
  `step` takes `entity_watch` (per tick: colliding entity in the walk box; first escape from the
  watch box; first entity the design did not declare); `rotate` turns entity data like structure
  templates (`Entity.rotate`/`mirror`).
* **Judge** (`crates/piston-door/src/lib.rs`, only when a design declares entities): category
  `entity`; each entity counts as a block; allowed types (minecarts but command block / spawner,
  boats and rafts, armor stands, item frames, paintings) and data keys (`Rotation`, `Items`,
  `equipment`, `Item`, `ItemRotation`, `Facing`, `variant`); at rest exactly the declared entities,
  at rest, not inside a block, none in front of or behind the door (same 3x3 columns, any distance),
  none with collision in the walkway; open: no colliding/pushable entity in the opening or walkway
  (the open tick counts it), same entity set; close: every entity back at its rest position (1e-6),
  at rest, same key state; no foreign entity at any tick; never outside the watch box; every flip
  0-20 ticks after idle (per position/cycle/edge) so a door cannot lean on a sensor's re-check phase.
  Designs without entities take the old code path: fc2, sf-holdA-b38, flat43 and gravity2-g33
  re-verified at 32 positions, same timings; every gadget PASS.
* **Boards**: "Entities allowed" (solid + entity designs whose door is otherwise solid, with entity
  count and timings, plus a list of the doors that use entities); entity designs are off every other
  board (the any-category speed and Pareto boards exclude them) and get an "entities allowed" scope.
  `video.rs place`, canonicalize, `rest_state`, `door world` (an Entities row, `summon` commands)
  and the showcase boards summon them.
* `door drive`: `summon <type> x y z [snbt]`, `entities`, `etrace <n>`, `expect_entity`,
  `expect_entities`.

## Mechanics measured (gadgets in designs/gadgets/entity-*.drive, all PASS)
| gadget | finding |
|---|---|
| entity-cart-detector | a piston pushing a cart onto a detector rail turns it on in the block-entity phase of the push tick (`move(PISTON)` -> `applyEffectsFromBlocks` -> `entityInside`): a piston next to the rail extends 1 tick after the pusher (observer: 2). A cart resting on the rail = steady 20 gt re-checks (idle for the judge) |
| entity-rail-pass | a rolling cart over a detector rail = exactly 20 gt of power whatever its speed |
| entity-wall-stop | carts stop against a block at exactly face - 0.49, whatever their speed; on plain rails they never stop (x0.96 per tick); on an unpowered powered rail they stop where their speed drops under 0.03 (speed-dependent: not reproducible) |
| entity-push-exact | pushes land at moving face + 0.01 + half-width (float 0.49F for a cart): x.5 -> x+1.500000009536743 -> x+0.499999990463257, then identical every cycle |
| entity-plate-stand | an armor stand (2 cells tall) can be shoved off a stone plate and back through the cell above it; plate on at once, off only at its 20 gt re-check |
| cart-basic, cart-wall (track gadgets/) | lever -> powered rail kick (0.02, then +0.06/tick on it); halt-rail stops differ with speed |

## Search: why entities do not beat the solid boards (38 blocks; 9/7 ticks solid)
* **Every entity sensor turns off late.** A detector rail or pressure plate turns ON in the tick the
  entity arrives (even mid-push) but OFF only at its own re-check (20 gt; weighted plates 10, tripwire
  10), on a clock started by the last press. The compact doors (fc2, sf-holdA b38) run on 2 gt
  observer pulses into pull-type pistons; a long pulse makes a pull-type piston pull again on close.
  No signal in fc2 or b38 tolerates a late OFF (P_R must pull T back at once, B's window must end at
  t20, Q's edges set the close order, P_L/P_M/side pairs need 2 gt), and a hold would need a minimum
  length the re-check phase does not give (the judge's flip waits expose it).
* **One-way switches.** A cart on a rail cannot be pushed off without a block entering the rail's
  cell, which pushes the rail along with it (and a plate in the path breaks). Pistons do not pull
  entities. Only an armor stand (two cells tall) can be shoved back and forth over a plate.
* **As a movable power source** a cart + detector rail costs 2 blocks where a redstone block costs 1
  (sliding the rail out from under a resting cart does give instant on/off, exactly like the
  redstone-block shuttle).
* **Rolling carts** stop reproducibly only against a block; every rail is a block; a repeater gives
  8 gt per block, a rail ~2.5 gt per block at full speed. A cart run needs energy on BOTH lever edges
  (the off edge needs an inverted signal or a second launcher), and a cart back on a powered start
  rail is boosted into the wall and kicked out again (it oscillates while the lever is on).
* Item frames (comparator 1-8), chest/hopper minecarts (comparator by fill level) are constant
  sources here (a redstone block is cheaper); hopper minecarts move items (foreign entities fail).
* The one new timing primitive is the block-entity-phase trigger (piston -> cart -> detector rail ->
  next piston 1 tick later instead of 2), but its late OFF breaks the fast doors' pulse logic too.

## Designs
| file | blocks | note |
|---|---|---|
| test-static-armor.txt (= BEST.txt) | 39 | sf-holdA b38 + an armor stand standing on its smooth stone: the category's first verified entry (32 positions, rotation-safe); the entity does nothing |

## Tools for the next searcher
* `door grow --to-entity --states cart,chestcart,stand,detector,rail,plate,wplate,...`: entity tokens
  go in free cells or share a cell with a rail or plate (with `--k 2` an entity and its sensor
  together); candidates with entities are screened by the judge's own code. Smoke test: b38 + a
  stand or a cart on its smooth stone, both verified at 39.
* `door drive`: `summon`, `entities`, `etrace`, `expect_entity`, `expect_entities`.

## Next ideas (none looked like a win)
* A door built around slow-OFF signals (every close step waiting >= 20 gt) could use entity
  sensors as level sources; it would be slow and is unlikely to be small.
* The block-entity-phase trigger (piston -> cart -> detector rail -> next piston 1 tick later) is a
  parity tool for the speed doors (an odd tick without the repeater + note block + observer trick),
  if a use can live with the rail staying on 20 gt.
* A rail riding a slime structure under a parked cart is a power source that is on only at rest
  (instant on/off: the rail is re-checked when it lands): a redstone-block rider that does not
  power anything at the moved position, for one block more.

## Judge integration test with a moving entity
`test-lift-stand.txt` (41, category entity): b38 + a sticky piston facing up next to the lever
(1,5,-4) with a smooth stone on it and an armor stand on the stone. Lever on: the stand is lifted to
y 8 (8.01 mid-push, then 8.0); lever off: the stone is pulled down and the stand falls back to
exactly y 7 four ticks later (`door drive`: etrace). WORKS at 32 positions, rotation-safe: the
judge's moving-entity path (escape and foreign checks per tick, return within 1e-6, a living
entity at rest with -0.0784 vertical motion on the ground, flip waits) on a real door.
