# HANDOFF: track c70b, 2026-10-19 (newest first)

## 5. Provenance (vjeux 2026-10-20: the site publishes only our own doors)
- `mctas door provenance` -> designs/piston-door-3x3/archive/provenance.tsv (name, origin = ours |
  community-derived, reason). Rules: (a) name / `archived from` path / a pockets run's seed names a
  root or a community lineage (c79, community, c88, m79s, m10top, s79, c78z, s26/s27, closed-b,
  c77's comm-/cst-/c47- runs); (b) mechanism (audit's redsim cycle, mirrors merged) within 2 pieces
  of a root's, or the same door-block paths; (c) lineage to a fixpoint (a run's outputs inherit an
  archived seed's origin; mechanisms within 2 pieces of a community-derived design).
- Main's archive (1694): 1175 ours, 519 community-derived. The 257 pockets-63b-43-* are ours: their
  runs (rf-lw-k4, t7b-lw-k3, t7b-rw-k3) are seeded by t7b (e88's record from flat-seam e89b, which
  predates the c79 reproduction); "63b" = the 7x9 volume.

## 4. Smaller than 56 (QUESTION 1: does any mechanism exist per box): smaller/NOTES.md
- Per box (ideal inputs): 7x7 YES (c77 w1; mechmut mm01 11 pistons), 8x6 YES (w86 a3r), 6x8 OPEN,
  7x6 YES (w86 a3s; a3s-11), **6x7 YES** (a3s turned + repaired: 9 variants WORK in redsim, 4 in
  the real game; not wirable as found: pistons fire inside the door square), 6x9 YES (c78l m69a/T/V),
  9x6 YES (m69a turned, fullmech plan WORKS; m69g-12), 6x6 no within families two <= 3, 10x5 / 5x10 /
  7x5 / 5x7 NO in the strip model (exact re-run).
- Tools: families --ring-escape / --feedable; door audit --member + g7; `door mechmut` (local search
  from known mechanisms: --validate redsim replay, --keep / --reserve / --repair / --outside-only /
  --lever-held, ranked by redsim, inside firings, door connect, never-fed, weak feeds, unfed,
  pistons, events; exports FULL designs + ideal plans); fullmech --check --export / --keep-stone;
  families' Wire.weak.
- Running at handoff time: 6x7 search from the inside = 1 variants (smaller/mm/6x7-in2*), aiming at 0
  piston actions inside the door square.
- Wiring status: everything with pockets3 (the wiring service); satwire cores so far: 6x7 mm01
  (1,-2), mm02/03 (3,-1), mm05 (2,2); 9x6 m69g-12 W' (3,-1); m69T variants need lever-faithful plans
  (the macro model refuses a held piston that extends late).

## 3. Generator: extended-at-rest pullers by default (merged)
- families / mechspace / fullmech: `--ext-max 3`, adjacent (`--ext-dist 1`); docs/audit.md section 2.

## 2. The community 7x8 (SacredRedstone) reproduced (community/NOTES.md); community-derived now
- `community-7x8.txt` WORKS at 32 positions; the site no longer publishes it (provenance).

## 1. Parked: t7b in 8 rows (partials/t7b8.md), c070x (partials/c070x.md)

## Running processes
- smaller/mm/6x7-in2 (door mechmut, 10 threads) if still running; nothing else.
