# Track mech-synth: mechanism synthesis around the 3x3 door

Tool: `mctas door mech-synth` (crates/piston-door/src/mechsynth.rs).

## How it works
* **Macro piston model** (pure Rust, no JVM): a port of `PistonStructureResolver` (push limit 12,
  slime/honey branching, honey-slime non-stick, glazed terracotta push-only, obsidian and extended
  pistons immovable, heads immovable) and of the sticky-retract rules of `PistonBaseBlock`
  (pull only PUSH_PULL blocks or retracted pistons). One action = one settled piston action:
  `L` toggles a piston's level signal (extend, or retract with the sticky pull), `P` is a 2 gt pulse
  from rest (push and spit; or, with a gap in front, extend into it and pull the block beyond).
  A world = the door (9 stone at x 0..2, y 0..2, z 0) + pistons/slime/honey/obsidian/gt/blocks
  with identities and level bits.
* **Solve**: the whole reachable state graph (capped) -> cheapest open path (door + walkway clear)
  + close path (back to the exact rest state); big mechanisms use weighted A* (h = blocks in the
  corridor / blocks off their rest cell).
* **Symmetric schedules** (cheapest control: the same delays on both lever edges): ONE event
  sequence S with S(rest) = open and S(open) = rest. Graph DFS with distance pruning, or (big
  mechanisms) "edge" pistons fired as one block at the start/end + iterative deepening on the
  core pistons. Swallowed pulses (pulse on a powered piston) and blocked pushes are no-ops, which
  is exactly what fc2's symmetric control exploits.
* **Enumeration**: a base design + up to `--add k` candidates (pistons of 4 planar or 6 facings,
  retracted or extended at rest, whose push/pull reach touches the mechanism or the ring of cells
  around the door; slime next to the mechanism), `--require kinds:box` constraints, screened by a
  capped BFS, then solved; ranked by (blocks, control events).
* **Ideal-input plans**: each macro event becomes redstone blocks set/cleared next to the piston
  (`door ideal` schedule): cells kept air, away from slime/honey and from other pistons' bases
  over the whole time they exist; commuting pulses share a tick (needed: stacked pistons
  quasi-power each other); swallowed pulses are not emitted. `--verify N` replays them in the
  real simulator (4 + `--extra-offsets` positions, 2 cycles).

```
mctas door mech-synth <base.txt> --just-base --cap 20000 --sym-len 8 --verify 1 --out-dir <dir>
mctas door mech-synth <base.txt> --add 3 --lo=-2,3,0 --hi=4,6,0 --require 'piston,slime:1,3,0:1,6,0'
```

## Results (all mechanism blocks; the door's 9 included; wiring not included)

| mechanism | blocks | slime | schedule | simulator (ideal inputs, 16 positions x 2 cycles) |
|---|---|---|---|---|
| fc2 (record 40) mechanism part | 25 (+4 floors) | 9 | open 8 + close 5 | (record) |
| `fc2like22`: fc2 without Q/R and the P_R connector, P_R a pull-type pulse piston | 22 | 8 | open 8 + close 5, NO symmetric sequence | WORKS (open/close) |
| **`abpt21`**: side columns on 3 slimes + 1 pull-type piston each, M pull-type, elevator A (extended at rest) on B, T-mover P_T at (0,3) facing east | **21** | 6 | open 9 + close 6; **symmetric 11** | WORKS (open/close and symmetric) |
| **`pist19x` / `pist19`**: no slime: every edge block on its own pull-type sticky piston (2 stacks of 3 + M), elevator A on B, P_T at (-1,3) | **19** | 0 | open 13 + close 10-12; **symmetric 15** (pist19x) | WORKS (open/close and symmetric) |

Ideal designs + schedules: `ideal/<name>/base-{oc,sym}.{txt,ideal}` (replay: `mctas door ideal
ideal/abpt21/base-sym.ideal --extra-offsets 28`).

