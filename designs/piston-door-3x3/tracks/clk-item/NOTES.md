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

## s69r (clk-rt's s69 with the riding observer O at (0,-2)): B1 c66 and Qv@ c18
- Qv@(1,-2) close t18: SOLVED in redsim with O itself. O watches (-1,-2), a free cell of the
  bottom-left area, and fires east into (1,-2), which is exactly the ideal input's power pattern
  (direct into Qv, and quasi-connectivity into B1). A change at (-1,-2) at close t16 that reverts
  anywhere in t40..t60 WORKS (redsim 4 x 2; the second O pulse is harmless). In wiring terms: any
  component at (-1,-2) whose state changes at close t16 and back in that window.
- B1 (1,-3) close t66: NOT through O. O's pulse also powers Qv (then at (1,-2)) directly, Qv's
  block event is queued first, and an extending Qv cannot be pushed, so B1's lift fails (every
  revert tick 62..66: BROKEN, Qv left at (1,-2)). The only signal that fires B1 but not Qv enters
  (1,-2) from ABOVE (Qv ignores its front face), i.e. a source in (1,-1), which is the cell Qv must
  move into. In box y -4..4 instead of y -3..5, B1 gets a direct feed from (1,-4), but the top loses
  row 5 (P_T is then fed only from (0,4), and fly's lever-on-observer at (1,5) is gone).
- `door shuttle` (clk-rt) on s69r for both close pulses: 0 placements.

## m69T (pext's m69T-mm02-oc + fly's bot6 train): R0's level from the bottom-right
fly asked for a copper-bulb T flip-flop in the bottom-right free cells (1..4,-3..-1 minus U, K and
the floor frame cells) feeding R0 (4,0) through (4,-1). Measured in redsim on m69T-bot6-train:
- R0's level really is timing-free: the (5,0) stand-in switched at open t20 / close t20 WORKS.
- But (4,-1) is K's BACK cell (K (3,-1) faces west): any level into R0 from (4,-1) also powers K.
  A redstone block at (4,-1) switched like a perfect flip-flop (off open t20, on close t20), (5,0)
  removed: BROKEN, and the plane at open t1 shows K extended at closed rest (head in (2,-1)).
- Nothing in (4,-1) powers R0 alone: an observer facing down gives pulses (an extended piston needs
  continuous power), comparators/repeaters cannot face up, dust does not power upward, a torch
  powers K, and a wall torch hung on K (powers R0, not K) can never switch off.
- R2 is the mirror case: its only in-box feed (4,3) is C's back (C (3,3) faces west). One lever on
  the right holds two of R0/R1/R2 (at (5,1): R1 + R0 by QC; at (5,2): R2 + R1 by QC).
So m69T needs K (or C) moved off R0's (or R2's) only feed; memory does not help.

