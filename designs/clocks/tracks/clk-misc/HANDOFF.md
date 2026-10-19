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

## Tools (crates/clock-misc)
- `clock misc ring --relay R --n N [--check K]`: relay rings.
- `clock misc crafter-loop --n N`, `clock misc cart-pingpong --n N [--bare|--short|--track S/P/D/L...]`,
  `clock misc cart-loop --n N [--three|--long-only]`, `clock misc boat-ring --a A --b B`: generators (judge with
  `clock verify`: redsim models neither crafting nor entities).
- `clock misc trace <design> [--ticks N] [--every K] [--only prefixes]`: built like the judge, prints every entity's
  position / motion and the changed cells per tick (how the boat rings were found).
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
  Ice is not needed.
- Leaves / scaffolding: distance updates are 1 gt scheduled ticks; persistent leaves never decay; scaffolding falls at 7.

## Next ideas
- Bubble columns (soul sand / magma) as a redstone-less boat lift or launcher (a vertical boat loop: current, waterfall,
  upward column); kelp / water updates.
- Hopper-minecart item loops; chiseled bookshelf loop (needs an upward lift: hoppers only pull from above, so a dropper
  or a hopper minecart).
- A boat ring with a tripwire string (strings wash away: it needs a dry 1-cell row the boat crosses while touching water).
- Deterministic lava; leaf chains of 2..4 (P12-P16) with other geometries.