### Minimality inside the searched spaces
* Base `sidesM` (18: door, 2 slime columns, their pull-type drivers, M's pull-type piston) + up to
  3 blocks (sticky pistons of 4 facings, retracted/extended, slime) anywhere in x -2..4, y 3..6:
  19,967 combinations, **nothing below 21**; 50 distinct 21-block mechanisms, all "A + B + one
  T-mover": A at (1,4) (retracted OR extended at rest), B at (1,5), the T-mover at (0,3) facing
  east, (-1,3) facing east (pull-type), (2,3) facing west or (3,3) facing west (pull-type).
* Base `sides` (17, no M piston) + 3 blocks anywhere in x -3..5, y -4..6 with one block above and
  one below the door (570k combinations): none. M needs its own piston, the core needs 3.
* Argument for 19 as the floor of this family: every block outside the window touches at most one
  door block, so the 8 edge blocks need 8 handlers (a slime or a piston each; a slime column also
  needs a driver, so 3 pistons per column beat 3 slimes + 1 piston); C needs two pulls from the
  exit side (heads at (1,2) then (1,3)): A plus something that moves A (B); T must clear C's exit
  column after it is lifted (+1 move: P_T, or 2 slimes riding a side structure as in fc2).
  19 = 9 + 8 + B + P_T. The slime-free board's design (`slime-free-combo-48b`) has the same 19.

### Symmetric control found (abpt21, the fc2 trick re-derived by the search)
`S = [P_L P_M P_R + P_A] [P_T] [L_A] [P_T] [L_B] [P_A] [L_B] [P_A]` (one tick column per step):
* open: P_A is swallowed (A powered at rest), the first P_T is a no-op (nothing at (1,3)), L_A
  lifts T, P_T pushes T to (2,3), B pushes A down, A pulls C up, B lifts A, A pulls C up again;
* close: P_A spits C down to (1,2), P_T brings T back to (1,3), L_A pushes T and C home and holds;
  the second P_T is blocked by A's head, B's window is blocked (A extended), the later A pulses
  are swallowed.
Signals: one edge pulse (P_L, P_M, P_R, A), A's level = NOT lever delayed (fc2: the shuttle), two
P_T pulses around A's level change, B's window, A pulses on B's edges (fc2/v2: an observer on B's
line). fc2 needs the same signals except that its shuttle also drives P_R (the right structure
drags T), which is why fc2 pays 4 more mechanism blocks (Q, R, 3 slimes) but no P_T pulses.

### Pulse-only symmetric control (`--pulses-only`): 12 mechanisms of 21 blocks
Every piston retracted at rest and driven by 2 gt pulses only, the SAME pulse train on both lever
edges (no level signal, no shuttle, no hold, no torch). Enumeration `sidesM + 3` in 3D
(x -2..4, y 3..6, z -2..2, 125k combinations): 74 mechanisms of 21 blocks open+close, **12 of
them have a pulse-only symmetric sequence (9-10 pulses)**, all verified in the simulator with
ideal inputs (open/close and symmetric, 16 positions x 2 cycles): `catalog-21/m00..m11`
(`.info` = sequences; `-sym.ideal` replays with `mctas door ideal`). They differ by the T-mover:
(0,3) east pusher (= `abpt21r`), (-1,3) east (extended or pull-type), (2,3) west, (3,3) west,
or along z: (1,3,-2)/(1,3,-1) south, (1,3,1)/(1,3,2) north (T leaves into z = +-1 above the
walkway, as in slime-free-combo).

`abpt21r` (`ideal/abpt21r/base-sym.ideal`, 32 positions x 3 cycles):
`S = [P_L] [A] [P_T] [B] [A] [B] [A] [M] [P_R]`
* open: A pulls T up, P_T pushes T to (2,3), B pushes A down, A pulls C to (1,2), B pulls A back
  up, A pulls C to (1,3), M is pulled down;
* close: A spits C down to (1,2), P_T pulls T back to (1,3), B pushes A, T and C down (T, C home),
  A spits T and C one further down (M is still out), B pulls A up, A's pulse pulls air, and M's
  piston pushes M, C and T back up together.
