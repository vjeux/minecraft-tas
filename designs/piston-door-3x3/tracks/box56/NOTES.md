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
