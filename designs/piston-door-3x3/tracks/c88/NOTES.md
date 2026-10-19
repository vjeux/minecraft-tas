# Track c88: the community 7x9 (c79) mechanism in an 8x8 box (x -3..4, y -2..5)

## 1. Step 1: c79 in 8x8 with an ideal bottom (WORKS)
`s1.txt` + `s1.ideal`: c79's top (lever, level-held MR/TR, elevator A/B, top-left B-watcher dust) and BR's
puller unchanged; row -3 and the bottom-left wiring removed; U (1,-2), K (-1,-1), L0 (-2,0) ideal at c79's
own ticks. One fix: without c79's normal piston (3,-2) nothing UPDATES BR's puller (it is only QC-powered
through (3,-1)), so (3,-1) is now dust (on S (3,-2)): it powers and updates the puller.
- `door ideal-rs`: WORKS; `door ideal`: WORKS 4 positions and 32 positions (`--extra-offsets 28`), open 14 /
  close 14; `door sheet --fill`: frame 12/12.

## 2. Why a top-left-woken bottom cannot be a fixed pulse train (redsim sweeps)
- Windows (s1, others fixed): open K set 2..8, U >= K+2, L0 any; close U1 0..6, K 7..9, U2 9..11 or >= 14
  (U2 >= 14 also works: A's t13 push spits TM into (1,1) and U2 then pushes BM, C, TM up together).
- LATE bottoms work (s2.ideal, s3.ideal: everything after t14-t28) but only ASYMMETRIC by edge: open K then
  U; close U1, K, U2.
- `s1-sym.ideal` (the same U, K, U pulses on both edges), full grid a,b 0..34, c 0..40: every working
  schedule has one U pulse at set tick 2..5, i.e. inside A's hold (A's head blocks U on open). A fixed train
  needs an EARLY bottom-left signal (t3-t6). In 8x8 the bottom-left can only hear: U (fed only from (0,-2)),
  its own pistons, and the top-left via the left column; the top-left only hears B (moves >= t8, after A's
  hold, so the top-left pulse is >= t10). Circular: no symmetric train.
- So the 8x8 needs an EDGE-ASYMMETRIC element at the left: U must get 1 pulse on open (after K) and 2 on
  close (around K). `door satwire` (s2, s3, palette incl. bulb/cmp/note, --keep-wiring): UNSAT in seconds,
  core = close U (1,-2) alone.
- Found asymmetry: an observer at (-1,3) facing down watching TL's park (-1,2) pulses TWICE on open
  (t14, t18: the puller's head enters, then TL lands) and ONCE on close (t14) (probe, redsim). Its output
  goes up into (-1,4) (c79's top-left net), so the top-left net must be re-laid to use it. Same 2-vs-1 for
  every park/floor-cell watcher (the cells where a pulled block arrives), the wrong way round for U.
- Flat circuits cannot lock repeaters (their sides are off the plane).
