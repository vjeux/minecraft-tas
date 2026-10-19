# pockets2 HANDOFF (the wiring service; updated 2026-10-19 13:41Z)

## State
- Track best (BEST.txt): `finds/t7b-rwfast-43.txt`, 63b (7x9x1), 43 blocks, solid, FULL, rotation-safe, open 39 /
  close 27 (shrink has since taken t7b to 40). 8x8: `finds/s27-bls49.txt`, SCULK 64b, 49 blocks (archived on
  main; c77's c074-64b-42 leads sculk 8x8, so the 8x8 target is the SOLID 50 s27-tlp50) and
  **`finds/s27-trbl51.txt`, solid 64b, 51** (shrink -> solid 50 with pistons);
  all verified `--extra-offsets 28`, archived in `finds/archive/`.
- Role (vjeux's 12:21Z protocol): wiring service on the live board, Space memory namespace 2034923423883327,
  `minecraft-tas/board/` (README.md rules, roster.md, partials/*.md). Owners message this session directly;
  ping the coordinator (f1a42990-e9ee-4714-8593-b34b64ed175d) only for a verified door or a tool commit.
  Coordinator priority (13:42Z): the 7x8 seeds (e78k's K, c77's e78 stone variant B', c78l's c78z).
- Tool commits: f7e7023b (tile-phase assists) merged to main as cf9e8bc5; 1e14c397 (`--region-pistons`)
  waiting.
- Nothing running on devvm49255.

## Exact next steps
1. s27-bls49 (sculk 49) / s27-tlp50 (solid 50): shrink owns the top-left A part and the left column (lever
   support + dusts, with sculk); measured minimal with pistons + sculk: BR driver 3, B's bulb line 4,
   top-right 6, solid bottom-left 6 (sculk bottom-left 5 = the 49). A solid 49 needs another region.
2. c78l (c78z / lb5, 7x8): only P2 (U open pulse >= 4 gt at 18..28) + M/s left; the 4 bottom-right cells
   (2,-2) (3,-2) (4,-2) (3,-1) have NO P2 driver (exhaustive k=4 x 61, `runs/t2a-bot-k4`); plan for that
   region = `svc/c78l-t2a-bot.ideal` (plan-from missed U's close re-hold; c re-hold 8..20 works).
3. mechspace's z1-s78 top 10 (`svc/ms/z1-s78-*-oc.*`, between-ticks plans): blocked, the designs are not flat
   (the held stack's z=1 rest blocks are in the design file): ask mechspace for flat designs (rest blocks
   only in the plan) + a `port lever:` line, then batch with `--between-ticks --gaps --k 3 --fill-palette
   all,sculk,calibrated,wool`.
4. Walls (board notes): e78 K (torch-off 1 gt late), c070x U+M (coupled 3 gt), pn31 P3 (power 1 gt longer
   than the observer pulse feeding L2).

## How to run a wiring job (recipe)
1. Copy the owner's design + plan into `tracks/pockets/svc/<owner>-<name>.txt/.ideal`, design line rewritten.
2. `door ideal-rs <plan>` must WORK (tile phase is the default now; `--between-ticks` for old plans).
3. `door cosearch pockets <seed> --box=... --assist <plan> --keep-lever --no-hallway --k 3 --gaps
   --fill-palette all,sculk,calibrated,wool --threads 160 --out-dir runs/<name>` (the "all" palette has
   pistons: a piston popping its head for an observer is a 1-block relay, today's 51/50/42/40 used it);
   then k=4/5 with a pruned `--fill-states` (include sticky/normal pistons) on the pocket that drives
   something; `--pocket-k`/`--pocket-fill` for per-pocket sizes.
4. Region re-wiring: `door cosearch plan-from <door> --region=... --close --out <plan>`; if the erased seed
   does not screen perfect, sweep the plan's timings with `door ideal-rs --sweep` or add a missing level edge
   (s27 top-right close pulse c22 -> c15; c78l's U close re-hold). `--region-pistons` erases retracted wiring
   pistons; then drop plan-from's events at the erased piston (its z=1 blocks power fillings: fake covers).
5. Small exhaustive grids: `door ideal-rs <lever-only plan> --vary='x,y=a|b|...' --ok-only` (inputs real).
6. A partial with fewer inputs ideal: re-run pockets on `pockets-partial-Nas.txt` with `--assist-cells`.

## Files
- NOTES.md section 8 (everything of pockets2), runs/ (logs + set/partial files), svc/ (owners' copies),
  s26/ (s26/s27 region plans), t7b/, cpt/ (compacted mech-synth plans), finds/ + finds/archive/.
- Never commit: `tracks/mechspace/mech/*` (untracked copies restored for the cpt plans), archive/properties-v1.tsv.
