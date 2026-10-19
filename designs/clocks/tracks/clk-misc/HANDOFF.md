# clk-misc HANDOFF (session da291dc5, devvm47239, branch explore/clk-misc)

Track: clocks made of the blocks no other track covers (coordinator 19:35Z): crafters, rails and minecarts, fluids,
daylight detectors, target blocks, tripwire, doors, fence gates, heads, leaves. Claims agreed with clk-obs (rails, target,
doors, gates, heads, bells, leaves, scaffolding, crafters) and clk-item (crafters). Board notes: Space memory
`minecraft-tas/clocks/families/misc.md`. Crate: `crates/clock-misc` (`mctas clock misc ...`).

## Archived (all `mctas clock archive`, robust)
| what | P (gt) | pieces | family |
|---|---|---|---|
| relay rings x8 (extendable, n = 0..47) | 10 + 4n | 6 + 2n (doors 7 + 2n) | crafter / powered_rail / activator_rail / oak_fence_gate / oak_trapdoor / skeleton_skull / iron_door / oak_door + observer |
| crafter census picks | 4 (x3), 6 (x2), 8 (x2) | 3-6 | crafter+observer |
| clk-misc-crafter-loop (extendable, n = 0..20) | 91 + 28n | 7 + 2n | comparator+crafter+hopper+observer |
| clk-misc-daylight-spit | 40 | 5 | daylight_detector+observer+sticky_piston |
| clk-misc-water-channel | 15 | 24 | observer+sticky_piston+water |
| clk-misc-cart-pingpong (extendable, n = 0..20) + bare | 38 + 5n, 40 | 15 + 2n, 13 | detector_rail+minecart |
| leaf clocks | 9 (x3), 10 (x4), 18, 20 | 4-9 | oak_leaves+observer+sticky_piston |

## Tools (crates/clock-misc)
- `clock misc ring --relay R --n N [--check K]`: relay rings (R = crafter, powered_rail, activator_rail, oak_fence_gate,
  oak_trapdoor, skeleton_skull, iron_door, oak_door).
- `clock misc crafter-loop --n N`, `clock misc cart-pingpong --n N [--bare]`: the two item / entity families (judged only
  in the game: redsim models neither crafting nor entities).
- `clock misc rename --prefix P FILES`: rename picks (other tracks' tools name designs after themselves).
- Searches: `clock obs census --k K --palette "obs,sticky,stone,<raw states>" --require X --out-dir D` (clk-obs's polycube
  census accepts raw states) and `clock census --box ...`; `clock obs pick --top 3 --mechanisms` picks per (P, family).

## Facts (also in designs/clocks/impossible.tsv)
- target: power only from projectile hits; tripwire: power needs an entity on the string; daylight detector: changes only
  at game time % 20 == 0 -> P a multiple of 20, >= 40.
- Lava: random spread delay when a non-empty lava cell rises -> only appear-from-empty / decrease cycles can be exact.
- Judge world: time frozen at dawn (dayTime 0): a normal daylight detector reads 7 under open sky and under a 1-block cover;
  inverted reads 0 / 1; piston heads do not block sky light.
- Crafter: rising edge of hasNeighborSignal (no QC), craft 4 gt later, crafting=true for 6 gt; comparator = filled or
  disabled slots; 1 iron ingot -> 9 nuggets (unsafe partial grid), 1..8 slime balls -> nothing (safe).
- Minecart (old behavior): empty carts slow x0.96 per tick; powered rails +0.06 b/t per tick (cap 0.4); a resting cart on
  a powered rail with a conductor behind it is launched at 0.02 b/t; bounces clamp the cart against the bumper. Detector
  rails stay on until a 20 gt check finds them empty.
- Leaves: distance updates are 1 gt scheduled ticks; persistent leaves never decay (non-persistent ones at distance 7 drop
  items = entities).

## Next ideas
- Scaffolding distance clocks (redsim models scaffolding; distance 7 makes it fall = entity: keep < 7).
- Hopper minecart item loops: a hopper minecart oscillating over a hopper and a dropper/hopper feeding it back.
- Deterministic lava (cells only appear from empty or decrease).
- Leaf chains of 2..4 leaves (P12-P16) need other geometries (the watched leaf must sit 1-3 steps from the first leaf and
  next to an observer whose back reaches the piston).
