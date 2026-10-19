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
- Close-edge order found with ideal inputs (m105r-t.ideal sweeps): Q must come AFTER A3 on close,
  not just after A2 (Q restores the column; an A3 after that pulls T back up into (1,3)).

## 2b. Why K1 is the wall (feeds read cell by cell)
- A at (1,4) and at (1,3) (quasi cell (1,4)) can be fed only from (0,4) emitting east: (2,4) is R,
  and B's cell (1,5) as A's quasi cell is useless (any power into (1,5) also powers B). (0,4) has no
  floor (L's head / T at (0,3)), so it is an observer facing west (trigger = changes of (-1,4)) or a
  conductor fed by an observer at (-1,4) facing west (trigger = changes of (-2,4)).
- B can be fed only from (0,5) (K1) before K2 knows anything.
- L (-1,3): fed from (-2,3), from (-1,4), or by quasi-connectivity through (-1,4), whose
  neighbors (-2,4), (0,4), (-1,5) are exactly the cells A's and B's triggers must use: every
  powered conductor or emitter there also arms L (harmless only while nothing updates L: L is
  updated at its own pulse and its end, when T lands in (0,3), when TL lands in (-1,2)).
- P2 / P1 (one pulse): only from (-2,3) (or quasi through (-2,4)).
- K1 can watch nothing but L (from (-1,4)/(-2,3)) and P2: A's and B's only K1 neighbors are their
  own feeds, so the A/B train cannot be self-timed from the elevator; it must come from a timer in
  the 4 cells (-2,4) (-1,4) (-2,5) (-1,5) plus the lever, producing a1 < L < b1 < a2 < b2 < a3 with
  >= 3-4 gt gaps (measured windows: L 3..9 after a1, b1 >= L+3, a2 = b1+2..5, b2 >= b1+7,
  a3 >= b2+4). Harmless extras found: an A pulse between L and B1 (A extends into the empty (1,3)),
  an L pulse before A1 (m105r). Every hand layout tried ran into: A's trigger cell (-1,4) is also
  L's quasi cell and L's top neighbor, and the chain into (0,4) must change 3 times while (0,5)'s
  trigger changes twice, all from one lever event.

## 2c. 8x8-104 (the mirror, center down): blocked by one cell
- A' (1,-2) at rest is fed only from (0,-2) emitting EAST ((2,-2) is Q', (1,-3) is B'; its quasi
  cell (1,-1) has L's head, an empty ring cell (BR must move in) and BM (unpowerable) around it).
- A' lifted to (1,-1) for A2 (open) is fed only through the parked BM at (0,-1) (BR is still at
  (2,0), the quasi cell (1,0) is surrounded by door blocks, (1,-2) is B's head or air), and BM can
  only be strongly powered from below, (0,-2) emitting UP.
- So (0,-2) must emit east at a1 and up at a2: an observer emits one way; a conductor does not
  power the conductor BM; dust and diodes cannot point up; a torch (east + strong up) is rest-ON and
  its support (0,-3) (bottom row) has no rest-ON strong source. 104 needs a non-flat or non-redstone
  trick there; not pursued.

## 3. SAT wiring (agent satwire's `door satwire`)
- First run on families' sym ideal: UNSAT in 38 s. Cause: families' ideal plans put some inputs out
  of the plane (A at (1,4,1), (1,3,1)); satwire's recorder only counted in-plane redstone blocks
  as wanted power, so A looked unpowered while it moved. Fixed in satwire.rs (record_ideal: z+-1
  redstone blocks count, direct or next to the quasi cell); re-running.
- With the fix, families' gap-6 timing: still UNSAT (36 s): exact traces at arbitrary gaps are not
  realizable. New `door satwire --set` / `--grid` (sweep the ideal's variables, one SAT problem
  each). Template m105/sat/m105r-t.ideal (families' feed cells, every gap a variable; the default
  compact timing WORKS at 32 positions, open/close 29 ticks): grid tl 4..7, db 3..4, da 2..4,
  dbb 7,8,10, da3 4..5 in the 8x8 box (lever on a K1 face) and in 9x8 (x=-3..5): see the log.

## Log
- 22:53Z m105 ideal WORKS (32 positions); 23:03Z satwire UNSAT (recorder artifact); 23:11Z m105r
  (pulse-only parker) WORKS (32 positions).
- 23:52Z-00:02Z satwire grid (exact traces from m105r-t.ideal): 8x8 with the lever on a K1 face:
  20 of 144 combinations run, all UNSAT (~30 s each); 9x8 (x=-3..5): 3 run, UNSAT. Stopped at the
  retarget (00:01Z: 7 wide x 9 tall = 63 is the new box; 8x8 = 64 is no longer interesting).

## State of 8x8-105 at the retarget (for whoever picks it up)
- Works: the mechanism with ideal inputs (m105, m105r), the compact timing template, the close-order
  rule (Q after A3), the K2 plan (memory link watching B: wire56's gadget, 7 blocks, then R, P3+P4
  by quasi-connectivity from (4,2), Q delayed past A3) and K0 (watch Q from (0,-2), merge, P0).
- Open: K1. Plan was lever on a K1 face, (0,4) observer-west driving A, (0,5) B's feed, (-2,3)
  conductor driving L+P2+P1 (m105r), a timer in (-2,4)/(-1,4)/(-2,5)/(-1,5); no layout found, see 2b.
