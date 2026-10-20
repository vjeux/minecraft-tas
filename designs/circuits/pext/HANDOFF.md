# pext HANDOFF (study 6, piston extenders) — track pext, branch explore/pext

## State (all archived with `mctas circuit archive`, robust 16/16 in the game)
| N | slime/honey (smallest first) | slime-free | one button |
|---|---|---|---|
| 1 | | n1-sticky (2) | |
| 2 | rider7 flat/3D (7), rider 8, flat-cascade11 | sand6 (6, gravity), sf7-1/2/3 (7), sf8-flat (8) | sand6, rider7, sf7 buttons; flat-cascade11-button |
| 3 | flat-cascade19 (19, 27/27 gt), cascade22, rider24 | sf38 (38), sf43 | flat-cascade19-obutton (oak) |
| 4 | flat-cascade26, cascade30/33 | sf60 (60, 49/47), sf-sweep96 | |
| 5 | flat-cascade33, cascade38/46 | sf-sweep136 | |
| 6 | flat-cascade39, cascade47 | sf-sweep214 | |
| 7 | flat-cascade48, cascade58 | sf-sweep276 | |
| 8..12 | flat-cascade54/61/69/76/82, cascade66 | | |
| any | family cards pext-cascade-r2/r0/r1 (12N-7-2floor((N-1)/3)) and pext-cascade-flat-r2/r0/r1 (10N-5-floor((N-1)/3)); extend 14N-6, retract 16N-8 | | |

## Tools (crates/circuit-pext, `mctas circuit pext ...`)
* `sim`, `trace`, `verify` (real game, 4 positions x 4 rotations, the judge's rules incl. "once
  extended the payload stays"), `grow` (exhaustive k-block additions), `minimize` (removals,
  substitution+removal, place-in-empty+2 removals, `--plane-z`; its quick screen scales with the
  design): run on generator output it gave most of the bests.
* Generators: `cascade [--flat] [--family/--group]` (any N), `stack --cells` (slime-free, any N up
  to 7 so far), `spine` (observer chain along a path). Schedules: `trains` (pulse-toggle trains,
  `--bfs`, `--interleave`, `--cells`). `drop` removes archived entries a family card covers.
  Exploration-only `ideal:` header (scripted writes per phase).
* Shared-judge fix for review: a607f735 (circuit archive judged family members against the base's
  kind; piston-extender N members all read the base N).

## Mechanisms and lessons: designs/circuits/pext/NOTES.md

## Next ideas
* Slime-free N >= 8: a better spine router (more room per target, or two spines); minimize the
  slime-free N=5..7 (slow: long phases).
* Smaller N=2: grow found nothing <= 5 blocks on the sand base (k <= 3, near 2).
* Flat slime-free N >= 3 (the stack router in one plane: top/bottom/QC slots only).
