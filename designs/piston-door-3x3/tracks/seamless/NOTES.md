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
| [full-e41.txt](full-e41.txt) (archive/e41-full-44b) | 44 | FULL | yes | 36/28 | sf-holdA-e41 (41, slime-free) repaired: `resynth --seamless full+hall --extra 3/5 --margin 2` moved the floor-ring observer (P_M's trigger) and the wall-ring observer (left pistons' trigger) out of S (+5), then shrank to 44 under the filter. Contained, dentless, weatherproof, repeater input OK |

## SUPER is impossible under props' definition (proof)

SUPER needs every frame of opening and closing free of circuitry in T and S, where circuitry
includes moving blocks (moving_piston cells) and piston heads, and every S cell counts, the door
plane's ring included. Take the first tick at which any door block moves. In vanilla a block moves
only by a piston (or by falling): pushed as part of a push line, or pulled by a sticky piston head
directly in front of it or by a slime/honey block of the moving structure next to it. Door blocks
are not sticky. Every face neighbour of a frame cell is in T or S. So:
* a pull needs a sticky head or a slime/honey block face-adjacent to the door block: in T or S,
  and it was there in the frame before (or at rest, which is the last closing frame): seen;
* a push along the door axis has its piston in T; across it, the push line runs from the piston
  through the S ring cell of that row/column into the frame, and that S cell holds a block that
  moves (a moving_piston cell); it is not a door block, since all door blocks still sit in the
  frame: seen.
Gravity doors are no exception (a sand door block's support is its floor cell, in S). Hence no
door can be SUPER as props measures it.

## SUPER (covered): the refined reading, and why the center blocks it

A piston head in the door plane's ring right behind a door block is not visible: the ring cell
touches T only through the face the door block covers. `door properties` now also reports
`FULL, SUPER (covered)` (Tier::SuperCovered, `--seamless supercov`): an S cell counts as seen only
while its one T neighbour holds no door block (resting or moving). Under this reading the side
columns are fine (a sticky head in the ring pulls its door block into the ring; on the way back the
head follows the block; slime columns too), but every FULL design we have fails it at the CENTER:
the piston that lifts or drops the center (fc2's A riding B, rb's A, flat-v2's) enters the ceiling
ring cell above an EMPTY frame cell (e.g. moving_piston at (1,3,0) after T left (1,2,0)).

The center has no S neighbour. Pulling it needs a sticky face in T (seen). Pushing it needs a push
line entering the frame from a ring cell whose block is a door block and that is contiguous up to
the center; a door block can only be in the ring after it was pulled there, which leaves a gap
next to the center. A single piston cannot close that gap and push again (its head now fills the
ring cell; an extended piston cannot be pushed). A SUPER (covered) 3x3 needs a multi-step shuffle
(door blocks sliding along the ring with pistons in the ring's CORNER cells, which are not in S,
and two-stage pushers): left as the next idea; no design here.

## Log

* fc2's only open blocker is O_m (2,-1,0) (P_M's trigger). P_M's neighbors outside S: (0,-2,0),
  (2,-2,0), (1,-3,0), (1,-2,+-1); an observer there must watch (-1,-2,0) / (3,-2,0) / (1,-4,0) /
  (1,-2,+-2). Cheapest found: glue a slime + stone under one side structure so the stone leaves the
  watched cell when the structure moves (left: fc2-L, right: fc2-R; both 41, both work, rotation-safe).
  A 40-block fix would need the watched cell to change with ONE added block: none found.
