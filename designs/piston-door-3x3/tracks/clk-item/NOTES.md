# clk-item: door wiring (overnight push, from 2026-10-21 01:00Z)

Joined fly's sub-56 door push (vjeux's #1). Line taken: copper-bulb open/close memory for q69m's wiring.

## q69m (fly, explore/fly designs/piston-door-3x3/tracks/fly/wip/q69m.txt, 6x9 = 54)

### Copper bulbs do not rescue the areas (exhaustive, redsim)
Against fly's ORIGINAL q69m.ideal (island-consistent timings; the compacted plan acts at t0 everywhere),
each area's ideal inputs removed (and its rest-on redstone block, where it had one), every filling of
the area with 25 states per cell: air, stone, redstone block, dust, observers (up/down/east/west),
repeaters (east/west, delay 1-4), comparators (east/west, compare/subtract), torch, wall torches
(east/west), copper bulb lit and unlit:
- top-left (-2..1,5), P_T and Pl removed: 0 of 390,625.
- bottom-left (-2,-3) (-1,-3) (-2,-2), N, D_O and W removed: 0 of 15,625.
- bottom-right (2,-3) (3,-2) (3,-3), K, A and B1 removed: 0 of 15,625.
Calibration: the same machinery passes the all-air filling with the inputs kept.
Why bulbs cannot help much in one plane: a bulb is read only by a comparator (2 cells with its
support), and an in-plane comparator has no side inputs, so it cannot subtract. A bulb would buy a
LEVEL output from a once-per-edge trigger (Pl is exactly "on while open"; D_O is its inverse, with
the bulb lit at rest): about 3 cells next to the piston's feed, which no area has to spare.

### Blocker: W has no in-plane feed
`door feeds` on q69m.ideal (`--box=-2..3,-3..5`): "no feed at all: close t46 NE(-1,-1)". Checked on
the plane at close t46: W (-1,-1) faces east, its back (-2,-1) is empty (it is W's own open-state
cell: the slime drags W there on open, which is the bottom-left's open trigger through O), above is
the slime (-1,0), whose other neighbours are all mechanism cells, below is N, and its front holds
BM. No free cell touches W or its QC cell, so q69m cannot be wired as it stands: W's close push of
BM from (0,-1) to (1,-1) needs a piston with a free neighbour.
