# c77 handoff (2026-10-19 20:45Z)

## 7x7 (49b) campaign: w1 = first sub-56 mechanism (ideal inputs, not wired)
- seven/w1.txt + w1.ideal / w1v.ideal: `door ideal-rs --between-ticks` WORKS (16 pos x 3 cycles), `door ideal
  --between-ticks` (real game) WORKS. 13 sticky pistons, FULL by construction. Board: partials/7x7-w1.md (schedule,
  windows, obstruction). NOTES.md "7x7" section has the mechanism and why the wiggle piston P (1,-1) is boxed in.
- c70b (9182fde5) is adding w1's two move types (ring-resting pistons escaping to corners, the center-column wiggle) to
  the families generator and will rank 7x7 variants by feedability: wire the best one (pockets3 = wiring service).
- Selftest anchor for w1 requested from the coordinator (selftest belongs to boxproof).

## Earlier results (verified at 28): e78st-56b-38-1s (BEST, sculk 7x8), community-56b-47-elig (eligible solid 7x8),
  c074-64b-42 (sculk 8x8). Details in NOTES.md.

## Running processes: none.