## q69h / q69c load tables for the sculk board (fly, 2026-10-21 01:15Z)
Box x -2..3, y -3..5 (6x9 = 54). Regions = cells free all cycle (`door feeds`, which now prints
each region's cells). Shapes: o/c = open/close edge, +t = on, -t = off; `sensor` = `ideal-rs
--holds 100`, the modes that work with the SAME start on both edges (what one sensor hearing the
lever gives); a start S means the sensor's output begins S gt after the flip.

### q69h
Regions: A5 (-2,-3)(-2,-2)(-1,-3)(0,-3)(0,-2) | B4 (-2,5)(-1,5)(0,5)(1,5) | C3 (2,-3)(3,-3)(3,-2) | D3 (3,3)(3,4)(3,5)
| input (piston) | feeds from | shape | sensor (same start both edges) |
|---|---|---|---|
| (2,3,1) Qr | D (weak) | o+2-3, o+8-12, c+48-49 | hold10 S 0..23 replaces ALL (the close pulse is not needed); taps2 S 0..17 |
| (2,4,1) | B or D | o+42-46, c+36-37 | pulse S 42..48/52..58/77..100; hold10/30 S 77..100 |
| (2,5,1) | B or D | level: off o36 .. on c42 | none |
| (0,4,1) Pl | B | level: on o16 .. off c66 | none same-start (pulse works at o3..22 + c59..67: two different triggers) |
| (1,4,1) P_T | B | rest-on; o-0, o+30-34, c+60-61, c+74 | none |
| (-1,-2,1) N | A | o+24-60 | hold30 S 20..33 (a close firing is harmless) |
| (-2,-1,1) W | A | c+16-20 | pulse S 11..17 / 24..29 (an open firing is harmless) |
| (1,-3,1) B1 | A or C | level: off o50 .. on c30 | none |
| (3,-1,1) K | C | o+70-71, c+6-10 | pulse S 67..70 |
| (2,-2,1) A | C (weak) | o+76-80, c+0-1, c+54-55 | none |
Load after sensors take what they can (Qr, (2,4,1), N, W, K): B must make Pl's level and P_T's 4
transitions from 4 cells; C must make A's three pulses (o76, c0, c54) from 3 cells; B1's level from A or C.

### q69c
Regions: A7 (-2,-3)(-2,-2)(-2,-1)(-1,-3)(-1,-2)(0,-3)(0,-2) | B4 (-2,5)(-1,5)(0,5)(1,5) | C4 (2,-3)(3,-3)(3,-2)(3,-1) | D2 (3,4)(3,5)
| input (piston) | feeds from | shape | sensor (same start both edges) |
|---|---|---|---|
| (1,4,1) | B | o+0-4, c+48-49 | none same-start (pulse o0..13 + c42..100) |
| (0,4,1) | B | level: on o8 .. off c40 | pulse S 16..17; hold30 S 0..9; taps3 S 12..17 |
| (2,4,1) | B or D | o+8-12, o+46-50, c+0-1, c+34-35 | none |
| (2,5,1) | B or D | o+24-40, c+6-18 | none same-start |
| (3,3,1) K' | D | o+18-19, c+24-28 | pulse S 18..21; taps2 S 6..7/20..21 |
| (2,3,1) | D (weak, qc only) | o+30-34, c+12-13 | pulse S 26..35; hold10 S 18..27; taps3/taps2 several |
| (1,-2,1) | A | o+14-18, o+52-56, c+50-51, c+80-81 | none |
| (1,-3,1) | A or C | o+30-46, c+56-68 | taps3 S 28, taps2 S 32..35 |
| (1,-1,1) | A (via (0,-1)) or C | o+36-40, c+62-63 | pulse S 32..41; hold10 S 27..33; taps2/3 many |
| (-1,-1,1) G | A | level: off o24 .. on c74 | none |
| (2,-2,1) A | C (weak) | o+20-24, c+20-21 | pulse or taps3, ANY start 0..100 (a free input) |
Load after sensors: B keeps (1,4,1), (2,4,1)'s 4 pulses and (2,5,1); A keeps (1,-2,1)'s 4 pulses and G's level.

### Sensor starts on q69h / q69c, built for real in redsim (step 3): all broken by the ECHO
Lever on the static right wall's back face (3,1,1) (pops nothing), the input's ideal lines removed:
- q69h Qr <- calibrated sensor (3,3) facing west: wakes t2, re-arms t22, wakes again t29 and t57
  from the door's own pistons and re-fires Qr. BROKEN.
- q69c (0,4,1) <- regular sensor (0,5): wakes t5, re-arms t45, re-wakes t49 (power 11: the top piston
  (2,4) extending at t47, 2.2 blocks away) and holds to t79. BROKEN; a WOOL door (every F ->
  white_wool, the plan still WORKS) does not help: the line (2,4) -> (0,5) crosses only free cells.
- q69c A (2,-2,1) (any start works with one pulse) <- sensor at (3,-2): regular, calibrated facing
  west, calibrated facing north: the open works, the close BREAKS (echoes during the 82-tick close).
From the source: piston extend / retract emit BLOCK_ACTIVATE / BLOCK_DEACTIVATE, the same game
events (frequencies 10 / 9) as lever on / off, so a calibrated sensor's frequency filter cannot
separate the lever from the door. Every piston of a 6x9 box is in range (8 regular / 16 calibrated).
`ideal-rs --holds` models ONE hold and no echo, so its windows are optimistic: a sensor input must
be checked with a real sensor block in redsim.

### q69c wired with real sensors: 3 of 13 inputs (wip/q69c-A3, redsim 32x3; A2 also the game 4x2)
The working pattern: a calibrated sculk sensor read by an OBSERVER facing it. The observer turns every
sensor phase change (wake, cooldown, re-arm, and every echo wake) into a 2 gt pulse, so the echo
becomes extra pulses instead of extra holds; it only suits inputs that tolerate extra pulses.
- A (2,-2,1): sensor (3,-3) + observer (3,-2) facing down + wool (3,-1) (observer -> wool -> QC into
  A through (2,-1)). Stone instead of the wool fails: the wool also occludes echo sources.
- (1,-1,1): sensor (0,-3) facing west + observer (0,-2) facing down (-> block at (0,-1) -> (1,-1)).
- P_T (1,4,1): no sensor: a stone at (-1,5) on top of the slime column, dragged on every slime move,
  watched by an observer (0,5) facing west (QC into P_T through (1,5)).
- Lever on the static right wall's back face (3,1,1) (legal for a flat door: it is excluded from the
  volume and counts as 1 wide).
Screened and failing (real sensors, observers x4, wool, stone, dust, torches; every filling of the
region's free cells; after `door prune-plan` dropped (1,-2,1) o52, (2,3,1) c12, (2,5,1) c6 too):
- top: (0,4,1), (2,4,1), (2,5,1), K' (3,3,1), (2,3,1) over (1,5) (3,4) (3,5): 0 of 5,832 each
  ((-2,5) must stay air: the dragged stone lands there on open).
- bottom-left: G (-1,-1,1), (1,-2,1), (1,-3,1) over (-2,-3) (-2,-2) (-2,-1) (-1,-3) (-1,-2): 0 of
  1,889,568 each.
- sh0p's S (3,-3,1) open pulse from (2,-3): 0 of 18. A sensor's wake is floor(distance to the lever),
  and every legal lever spot (the static right wall, x = 3, y 0..2) is 3-5 blocks from (2,-3), so it
  wakes at t3-t5; S needs o16.

### q69h pass (fly asked for completeness): 1 of 11 inputs drivable
Lever on (3,2,1): (3,1,1) QC-arms K (3,-1) through (3,0) and breaks the door at t6. `door prune-plan`
dropped Qr o2 and c48, (2,4,1) c36, K c6. Every input screened over every filling of each region it
can be fed from, palette: air, wool, stone, sculk, calibrated x4, dust, observers x4, torches; the
rest-on redstone blocks of P_T, (2,5,1) and B1 removed with their input (control: BROKEN without a
filling):
- W (-2,-1,1): WORKS — calibrated sensor (-2,-2) (powers W, which the slime dragged there) + wool
  shield (0,-2): 4,112 fillings, cheapest 2 blocks; wip/q69h-W, redsim 32x3 and the game 4x2.
- A5: N (-1,-2,1), B1 (1,-3,1): 0 of 1,889,568 each.
- B4: Pl, P_T, (2,4,1), (2,5,1): 0 of 104,976 each.
- C3: K, A (2,-2,1), B1: 0 of 5,832 each.
- D3: Qr, (2,4,1), (2,5,1): 0 of 5,832 each.
