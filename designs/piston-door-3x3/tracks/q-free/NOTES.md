# Track q-free: beat the solid 40 (fc2 / fx1 / m05) via a free Q delay or a shared bottom pulse

## Lead 1: "free Q delay" (Q silently quasi-powered by the lever, woken later) -- FAILS as stated
Files: `l1.patch`/`l1-mech.txt`/`l1.sched`, `l2.patch`/`l2-mech.txt`/`l2.sched`/`l2-open.drive`.
* The silent part works: with the lever (or its attachment block) next to Q's QC cell (0,5,1) and
  nothing updating Q, Q waits; an observer pulsing (0,5,1) at t4 (O_w (0,5,0) facing north) wakes
  it and the open runs exactly like fc2 (l2 open trace: Q E at t4, P_R at t7).
* The close breaks whatever p0's path is: on close Q is EXTENDED and its HEAD sits at (1,4,1),
  right next to A. A piston head forwards neighbor updates to its base (PistonHeadBlock
  .neighborChanged). So any p0 source that updates A's neighbors (an observer/repeater pointing into
  A, or dust within 2 cells: X at (1,4,-1) reaches (1,4,1) at distance 2) wakes Q at p0, i.e.
  Q_close = p0, but fc2's close needs Q >= p0 + 2 (T needs (1,3,0) free). l1 shows it: P_R pulls at
  t2 into A's moving head, the right structure stays out.
* The only update-isolated p0 path is a CONDUCTOR at (1,4,-1) powered by an observer/repeater next
  to it (its neighbors exclude (1,4,1) and (0,4,1)). From any lever cell that can QC-power Q
  ((0,6,1), (-1,5,1), (0,5,2), or 2 away through its attachment block), reaching it costs >= 4
  blocks (e.g. note block + observer + dust + conductor); the dance pulses through that conductor
  also fail (dance 1: A in (1,3,0) is not updated when the pulse ends, retracts only when its head
  lands at t18-19, B's window ends at t20: A stranded; l2-open.drive). Net cost > fc2's R_q + floor.
* A hold does not rescue it: Q would have to be powered at p0 on close (head update) but not woken
  before t4 on open (T must land before RB reaches P_R). Any pulse that holds Q at t2 on close also
  wakes it at t2 on open.

## Timing freedom of the side pistons (fc2, `cut-plpm.txt` + `plpm.sched`, `door ideal --sweep`)
P_L's pulse and P_M's pulse each work at ANY tick 0..30 on both edges, independently (open stays
24 while pl <= 21; close 9-11). So both only need ONE 2 gt pulse per edge (or an odd count) from
anywhere: the problem is purely geometric (no once-per-edge source sits next to them).
