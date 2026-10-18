# Track box56: a flat (1-wide) 3x3 door in a volume <= 56 (7x8 / 8x7), stretch 7x7

Goal (vjeux 20:20Z + coordinator 20:25Z): flat (`--shape thick<=1`), community VOLUME <= 56
(`door properties` volume, lever excluded), FULL seamless + hallway, rotation-safe, judge at 32
positions. Leaders today: flat-b-v7 80 (8x10), flat43 81 (9x9).

## What the volume and FULL seamless mean for a flat door (read from properties.rs)
- Volume = bounding box of circuitry over rest, every animation tick and the open state; door
  blocks, the lever, and quartz-like blocks RESTING in ring (S) cells never count. Door blocks may
  park outside the box.
- FULL seamless for a flat door = the OPEN state has no circuitry in the 12 ring cells (door
  blocks, air or resting stone only), and no circuitry right behind an open-state hole (a ring
  cell swept during the cycle but air when open). The rest state's ring is free (hidden).
  => pistons live in the margin (distance 2) or the 4 ring corners; side columns = pull-type
  pistons at x=-2 / x=4.

## Mechanism floor
- pist19r (elevator A (1,4) + B (1,5), P_T at a ring corner, M pulled down by (1,-2), stacks
  at x=-2 and x=4) occupies exactly x=-2..4, y=-2..5 = 7x8 = 56; ideal inputs: WORKS
  (`door ideal tracks/flat/pist19r.ideal`, 39/39). All wiring must fit in the 25 free cells.
- 7x7 (margin 2 everywhere): the center must travel 2 cells; the puller for its first step
  sits in the ring cell next to the edge-middle (e.g. (1,3)) and must be in the margin when
  open; the only margin cell next to (1,3) is (1,4), so the puller moves vertically, which
  needs a pusher at (1,5) (outside). No elevator fits a 7x7 without slime tricks.

## The wiring split (the core problem of the 56 box)
- In a 7- or 8-wide box the side-middle blocks L/R exit sideways through pistons at (-2,1) /
  (4,1) with their gap cells swept: both side margins are cut at y=1, so the region above the
  door and the region below it share NO wiring path. Signals cross only through motion:
  - a pull-type (pulse-driven) piston watched by an observer double-fires (extend, then
    landing) -> two pulses 4 gt apart: useless for a pull-type receiver;
  - a LEVEL-driven piston (extended at rest, e.g. powered by a torch) moves once per edge:
    an observer on its base gives ONE pulse per edge (a clean relay);
  - a cell a block is PUSHED into, or a rest cell a block leaves, also gives one pulse.
