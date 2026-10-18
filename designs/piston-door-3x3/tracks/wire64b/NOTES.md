# Track wire64b: a 1-wide (flat), slime-free, FULL seamless + hallway door in an 8x8 box (64b)

Goal (coordinator 22:43Z): 8x8 = 64 (record 72b: flat-seam e89b, 8x9x1). Targets from agent
families' v1 catalog (6b2d389): **8x8-105** (center UP through the elevator A (1,4) + B (1,5),
T parked aside at (0,3) by (-1,3)), then 8x8-104 (the same mirrored down). Box x=-2..5, y=-2..5.

## 1. Mechanism (ideal inputs)
- `m105/m105-pure.txt` = 8x8-105 as catalogued (T parker SE+(-1,3) extended at rest, a level).
  `door families --check m105/m105-pure.txt --verify --extra-offsets 28`: open 13 + close 10, sym
  13+L `A L B A B A` + 7 toggles; **oc and sym WORK with ideal inputs at 32 positions**
  (m105/ideal/m105-pure-{oc,sym}.ideal).
- `m105/m105r-pure.txt` = the same with the parker RETRACTED at rest, SE(-1,3): it pulls T from
  (1,3) into (0,3) on open and pushes it back (spit) on close, so the sym train is **13 pulses,
  no level**, and no piston head sits in the ring at rest. **oc and sym WORK at 32 positions**
  (m105/ideal/m105r-pure-{oc,sym}.ideal). With pulses only, the parker L, P2 (-2,2) and P1
  (-2,1, quasi-connected through P2's cell) can share ONE feed: a conductor at (-2,3).

## 2. Wiring topology (door topology on the v1 TSV)
Regions (free cells): K0 bottom-left 5 cells (feeds P0 (-2,0), Q (1,-2)), K1 top-left 7 cells
(-2,3) (-2,4) (-1,4) (0,4) (-2,5) (-1,5) (0,5) (feeds P1, P2, L, A, B), K2 right 21 cells (feeds Q,
B, R (2,4), P3 (4,0), P4 (4,1)). Links: Q watched from (0,-2)/(2,-2) (1 change), B watched from
(0,5)/(2,5) (2 changes). Topology says "memory WIRABLE with the lever in K0"; that verdict ignores
ORDER: the core starts with A (fed only from K1) and L (K1 only) before B ever moves, and K1's only
incoming link is B. So **the lever must be in K1** (lever outside the box, on a K1 cell's face:
the lever is excluded from the volume). Then:
- K2 sees only B (2 changes per edge), and every K2 piston (R, P3, P4, Q) is push-and-spit on
  close: a single bulb + comparator WINDOW fails (it pushes, then the sticky retract pulls back;
  wire56's memory-window gadget). It needs the full memory link (two flip-flops: bulb, comparator,
  bulb, comparator, observer = one 2 gt pulse per edge, 6 gt after the first input; wire56's
  designs/gadgets/memory-link). Q must fire after A2 on close (A2 pushes C into (1,0)): the
  memory link's output (~b1+8) is late enough.
- K0 watches Q from (0,-2) (one Q pulse = two observer changes 4 gt apart: merged) and fires P0.
- K1 (7 cells + the lever) must generate A x3 (a1 first, a2 ~ b1+4, a3 ~ b2+4), L (after A1 has
  landed T in (1,3), before B1), B x2 (or one window), P1+P2.

## 3. SAT wiring (agent satwire's `door satwire`)
- First run on families' sym ideal: UNSAT in 38 s. Cause: families' ideal plans put some inputs out
  of the plane (A at (1,4,1), (1,3,1)); satwire's recorder only counted in-plane redstone blocks
  as wanted power, so A looked unpowered while it moved. Fixed in satwire.rs (record_ideal: z+-1
  redstone blocks count, direct or next to the quasi cell); re-running.

## Log
- 22:53Z m105 ideal WORKS (32 positions); 23:03Z satwire UNSAT (recorder artifact); 23:11Z m105r
  (pulse-only parker) WORKS (32 positions).
