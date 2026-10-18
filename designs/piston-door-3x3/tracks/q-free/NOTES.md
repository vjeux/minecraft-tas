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

## Other seeds tried
* `pmleft.txt` (40, verified): fc2 with P_M fired by the LEFT column (observer (0,-1,0) facing up
  under the bottom-left door block + smooth stone (0,-2,0)) instead of the right column. Same
  count; `door minimize --moves all --plateau --depth 2` from it: 282 level-2 neutral designs, no
  removal (`pmleft-min.log`).
* `m05x` (wire21's 21-block mechanism): P_L fed X-style 3 pulses (t2/t14/t22) + M fired by the left
  column + an early single P_R pulse: BROKEN for every P_R tick 0..30 with ideal inputs (the close
  needs M's push to carry M, C, T home after A3; the left column's 3 moves give M the wrong last
  move).
* Merged explore/riders into this branch for `door grow --glue --k 2 --changed --then-search`
  (the riders track's unexplored "2-block riders" search) and ran it on fc2's left and right
  structures (results below).
* Results: 2-block riders on the LEFT structure (box x-2..0 y-2..4 z-2..2, obs/rblock/block/slime,
  `--glue --k 2`): 4584 candidates, 3418 still-working 41-42 block seeds, 0 change the door's
  timings (`--changed`), so nothing to search from (`glue2-left.log`). The right-structure run
  (12926 working seeds) was stopped during its `--changed` pass to stay within the worker cap.
* `door regrow --size 2 --any --margin 2` on pmleft (states incl. note block, slime, repeaters):
  482k candidates, nothing (`pmleft-regrow.log`).
* New palette state `note` (note block) for `door synth`/`door grow`: a conductor whose own
  `powered` state flips the tick its power changes, so an observer can watch any powered spot
  through it (gadget `designs/gadgets/note-watch.drive`). Searched with it: fc2 minus P_L's
  observer pair and O_b, `door grow --k 2` in x-4..1 y1..7 z-1..2 (obs, note, block, dust): 580k
  candidates, nothing (`cut3-grow.log`).
* Shared side pulse (`plpm-shared.sched`): ONE 2 gt pulse at the same tick fires both P_L and P_M
  correctly for every tick 0..24. So one free once-per-edge source next to both would give 38; the
  only cells next to both (with P_L push-type at (0,-1,0), fresh-e's pb) are (0,-2,0) and (1,-1,0)
  (M's path), and no 1-block source can pulse (0,-2,0): every observer cell next to it watches a
  cell where nothing moves ((-2,-2,0), (0,-4,0), (0,-2,+-2)); torches are on at rest, dust does
  not power upward, a redstone block or lever is a level.

## Why I think 40 holds for this family (summary)
* Free Q: impossible as stated (head forwarding, above). A hold that saves it breaks the open.
* Each control pair is pinned: P_L and P_M need a once-per-edge (or odd) pulse next to them and
  nothing moves next to them except the structures they move themselves (plus the right column
  for P_M, which fc2 uses); the window needs a repeater whose input comes from a horizontal
  observer, and O_b's back faces up (J); X needs a floor; the lever must feed O_a (p0) and K
  (R_q). Note blocks make more spots watchable but none of the useful ones (searched).