- The box's bottom row cannot hold dust or repeaters (no floor inside the box).
- Observer trick: an observer at (-2,2) facing up (output into the piston (-2,1)) fires (-2,1)
  directly and (-2,0) by QC (its output enters (-2,0)'s quasi cell), so a 2-stack at y=0,1 is
  fed from y=2 when TL exits up.

## Quadrant argument (why the known families do not fit 7x8 FULL; 21:30Z)
- Box cells left for wiring split into four QUADRANTS: the side cuts at y=1 (L/R pullers at
  x=-2/x=4 + their swept gaps) separate top from bottom; the center column (A/B or U on top,
  P_M or A'/B' below, plus the swept center path) separates left from right.
- Links between quadrants exist only through pieces both can touch: one side-pair piston (a
  2-stack whose corner exits vertically: fired from y=2 by the observer trick, watched from
  y=-1) links top->bottom on that side; A/B link the top halves; P_M (or A'/B') the bottom
  halves.
- Parity: a pulse-driven (pull-type) piston moves out and back, so an observer on it fires
  TWICE; B in window mode moves twice (b1, b2); A fires an observer 6 times per edge (measured,
  tlupA). Every top-half link therefore carries an EVEN number of pulses, but each top half owns
  a TL/TR mover (it can only be fed from its own quadrant) that needs an ODD count (1 or 3).
  => With the lever in one top half, the other top half cannot fire its corner mover.
- cd19 (center down): T's puller U at (1,4) is fed only from (0,4) or (2,4); with TL or TR going
  up the other cell holds a piston, so one top quadrant is isolated outright.
- Escape routes not closed: a LEVEL-driven link piston (torch-held, moves once per edge: one
  observer pulse); door blocks pushed into / leaving a watchable cell; slime structures.
- tlup (19 blocks, ideal inputs WORK): pist19r with TL up (puller (0,4)), T aside right
  (P_T (3,3)), left pair (-2,0..1), right 3-stack; windows: pt 3..9, M >= a3, left pair, TL
  puller and right stack anywhere in 0..40.

## Verdict (21:45Z): no flat FULL-seamless slime-free door fits 7x8 / 8x7 with the known families
Assumptions: slime-free, flat, FULL (no circuitry in the 12 ring cells when open), redstone
wiring (observers, dust, repeaters, torches, lamps, bulbs...), no sculk or entities; door blocks
leave by pistons only.

1. **Box shape.** The center travels 2 cells, so its elevator (A + B in a line) needs 3 rows on
   one side: height >= 8. So a 56 box is 7 wide x 8 tall (8x7 has no vertical elevator; a
   horizontal one needs 3 columns on one side and cuts that side, see 5).
2. **Side cuts.** L and R can only leave sideways (up/down they would travel 2 cells): pullers
   at (-2,1)/(4,1) with swept gaps (-1,1)/(3,1). With 2 margin columns per side, both sides are
   cut at y=1: no wire joins the region above the door to the region below.
3. **Center cuts.** The elevator column (A/B on top for pist19r, A'/B' below for cd19) and the
   piston opposite it (P_M below, or T's puller U on top) cut the top and the bottom into
   halves: four quadrants. Each quadrant holds a piston that only it can fire: TL's and TR's
   movers on top, BL's/BR's movers or P_M below.
4. **Links.** Quadrants talk only through a piston both can touch (fed from one, watched from
   the other) or one that is fed directly from a cell of the other. Pulse mergers (flat-seam
   e89: an observer on a delay-2 repeater turns a double pulse 4 gt apart into one) make every
   pulse-driven piston a clean one-pulse link, so parity alone is not the obstacle. Geometry is:
   * a side link needs one corner of that side to leave vertically, so the side is a 2-stack
     with a free cell at y=2 or y=0. That corner's puller then sits at (0,4)/(2,4) (going up) or
     (0,-2)/(2,-2) (going down), right beside the center piece;
   * the top-center link through A needs BOTH (0,4) and (2,4) free (A fed from one, watched
     from the other); through B it carries b1 and b2, 7+ gt apart: two pulses no merger joins,
     so it can never toggle the other half's corner mover once. The bottom-center link through
     P_M needs both (0,-2) and (2,-2) free;
   * a spanning tree of 4 quadrants needs 3 links: {top center, bottom center, one side} loses a
     center link to the side's corner puller; {top center, two sides} sends both corners DOWN,
     leaving P_M with three pistons in its row and no feed; {bottom center, two sides} sends both
     corners UP, leaving T no park (C takes (1,3)). Level (torch-held) links do not help: A
     and B cannot be level-driven, and the routes around the bottom need a side with both corners
     vertical, which collides with T's park, M's park and A's X cell.
5. **cd19 and the horizontal family** fail the same way: U (T's puller) is fed only from (0,4)
   or (2,4), so a top link needs TL and TR sideways (two 3-stacks: no side link); with a corner
   down, the bottom ring has no room for M's park + X. The horizontal elevator (8x7) cuts its
   own wide side, and its R park pushes TR up (kills the U link) or BR down (kills the P_M link).
Escape hatches left open (not explored): slime flags (vjeux asked for slime-free), sculk
(category), entities, door blocks parked 2+ cells out to free ring cells for links, and centers
that are not elevators.

## Machine check against agent families' catalog (22:10Z): `mctas door topology`
Claim 2 above is too strong: agent families' catalog (explore/families 6b2d389, flat FULL-seamless
mechanisms, <= 10 pistons, no slime) has 9-piston mechanisms where R leaves through a 2-move path
and the right side keeps a free cell at (4,1) (7x8-000). So the argument is now checked by a
tool rather than by hand: `door topology <CATALOG.tsv>` (crates/piston-door/src/topology.rs;
`door mech-synth <base> --just-base --topology x0,y0:x1,y1` for one design).
- Model: free cells = box cells of the door plane outside the opening and the ring that no block,
  head or pulse head ever occupies during the catalog's run (symmetric train, else its open and
  close paths); unswept ring cells hold stone and join the components they touch. A piston is fed
  from a component holding a neighbor of any of its positions (not its front) or a neighbor of
  the cell above it (quasi-connectivity through another piston's cell only when both need the
  same pulse count, as a side stack does). A component watching a position cell receives one pulse
  per change of that cell (pulse mergers). Both edges must work with the lever in one component:
  odd-count pistons need a component reached by an odd link (or the lever's own), even-count ones
  (B) any reached component. `strict` = a link is odd iff its change count is odd; `permissive` =
  also when it has >= 3 changes (three close pulses might chain-merge into an odd number).
- Sanity: the real doors e89 (8x9), h44 (9x9), flat-b-v7(-full) (8x10) are WIRABLE (strict).
- **7x8: 0 of 50 wirable (strict); 8x7: 0 of 49.** Permissive: 5 + 6 survive (7x8-000/001/003/004/
  005, 8x7-050..055): all 9-piston mechanisms WITHOUT a symmetric train (asymmetric control: e.g.
  7x8-000's left bottom puller takes 1 pulse to open and 7 to close in the model's paths), whose
  links only work if 3+ pulses chain-merge. Every mechanism with a symmetric train (88 of 99)
  fails even permissively. Other boxes: the first 8x8 entry (8x8-102, symmetric) and 8x9-111 are
  WIRABLE (strict).
- box72's torch-held side puller (one observer pulse per edge from a level source) is a side link
  my model already counts as odd (one change per edge); it needs the same free cell beside the
  side stack as any watcher, so it adds no crossing in the 99 entries.
- tlup, pist19r and cd19 in their 7x8 boxes: NOT wirable (strict and permissive).

**Final verdict:** within agent families' bounds (flat, FULL, slime-free, <= 10 pistons, 1-2 unit
moves, elevator lifts), no 7x8 / 8x7 mechanism can be wired with symmetric pulse control; the only
candidates left are the 11 asymmetric 9-piston mechanisms, and only if three or more relayed pulses
merge into an odd count, with open and close trains that differ (up to 44-48 macro events to
close), which 26 free cells cannot plausibly carry.
