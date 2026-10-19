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

## 3. t1-001 timing windows (door ideal, every event its own variable, frame stones in)
- oc plan (t1/t1-001-f.ideal, base: se 0, bl 6, bm 12, a1 12, t 18, b1 24, a2 30, b2 36, a3 42, r 42, m/n 42;
  close se 0, bl 6, bm 12, a1 12, t 18, a2 24, r/m/n 24): FREE 0..60 for SE-left, SU+ level, SU(1,-2), SD(2,4)
  on both edges; the TL train is pure order with minimum gaps: open a1 <3 t <2 b1 <1 a2 <3 b2 <4 a3, close
  a1 <3 t <1 a2. SW(4,1)/SW(4,0) only work together (a lone SW(4,1) input QC-fires SW(4,0) too): one stack.
- sym plan (t1/t1-001-sym-t.ideal, the same a1 t b1 a2 b2 a3 on both edges): close windows a1 0..9, t 9..15,
  b1 15..22, a2 20..27, b2 27..35/38..40, a3 31..42; SU(1,-2) on close >= 36 (after the last A shot, else it
  reach-through-pulls TM back up); everything else free.
- Harmless extras: P_T before a1 (open), A between t and b1 (open); a P_T shot while TM is parked at (0,3) and
  (1,3) is occupied is blocked only if the push line TM, X, (2,3)... ends on an immovable block.

## 4. cosearch pockets on t1-001 (cosearch2's tool; assist plan with every input off the plane:
   t1/t1-001-zl-sym.ideal + seed t1/t1-001-zl.txt)
- k=3, every pocket: BL drives the SU+ level with 3 blocks (but those fillings move the lever into BL: a torch
  link), BR drives SU(1,-2) with 3 (observer on SW(4,0) -> sticky (4,-2) shuttling an observer onto (2,-2): its
  landing pulses the frame stone (2,-1) -> QC), TR drives the SW stack with 3. TL (6 cells + lever): only P_T.
  Nothing drives A, B, SD(2,4) or the left stack. logs/pockets-k3.log.
- Tool: `door cosearch pockets --dump N` saves the N cheapest fillings of every (pocket, driven set) as designs.

## 5. Why TL is the wall (t1 family, lever in TL, by hand)
Every t1 mechanism has the same top: A = SD(1,4) x3, B = SD(1,5) x2, P_T = SE(-1,3), plus the left stack, and in
t1-001..004 TL's 7 cells are the ONLY feeders of A (only (0,4) emitting east drives A at both positions with an
update), of B ((0,5), or TR's (2,5)), of P_T and of the left stack ((-2,3) emitting down). TR can only wake by
watching B from (2,5), so TL must fire B too. Inside TL only P_T's motion is observable (an observer at (-1,4)
facing down): A and B are watched only from their own feeder cells. So TL must produce the whole interleaved
train a1 t b1 a2 b2 a3 (>= 13 gt long) from one lever event, with no feedback from A or B, in 7 cells: the
same K1 wall wire64b (8x8-105) and w63top (7x8-014 in 7x9) hit. Not solved.
