# w86 HANDOFF (smaller doors; box devvm59213 since 2026-10-20 05:20Z)

## State (2026-10-20 ~08:00Z)
No wired door under 56. All findings in tracks/w86/NOTES.md ("7x7 (49)" items 1-8, "6x8 (48)").
Mechanisms that WORK with ideal inputs (redsim; most also real game), FULL frame 12/12:
- wip/a3s 7x6 (42b), wip/a3r 8x6 (48b); m69t/m73..m83 (8x6/9x6, lever trio); six8/s1 6x8 (48b, 12 sticky);
  seven/t1 (7x7, c77's w1 transposed); seven/q7c, q7d (7x7, d67's q7b with pulse-type trigger / shared long pulse).
Verdicts (argued, cross-checked by c70b / clk-misc / satwire / pockets3):
- 7x7 w1 class (middle piston fires from F1): unwirable. Lever trio: impossible in 7x7.
- 7x7 turned class (t1, middle piston in L1): dead under the STRICT extras reading (coordinator: extras sit in the same
  frame cell at both rest states; vjeux asked, answer pending).
- 7x7 inside-carried class (q7b/q7c/q7d): not wirable (dead bottom-right pocket, sealed SD(0,4), overlap timing).
- 6x8: unwirable solid and sculk (corner slide pusher shares its sealed feed cell with the column's final puller).

## Live
- c70b (9182fde5) searches from my t1 seeds: t1-r22 (explore/c70b smaller/mm/t1-r22) works with an ideal stand-in
  for a lever-held SD+(0,4); with a real lever it breaks (QC-arms (-1,4), (1,4)); satwire core with (0,4) ideal:
  the F1-firing piston (1,-1) (its F0 neighbour is occupied by a shifted static at the time). Results in
  smaller/mm/t1r22-u*.
- Team for the inside-carried class: pext (ec7bcf52, door interleave), clk-misc (da291dc5, door feeds on explore/d67).
- clockinfra (d650ed4f) asked to check the 6x8 argument with its SAT model.

## Tools/worktrees on devvm59213
- ~/agents/w86/minecraft-tas (explore/w86), ~/agents/w86/mm (explore/c70b build: mechmut --add/--add-redstone),
  ~/agents/w86/pext (explore/pext + main merged locally: door interleave). Runs in ~/agents/w86/runs/.
