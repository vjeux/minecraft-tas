# pext HANDOFF (study 6, piston extenders) — track pext, branch explore/pext

## State (all archived with `mctas circuit archive`, robust 16/16 in the game)
| N | design | blocks | extend/retract gt | family |
|---|---|---|---|---|
| 1 | pext-n1-sticky | 2 | 3/3 | sticky piston + lever |
| 2 | pext-n2-sand6 (+ -button) | 6 | 7/12 | GRAVITY: sand payload, travel up, flat, slime-free |
| 2 | pext-n2-rider7-flat / -3d (+ stone/oak button) | 7 | 9/9 | slime rider (A held pushes a frame, B rides it) |
| 2 | pext-n2-sf7-1/2/3 (+ -button) | 7 | 7/12 | slime-free (pulsed A, input-held B, carried observer) |
| 2 | pext-n2-sf8-flat | 8 | 7/12 | slime-free, flat |
| 3 | pext-n3-cascade22 (+ -v1 26) | 22 | 27/31 | CASCADE |
| 3 | pext-n3-rider24 (+ -v1 31) | 24 | 25/25 | one-frame rider (inner train C B C) |
| 4 | pext-n4-cascade33 (+ -v1 40) | 33 | 41/53 | cascade |
| 5 | pext-n5-cascade46 (+ -v1 52) | 46 | 55/59 | cascade |
| 6/7/8 | pext-n6-cascade47, n7-cascade58, n8-cascade66 | 47/58/66 | 67/85, 79/99, 87/109 | cascade, minimized |
| any | pext-cascade-r2/r0/r1 (group pext-cascade) | 12N-7-2floor((N-1)/3) | 14N-6 / 16N-8 | EXTENDABLE cascade family cards, generator below |

## Tools (crates/circuit-pext, `mctas circuit pext ...`)
* `sim`, `trace`, `verify` (real game, 4 positions x 4 rotations), `grow` (exhaustive k-block
  additions), `minimize` (removals, substitution+removal, place-in-empty+2 removals: it found most
  of the small designs and shrinks the generated cascades by ~25%).
* `cascade --n N` (or `--base B --step 3 --n k --family ID --group G` for family bases with the
  `extend` headers): the generator; `trains` (pulse-toggle trains, static-cell delivery, `--bfs`);
  `drop <files>` (remove archived entries a family card covers).
* Exploration-only `ideal:` header (scripted writes per phase) to validate mechanisms before wiring.
* Shared-judge fix for review: a607f735 (circuit archive judged family members against the base's
  kind; piston-extender N / counter N members all read the base N).

## Mechanisms and lessons: designs/circuits/pext/NOTES.md
Pulse-toggle trains (`B A B`, `C B C A B C`), frames cannot nest, the one-frame rider's push-limit
ceiling (N=4), gravity's QC limit (N=2 only), the cascade (a moved unit fires itself; only round
starts are addressed; pulsers diagonal to slime cells; a pulse may serve different rounds on the
two edges; rounds may overlap: 6 gt on-edge, 4 gt off-edge spacing).

## Next ideas
* Slime-free N=3 (train AACBACBC is fully deliverable by static cells: see `trains --bfs`).
* One-button cascade (the button's release = the off edge: should work as is, untested).
* Smaller cascades: a generator using repeaters for the spine delays (the minimizer's trick),
  shared trigger columns; flat cascades (spine and pulsers in one plane?).
* N=3 below 22: minimize from other seeds; a sand cascade is impossible (QC of the top piston).
