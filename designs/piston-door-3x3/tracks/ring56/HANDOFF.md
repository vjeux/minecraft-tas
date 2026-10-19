# ring56 handoff (2026-10-19 15:00Z, track ended: slot reassigned)

Track: hand-designed 7-wide flat FULL-frame doors ("the door is the wire"). Branch `explore/ring56`,
files under `designs/piston-door-3x3/tracks/ring56/` (NOTES.md sections 1-14 have the full story).
Nothing of mine is running any more.

## What came out of it
- jf / p2 (7x9, `mech/p2.txt`): finished by track bridge with calibrated sculk sensors
  (`bridge-p2-sculk-49b`, rotation-safe `-r`, on main). Its 7x8 refit is e78 (partials/e78.md, owner e78k).
- Tool commits on main: the cosearch screen accepts a lever ON at rest; the halo check (heads poking
  out of the box mid-cycle).
- The t8 family (families 7x8-014 with BR pulled right, ONE piston below the door): the only single-U
  7-wide topology I found where top and bottom halves connect without sculk (TL goes UP, so the left
  column has 2 pullers + a conductor cell). Board file: partials/t8.md (now ORPHAN).

## Proven
- 7x8 t8 (`mech/t8Ls.txt`, lever on TR's puller) is unwirable in its 6-cell top-right: exhaustive
  pockets k=4 (full palette + sculk) and k=6 (all cells, 22 states) with the two-pulse plan
  (`t8Ls.ideal`): B2 never driven; and k=6 with the window plan (`t8Lw.ideal`): B's window never driven.
- Topology (NOTES section 13): with all six side blocks sideways in 7 wide each side splits into a blind
  top and bottom half; t8y (TL left) and the catalog single-U mechanisms are trigger-isolated.

## Live: t9e (7x9 = 63, same mechanism, box y -2..6)
- `mech/t9e.txt` + `mech/t9ew4.ideal`: A1 and P_T REAL; WORKS (door ideal and ideal-rs, 4 positions x 2
  cycles) with only B (from above, in-plane (1,6)), A2 (1,3) and A3 (1,4)@36 ideal, plus the pinned
  left (0,4) (-2,1), BR (4,0), U (1,-2).
  - A1: observer (4,3) on TR's puller -> normal piston (4,4) -> its head enters (3,4) -> the observer
    resting at (2,4) (facing east) fires into A.
  - P_T: observer (2,5) facing down watches it -> stone (2,6) -> repeater (3,6) facing west -> observer
    (3,5) facing up -> QC through (3,4).
- Plans for the new tile-tick phase (main de6ecbce) must feed B from ABOVE ((1,6)); B's old input at
  (1,5,1) lets A QC-extend first. B works as one window (`t9w3.ideal`: on 14..21, off 28..33) or as two
  pulses (`t9ep.ideal`: b1 14..21, b2 26..31, whole rectangles).
- Constraints: anything re-powering (2,6) re-fires P_T; the piston (4,4) can only be re-fired (for A2/A3)
  through (4,5); the top-left is blind until B moves (it can't start B). `t9ew5.ideal` = t9ew4 + P_T
  assisted, for region searches that clear the P_T chain.
- pockets3 (1d42739b) was running, on its own box, the exhaustive screen of t9e's top-right with the P_T
  chain cleared (`--region-cells '2,5;2,6;3,5;3,6'`, k=6 x 24 states, runs/t9e-tr6.log on
  explore/pockets): it lists which of P_T / B / A2 / A3 that region can drive.
- Next steps: if a filling drives all four, add the left (watch B at (0,5); q8seed's lamp chain) and the
  bottom (pockets k=3 already drive U from the bottom-left watching BL's puller and BR's puller from the
  bottom-right watching U), then `door verify`. If not, the top needs more room: B fed from above gives
  (2,5) back for a B watcher (A2/A3 as in `pk/t8Lw-k4/pocket3-set0.txt`), but then P_T needs a new feed.
- 8x8 `mech/t8xL-b.txt`: 79/79 with A, B, U ideal (left + corner wired); B can't be fed from above there.
