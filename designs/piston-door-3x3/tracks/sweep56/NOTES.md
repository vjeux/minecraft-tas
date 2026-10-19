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
