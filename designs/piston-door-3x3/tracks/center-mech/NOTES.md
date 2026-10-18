# center-mech: new middle-column mechanisms

## Facts read from the source (PistonStructureResolver / PistonBaseBlock, 26.3)
- A slime-dragged neighbour that is immovable (extended piston, piston head) is SKIPPED (branch
  returns true); only an immovable block in the PUSH line aborts the move.
- Sticky retraction pulls the block 2 in front only if it is PUSH_PULL or a retracted piston.
- PUSH_ONLY (glazed terracotta) is pushable only along its own line, so slime never drags it.

## Why C always needs an "elevator"
C must travel 2 cells in one direction (every escape cell at distance 1 is still in the opening,
and z moves land in the walkway). An actor (piston head or slime) must be adjacent to C for
each move, but the edge block in that direction leaves through exactly the cell the actor would
start from, so the actor has to come 2 cells in: either a piston riding another (A on B, v2) or
two pulses. A perpendicular entry is impossible (perpendicular cells are door or walkway cells).
So the floor is 5 pistons: two sides, the opposite edge (M or T) and the elevator pair.

## M1: P_T replaced by a left-group slime (proven with ideal inputs)
`wip/c1-mech.txt` + `wip/c1-drive.txt`. Left backbone gets two more slime, (-1,3,0) and
(0,3,0). A lifts T to (1,3); then the left group retracts and (0,3) drags T to (0,3) (A's head,
when still there, is skipped as immovable). Close: B dance brings C home, the left piston
pushes T back to (1,3) (T is in the slime's push line), A pushes it down.
Mechanism: 5 pistons + 8 slime (+1 block vs v2's 6 + 6) but no P_T torch/net; the left piston
inherits P_T's NOR(L+8, L+28) window instead of NOT L.

## M2 (c2, proven with ideal inputs, then wired as v2u2 = 96 blocks, verified at 32 positions)
`wip/c2-mech.txt` + `wip/c2-drive.txt`; wired: `v2u2.txt` (built by `designs.rs::v2u2`).
- Side groups are PUSHED out, not pulled: two sticky pistons under the walkway at (0,-1,1)
  facing west and (2,-1,1) facing east push slime fronts (-1,-1,1)/(3,-1,1) that hang off the
  backbones. Both sit next to ONE obsidian block X (1,-1,1): no torches, no sides line.
- M rides a honey block (1,-1,0) (honey never sticks to the side slime) pulled down by P_M at
  (1,-3,0). P_M is held extended by a redstone block R_M (2,-3,0) riding the right group; R_M
  must touch P_M itself (a QC-only position never updates P_M when R_M comes back).
- A's torch is replaced by a redstone block R_A (0,4,0) riding the left group (2 more backbone
  slime). X is fed L OR L_24, so the sides (and R_A) leave at once on open and come home late
  on close: A's "NOR(L, L_late)" for free, no A-early/A-late wiring.
- L_24 joins the X line one-way: the late chain ends in a solid block (3,4,3) (strongly powered
  by its last repeater, so it powers the P_T net dust (3,4,2)) and a repeater (3,4,4) out of
  that block starts the X line. (Dust-under-a-dust-powered-block does NOT work: shouldSignal.)
- One repeater refreshes the 19-dust line from the lever to X.
- Top is v2's (A, B, P_T, B window, P_T NOR net).
- `door minimize --moves all`: 96 -> 88 (`v2u2-min.txt`).

## M3: C goes DOWN (proven with ideal inputs, `wip/c3-mech.txt` + `wip/c3-drive.txt`)
Top is only A (T goes up one and stays; no drag, no pulses), held by a redstone block riding
the right group. Bottom: P_M pulls M down, P_MA (sticky, z=-2 side) pulls M aside to
(1,-1,-1), then the elevator: B_M pushes P_M up, pulse, B_M pulls it down, pulse: C ends at
(1,-1). Close: B_M pushes P_M+C up, spit pulse, P_MA pushes M back, P_M (redstone block on the
left group, late) pushes M up, A (late) pushes T down. It is v2/rb2 mirrored: same 6 pistons,
so it only pays if the control is cheaper at the bottom (it is not obviously).

## Sculk sensor = free 30gt hold (rb2-sculk, 50 blocks, verified 32 positions)
rb2 (redstone-block track, 57) holds Q for 24gt after the lever goes off with 3 repeaters + 4
dust (14 blocks) so the closing C dance finishes before T is pushed back. A sculk sensor next
to Q does it in one block: every lever flip is a vibration (BLOCK_ACTIVATE/DEACTIVATE), the
sensor goes ACTIVE for 30gt then COOLDOWN 10gt (it ignores the door's own piston vibrations
meanwhile), and it powers adjacent components while active. The sensor reacts 1-2gt late (the
vibration travels, then the block entity ticks after the block-event phase), so Q would retract
at once when the lever's block goes dark: one repeater (0,4,-2) keeps Q powered across that gap.
Patch: `rb2-sculk.patch` (57 -> 50).
