# Track sweep56: a systematic satwire sweep of the 7x8 / 8x7 catalogs (56b, flat, FULL+hall)

Goal (coordinator, 2026-10-18 23:47Z): a 1-wide (flat), slime-free, FULL seamless + hallway-compatible
door in 7x8 or 8x7 = 56b (record 72b, e89b). vjeux (00:00Z): a flat 7 wide x 8 tall door EXISTS, so
every "7 wide is blocked" verdict is an artifact of some bound.

## Tools (this branch)
* `door satsweep <plan.ideal>... --box-dims 7x8 --variants base,gap,triple` (satsweep.rs): every plan
  x every placement of the box around the mechanism x every timing variant; each variant replayed
  with ideal inputs first (4 positions x 2 cycles; INVALID if the mechanism cannot follow it), then
  `satwire`; one row per run in [SWEEP.tsv](SWEEP.tsv) (unix time, plan, variant, box, result,
  wiring, judge, UNSAT core, seconds, log), logs + any judged design in `runs/`.
  * `gap`: every gap between two consecutive steps of a phase set to 2..5 gt (the overlaps the time
    warp cannot reach; longer gaps are the warp's stalls); `triple`: an input pulsed once becomes 3
    pulses 4 or 6 gt apart (net one move, odd change count for watchers).
* satwire changes: the judge JVM starts only with the first model (UNSAT runs hold no worker);
  `--core-tries N` bounds the core shrinking (each try is a full re-solve; 40 tries took 13 min);
  hole-backs only behind OPEN-state holes (`properties::seamless` skips depth-0 holes at rest for a
  flat door, so a park cell that is air only at rest no longer makes the cell behind it plain-only);
  `--hall-ring`: never-swept ring cells hold smooth stone, exactly what props-dyn's hallway check
  adds there, so a model is hallway-compatible by construction.

## Structural findings (before any SAT time)
* v2/v3 7x8 C-class (v2 005-012 = v3 008-013, 016, 017): the bottom-left pocket K0 (5 cells:
  (-2,-1) (-1,-1) (-2,-2) (-1,-2) (0,-2)) is SEALED: SE(-2,0) and SU(1,-2) can only be fed from K0
  (SE(-2,0)'s other sides: SE(-2,1) above, its QC cell is a piston whose free neighbors are none;
  SU(1,-2) sits between K0 and SU(2,-2), QC cell = the ring cell (1,-1)), and no cell outside K0 can
  watch either piston, BL's park cell (-1,0) or BM's park cell (1,-1). So K0 has no link at all: with
  the lever in K0 the other pockets get nothing, with the lever elsewhere K0 gets nothing. No timing
  or parity trick can help. v2 007/010 (= v3 009/011, SW(4,0) instead of SU(2,-2)) split into two
  sealed clusters {K0, bottom-right} and {top-left, top-right}. topology's `memory` column agrees
  (unreachable pistons with no link at all).
* v3 014/015 (= v2 013/014, wire56's m028/m029): the (4,4) lemma (wire56 NOTES section 5).

## Sweep log (SWEEP.tsv)
Every UNSAT in SWEEP.tsv is relative to satwire's ENCODING (docs/coverage.md, "the rule for
claims"): "no wiring with palette P in the 7x8 box under satwire's encoding" (block updates
re-read every tick, QC fires at once (no BUD), no 0-tick transients, no moved redstone blocks or
observers). A geometric core (one piston alone, e.g. "open piston (1,4)" for a piston boxed in by
pistons and the box edge) is reliable; a timing core is not yet. Rows written after 18:01Z say
`UNSAT-enc`.
* 17:17Z v3 7x8 C-class 008-017, sym + oc, base, 7x8 box, `--forbid-ring`, full palette
  (block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,obsidian), time warp 8/8: 20 UNSAT
  (as predicted by the sealed pockets). Cores (6 tries) all on CLOSE requirements.
* 17:25Z v3 7x8 021-038 (families' NO-FEED filter, but satwire models parked door blocks as relays
  and the filter does not): sym + oc, base, `--forbid-ring --hall-ring --lever-oop`: 36 UNSAT.
  Geometric cores "open piston (1,4)" alone for 028/030/035/036 (SD(1,4) between SD(0,4) and
  SD(2,4) under the box's top edge).
* 17:36Z pistmodel's 32 exact 7x8 plans: the 10 that are the sealed C-class skipped; 22 with
  `--widen 2` (their 1 gt pulses can't be made by flat wiring): 22 UNSAT.
* 17:40Z two new mechanisms outside the v3 catalog, both replay with ideal inputs (oc + sym; files
  in `mech/`), built to unseal the bottom-left pocket: h1 = SE(-2,1) SU(0,-2) SD(0,4) SU(1,-2)
  SD(1,4) SD(1,5) SW(3,3) SW(4,0) SW(4,1) SW(4,2) (ML left alone, so (-2,0) and (-2,2) are free and
  SE(-2,1) links the bottom-left and top-left pockets; BL down, TL up); h2 = SE(-2,1) SE(-2,2)
  SE(-1,3) SU(0,-2) SU(1,-2) SD(1,4) SD(1,5) SW(4,0) SW(4,1) SW(4,2) (BR right so (2,-2) is free).
  base + gap (2..5) + triple (4, 6) variants, sym + oc: 212 runs, 20 INVALID, 192 UNSAT.
* 17:56Z pistmodel's 3-signal group plans (`fm-7x8-NNN-groups.ideal`), the 22 non-sealed,
  `--widen 2 --even` (1 gt -> 2 gt, 3 gt -> 4 gt): all UNSAT.

## Calibration (18:05Z): is an ideal plan wirable at all where a wiring is known?
e89b (the 72b record, 8x9) has the mechanism SE(-2,0) SE(-2,1) SE(-2,2) SE(-1,-1) SU(1,-3)
SU(1,-2) SD(1,4) SW(4,0) SW(4,1) SW(4,2) (= v3 7x8-022 = fm-7x8-019: it FITS 7x8); its wiring uses
one more column (x=-3) and one more row (y=-4). Its lifted SU(1,-2) at the ring cell (1,-1) is fed
through a smooth stone in the never-swept ring cell (2,-1) (observer below it): families' NO-FEED
filter misses such relays, satwire models them.
* `satwire --check` on e89b: 0 / 864 piston-tick mismatches (the encoding admits its wiring).
* `--from-wired` e89b in its own 8x9 box (real power trains, `--forbid-ring`, small palette): SAT,
  judge OK at 32 positions, 61 blocks (calib/e89b-real-8x9-nohall.log).
* families' IDEAL plans of the same mechanism (calib/ideal/e89b-{sym,oc}.ideal) in the same box:
  UNSAT (sym and oc), and the elevator-hold variants UNSAT in all four 8x9 placements
  (CALIB.tsv). So the PLAN is what fails, not the mechanism: e89b's real trains hold the elevator
  8 gt (one level instead of push-spit + pull-back pulses), give the lifted SU(1,-2) and the resting
  SU(1,-2) 1 gt pulses, fire BR at t10 but MR/TR at t32, the left column during the elevator hold.
  Every UNSAT of this sweep is therefore "this train (+- warp) cannot be wired in this encoding"
  (UNSAT-train), never "this mechanism cannot be wired".
* e89b's real trains in a 7x8 box (x -2..4, y -3..4, warp 12/12): UNSAT. Geometrically its 7x8 box
  has no top-bottom link at all: both side columns are full walls of pullers, the top pockets see
  only SD(1,4) and each other, the bottom pockets only the elevator column; e89b's x=-3 observer
  column is exactly that missing link.

## Report (20:25Z)
No door: 0 SAT in ~400 satwire runs (SWEEP.tsv: 335 rows, 314 UNSAT-train, 20 INVALID plans, 1
ERROR; CALIB.tsv 52 rows; ~10 direct runs), plus the dedicated-box queue still running
(devvm52946: SWEEP79.tsv = the 7x8 mechanisms in both 7x9 placements with menus, SWEEP-c2e*.tsv =
mechspace's column-2-elevator mechanisms in 7x8 and 7x9).
* What is dead and why (timing-independent, by inspection of each pocket's boundary): the v3 7x8
  C-class (sealed bottom-left pocket; two sealed clusters for the SW(4,0) variants); e89b's
  mechanism in 7x8 (both side columns are full walls of pullers, so its top and bottom pockets
  never see each other; e89b's x=-3 observer column is that link); boxed-in pistons (geometric
  cores like "open piston (1,4)" alone).
* What is NOT shown: every other UNSAT is a property of the TRAIN, not of the mechanism. The
  calibration proves it: families' ideal plan for e89b's own mechanism is UNSAT in e89b's own box,
  with recorded power, with motion requirements, with single-input menus. The real e89b trains
  hold the elevator, give 1 gt pulses, and split the right column from the top puller.
* Tools left for the next round: `door satsweep` (plans x boxes x variants base/gap/hold/triple,
  menus `--menu-core --menu-coshift --menu-shifts --menu-widths --menu-triples`, `--widen/--even`,
  `--box`, `--recorded-power`), satwire menus (alternative recordings per cell group, composed
  with satwire's motion requirements), `--hall-ring`, open-state hole-backs, `--core-tries`.
* Next ideas: (1) satwire's measured windows (per-event tolerance windows from redsim sweeps)
  inside the menu groups, and the coshift calibration (e89b-sym, own box) as the acceptance test
  (it was still solving after 50 min, the only long solve of the night); (2) QC-as-BUD in satwire
  (an armed piston firing on a later update is how real compact wiring sequences pistons; the
  encoding forbids QC power whenever a piston must be off); (3) mechanisms whose side column has at
  most 2 pullers, so top and bottom pockets link through a free side cell (h1 does it on the left;
  its full right column then needs the TR relay); (4) in 7x9, the free row links the top (or
  bottom) pockets: the dedicated-box queue runs exactly that.

## Menus (18:00Z): let the solver pick the timing per toggle and the core variant
`satwire` alternative groups (`Rec.groups`, `add_menu`): a group's cells read their frames and
piston power from the alternative the solver picks (one selector per alternative, exactly one per
group); everything else from the base. `door satsweep --menu-core --menu-shifts=... --menu-widths
... --menu-triples` builds them: one group per toggle (input pulsed once outside the core span:
shifted, re-widthed, tripled) and one CORE group (one core pulse shifted -3..+6 or re-widthed
1/3/4 gt, or two pulses of one input held as one level). Alternatives are recorded one by one (the
other phase played without snapshots) and kept when they start and end like the base trace;
groups whose cells meet an earlier group's are dropped. satwire's owner (session 1e942b23) adds
motion-derived requirements and measured windows on top (agreed design: menus as they are,
windows replace the shift lists).
* 18:47Z merged explore/satwire b9d7ff1 (motion-derived requirements, the default; menus compose:
  `motion_req_ph` runs per alternative phase). Under motion requirements a menu group's cells are
  the FRAME differences only (power differences were the ideal inputs' QC side effects, which
  glued every toggle to its neighbors and dropped the groups).
* Calibration, e89b-sym in its own 8x9 box, motion on: base UNSAT; per-input shifts +4..+12 UNSAT:
  the right-column toggles' shifted alternatives fail the start/end check (the ideal inputs at
  (4,-1), (5,1), (5,2) QC-couple the column: each also powers the piston below the one it feeds,
  so shifting one alone re-extends its neighbor). `--menu-coshift`: one group per step and side of
  the door (all its single-pulse inputs shifted together), so the right column can move as a unit
  after the top puller (satwire's diagnosis: the top-right pocket's only trigger is an observer on
  the top puller, and the plan fires both at t36).
* Bugs found and fixed on the way: satsweep recorded 80 ticks per phase, too short for vmid's plans
  (last event at t91): their first rows were spurious UNSATs (empty cores), rerun. `--hall-ring` is
  UNSAT before any piston requirement on vmid's vb1/vb2/vb3/vc1: a never-swept ring cell sits two
  ahead of a sticky piston (vb3: probably (-1,1), ahead of the carried SU at (-1,-1)), so the
  hallway check's stone there would be pulled; without `--hall-ring` they get real cores.
  Rewriting SWEEP.tsv while sweeps append to it loses their rows (their fds keep the old inode):
  fm-7x8-016 and fm-7x8-030 (recorded power, widen 2) were still solving after 54 min and were
  killed by the task bound; fm-7x8-017-groups was killed after 80 min.
