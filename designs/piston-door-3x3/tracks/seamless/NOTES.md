# seamless: the smallest SEMI / FULL / SUPER seamless doors (Redstone Squid 2.1.4.1)

Brief (vjeux, 2026-10-18 19:09Z): SEAMLESS is the #1 property the piston door community cares
about. Goals: (1) smallest SEMI and FULL seamless solid doors, (2) smallest SUPER seamless door,
(3) fastest FULL/SUPER door. Tiers are `door properties`' (explore/props) definitions: tunnel T =
the 3x3 columns along the door axis, hallway surface S = the ring of cells face-adjacent to T (door
plane included), circuitry = every block except the 9 door blocks, the lever and quartz-like blocks
resting in S. QUART: closed, nothing in T/S on the front side (door plane skipped); SEMI: + open,
nothing in T/S anywhere; FULL: + closed, nothing on either side; SUPER: + every tick of opening and
closing.

## Tools (this branch)

* `--seamless <quart|semi|full|super>[+hall]` on minimize / regrow / grow / harvest (SearchArgs)
  and on `resynth`: candidates are accepted only if one recorded cycle at the design's own
  position (`properties::record`) reaches the tier; `+hall` also requires the ring-aware hallway
  (below) to build. Checked before the judge (one cycle).
* `resynth --seamless <tier> --extra N`: REPAIR mode. While the design is below the tier, only
  groups through an offending cell (`properties::seamless_offenders`: visible circuitry of the
  open state / closed sides / animation) are cut, singles included, and each may grow back with up
  to N MORE blocks; a replacement that removes some offending cells is progress. Once at the tier,
  the usual shrinking resynth continues under the filter.
* `door properties` gains `hallway_ring` (dynamic): the ring-aware hallway.

## Hallway: props' check vs the ring-aware one

The rules make the hallway composition mandatory for SUPER/FULL seamless (2.1.2.4 note). props'
`hallway` fills EVERY empty S cell (depth extent +-3) with smooth stone, including the door plane's
ring, where the door blocks retract to: it fails every door we have, flat-v2-57b included. The
ring-aware fill (`with_hallway_ring`) leaves the door plane's ring cells that hold a block in the
OPEN state empty (closed they touch T only through the door blocks; open, the door blocks there
ARE the wall); every other S cell empty at rest is filled. Results (one cycle, design position):

| design | blocks | tier | props hallway | ring-aware hallway |
|---|---|---|---|---|
| flat-v2-57b | 57 | FULL | no | yes |
| rb6-48b | 48 | FULL | no | no (slime columns in the ring drag the depth +-1 walls: push limit) |
| fc2-40b, mp8-43b | 40, 43 | QUART | no | no (same) |
| sf-holdA-e41, lamp-sweep-w41, wire19sf-w42, fresh-d-s4-45b-2, flat-v7-50b, flat-v6-51b | | QUART | no | yes (slime-free) |

So a FULL door that keeps its hallway must not have slime next to S cells: slime-free side columns
(one sticky piston per door block, heads in the ring) are the family to build on.

## Results

| design | blocks | tier (props) | hallway (ring-aware) | open/close | notes |
|---|---|---|---|---|---|
| [semi-fc2L.txt](semi-fc2L.txt) (archive/fc2-seamL-41b) | 41 | FULL geometry | no | 24/9 | fc2 + O_m moved out of the floor ring: P_M fired by an observer (0,-2,0) watching a stone carried by a slime (-1,-1,0) glued under the left structure. SEMI claim (hallway optional for SEMI) |

## Log

* fc2's only open blocker is O_m (2,-1,0) (P_M's trigger). P_M's neighbors outside S: (0,-2,0),
  (2,-2,0), (1,-3,0), (1,-2,+-1); an observer there must watch (-1,-2,0) / (3,-2,0) / (1,-4,0) /
  (1,-2,+-2). Cheapest found: glue a slime + stone under one side structure so the stone leaves the
  watched cell when the structure moves (left: fc2-L, right: fc2-R; both 41, both work, rotation-safe).
  A 40-block fix would need the watched cell to change with ONE added block: none found.
