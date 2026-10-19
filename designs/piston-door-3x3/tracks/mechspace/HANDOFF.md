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
  sym + oc), side78 (held side stacks: 723 sym + oc), heldtop78 (one stack + top pistons held:
  2,521 working), hf78 / hft78 (HELD-FIRST plans of the one-side-held side78s: 740 working;
  hft = tile phase), hfh78 (held-first plans of heldtop78, running), z1-top10 (pockets2: z=1
  inputs, held-first, between ticks).
* 7x8 offset 2,3 single U: u23 (268 sym + oc). 7x8 offset 1,2: nothing (single U or not).
* 7x9 offset 2,3: n79 (14 sym + oc; n79-009 = c79's mechanism; hf79 / z1-79 = c79m held-first).
  7x9 offset 2,2 and 2,4: nothing beyond 7x8 (2,2) / (2,3).
* 8x8 offset 2,2: n88 (9 sym + oc after the frame check; 8x8-only ones use column x = 5);
  8x8 offset 2,3: n8823 (1 sym + oc 8x8-only).
* Every exported WORKS plan passed the simulator judge AND the redsim frame check (12/12 door
  block when open, 2 cycles); plans are tile phase by default since main de6ecbce
  (MECHSPACE_BETWEEN=1 for the old rule); MECHSPACE_HELD_FIRST=1 / MECHSPACE_Z1=1 for held-first
  and z=1-input plans (z1 + tile breaks the elevator in the simulator: use between for z1).

## Running (check with `ps -eo pid,etimes,args | grep mctas`)
* box4: `export` hft78 (held-first, tile; log /tmp/hft78.log).
* box3: `export` hfh78 (held-first of heldtop78, tile; log /tmp/hfh78.log).

## Next steps
1. hft78 / hfh78: push, tell satwire3 / t78.
2. Model the QC coupling of directly powered stacked pullers (satwire3): an ideal-plan input for
   a QC-coupled event powers the piston's cell directly and QC-fires the next piston below; the
   frame check demotes those plans now (56 of 3,607).
3. Max-held variants for satwire3 once its pulse/hold encoding wants them (every piston that can
   be held, held; held-first plans).
4. Why z=1 inputs + tile phase break the elevator in the simulator (pockets2 may need tile).
5. Unswept: 7x8 single-U all-retracted tail (12-13 pistons), ext variants of the 11-piston tail,
   8x8 offset 2,4, normal pistons (MECHSPACE_FREE_NORMAL), pistons in frame cells at rest
   (MECHSPACE_FRAME_SLOTS; needs a streaming enumerator), movable extras beyond frame cells.
