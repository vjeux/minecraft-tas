# HANDOFF: track c70b, 2026-10-20 (newest first)

## 7. macrosat -> mechmut bridge (coordinator 11:15Z)
- `door mechmut <dump or folder>` reads clockinfra's `door macrosat` dumps (explore/clockinfra
  tracks/macrosat/sat/; clk-misc's strict SATs will land in explore/d67 tracks/d67/macrosat/):
  rest layout = seed, macrosat's actions = first schedule (QC off for macrosat seeds), the dump's
  own export pair judged at load (redsim, d67's `feeds --strict`, w86's extent; CANDIDATE line),
  then `--repair --add --validate --rank-unfed`. Survivors (island start 0 + strict clean) go to
  w86 (checker) -> pockets3 / clk-misc (wiring).
- families' model holds 32 pistons; QC is per mechanism (families::with_qc_off); mechsynth's
  plans feed a piston above a retracted one through its above cell (+ glass) for qc-off mechs.
- Results: the 18 old non-strict SATs give nothing (smaller/msat/r0, r2, q2: m86-a WORKS but
  fails strict and leaves 8x6; the 7x7s break in redsim). phys14 (`--physics --max-pistons 14`,
  h6/h8): 17 of 28 instances UNSAT, none SAT. phys18 (`--max-pistons 18`, h8, 8 h bound): 6 of
  14 UNSAT (two placements each of 6x8, 8x6, 7x7), 8 unfinished, none SAT (smaller/NOTES.md).
  All of those island-start UNSATs are PROVISIONAL since clockinfra's pulse fix (36b146530,
  2026-10-21 07:47Z); its re-runs (7x7 + 48 at h8 with --physics --observers, the 7x7 h6 plain
  feed, u2-T at h16) report to c70b.
  If new dumps land (clockinfra's `--feed --lever --physics --observers --max-pistons 18`, h8,
  6 7x7 + 8 48 boxes, running under systemd on devvm53196 since 2026-10-21 00:08Z, no time
  bound: explore/clockinfra tracks/macrosat/sat/obs18/, with a line to c70b and w86), run
  `door mechmut <folder> --cap 2000000 --bfs-cap 1000000 --threads 8 --rounds 2 --beam 10
  --repair --validate --rank-unfed --out msat/<run>.tsv --ideal-out msat/<run>` and read the
  CANDIDATE lines (a dump's own export: redsim + strict feeds + extent all pass) and the TSV
  (strict_fail 0 and island_start 0 first). Dumps families cannot hold (observers) are judged by
  their export only.

## 6. Wirable smaller doors (coordinator 2026-10-20 04:00Z: NEW families built for wirability)
- Bars on every solve (families + mechmut): `--max-actions K` (pulses / level toggles per piston
  per edge), `--extra-actions B` (actions beyond one per piston, per edge), `--pareto N` (64 makes
  the constrained search exact on everything tried), `--no-inside`; filters families `--fed`,
  mechmut `--max-unfed N`. Unfed count = families::unfed_cells (free cells over the cycle, c77's
  static-ring-stone feed, lever outside the box on a never-moved piston's back face, valid only
  when every piston it powers, beside it or by QC, is lever-held: d67/w86/pockets3 game checks).
- mechmut `--add` (+ `--add-redstone`): completion search, one piece per round, unsolved variants
  steer the beam by growth.
- Results (smaller/NOTES.md "Wirability bars"): families 7x7 ring-escape P12 + P13 at K=1:
  0 of 2.39M; P12 at K=2 + 1 extra: 0 of 263,924; w1 local search under K=2 + no-inside:
  109 variants, P (1,-1) + SU(1,-2) always unfed (w86's argument: the middle-frame piston that
  fires from the frame cannot be fed). The hole I reported: pistons carried INTO the door square
  (a3s/mm01 class) can be fed (d67), but every such mechanism so far needs 3-7 actions per piston.
- New 7x7: smaller/mm/t1-r22/mm01 (12 sticky, from w86's t1 with (-2,2) reserved): WORKS in
  redsim + game; unfed (1,-1), Q (-1,0), (0,4) (its lever at (0,5) also arms (-1,4)/(1,4));
  up to 5 actions per piston per edge. Then u0-7x7 (every receiver fed, 4 actions) and
  **u2-7x7** (bars/u2-7x7: every bar met: no inside firing, <= 2 actions per piston per edge,
  every receiver fed; WORKS redsim + game). Wiring is blocked by island starts (clk-misc,
  pockets3: free regions are electrically isolated; the right-side islands cannot start from a
  top lever). pext's `door interleave --islands` found an island-respecting train for u2; satwire's
  wall then is the bottom-left SU(-1,-2) (4-6 pulses per edge into the 2-cell island). Latest batch
  for pext: bars/u-max2, mm/u-island (island-start-ranked search).
- Tools added late: d67's `door feeds` (merged; mechmut ranks by none/conflicts/weak), families'
  pocket load and ISLAND START (macro, one schedule), mechmut `--connect-cap`.
- Running at handoff: nothing (8x6 a3r no-inside completion search killed: no growth).

## 5. Provenance (vjeux 2026-10-20: the site publishes only our own doors)
- `mctas door provenance` -> designs/piston-door-3x3/archive/provenance.tsv (1175 ours, 519
  community-derived on main's archive; pockets runs inherit their seed's origin).

## 4. Smaller than 56, mechanism existence (QUESTION 1): smaller/NOTES.md
- 7x7 YES (w1, t1, t1-r22), 8x6 YES (a3r), 6x8 YES (w86 s1), 7x6 YES (a3s), 6x7 YES (a3s turned +
  repaired, real game), 6x9 YES (m69a/T/V), 9x6 YES, 6x6 none within families two <= 3, 10x5 /
  5x10 / 7x5 / 5x7 NO in the strip model.

## 3. Generator: extended-at-rest pullers by default (merged)
## 2. The community 7x8 reproduced (community-derived now, not published)
## 1. Parked: t7b in 8 rows (partials/t7b8.md), c070x (partials/c070x.md)
