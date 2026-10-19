# mechspace HANDOFF (keep current)

Track: mechanism search for full-frame flat doors (7x8 = 56, 7x9 = 63, 8x8 = 64). Branch
`explore/mechspace`. Boxes: devvm54942 (box3) and devvm52946 (box4), both dedicated, clones in
`~/agents/mechspace/minecraft-tas` (build: `~/bin/cargo build --release`).

## What exists
* `door mechspace` (crates/door-mech + crates/piston-door/src/mechspace.rs): exact macro-model
  decision procedure (plain enumeration + backward co-reachability, per-mask restricted search),
  wirable sub-model `--no-door-bodies`, FULL FRAME (`--full-frame --frame9`: 9 door blocks in the
  frame + 3 static stones; `augment_frame`), sweep mode (`MECHSPACE_SWEEP=1`).
* Piston-set enumerator `door mechspace enum` (side pullers with <= `--two` missing, `--carry` free
  pistons, `--bottom-su`, `--door-offset`; env MECHSPACE_RET_ONLY, MECHSPACE_K3, MECHSPACE_REACH,
  MECHSPACE_SHARD=k/n, MECHSPACE_RESUME=1, MECHSPACE_FRAME_SLOTS, MECHSPACE_FREE_NORMAL).
* Post-processing: `layouts` (group finds, drop supersets), `frame-export` (re-decide, stones,
  ideal plans, simulator replay + redsim frame check), `framecheck` (re-check older exports in
  place), `variants` (ext / MECHSPACE_SIDE_ONLY / MECHSPACE_HELD_TOP), `rank` (pockets2's ranking),
  `movable` (rule A), `gencheck`, `reachtest`.
* families generator flags for the audit bounds: `--lift-ext`, `--carry-in`, `--ext-pullers`.

## Results (tracks/mechspace/*.tsv, mech/<id>.txt + -sym/-oc.ideal; column 13 = sim verdict)
* 7x8 offset 2,2: c78, c78t, su8-*, su-layouts-e (single U), a78 (no single-U constraint: 403
  sym + oc), side78 (held side stacks: 723 sym + oc), heldtop78 (in progress).
* 7x8 offset 2,3 single U: u23 (268 sym + oc). 7x8 offset 1,2: nothing (single U or not).
* 7x9 offset 2,3: n79 (14 sym + oc; n79-009 = c79's mechanism). 7x9 offset 2,2: nothing beyond 7x8.
* 8x8 offset 2,2: n88 (9 sym + oc after the frame check; 8x8-only ones use column x = 5).

## Running (check with `ps -eo pid,etimes,args | grep mctas`)
* box4: `enum` 8x8 offset 2,3 (tag enum-8x8-23-ret, log /tmp/enum-88-23-ret.log).
* box3: `frame-export` heldtop78 (log /tmp/heldtop78.log).

## Next steps
1. Finish 8x8 offset 2,3: `layouts` + `frame-export` (MECHSPACE_SWEEP=1), framecheck, push, announce.
2. heldtop78: send t78 the variants that replay, ranked by pistons held.
3. Model the QC coupling of directly powered stacked pullers (satwire3): the frame check catches
   it now, the macro model should forbid / include it.
4. Max-held variants for satwire3 once its pulse/hold encoding is in.
5. Unswept: 7x8 single-U all-retracted tail (12-13 pistons, ~35% left), ext variants of the
   11-piston tail, 7x9 offset 2,4, 8x8 offset 2,4, normal pistons (MECHSPACE_FREE_NORMAL), pistons
   in frame cells at rest (MECHSPACE_FRAME_SLOTS; needs a streaming enumerator).
