# Smaller than 56: does ANY flat 3x3 mechanism fit? (c70b, QUESTION 1, from 2026-10-19 20:03Z)

Coordinator / vjeux: same rules (3x3, flat, solid, slime-free, FULL frame 12/12, nothing in front
or behind), volume < 56: 7x7 (49), 6x8 / 8x6 (48), 6x9 / 9x6 (54), 7x6 / 6x7 (42), 5xN / Nx5. My
part: per box, run the mechanism generators + macro solvers (defaults and every flag) and report
candidates, clean schedules, or an honest NO naming what the models leave out; then rank
feedable variants (wiring is the bottleneck).

## Answer so far (mechanism with ideal inputs, macro model; rotating a mechanism by 90 degrees
keeps it a mechanism, only the wiring changes: quasi-connectivity is "above"-specific)
| box | volume | mechanism? | how |
|---|---|---|---|
| 7x7 | 49 | YES | c77 w1 (explore/c77 2307edd8 tracks/c77/seven/w1.txt), 13 sticky: column wiggle + ring pistons spat into the bottom corners |
| 8x6 / 6x8 | 48 | YES | w86 a3r (explore/w86 26c190804 tracks/w86/wip/a3r.txt), 14 sticky; 6x8 = a3r turned 90 degrees |
| 7x6 / 6x7 | 42 | YES | w86 a3s (explore/w86 ba757d0ea tracks/w86/wip/a3s.txt), 13 sticky; 6x7 = a3s turned |
| 6x9 / 9x6 | 54 | YES | c78l m69a (explore/s79 tracks/s79/c69/m69a.txt), 12 sticky; 9x6 = m69a turned |
| 6x6, 7x5, 5xN | 36-35 | open | next |

Every one of them is OUTSIDE the default generator (families v3, P <= 10, two 1): `door audit
--member smaller/ideal-mechs.txt` (piece lists of ideal-input designs):

| mechanism | g1 | g2 | g3 | g4 | g5 | g6 | g7 | why (widest generator) |
|---|---|---|---|---|---|---|---|---|
| 7x7 w1 | out | out | out | out | out | IN | IN | |
| 8x6 a3r | out | out | out | out | out | out | out | center exits U: BM has no exit among the pieces (BM leaves through the center column: a 3-move path) |
| 7x6 a3s | out | out | out | out | out | out | out | same |
| 6x9 m69a | out | out | out | out | out | out | out | center exits D: every block has an option (the exact union is still not produced: corner pistons slide blocks already in the ring) |

(g6 = every flag P <= 16 two 7 carry 2 vmid any ext-apart; g7 = g6 + `--ring-escape`.) w1 was in
g6's space all along, but nobody can enumerate g6 on 7x7, and these mechanisms need 1-5M macro
states each (w1 5M / 35 s / 3.4 GB; a3s, m69a 3M), 5-25x the catalog's 200k cap.

## The models (what a generator NO covers)
- families (`door families`): a candidate = the union of one provider per unit move of every door
  block's exit path (the center straight out in exactly 2 moves, each other block 1 move, <= `two`
  of them 2 moves); providers = a sticky puller 2 ahead (retracted, or EXTENDED at rest:
  `--ext-max 3`, each next to an earlier one unless `--ext-apart`), a pusher 1-2 behind, a piston
  already placed reused from its rest cell or lifted 1 cell by a piston right behind it (elevator),
  `--carry N` general carrying. Pieces: sticky / normal pistons, stone, obsidian. P <= 10 by
  default. Solved exactly in the macro model (one settled piston action at a time, L = level
  toggle, P = 2 gt pulse incl. spit): open = 3x3 empty and SEAMLESS ring, close = back to the exact
  rest state; the solver plays any order (wiggles included) but only with the proposed pieces.
- Outside the generator (all four found mechanisms use some): paths of 3+ moves (BM through the
  center column), blocks moved again once in the ring (corner pistons sliding ring blocks), push
  chains with 2+ blocks between pusher and mover, ring-resting pistons that must leave the ring
  (now `--ring-escape`), pistons carried INTO the opening and back (a3r's W), corners as waypoints.
  Outside every model: slime / honey, gravity blocks, entities, 0-ticks beyond the macro spit.

## Runs (box1)
| run | result |
|---|---|
| families defaults, 6x8 (x -1..4 or mirrored, every offset) | 0 of 65,664 open+close; 18,874 unresolved at the 200k cap |
| families defaults, 8x6 | 0 of 65,768; 14,421 unresolved |
| families defaults, 7x7 | running (746,836 candidates; 0 of the first 460k) |
| families --ring-escape --ring-escape-only --feedable P<=12, 7x7 | 223,260 candidates; stopped at 20k (cap 400k: the w1-like ones need ~5M) |
| `door mechmut` from w1 / a3r / a3s / m69a (cap 5M, beam 4, 2 rounds) | running: smaller/mm/ |

## Tools added (explore/c70b)
- families `--ring-escape` (+ `--ring-escape-max`, `--ring-escape-only`): pistons resting in ring
  cells get escape carriers (carried 1-2 cells along the ring into a corner or out of the frame;
  carriers resting in the ring escape too). `--feedable`: structural filter, every piston has a
  free side / QC feed cell at rest (a ring cell counts when a free off-ring cell can power a
  static stone there).
- `door audit --member FILE`: generator membership (g1..g7) of piece lists; g7 = every flag +
  ring escapes.
- `door mechmut`: local search from known mechanisms (drop / shift / turn a piston, toggle sticky
  / extended), each neighbour solved exactly (cap) and kept with a FULL frame, ranked by (pistons
  no free cell can feed, pistons, wiring class, events); `--ideal-out` exports the best as
  ideal-input designs + plans.
