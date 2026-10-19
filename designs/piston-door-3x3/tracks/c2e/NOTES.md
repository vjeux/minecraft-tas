# Track c2e: wire a 7x8 (56) flat, slime-free, FULL seamless + FULL FRAME + hallway door by hand

Brief (coordinator 03:18Z): wire mechspace's column-2-elevator family 7x8-c2e-001..004 by hand; fallback
the t1 family (7x8-t1-001..016). Box x -2..4, y -2..5, door at (0..2, 0..2).

## 1. c2e-001..004 cannot be wired in 7x8 with in-plane redstone (TL is sealed)
TL = the 7 free cells (-2..0, 3..5). They touch only TL's own pistons (SE(-2,2), SE(-1,3), SD(1,4),
SD(1,5)) and TM's park (0,3); those pistons are fed only from TL cells, and they touch the rest of the box
only piston-to-piston (SD(1,4)/head (2,4), SD(1,5)/SD+(2,5), (1,3)/SD(2,3)). No observer outside TL can
see a TL-driven cell, no lever spot reaches two regions (a lever on a piston's back survives its
retraction, but SD(1,5)/SD+(2,5) need the opposite polarity), and a BUD across the piston contacts needs a
piston armed at rest, which the judge's settle pass (one self update per block) forbids.
`door connect` agrees: UNDECIDED (cap) in 7x8 for all four; WIRABLE only in 7x9 rows -2..6 (002, 004),
where the top row joins TL and TR through (1,6)/(2,6), the quasi cells of SD(1,5)/SD+(2,5).
Log: logs/connect-c2e.log.

## 2. t1-001 (fallback): WIRABLE in 7x8 (connect, lever in TL), and FULL FRAME with 3 static stones
`door connect` on new-7x8-t1a.tsv in 7x8: t1-001 and t1-002 WIRABLE (lever K1 = TL), the rest capped
(logs/connect-t1-7x8.log). Links: TL -> BL by an observer at (-2,0) watching SE(-2,1); TL <-> TR through
(2,5) (feeds or watches SD(1,5)); TR -> BR by SW(4,0) (stacked with SW(4,1), fed from (4,2)) watched from
(4,-1); BR feeds SU(1,-2) from (2,-2).
FULL FRAME (vjeux 03:36Z): static stone at the 3 empty frame cells (-1,0), (2,-1), (3,2) (unswept hops):
the oc plan still WORKS (t1/t1-001-oc-f.txt + t1/t1-001-f.ideal, 4 positions x 2 cycles, open 45 /
close 27), `door sheet`: frame 12/12.
