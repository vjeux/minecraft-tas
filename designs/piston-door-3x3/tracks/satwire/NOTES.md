# Track satwire: SAT synthesis of a door's wiring for a fixed mechanism

`mctas door satwire <mechanism> [--from-wired] --box=x0,y0,z0:x1,y1,z1 [--forbid-ring] [--palette ...]`
(crates/piston-door/src/satwire.rs). The mechanism and each piston's power per tick come from one
recorded run (an ideal schedule `.ideal`, a families catalog entry `--families <id>`, or a wired
design with `--from-wired`, wiring stripped); the wiring is a one-hot placement per free cell of
the box plus one lever among candidates; two traces (lever on from rest, lever off from open) share
the placement; the redstone semantics are unrolled per game tick and every piston must get exactly
its recorded power. Solver: CaDiCaL (rustsat). CEGAR: every model is judged by the real simulator
(4 positions, then `--extra-offsets 28`); a failing design is blocked; the wiring block count is
minimized with a totalizer. `--check` fixes a wired design's own wiring and reports where the model
disagrees with the recorded piston power (several designs at once: one line each + counts).

Usage:
```
mctas door satwire x.ideal --box=x0,y0,z0:x1,y1,z1 --forbid-ring \
  --palette block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor [--dmax 8 --skip 8] [--first] --out f.txt
mctas door satwire e89b.txt --from-wired --forbid-ring          # re-wire a design's mechanism
mctas door satwire tracks/families/CATALOG.tsv --families 8x8-119 # a catalog entry (sym train)
mctas door satwire plan.ideal --fixed partly-wired.txt          # keep those cells, solve the rest
mctas door satwire --check archive/*.txt --jobs 8               # replay check of the model
```
UNSAT prints a core: the pistons whose recorded power cannot be produced together; `--diag` also
names the conflicting constraint groups (supports, a cell's semantics, scheduling, trace ends).

## COVERAGE: what the encoding represents, and what it does not

Scope: FLAT doors only (every block in the door plane `z = door.z`; a lever may sit one cell in
front of / behind a free cell with `--lever-oop`); no `door-axis: z`, no entities.

Components (placement palette, `--palette`):
* block (smooth stone, conductor), obsidian (conductor, immovable), dust (exact levels 15..1,
  horizontal + up/down step diagonals as `RedstoneWireEvaluator`, a block powered only by dust never
  feeds dust; written as crosses: in a flat plane every dust points east and west and down),
  repeater (delay 1-4, facing E/W; no LOCKING: in a flat door a repeater's sides are out of the
  plane), observer (E/W/U/D), torch (standing, wall E/W; strong power up), note block (conductor,
  `powered` watched by observers; instrument computed by the game), waxed copper bulb (T flip-flop
  on a rising input, read by comparators as 15/0; observers see `powered`/`lit`), comparator
  (COMPARE mode, rear input only: flat comparators have no side inputs; reads a bulb directly
  behind it; rear dust not allowed), redstone lamp (conductor, on at once, off after a 4 gt tick),
  iron trapdoor (closed, bottom: non-conductor, `powered/open` follow its power, observers see it),
  dropper (conductor, `triggered` follows direct or quasi-connected power).
* NOT represented: targets, rails, dispensers WITH items, hoppers, sculk, slime/honey as wiring,
  redstone blocks as wiring, buttons, comparators reading through a block or containers,
  comparator SUBTRACT (no side input in a flat plane anyway), top-half trapdoors, levers anywhere
  but the chosen candidate.

Engine behaviors:
* Time: game ticks; the lever flip happens after tick 0; a component's state at the end of each tick.
  Scheduled ticks: repeaters fire first in a tick and read their input as at the start of the tick
  (phase R; the real game orders repeaters by priority -3/-2/-1 and a repeater chain can propagate
  inside one tick: APPROXIMATED, races between repeaters are not modeled); observers next, torches
  read their input after repeaters and observers (real: FIFO among priority-0 ticks); an observer
  tick scheduled by a block-event-phase change (moving block, lever) runs after the phase-S ticks;
  an observer's off-tick coinciding with an ambiguous change of its watched cell is forbidden.
  Dedup (`willTickThisTick`, one pending tick per block) is modeled exactly.
* Repeaters: pulse extension (`!on -> on`, reschedule when the input is off), exact. Comparators:
  out = rear input 2 gt after a change (a race at its own tick is forbidden). Torches: lit = !input
  at their tick. BURNOUT (8 toggles in 60 gt) NOT modeled (CEGAR catches it). Lamps: exact 4 gt off.
* Quasi-connectivity: QC power (a signal into the cell above a piston) counts as the piston's
  power when it must be on (`--no-qc` to forbid) and must be off whenever the piston must be off.
  QC as a BUD (armed but not fired until a block update reaches the piston) is NOT modeled: the
  update that fires a QC-powered piston is assumed (CEGAR catches the failures).
* Block updates are not modeled as such: every receiver re-reads its input at the end of each tick
  (what dust/repeater/observer/torch updates deliver in practice); 0-tick transients inside a tick
  (a dust level rising and falling, a 0-ticked piston) are NOT seen by observers in the model.
* Update ORDER (the position-dependent HashSet order of dust updates) is not modeled: the CEGAR
  judge runs the real 32 positions.
* Pistons are NOT wiring: the mechanism is fixed (its frames are recorded); observers DO watch
  mechanism cells (piston bases, heads, moving blocks, door blocks: every recorded state change is a
  trigger at its tick), moving door blocks are conductors at their recorded cells (a powered door
  block powers a piston next to it), pistons get power from any wiring next to them (torch-held,
  dust, repeaters, observers, powered blocks, QC). Moved redstone blocks / observers as wiring: no.
* Mechanism-safety: no wiring in any cell the mechanism occupies at any tick, nothing movable two
  cells ahead of a sticky piston (it would be pulled), supports (dust, repeaters, comparators,
  torches on a static sturdy block; levers on a static sturdy block).
Timing:
* Recorded profile: piston power per tick = the REAL signal into the piston cell or the cell above
  (simulator probes; ideal inputs may sit anywhere, also off the plane) - adopted from wire64's
  `--real-power`.
* Exact per-tick power, but with a TIME WARP: the solver may replay a static recorded tick (a stall,
  `--dmax` per trace) or skip static ticks (`--skip`): pauses between piston motions stretch or
  shrink, motions themselves play at full speed. Pulse widths stay exact (a 2 gt pulse stays 2 gt).
Box and ring:
* `--box`: wiring cells; the lever may sit outside the box (it is not counted in the volume).
* `--forbid-ring`: the 12 ring cells around the opening and the cells right behind hallway holes
  hold only plain blocks (FULL seamless + hallway).
* `--fixed <design>`: that design's wiring cells and lever are kept; a partly wired ideal plan
  (with `use` of its real lever) keeps its wiring automatically.

Replay check (`--check`, the model fixed to a design's own wiring, recorded power compared):
see the log below for the archive counts.

## Log
* e89b `--check`: the model reproduces e89b's piston power exactly (0 mismatches / 864 piston-ticks).
* e89b re-synthesis (mechanism only, 8x9 box, `--forbid-ring`, exact timing): the FIRST SAT model
  (52 s) passes the real judge at 32 positions (42 wiring blocks, `e89b-sat42.txt`); minimization
  43 -> 29 wiring blocks in 14 CEGAR iterations (every model judged OK) = e89b itself (49 placed,
  `e89b-sat29.txt`); <= 28 UNSAT = e89b is wiring-optimal for its own timing in this model.
* flat-seam r046x3-plane(-levels) 11x5: UNSAT (both schedules, full palette): agrees with strip's
  lemma (no static wiring for 5-tall strip doors).
* Archive replay check (287 designs on this branch): 254 not flat, 2 turned, 2 entities = out of
  scope; flat ones: see `door satwire --check` counts in the latest report.
