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

## m10f (7x8): close-safe top (c2e bottom + my top), only A's hold + L0 ideal
- B watcher (0,5) -> observer (-1,5) -> stone (-2,5) -> dust (-2,4) on (-2,3) (L1+L2) -> observer (-1,4)
  -> STONE (0,4): A at (1,4) is updated (last pull, open); A at (1,3) on close is only QC-armed. 32 pos.

## m79p (7x9): A's HOLD real by making B PULL-TYPE
- B retracted at rest, (1,4) empty: one 2 gt pulse per edge (pull at the pulse end on open, spit push on
  close), so B needs no level through (2,5) and c2e's right hold family fits: TRp dropped at t1 by the
  lever level through (3,4), TR lands (2,3) t3, repeater (3,3) d4 on the lever dust (4,3) powers TR ->
  A holds t4..t8. door ideal WORKS at 32 positions with only B's pulse (open plan tick >= 8 = B's event
  >= t9; close any), A's last pull and L1/L2 ideal.
- m79q: B's pulse real but 1 gt early: wall lever (4,4) on a LAMP (3,4) (conducts to TRp; weak power
  lights dust (4,3)), observer (3,5) facing down on the lamp's dark edge (t4 -> t6), observer (2,5)
  facing east -> B extends t8, retracts t10 while A's base still lands (t10).
- Why it is stuck (by hand + pockets k=3 over 55 states): B's only clean feed is an observer at (2,5)
  facing east (anything emitting into (2,5) QC-powers TRp and re-extends it mid-hold); (3,5) must never
  emit west, and every element that fits changes by t6 (lamp 4 + observer 2); the hold cannot end before
  t8 (A fires t4 = TR landing + 1; d3 spits). Needs one odd element or one more top-right cell.
- Ruled out: copper bulb (flips in neighborChanged), leaves (need a moving LOG neighbour), samplers
  (dropper/note/bulb get the lever's update at t0), a second B pulse from the left (lifts A on close),
  lever on the left (the right levels need it; the left is sealed: it only sees B, (1,4), L2).
- Next: an 8x8 with the extra column on the RIGHT (x=-2..5) gives B's chain room (repeater at (4,5)
  facing east from (5,5) into a weakly powered (3,5)) but needs a 2-row bottom compatible with the hold
  at t4..t8 (c79's needs row -3; c2e's 7x8 bottom wants the hold at 8..12). In 7x8 c2e's L0 can be fed:
  observer (0,-2) facing east on U -> stone (-1,-2) (K above) -> dust (-2,-2) -> observer (-2,-1) up -> L0.
