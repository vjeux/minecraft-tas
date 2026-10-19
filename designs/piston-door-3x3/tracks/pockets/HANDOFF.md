# pockets3 HANDOFF (the wiring service + owner of cosearch*.rs / fill.rs; updated 2026-10-19 16:57Z)

## State
- 7x8 = 56 doors now exist in all three lines (all verify --extra-offsets 28, 32 positions + rotations):
  - SOLID (FULL frame 12/12 required by the coordinator, 17:06Z): e78k's h2 45 -> **`finds/e78s-56b-43.txt` 43, 29/15**
    (mine, = BEST.txt); e78k's FULL line h5 41 (36/19) / h6 42 (29/15) leads (explore/q78 tracks/q78/e78s/).
  - SCULK stone (c77's line): e78st-56b-38-1s (one sensor) -> **`finds/e78st-56b-37.txt` 37 blocks, 48/15** (mine).
  - SCULK wool (mine): **`finds/e78-56b-40.txt` = BEST.txt, 40 blocks, 98/24** (archived on main with 45/42 too).
  All found/shrunk with `door fill --keep` (+ `--max-blocks N` pre-filter). The wool line is locally minimal (no 39
  within 4 changes over all 25 wiring cells); the sensors' re-arm cycles are its retry timing (partials/e78.md).
- Role (vjeux's 12:21Z protocol): live board in Space memory 2034923423883327, `minecraft-tas/board/` (README.md,
  roster.md, partials/*.md). Owners message this session; ping the coordinator (f1a42990-e9ee-4714-8593-b34b64ed175d)
  only for a verified door or a tool commit. Door team now: e78k (solid e78), c77 (stone e78, solid attempts), c78l
  (c78z/c78FR satwire), c70b (8x7 c070x), shrink, pockets3. Ended: t78, ring56, c70, mechspace, satwire3.
- Tool commits: ba5f30b8 (--keep), 7ddfc5a4 (in-plane guard) merged on main; d4eb7406 (--max-blocks pre-filter,
  --max-open/--max-close) and 28df017e (summary table) sent (door fill's flags live in
  crates/door-redsim/src/cmd/fill.rs FillArgs since main's CLI refactor d7c0d3f3).

## Running on devvm49255 (logs in tracks/pockets/runs/; all killable, nothing else of mine)
- `h5-all5`: e78k's h5 (41 FULL), exactly 5 changed over all 25 wiring cells, 13 states, --max-blocks 40 (20G).
- `h6-bot5`: e78k's h6 (42 FULL, 29/15), exactly 5 over the 15 bottom cells, 20 states, --max-blocks 41.
  A find = a FULL solid 7x8 below the leader: door verify --extra-offsets 28, ping the coordinator, tell e78k.

## Exact next steps
1. Shrink the solid 44 (k=4 result; then k=5 on e78k's likely-removable cells with a ~15-state palette and the
   pre-filter: ~4.6G enumerations ~10 min) and the stone 37 the same way.
2. Answer designers' screen requests first (e78k solid, c77 stone S2, c70b 8x7 m2/m2x, c78l c78FR).
3. Cross-region --keep (<= 3) is exhausted on: c78l L3a, c70b m2/m2w (+ m2 5-cell k <= 5), f131 pn31 (solid, 136M),
   e78k s11 (and subsets at k=4). Not yet: c78z p5-0 (stopped), satwire m79q-sat-topright (plans/p79q-sat-L.ideal).
4. Orphans: t9e top-right k=6 (partials/t8.md), mechspace z1-s78 seeds 0265 0937 0174 0755 (+1255).

## How to run a wiring job (recipe)
1. Copy the owner's design + plan into `tracks/pockets/svc/<owner>-<name>.txt/.ideal` (design line rewritten), or
   `git restore --source=origin/explore/<branch> --worktree -- <paths>` (untracked; never commit others' tracks).
2. `door ideal-rs <plan>` must WORK (tile phase default; `--between-ticks` for old plans).
3. REAL-door screens first: `door fill <design> --cells=... --keep --k 3 --gaps --allow-pistons --palette '<~37
   states>' --any-rotation --extra 0` (k = changed cells; add `--max-blocks N` to shrink, `--max-open N` for speed;
   drop `--any-rotation` when ranking finds). Use CROSS-region cell sets (all wiring cells): e78's door needed 3
   changes far apart.
4. Plan-driven screens: `door cosearch pockets <seed> --box=... --assist <plan> --keep-lever --no-hallway --k 3 --gaps
   --fill-palette all,sculk,calibrated,wool` (+ `--pin-assist`, `--only-pockets`, `--pocket-k`, `--region-cells`,
   `--region-pistons`). An in-plane plan cell blocks fillings next to it while assisted (the guard).
5. Exhaustive timing maps: `door ideal-rs <plan with lets> --sweep a=..  --sweep b=.. --ok-only` (hold/pulse shapes).

## Files
- NOTES.md sections 8 (pockets2) and 9 (pockets3), finds/ + finds/archive/, svc/ (owners' copies + shape plans),
  runs/ (logs + finds of every screen).
- Never commit: tracks/mechspace/mech/*, tracks/s79/*, tracks/t78/*, tracks/ring56/* (restored untracked copies),
  archive/properties-v1.tsv (a local cache `door properties` rewrites: `git checkout --` it before merging).
- Careful: `pkill -f <pattern>` / `pgrep -f` also match the shell running them (killed my own shell twice); anchor:
  `pgrep -af '^./target/release/mctas door fill .*<out-dir>'`, then `kill <pid>`.
