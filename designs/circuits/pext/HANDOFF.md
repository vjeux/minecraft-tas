# pext HANDOFF (study 6, piston extenders) — track pext, branch explore/pext

## State (all archived with `mctas circuit archive`, robust 16/16 in the game)
| N | best designs (blocks, extend/retract gt) | families |
|---|---|---|
| 1 | n1-sticky (2, 3/3) | sticky piston + lever |
| 2 | sand6 (6, 7/12; + button) · rider7 flat/3D (7, 9/9; + stone/oak button) · sf7-1/2/3 (7, 7/12; + button) · sf8-flat (8) · rider 8 · flat-cascade11 | gravity, slime rider, slime-free (flat too), cascade |
| 3 | flat-cascade19 (19, 27/27) · cascade22 · rider24 · sf43 (43, 66/64, SLIME-FREE) | flat cascade, cascade, rider, slime-free stack |
| 4 | flat-cascade26 (26, 43/55) · cascade30 · cascade33 | |
| 5 | flat-cascade33 (33, 50/66) · cascade38 · cascade46 | |
| 6..8 | flat-cascade39 / 48 / 54 · cascade47 / 58 / 66 | |
| any | family cards pext-cascade-r2/r0/r1 (12N-7-2floor((N-1)/3) blocks) and pext-cascade-flat-r2/r0/r1 (10N-5-floor((N-1)/3)), extend 14N-6, retract 16N-8 | generators `mctas circuit pext cascade [--flat] --base B --step 3 --n {n}` |

## Tools (crates/circuit-pext, `mctas circuit pext ...`)
* `sim`, `trace`, `verify` (real game, 4 positions x 4 rotations; the judge's rules incl. "once
  extended the payload stays"), `grow` (exhaustive k-block additions), `minimize` (removals,
  substitution+removal, place-in-empty+2 removals; `--plane-z` keeps flat designs flat): the
  minimizer run on generator output gave most of the bests.
* `cascade` (generator, `--flat`, `--family/--group` for family bases), `spine` (observer chain
  along a path), `trains` (pulse-toggle trains, static delivery, `--bfs`), `drop` (remove archived
  entries a family card covers). Exploration-only `ideal:` header (scripted writes per phase).
* Shared-judge fix for review: a607f735 (circuit archive judged family members against the base's
  kind; piston-extender N members all read the base N).

## Mechanisms and lessons: designs/circuits/pext/NOTES.md

## Next ideas
* Slime-free N=4: no fully static-deliverable train (BFS); needs per-edge pulse reuse (as the
  cascade does) or edge-selective sources.
* Smaller N=2: grow found nothing <= 5 blocks on the sand base (k <= 3, near 2); k = 4 not run.
* A one-button cascade fails (the 30 gt oak press ends mid-extension for N=3); would need a pulse
  extender.
* Flat cascades for N >= 9 minimized (only the generated sizes are archived as family members).
