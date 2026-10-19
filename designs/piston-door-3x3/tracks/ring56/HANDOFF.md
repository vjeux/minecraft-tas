# ring56 handoff (2026-10-19 13:15Z)

Track: hand-designed 7-wide flat FULL-frame doors ("the door is the wire"). Branch `explore/ring56`,
files under `designs/piston-door-3x3/tracks/ring56/` (NOTES.md sections 1-14 have the story).

## What came out of it
- jf / p2 (7x9, `mech/p2.txt`): finished by track bridge with calibrated sculk sensors
  (`bridge-p2-sculk-49b`, rotation-safe `-r`, on main). Its 7x8 refit is e78 (partials/e78.md, owner e78k).
- Tool commits on main: cosearch screen accepts a lever ON at rest; halo check (heads poking out of the
  box mid-cycle).
- The t8 family (families 7x8-014 with BR pulled right, ONE piston below the door): the only
  single-U 7-wide topology I found where top and bottom halves connect without sculk (TL goes UP, so
  the left column has 2 pullers + a conductor). Board file: partials/t8.md.

## Proven
- 7x8 t8 with the lever on TR's puller (`mech/t8Ls.txt` + `t8Ls.ideal`, two-pulse B): exhaustive pockets
  on the 6-cell top-right, k=4 full palette + sculk and k=6 with 22 states: B2 is driven by NO filling,
  no filling covers more than 4 of the 12 elevator groups. Unwirable as is.
- Topology note (NOTES section 13): with all six side blocks sideways in 7 wide, each side splits into a
  blind top and bottom half; t8y (TL left) and the catalog single-U mechanisms are trigger-isolated.

## Live (where to continue)
1. B as ONE window per edge (held from B1 to B2): `mech/t8Lw.ideal` (7x8), `mech/t9w.ideal` (7x9):
   B on in 15..22, off in 29..34. With it, A wires with 4 blocks (`pk/t8Lw-k4/pocket3-set0.txt`).
2. `mech/t9b.txt` + `mech/t9bw.ideal` (7x9 = 63): 79/79 with only P_T (3,3) and B (1,5) ideal (left
   (0,4), (-2,1), BR (4,0), U (1,-2) pinned). P_T's only feed (3,4) is taken by the A trick: the next
   step is an A wiring that leaves (3,4) to P_T, or P_T fed through (3,4) by an observer (3,5) facing up
   (as in `mech/t9a.txt`). B's window should come from above ((1,6): obsidian + a repeater at (2,6)).
3. Then the left (q8seed's lamp chain watching B at (0,5), or pockets on the left region) and the
   bottom (pockets k=3 already drive U from the bottom-left and BR's puller from the bottom-right).
4. 8x8 `mech/t8xL-b.txt`: 79/79 with A, B, U ideal (left + corner wired); B can't be fed from above
   there (box top y=5).

## Running processes (devvm51609; the box is shared with track c77 now)
- cosearch run 19 on t9b (finds19/), pockets k=6 on t8Ls top-right with the window plan
  (pk/t8Lw-k6/, log /tmp/ring56-pk-t8Lw-k6.log). Kill by PID if needed (never pkill -f).
