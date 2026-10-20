# w86 HANDOFF (smaller doors; box devvm59213 since 2026-10-20 05:20Z)

## State (2026-10-20 ~12:20Z)
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
- ROLE (coordinator 10:34Z): candidate checker for every macrosat / mechmut candidate (clockinfra d650ed4f, c70b
  9182fde5, clk-misc da291dc5 message me directly). Per candidate: `door ideal-rs` (both phases), `door ideal-rs
  --extent` (volume over the cycle, inputs + lever excluded; my tool, 8c3921609, merged with main 8742cbfa2), a real
  lever in place of any lever stand-in, satwire in the box (plain palette, then `--palette block,dust,rep,obs,torch,
  bulb,cmp,note --hold auto --retime --free-order`), the d67 `door feeds` table, mechanism-fix suggestions.
- Checked so far: u0/u2-7x7 (c70b): satwire cores top-left pair (plain) / SU(1,-2) close (wide; sealed (0,-2) observer
  fires on every SU(-1,-2) move); a real lever on (0,5) arms SD(1,4). m86-a (macrosat 8x6): extent 9x6 (its piston
  (-3,-1) extends outside the box in its own replay), cores SE(-3,0) / SE(-1,1) (clk-misc: no real feed for L1, one
  obsidian feeds 4 pistons). macrosat now has strict feed/lever rules (clockinfra 8e2cced03) and sweeps 48 + 7x7.
- 7x7 island-start mechmut search (from c70b's u-island seeds, ~/agents/w86/runs/isl): best island start 2; the barred
  version can't start from u2 (fails unfed 0 under the fixed lever rule). Parked.

## Tools/worktrees on devvm59213
- ~/agents/w86/minecraft-tas (explore/w86), ~/agents/w86/mm (explore/c70b build: mechmut --add/--add-redstone),
  ~/agents/w86/pext (explore/pext + main merged locally: door interleave). Runs in ~/agents/w86/runs/.
