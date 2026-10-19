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

Direction of the approximations (for UNSAT as a proof; every UNSAT so far is RELATIVE TO THE
ENCODING): OVER-approximating (the model allows more than the game; spurious SATs are rejected by
the redsim screen and the real judge): QC-powered pistons fire at once (the game needs an update),
blocked pistons' power is free (`--exact` pins it). UNDER-approximating (the model may reject a
design the game accepts): ambiguous same-tick orders are FORBIDDEN rather than branched (an
observer's off-tick coinciding with an ambiguous change of its watched cell, comparator races),
repeater-repeater races resolve one way, torches read after observers, intra-tick transients other
than the lamp's off-on are not seen by observers, QC power while a piston must stay off is
forbidden even when no update would come (BUD), droppers ignore quasi-connected power, supports
must be static, nothing movable two ahead of a sticky piston, the exact recorded piston power
(an ideal plan's own side effects become requirements: the warp only retimes pauses).
Recorder: the power of a piston = the real signal into its cell, minus a source that is only its
front neighbor (wire64's fix), or into the cell above (QC).

Known-answer test (`door satwire --check --jobs 8` over every FLAT archived door, 65 designs on
this branch): 61 of 64 in-scope REPRODUCED exactly (after: ambiguous observer off-tick
coincidences and comparator races branch instead of being forbidden, droppers re-read direct or
quasi-connected power on a neighbor change or an adjacent lever flip, a 1 gt pulse may be 2 gt);
left: fl81f (6 ticks: observer (2,5) fires at t14 in the game, not in the model: a transient of the
watched cell), flat-v5-52b (2 piston ticks), flat-v7-full (rejected). Before those fixes, on
this branch after merging main): 57 REPRODUCED exactly (e89b, every flat-b, flat-v2..v8 but v5/v7f,
s55, flat43-full + every lamp/dropper relever variant but the two `@support` ones, h44, h45, fl81m,
fl81n, f43-b1, lamp-sweep, fv58n, ...), 1 not flat, 4 MISMATCH (flat-v5-52b 2 ticks, fl81f 6 ticks,
the two dropper@support variants: their dropper is re-armed by quasi-connectivity on the lever's
update, a BUD), 3 REJECTED (fl72f, flat-b-v7-52b, flat-v7-full: cores name the cells around their
lever/observer chains; not diagnosed yet).

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
* Model fixes driven by the known-answer test: observer ticks scheduled by a moving block run after
  phase-S ticks (flat-v6/v7 observer chains), supports on retracting / down-facing piston bases
  (flat43-full's repeater on a piston), the lamp's 4 gt tick runs before observer/torch ticks and
  an input rising later relights it (the observer->lamp->observer relay of h44/flat43-full sees
  the off-on transient), a lamp lights at the flip itself (relever lamp variants), droppers are
  wiring and follow direct power: 17 -> 57 of 64 flat doors reproduced.
* e89b with the time warp (stalls/skips <= 6, palette incl. note/bulb/comparator): <= 28 wiring
  UNSAT after 44 min = 29 is minimal even when the schedule may be retimed (relative to the
  encoding at that time: before the observer-phase and lamp fixes).
* Ideals checked with `door ideal` before SAT: box64 f102b (8x8-130) BROKEN (not back at rest),
  box72 m2w BROKEN (not open) -> their UNSATs are moot; m710b and wire56 m028 (7x8-013) work:
  UNSAT relative to the encoding, cores: m710b open pistons (3,3)+(1,3); m028-oc close (1,4)+(1,5).

## Results 2026-10-19 (dedicated box devvm55676)
* **BEST: 49 blocks solid 7x9 (63), FULL SEAMLESS, frame 12/12, lever ON at rest, open/close 15/15,
  record-eligible as is (relever: repeater input 15/15)**: `archive/satwire-63b-49-49b.txt` (=
  `BEST.txt`). pockets-63b-50's mechanism with ALL wiring erased, re-wired from scratch:
  `--from-wired --full-frame --forbid-ring --one-warp --dmax 4 --skip 4 --sound --log-doors`,
  palette block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,glass,leaves,upiston; door
  and the 3 frame blocks are oak logs (the frame blocks patched from smooth stone to the same log).
* Known answer c79 (the community 7x9): `--check` 0/482; wiring erased (11 pistons, door, 3 frame
  blocks kept) and re-found from scratch: first judged door at 54b in 101 s, minimized to 51b (c79's
  own count; `archive/satwire-c79-closed-51b.txt`), <= 50 UNSAT with one-warp 4.
* m79q (m10top's 7x9 seed): the whole top-right (lever, R pair, TRp's drop, A's hold, B's 2 gt pulse
  and pull) re-synthesized together with `--keep-ideal "1,4;-2,2"`: `p79q-sat.ideal` WORKS 32 x 2
  with 50 blocks + L1/L2 ideal (A's last pull turned out real). Full synthesis (L1/L2 too) is open:
  in that partial the B chain starts from TL's arrival, which only L2 causes (circular).
* Batches over 262 mech-synth plans (mechspace incl. c78, frame/ff 90, seq56, ring56, vmid, w63top,
  fam-8x8-131/135): 0 doors; plans whose timing came from real wiring (c79, pockets, m79p) are the
  ones that solve. Plan timing (event order, simultaneous ideal pulses) is the blocker, not the model.
* Model fixes of the night: lever ON at rest; dust dots (power only down); carried redstone blocks
  (same-tick update); kept wiring keeps its rest state; un-updated QC holds may keep QC but not
  direct power (only where the piston's room to extend is certain); room to extend reads kept wiring
  (obsidian/droppers hold, components break); unmodeled blocks (sculk, rails, buttons, plates, glazed
  terracotta, non-iron trapdoors) are refused.
* New options: `--full-frame`, `--lever-on`, `--keep-ideal x,y;...` (partial synthesis), per-cluster
  time warps (default; `--one-warp` off) with order kept only between interacting events, batch mode
  (several plans: `--jobs N --out <dir>`, also with `--fixed`), `--local-nogood` (exact timing:
  block only the choices near the first misbehaving piston; heuristic).
* CEGAR false positives found and fixed while hunting 48b on the 63b mechanism: dust written in its
  placed shape (cross dust reshaped at placement fired observers), a retraction allowed one tick late
  must be unpowered the next tick, pulse tolerances only for ideal plans (wired timing is exact), no
  QC-armed piston at rest even with `--sound`, `--sound` QC holds only while no wiring update reaches
  the piston (diode/observer outputs into a neighbor count). Still open: an update-order effect
  (a piston checks power before a dust on a neighboring conductor recomputes: R2 under a ceiling
  lever stayed extended), not modeled; such candidates are blocked one by one.
* 48b status: exact model (no `--sound`) UNSAT at <= 27 wiring for both c79's and pockets' timing
  (one-warp 0 and 4); `--sound` finds 27-wiring models that all fail the judge so far (thousands of
  CEGAR iterations; pk6/pk8 runs on devvm55676 /tmp/sw-pk6-*.log, /tmp/sw-pk8-ln.log).

## Results 2026-10-19, second shift (satwire2, devvm55676)
* The first shift's background runs, checked: no door. 48b hunt (pk6-s0/s4/s8/scl, pk8-ln on
  pockets' timing, `--sound`): ~1600 candidates, every one rejected by the judge; m79 top (`m79gs`,
  18 window plans with `--sound`): 18/18 UNSAT; all-plans batches (bD, bF): no door; c79s2 (c79's
  timing, minimizing): its best stays the 52b c79 re-wiring; e89b calibration (cal1w) UNSAT; the 72b
  re-wirings (ff72) found nothing in 1.5 h.
* `--explain <candidate>` (new): the candidate's wiring fixed on the target recording (what the
  solver believed) against its real run. On the 48b failures: the R pair hangs under a ceiling lever
  on a note block that a dust on top also powers; on the lever flip the upper puller reads its power
  while the dust still powers the note block (QC through it), the dust then turns off without
  updating the piston (it is 3 cells away): the puller stays extended (a BUD), the lower one too.
* `--update-order` (new): which power changes UPDATE the piston (direct power always; QC power
  through a conductor next to the cell above, powered by a diode into it, a dust on it or a lever on
  it, never). A retraction needs those non-updating sources off the tick before (or a wiring update,
  or a justified neighbor mechanism change in the block-event phase); an extension needs an updating
  source rising (or held power plus an update, a moving block next to it finishing the tick before,
  or its own placement). Known answer `--check --update-order --sound` over the archive: 66 of 67
  flat doors accepted (flat-v7-full rejected as before, without the update clauses); all 5 saved
  48b judge failures rejected.
* **48b hunt with `--update-order`: UNSAT** for pockets' timing (exact, one-warp 4 and 8, per-cluster
  warps 6, local nogood) and c79's (exact, one-warp 4): no 48-block re-wiring of the 63b mechanism
  exists in the encoding (`--sound` QC holds + update order, retiming <= 8). The thousands of
  `--sound` 48s were all this BUD.
* `--exact-order` (new, with `--sound`): torches read after the observer ticks (FIFO): a 2 gt observer
  pulse into a torch's block has ended when the torch's tick reads it (e88-t7b re-wiring candidates
  used `--sound`'s free read order: torch (3,-4) under dust (4,-3) never turns off in the game).
* `--retime` (new, ideal plans): touching events of different clusters keep only their order (no
  3 gt safe gap); each candidate's timing is replayed in redsim with the plan's inputs moved with
  their pistons' clusters; a timing that breaks the mechanism is blocked for every wiring. Results:
  m79 top windows (18 plans) still UNSAT with order-only constraints; n79-009 (c79's mechanism, the
  calibration) UNSAT with and without it (mech-synth's motion pattern, every piston pulse-type and
  retracted at rest, is not c79's: the R pair lever-held at rest, the elevator sequence).
* mechspace single-U 7x8 sym plans (sl-027 = c78t, sl-030 = c78, sl-049, sl-053, sl-078, sl-079):
  UNSAT in seconds with ONE-piston cores (close (-1,-1) in sl-027/030, (-2,0) in 049/053, (3,-1) in
  078, (4,1) in 079), also without `--full-frame`/`--forbid-ring`: that piston's close pulse cannot
  come from the 2-3 free cells next to it and its QC cell (no clock source there in a 7x8 box).
  A layout filter worth adding to mechspace: every piston needs a free cell next to it (or next to
  its QC cell) that can see a mechanism change at the right time.
* c2e's m88c (8x8, A's hold the only ideal input): UNSAT, core A (1,3) alone, also with `--period2`;
  with the hold kept ideal (`--keep-ideal "1,3"`) it is SAT and judged OK: the model agrees with the
  design, the box has no room for the hold source.
* `--period2` (new): two cycles (open1, close1, open2, close2) share the placement, each with its
  own warp, each ending where the next starts: a copper bulb may toggle once per cycle.
* `--hint` (new, with `--from-wired`): solve once with the source wiring assumed (it also checks that
  the model accepts it), keep that assignment as the solver's phases: e88-t7b's 23-wiring model in
  ~50 s instead of no model after 20 min; the minimization then continues below the source.
* Unmodeled wiring of a wired source (sculk sensors, target blocks) is dropped (its cells become free)
  instead of refusing the design: solid re-wirings of the sculk doors and of target-using finds.
* Re-wirings (all `--from-wired --full-frame --forbid-ring --sound --exact-order --update-order
  --lever-oop`): e88-t7b (the 43b record) minimizing below 23 wiring (42b) in 7x9 (exact, one-warp
  4, cluster warps 6, local nogood, period2); e88-t7b's mechanism in 7x8 (its bottom wiring row cut):
  exact timing UNSAT, warps running; pockets' and c79's mechanisms in 7x8: UNSAT at once; the sculk
  doors without their sensors (m79s 7x9, m79s8 8x8): exact timing UNSAT (and one-warp 4 for the
  8x8); c79-49-bl5-br3's timing without its target block: a 49b door at once (ties), 28 wiring
  minimal with one-warp 4.
* More judge-driven model fixes (each found with `--explain` on a rejected candidate, each keeping
  the known answer at 66/67): `--exact-order` also resolves the COMPARATOR race (its NORMAL-priority
  tick runs after the observer that scheduled it: a 2 gt observer pulse never passes a comparator;
  e88-t7b 42b candidates used it); a DROPPER re-reads its power when a diode/observer whose output
  block is next to it changes (a 4-cell observer/dropper/note loop looked settled); CLOCK nogood: a
  candidate the judge finds "still moving" is replayed and only the wiring cells still changing at
  the end are blocked (the e88 7x8 candidates were all such loops).
* `--retime` exercised on ring56's j8: "TIMING FAILS (retimed ideal plan: not open): blocked 6
  too-close pairs" per candidate: the replay rejects the timings the order-only model admits.
* e88-t7b re-wiring: the old-binary runs reached 22-wiring (42b) candidates; all were rejected by the
  judge (comparator race, bottom puller (1,-2) never fires) and the fixed model rejects them too;
  fixed-model runs (exact, local nogood, cluster warps 6, all with `--hint`) restarted at 05:32 local.
* e88-t7b's mechanism in 7x8 (bottom wiring row cut): exact timing UNSAT (also with `--hint`),
  cluster warps 2 UNSAT; cluster warps 4 is SAT but every candidate so far is a clock (44 judged).
* Coordinator batches: n79 (mechspace 7x9 set, 61 plans, `--retime`): no door so far (10 plans fail
  `--full-frame`'s frame check, the rest UNSAT); a78 (427 7x8 sym plans): running in batch
  (`/tmp/a78s`), all UNSAT so far.
