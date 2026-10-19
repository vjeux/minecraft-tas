# mechspace HANDOFF (final, 2026-10-19 15:00Z: slot reassigned, NOTHING running)

Track: mechanism search for full-frame flat doors (7x8 = 56, 7x9 = 63, 8x8 = 64). Branch
`explore/mechspace` (last commit 61a83d07 or later). Boxes used: devvm54942 and devvm52946, clones in
`~/agents/mechspace/minecraft-tas` (build: `~/bin/cargo build --release`). No process left running.

## Tools (crates/door-mech `door mechspace <cmd>`, logic in crates/piston-door/src/mechspace.rs)
* Exact macro-model decision (plain enumeration + backward co-reachability, per-mask restricted
  search), wirable sub-model `--no-door-bodies`, FULL FRAME `--full-frame --frame9` (9 door blocks
  in the frame + 3 static door-type blocks, `augment_frame`), sweep mode `MECHSPACE_SWEEP=1`.
* `enum`: piston-set enumerator (side pullers <= `--two` missing, `--carry` free pistons,
  `--bottom-su`, `--door-offset`; env MECHSPACE_RET_ONLY, _K3, _REACH, _SHARD=k/n, _RESUME=1,
  _FRAME_SLOTS, _FREE_NORMAL). Acceptance: generates and solves c79 (7x9 offset 2,3).
* Post-processing: `layouts`, `frame-export` (re-decide, stones, ideal plans, simulator replay +
  redsim frame 12/12 check), `export` (MECHSPACE_HELD_FIRST, MECHSPACE_Z1, MECHSPACE_QC_QUIET,
  tile phase by default, MECHSPACE_BETWEEN for the old rule), `framecheck`, `variants`
  (MECHSPACE_SIDE_ONLY / _HELD_TOP), `flat` (pockets form), `rank` (MECHSPACE_RANK_QC),
  `movable` (rule A), `gencheck`, `reachtest`.
* families flags for the audit bounds: `--lift-ext`, `--carry-in`, `--ext-pullers` (families,
  fullmech, mechspace gen).

## Results (tracks/mechspace/*.tsv; mech/<id>.txt + -sym/-oc.ideal; column 13 = sim verdict:
WORKS = simulator judge + redsim frame check)
* 7x8 offset 2,2: c78, c78t, su8-*, su-layouts-e/f (single U), a78 (403 sym + oc, no single-U
  constraint), side78 (held stacks), heldtop78, hf78 / hft78 / hfh78 (held-first plans), z1-top10
  + flat copies (pockets3), qcq78 (QC-quiet recoveries), rc10 / rc11 (formerly capped sets), nm78
  (normal pistons), rank-7x8*.tsv, corner-ok-*.txt (satwire3's corner rule).
* 7x8 offset 2,3 (door 3 rows up) single U: u23, hs23 (held-first). Offset 1,2: nothing.
* 7x9 offset 2,3: n79 (n79-009 = c79), hf79 / z1-79 (c79m held-first). 7x9 2,2 / 2,4: nothing new.
* 8x8 offset 2,2 / 2,3: n88, n8823 (8x8-only ones use column x = 5).
* Structural facts: every working 7x8 / 7x9 full-frame mechanism uses a corner piston; satwire3
  finds the held-first 7x8 plans UNSAT so far, cores = a corner piston + its diagonal side puller
  sharing feed cells (corner-ok lists = diagonal side held).

## Exact next steps for a successor
1. satwire3 / pockets3 results on corner-ok-*.txt and z1 flat copies: if all UNSAT in 7x8, search
   mechanisms whose corner pistons are fed differently: pistons in frame cells at rest
   (MECHSPACE_FRAME_SLOTS: 7x8 single U free <= 3 = 181k sets, free <= 4 = 2.56M), movable extras
   beyond the frame cells, 3-move door-block paths, normal pistons with free 5 (13M sets).
2. QC coupling of directly powered stacked pullers is not in the macro model; QC-quiet plans and
   the frame check work around it (qcq78).
3. Remaining unknowns: 7x8 single U all-retracted sets over the cap: 63 (10 pistons, at 3M),
   3,575 (11, at 1M), 71,408 (12; shard 0 ~70% at 500k, nothing found).
4. z=1 inputs + tile phase break the elevator in the simulator (use between for z1 plans).
