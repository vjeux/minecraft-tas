# c77 handoff (2026-10-19 23:25Z)

## 7x7 (49b): first sub-56 MECHANISM, not wired
- seven/w1.txt + w1.ideal (+ w1v.ideal windows): 13 sticky pistons, FULL 12/12, `door ideal-rs --between-ticks` WORKS
  (16 pos x 3 cycles) and real-game `door ideal --between-ticks` WORKS. Selftest anchor ideal:7x7-w1 is on main.
- Mechanism: P (ring cell (1,-1)) pushes BM/C/TM up and pulls BM back (the center emptied by a pull); ring pistons
  P / Q escape 2 cells sideways into the bottom corners (Q push-spits P, margin pullers pull them in); close reverses.
- Variants: w2 (12 pistons, observer helper (4,0)); c70b's mm01 (11 pistons, seven/mm01-11.txt + .ideal).
- Wiring wall: the center-pulling piston P has no free neighbour at its firing ticks in this whole family (argument in
  NOTES.md; pockets3 k=4 solid+sculk screens: P never driven). fullmech --groups: w1 needs only 5 signals.
- Next: c70b's mechmut with a timing-aware feed metric (neighbours AT the firing ticks), then wire the best variant.
  Board: partials/7x7-w1.md.

## Records / doors (verified at 28)
- e78st-56b-38-1s (BEST: sculk 7x8, one sensor; -37 archived by pockets3), community-56b-47-elig (eligible solid 7x8),
  c074-64b-42 (sculk 8x8). Details in NOTES.md.

## Running processes: none.
