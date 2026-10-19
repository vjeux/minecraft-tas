# Track m10top: wire the top of w63top's u7-m10 (7x8) for real

## m10b (A's last pull + L1/L2 real, c79's dust line)
- m10b.txt = m10a + observer (0,5) facing east on B, dust (-1,5) on stone (-1,4), dust (0,4) on the static
  (0,3) pointing into (1,4), dust (-2,4) on stone (-2,3) (L2 direct, L1 by QC). Plan m10b.ideal: only A's
  hold, K, U, L0 (and the close U/K/L0) ideal; L1/L2 get exactly ONE pulse per edge (the B watcher), so the
  close L12 pulse is removed.
- WORKS (door ideal, 4 positions) when the close U2 is at 10..11 (c79's trick: A lands in (1,3) at close
  t12, powered through (0,3), but the column is blocked by U2's push in flight) or >= 14 (A spits TM into
  (1,1), U2 then pushes BM+C+TM up 1). U2 = 12..13 fails.
- Windows (m10.ideal, independent): with af = b+2, B anywhere 10..20 works; K >= 3, U >= K+2, any lateness.

## The A-hold wall, restated (by hand)
- B fed from the east ((2,5)) QC-couples TRp to (2,5)'s input: B - TRp <= 8. The TR-gated hold needs
  B - TRp >= 9 (TR lands t+2, A fires t+3, push lands t+5, retract t+6, A lands t+8, B >= t+9): 1 gt short.
- Lever on the left: the 8 top-left cells cannot hold B's line + the hold pulse + a delayed final-pull
  pulse (the final pull needs a pulse at (0,4) >= B+2 and the only B watcher cell (0,5) is then B's feed).
