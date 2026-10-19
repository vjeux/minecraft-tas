# pockets3 HANDOFF (the wiring service + owner of cosearch*.rs / fill.rs; updated 2026-10-19 15:47Z)

## State
- **THE FIRST 7x8: `finds/e78-56b-40.txt` = BEST.txt, 56b (7x8x1), 40 blocks, SCULK, FULL, rotation-safe, open 98 /
  close 24, verify --extra-offsets 28 WORKS** (archived on main: e78-56b-45-45b, the 45-block original; the 42 and 40
  were pinged). Chain: e78k's r1 (explore/q78 tracks/q78/e78k/r1.txt) + 3 cross-region changes (door fill --keep over
  14 bottom cells) = e78-56b-45 (105/111); `--keep --max-blocks` steps: 44 x3 edits -> 42 (98/24) -> 41 -> 40.
  Coordinator split (15:31Z): c77 keeps the STONE e78 line (e78st-56b-42, 51/15, solid attempts), pockets3 the WOOL.
- Role (vjeux's 12:21Z protocol): live board in Space memory 2034923423883327, `minecraft-tas/board/` (README.md,
  roster.md, partials/*.md). Owners message this session; ping the coordinator (f1a42990-e9ee-4714-8593-b34b64ed175d)
  only for a verified door or a tool commit. Door team now: e78k (e78 owner, fast clean variant), c77 (stone e78),
  c78l (c78z/L3a), shrink, pockets3. Ended: t78, ring56, c70, mechspace, satwire3 (wrapping up).
- Tool commits: ba5f30b8 (--keep), 7ddfc5a4 (in-plane guard) merged on main; d4eb7406 (--max-blocks pre-filter,
  --max-open/--max-close) sent to the coordinator (branch already merged with main d7c0d3f3 = CLI refactor: door fill's
  flags live in crates/door-redsim/src/cmd/fill.rs FillArgs).

## Running on devvm49255 (logs in tracks/pockets/runs/; all killable, nothing else of mine)
- `e78s-shrink39k4`: e78-56b-40, exactly 4 changed over 25 wiring cells, --max-blocks 39 (21G fillings pre-filtered,
  ~35 min from 15:33Z). A find = a 39-block 7x8: verify --extra-offsets 28, ping.
- `e78s-fast3r`: rotation-safe speed search on the 40 (<= 3 changed, --max-open 90); so far 41 blocks 79/54.
- `L3a-keep25`: c78l's L3a (TP + L ideal) cross-region --keep over 25 wiring cells, <= 3 changed.
- `t9e-tr6`: ring56's t9e top-right region at k=6 x 24 states (orphan partial t8): post the driven sets to
  partials/t8.md under Helpers (ring56 asked).
- `h8Q-k5`: combination phase (best partial 4 left; MR close c18 never drivable from pocket 4): results already on
  partials/hs23.md.

## Exact next steps
1. Shrink the wool e78 below 40: exactly-4 result; if none, k=4 with a pruned palette on the bottom 14 cells, or
   cross-region pairs with `door ideal-rs --drop 2` on each 41-variant.
2. Speed (rotation-safe): open 98 = attempt-4 convergence (e78k's trace); a fast variant needs B1 >= t21 in attempt 1
   and B2 landing A' at the gate's G3 start (e78k works on the clean variant from timing windows).
3. Apply the cross-region --keep recipe to every near-door (c78l L3a: running; others when owners ask).
4. mechspace z1-s78 batch: 6 of 10 seeds done (runs/msz1-k3.log: best partials 7-8 left), stopped when mechspace
   ended; remaining 0265 0937 0174 0755 (+1255) if anyone revives that line.

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