* B may be one window (level on at the first B, off at the second): `wire/abpt21r-window.ideal`.

Timing windows (`door ideal --independent --sweep`, each pulse moved alone, others at their
default; ticks after the first A pulse): A1 0..2, P_T 4..7, B on 11..16 (>= P_T + 5), A2 14..19
(>= B on + 2), B off 23..28 (>= A2 + 5), A3 26..36 (>= B off + 2), **M >= A3** (any earlier and
A3 lifts T on close), P_L and P_R anywhere (0..50). fc2's control produces A1 (lever observer ->
dust -> A), the B window (8 gt repeater) and A2/A3 (observer on the window repeater) exactly.

### k = 4: 22-block mechanisms (`catalog-22-k4.txt`)
`sidesM + 4` pulse-only, x -2..4, y 3..6, z -1..1 (896k combinations, 11 min on 44 threads):
3,031 mechanisms open+close (50 of 21 blocks, 2,577 of 22), **396 of 22 blocks with a symmetric
sequence**. Each symmetric line ends with `[M pulse at step i, riders: ...]` (riders = pistons
pulsed at two places on the open run). Notable:
* M's pulse FIRST and the last pulse at the top (no late pulse at the bottom): #356, #358, #359,
  #361, #362, #377 (most use a slime next to the T-mover so that a side column carries it).
* **cores without the B elevator piston**: `bases/k4-356.txt` (X sticky down at (1,3) right on
  T, slime (2,3), Y extended down at (2,5), Z west at (3,3)) and `bases/k4-355.txt` (P east at
  (-1,3), slime (0,3), X down at (1,3), Y extended down at (0,5)): 22 blocks, symmetric 10
  pulses, **verified in the simulator** (open/close and symmetric, 32 positions x 2 cycles):
  `ideal/k4-35{5,6}/`.

### Why none of these was wired below 40
* All 12 pulse-only schedules need either **M's pulse last** (after A3, >= ~22 gt after the flip,
  at the bottom of the door, far from the elevator's control) or a T-mover that rides a side
  structure (two positions to power). Every late cheap signal near the top fires twice per edge
  (observers on the window repeater, on B), and a once-per-edge late pulse needs the lever level
  through ~3 repeaters + an observer.
* Estimate on fc2's control layout: 15 - 2 (Q's repeater + floor) - 2 (P_L observer chain, now
  driven late) + ~7 (late delay line for P_L, M via a BL motion trigger) + ~2 (P_T at 6..7) + ~2
  (P_R) = ~22 control + 21 mechanism = ~43. fc2's shuttle couples A's level with P_R's level and
  reverses their order on close for 2 mechanism blocks; that coupling is worth more than the 4
  mechanism blocks these designs save.
* `--edges start` (side/M pulses forced first): no pulse-only symmetric sequence <= 10 core pulses
  for abpt21r; m10/m11 put M early but their T-mover rides a side structure.

### Next ideas
* Wire abpt21r (B as one window, fc2's D-repeater + observer-on-D for A2/A3) with an observer
  chain whose tail runs down the side to M's piston: the ~16 gt of travel IS the required delay
  (M >= A3); the other agent `wire21` is on it.
* A control-cost objective for mech-synth (merged steps x distance from the lever, late pulses far
  from the elevator) instead of "fewest events"; then rerun k = 4..5.
* Model fc2's coupled signals (a shuttle: A off then X on, X off then A on) in the macro model to
  search mechanisms around fc2 with its control topology fixed.
* Faster search (occupancy grid instead of linear scans, partial-order reduction of independent
  pulses) to reach k = 5 additions or bases with fewer fixed parts.

### What failed / caveats
* `fc2like22` (pulse-driven P_R dragging T) has no symmetric sequence: dragging T needs the
  order "A then P_R" on open and "P_R then A" on close: an order reversal, which is what fc2's
  redstone-block shuttle provides.
* Ideal inputs ignore wiring cost; a redstone block next to B also quasi-powers A whenever A sits
  right under B (unavoidable in the elevator; harmless because A is not updated then).
