# w86 HANDOFF (wide 6-tall boxes: 8x6 = 48b, 9x6 = 54b)

## State (2026-10-19 ~21:50Z)
- Mechanisms with ideal inputs that WORK (ideal-rs + real-game `door ideal`, FULL frame 12/12, flat, slime-free):
  - wip/a3s (7x6 = 42b, 13 pistons), wip/a3r (8x6 = 48b, 14 pistons), wip/b3s (7x6, a3s upside down). Unwirable
    as is (row-4 piston trio boxed in, see NOTES "Wiring"). Selftest anchors ideal:w86-a3s-7x6 / ideal:w86-a3r-8x6
    (commit f50c9e22a, sent to the coordinator for merge).
  - m69t/m69u.ideal: c78l's m69a turned into 9x6 (c78l's m69t.txt), my re-timed plan (QC coupling s0/s1).
  - m69t/m69f, m69g: the same upside down (Box A); m69g has the top trio lever-held by the real lever.
- No wired door yet.

## Board
- partials/7x6-a3s.md (a3s/a3r), partials/9x6-m69t.md (the m69 family; helpers c78l satwire, pockets3, c70b mechmut).

## Exact next steps
1. Fix m69g's right half so that every piston is fed: the right row-0 elevator (carried s0 at (3,0)) has no feed
   and W' (3,-1) sits under it. Ideas: a right side whose elevator works in row 1 (C exits right) while BM goes
   left in row 0 needs a fed left row-0 elevator (a wall torch on (-2,-1) at (-1,-1) strongly powers (-1,0) from
   below; it is a NOT gate on (-2,-1)); or keep BM on the right and drive carried s0 by a redstone block moved next
   to (3,0)/(3,1) at the right tick. c70b's mechmut was given m69g as a seed (unfed metric).
2. Then wire m69g: lever trio = the level; observers at (-1,4) facing east and (3,4) facing west watch the outer top
   pullers (2 pulses each per flip: extend/landing) and start each half; left half events: U1 t2 (QC-fires U0),
   P3b' t8, MB level [18,30), U1 carried at (-1,1) t22 (feed via (-2,2) into its quasi cell (-1,2)), U1 home t34
   long; close: MB [8,18) pushing the line U1+C, U1 carried t12, P3b' t20, U1 home t26 (QC-fires U0).
3. Sculk fallback for m69v (row below, lever under R1): calibrated sensors in each half hear the lever.

## Running processes
- none of mine (all runs finished; c78l runs satwire on m69v, pockets3 pockets on m69t, c70b mechmut).
