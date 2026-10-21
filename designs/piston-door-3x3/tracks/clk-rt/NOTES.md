# clk-rt: a 3x3 door smaller than 56 (from 2026-10-20 22:42Z)

Coordinator brief: an independent line on a sub-56 door (Solid board: volume < 56, full frame
12/12, rotation-safe, flat, slime-free per vjeux's target). Others: fly + pockets3 on 6x9 (m69T,
s69); c77 / c70b found the 7x7 mechanisms w1 / mm01 (ideal inputs) whose wiring failed on a
boxed-in wiggle piston P and on islands.

## 1. 7x7 with e78k's lever-held top trio: no 1-row bottom (k <= 3)
- Seed: the trio (0..2,4) held by the lever on top, side pullers (-2,0) (-2,1) (4,0) (4,1),
  e78k's statics (-1,2) (3,2) (0,-1). With the trio, C must go down the middle column, and its
  first move needs a piston body in (1,-1) after BM has passed through it (c77's lemma, here
  reached from the bottom side).
- `door fullmech --anneal 7x7 --offsets 2,2 --anneal-seed seed7-trio-x --keep-stone --frame-full
  --complete <13 bottom cells> --complete-k 3 --complete-palette sticky --levels`: 0 of 70,367.
  Calibration: the same command in 7x8 (seed78-trio, 10 cells) re-finds e78k's own bottom (10 of
  39,433, 5 min at 12 threads).
- Hand design s77a: P rests at (0,-1); the corner piston W0 (-1,-1) shuttles it into (1,-1) to
  pull C down and back; a 2 gt spit of the floor row by W1 (3,-1) parks P in the corner and BR
  drops. The OPEN works with ideal inputs, but the close cannot reverse: the spit leaves W0 in the
  margin (-2,-1) and nothing can bring P back to (1,-1) to lift C (a 1-wide margin gives a
  shuttle only one cell of range plus the corner).

## 3. s69 (fly's 6x9) with Qv fed by the carried observer: wip/s69z.txt + wip/s69z.ideal
- pockets3's pick (00:19Z): s69 is the closest on structure; its hard blockers are the 2-cell
  bottom-right corner (sees only its own A; 9 ticks to make) and Qv (1,-1), boxed, never driven
  (open t6 spit, t18 reach-pull).
- O (observer facing down) rests at (0,-2); Z (-1,-2, sticky east) pushes it into (1,-2) (Qv's drop
  cell, free until B1 pulls Qv down at t24): O fires on arrival up into Qv. Z pulses 2-8 and 14-20.
- Catch: Z sits under G (-1,-1), so whatever powers G also QC-powers Z, and at G's old ticks (open
  36, close 24) Z shoves O, Qv and A one cell east. Re-timing G to open 32 / close 32 (windows 30..34
  and 21..22 / 30..34) jams that push; an immovable block at (3,-2) (obsidian, a container) also works.
- WORKS: redsim 32 x 3, the real game 4 x 2, open 81 / close 63. Qv has no ideal input.

## 2. mm01's boxed-in P fed by a carried observer: wip/mm01z.txt + wip/mm01z-e.ideal
- c77: "P needs ONE >= 3 gt pulse on open; it can only come from (2,-1) temporarily or a moving
  source"; in mm01 P needs pulses at open t6 (P at (0,-1)), open t48 (P at (1,-1) between stones)
  and close t54 (P at (0,-1)). pockets3 never found a static feed ((0,-2) / (-1,-2) screened).
- Fix (2 blocks): an observer O facing DOWN rests at (-1,-2) under the corner piston Q; Z (-2,-2,
  sticky east) pushes O to (0,-2) and pulls it back. A moved observer fires 2 gt after landing,
  into the cell behind it: at (0,-2) up into (0,-1), which holds P at t6 / close t54 and the stone
  left of P at t48 (strongly powered, it powers P); at (-1,-2) up into Q.
- So one Z cycle = one P-side pulse + one Q pulse, and Z's cycles also give Q six of its pulses
  (open t18 t36 t54, close t24 t48 t60). Q keeps a pulse at t2 on both edges (a lever-watching
  observer into the static (-1,0) can make it; the extra close pulse is harmless) and open t60
  (window 55..64 except 57; one shuttle cannot give Q two pulses 6 ticks apart).
- WORKS with ideal inputs everywhere else: redsim 32 positions x 3 cycles, the real game 4 x 2,
  open 75 / close 63. Volume still 7x7 (Z and O inside the box).
- Next: pockets3's feed / island checks; Z's own pulse train (3 per edge) and Q's open t60 from
  the mechanism's motion.
- pockets3's checks (2026-10-21 00:00Z): every piston has a feed EXCEPT M (1,-2): O's landing cell
  (0,-2) was M's only free neighbour, so feeding P un-fed M. Load: the bottom-left (Z + Q) must make
  8 distinct ticks from 2 cells. Island starts: no lever placement starts every area (bottom-left
  first pulse at open t2, top-left at close t0, bottom-right at open t13 / close t21, top-right at
  close t3).
- `door interleave --search 3000000 --islands --levels --stone --between-ticks`, lever cell
  (-2,0) or (-2,3): NO train (best 7 cells off). Islands: (-2,-1)(-2,0) | (-2,3)(-2,4) | (0,4) |
  (3,-2) | (3,3)(4,2..4) | (4,0).
- Lever-held left pullers (extended at rest, open retract, close extend): the open's retract must
  come at t >= 10 (P's column-0 push at t7 needs ML/TL in place), so a lever level at t0 fails.
