# pockets2 HANDOFF (the wiring service; written 2026-10-19 12:36Z, keep current)

## State
- Track best (BEST.txt): `finds/t7b-rwfast-43.txt`, 63b (7x9x1), 43 blocks, solid, FULL, rotation-safe, open 39 /
  close 27 (shrink's t7b-nb2 43 at 37/25 is faster now). Solid 8x8: **`finds/s27-trbl51.txt`, 64b, 51 blocks**
  (verify --extra-offsets 28 WORKS, archived `finds/archive/s27-trbl51-51b.txt`; earlier s27-bl52 52, s26-bl53 53).
- Role (vjeux's 12:21Z protocol): wiring service on the live board, Space memory namespace 2034923423883327,
  `minecraft-tas/board/` (README.md rules, roster.md, partials/*.md). Owners message this session directly;
  ping the coordinator (f1a42990-e9ee-4714-8593-b34b64ed175d) only for a verified door or a tool commit.

## Running (devvm49255, ~/agents/pockets/minecraft-tas)
- `runs/b51-tr-k5` (started 12:35Z, ~40 min): s27-trbl51's top-right, 8 cells incl. the wiring piston (3,5),
  k=5 x 24 states (pistons included), `--region-pistons --gaps`, plan `s26/b51-tr.ideal`. A FIND = a 50-block
  solid 8x8: `door verify <find> --extra-offsets 28 --archive`, copy to finds/, ping the coordinator.
  Check: `grep -c FIND designs/piston-door-3x3/tracks/pockets/runs/b51-tr-k5.log`.

## Exact next steps
1. b51-tr-k5's result (above). Other s27-trbl51 regions are minimal for their palettes: bottom-left 6
   (k=6 x 16), BR driver 3 (k=4 x 61), B's bulb line 4 (k=3 x 61), B line + its support 7 (k=6 x 16: none);
   shrink owns the top-left A part (`door fill`).
2. Answer owners' requests (partials/*.md). Walls found today: e78 (only K: its torch-off must come 1 gt
   later; pockets k=3 + ideal-rs --vary 83k combos: none), c070x (U + M coupled 3 gt apart), pn31 (P3 needs
   power 1 gt longer than the observer pulse that also feeds L2), t7b 42 (coupled 13-cell bottom).

## How to run a wiring job (recipe)
1. Copy the owner's design + plan into `tracks/pockets/svc/<owner>-<name>.txt/.ideal`, design line rewritten.
2. `door ideal-rs <plan>` must WORK (fix coupled defaults, e.g. c070x's m = u+3).
3. `door cosearch pockets <seed> --box=... --assist <plan> --keep-lever --no-hallway --k 3 --gaps
   --fill-palette all,sculk,calibrated,wool --threads 160 --out-dir runs/<name>`; then k=4/5 with a pruned
   `--fill-states` on the pocket that drives something; `--pocket-k`/`--pocket-fill` for per-pocket sizes.
4. Region re-wiring: `door cosearch plan-from <door> --region=... --close --out <plan>`; if the erased seed
   does not screen perfect, sweep the plan's timings with `door ideal-rs --sweep` (s27's top-right close
   pulse: plan-from said c22, only c0..16 works). `--region-pistons` erases retracted wiring pistons.
5. Small exhaustive grids: `door ideal-rs <lever-only plan> --vary='x,y=a|b|...' --ok-only` (inputs real).
6. A partial with fewer inputs ideal: re-run pockets on `pockets-partial-Nas.txt` with `--assist-cells`.

## Files
- NOTES.md section 8 (everything of pockets2), runs/ (logs + set/partial files), svc/ (owners' copies),
  s26/ (s26/s27 region plans), t7b/, cpt/ (compacted mech-synth plans), finds/ + finds/archive/.
- Never commit: `tracks/mechspace/mech/*` (untracked copies restored for the cpt plans), archive/properties-v1.tsv.
