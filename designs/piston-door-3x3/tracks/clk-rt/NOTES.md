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

## 4. `door shuttle` (2026-10-21 00:40Z ->, coordinator: build it, door stays #1)
- The shuttled-observer feed as a search (crates/piston-door/src/shuttle.rs, docs/tools.md): every
  placement + arrival/return timing on redsim, then 32 positions; `--absorb`, `--unfed`,
  `--per-cell`, `--edge`, `--payload redstone_block` (fly's carried RB). Selftest shuttle:mm01z.
- Calibration: re-finds mm01z's placement (and only it) and s69z's; on plain s69 it also finds the
  riding observer (S (0,-3) up, O (0,-2) facing west into (0,-1)) feeding Qv o6/o18, B1 o24 (QC
  through the empty drop cell on its return) and Qv@ o30: wip/s69r, real game 4 x 2, redsim 32 x 3.
- Spare pulses s69 never needed: G o36, A@(2,-1) c48.
- No shuttle reaches s69's corner (A, B, K: no 3 collinear free cells), nor A@(2,-1), nor
  mm01z's M (O's landing took its only free neighbour; mm01's P and M need the same cell (0,-2)).
- Batch runs (`--unfed --per-cell --radius 0 --payload observer,redstone_block`): fly's wip
  (q68*, q69*, r69*, vq78), c70b's smaller/mm/* and mm-w1 (65 plans, in-plane inputs moved off the
  plane first) and pockets' svc (137 plans): no other shuttle works. Most of these plans have no
  unfed piston at all (their walls are islands and loads, not boxed pistons).

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
- pockets3's checks (00:27Z): Qv's two open pulses are solved, but O at (0,-2) took the free cell
  that fed Qv@(1,-2) (o30, c18); A@(2,-1) is still unfed (as in s69); the bottom-left now fails its
  OPEN island start (Z's first pulse is due by t2); the corner still fails on both edges.
- Timing windows on plain s69 (redsim sweeps): A's first pulse open 0..44, close 0..34; K close
  31..37; B1's close push >= 38. So an observer facing west landing at (1,-2) before t24 could
  start the corner on the OPEN edge (A pulls BR down early, harmlessly). On the CLOSE edge nothing
  outside the corner can power A, B or K: (1,-2) holds Qv@ until B1's push (>= c38), past A's
  window. The corner can't start on the close edge with this bottom-right, however it is fed.
- Riding variant s69x (O facing west, lifted from (0,-2) into (0,-1) by a piston at (0,-3)): one
  observer gives Qv all its open pulses (t6, t18 directly; o30 into Qv@ on its second return), but
  its first return fires east into an empty (1,-2), QC-firing B1 (Qv pulled down early) unless B1
  rests EXTENDED (a level), and B1 has no feed cell for a level.

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
