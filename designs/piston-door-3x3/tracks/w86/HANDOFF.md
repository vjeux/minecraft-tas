# w86 HANDOFF (wide 6-tall boxes: 8x6 = 48b, 9x6 = 54b)

## State (2026-10-19 ~23:35Z)
No wired door. Many mechanisms that WORK with ideal inputs (ideal-rs; most also real-game `door ideal`), FULL frame
12/12, flat, slime-free. All files in designs/piston-door-3x3/tracks/w86/ (explore/w86):
- wip/a3s (7x6 = 42b, 13 pistons), wip/a3r (8x6 = 48b, 14), wip/b3s (7x6, a3s upside down): unwirable (the row-4
  trio boxes Q in). Selftest anchors ideal:w86-a3s-7x6 / ideal:w86-a3r-8x6 (commit f50c9e22a).
- m69t/: the 9x6 m69 family (c78l's m69a turned): m69u.ideal (Box B), m69f/m69g (Box A; m69g = top trio held by
  the real lever ON on top of (1,4), the 7x8 doors' trick, crossing observers at (-1,4)/(3,4)).
- m69t/m73..m81: lever-trio Box A mechanisms built against the wiring obstacles (see the board partial
  partials/8x6-m73.md and NOTES "obstacle 6"): m73 (8x6, row-0 wiggle), m74-76 (its plans), m77 (= c70b's
  m69g-12 + lever), m78 (no bottom-corner piston, V at (3,4)), m79/m80 (column 5 free, stacked left carriers),
  m81 (m78's left + W' climb, (3,4) free).

## satwire verdicts (box 9x6 = --box=-3,-1,0:5,4,0, --lever 1,5,0 --keep-wiring --lever-on --full-frame)
Every one UNSAT. Single-piston solid cores that moved as I fixed them: W''/W' bottom-corner pushers fed only from
isolated bottom-row pockets (m73, m77, m81), RP0's long close pulse (m76, even at 10 wide), the right half not
seeing the close flip when (3,4) is taken (m78), carried pistons in the left wall fed only through a door block
(m79, m80). Sculk (c78l): the pockets get fixed by sensors, but m79/m80/m81 then have big cores across the left
elevator, the right carrier pair and the held top (logs: explore/s79 runs/sw-m7*s.log).

## Exact next steps
1. The structural problem (NOTES obstacle 6): with the lever trio the side receiving BM through row 0 also has its own
   row-0 block, so one block must climb from (+-,0). Untried climber: a down-facing puller carried ONE cell sideways
   into the wall top (P resting at (4,2), carrier at (5,2), it pulls (3,0) -> (3,1)), but it sits under P3t' (QC)
   and takes s1's feed cell (4,2); needs a right side without M1 (so C must get its first leftward step elsewhere).
2. c70b's mechmut: feed metric should require a feed cell in a free region that the lever's flip reaches (flood
   fill avoiding trajectories); with that, m69g-12's W' is unfed.
3. If a satwire per-cell sculk restriction appears, retry m81 with sensors only in (3,-1)..(5,0).

## Running processes
- none of mine.
