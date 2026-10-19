# c79: the community 7x9 (63b) door, reproduced from vjeux's screenshot (2026-10-19)

vjeux showed a screenshot of the community 7x9 door in its OPEN state (plus a second view). Read
cell by cell and rebuilt here; it works in redsim and in the real game:

- `c79-open-b.txt`: the screenshot exactly (lever OFF, door OPEN, 9 door blocks in the frame + 3
  static frame blocks at (0,3), (2,3), (3,0)), plus 4 redstone dusts the first view could not show
  and the second view confirms: (4,4) on the dropper, (-1,5) on the purple block (-1,4), (-2,4) on
  (-2,3) (the step between them is the dust on the side of (-1,4) in the second view), (0,4) on the
  ceiling block (0,3). The real lever hangs on the front face of (4,5); here it stands on top of it
  at (4,6) to keep the file flat (same power, same updates of (4,5)'s neighbors).
- `mctas door cycle c79-open-b.txt`: CYCLES over 6 flips at the judge's 4 positions, frame 12/12
  when open. `mctas door trace c79-open-b.txt --close` (the real game): closes on lever ON, reopens
  on lever OFF to the screenshot.
- Earlier readings (c79-r*.txt, c79-open-a.txt) are kept as the record of what was wrong: r1
  assumed the door rests closed with the lever off and had no hidden dust (only the elevator moves);
  open-a lacks the dust at (0,4) (TM never gets back up into (1,3)).

## How it works (what our searches missed)
1. LEVER POLARITY: it rests CLOSED with the lever ON (a wall lever's handle points up when off,
   and the open screenshot shows it up). Our judge requires the lever off at rest, which excludes
   this whole family.
2. LEVEL-HELD right side: lever -> block (4,5) -> dust (4,4) -> dropper (4,3) (a conductor) ->
   sticky (4,2) directly and sticky (4,1) by quasi-connectivity. Extended while closed; lever off
   retracts both, pulling TR/MR into the right wall. No pulses needed for that side.
3. DROPPERS as immovable conductors and power sensors: the observer (3,3) watches the dropper's
   `triggered` state and pulses into the static ceiling block (2,3), which powers the elevator's
   lower piston when it sits at (1,3); the bottom dropper (0,-3) does the same for the bottom chain.
4. A NORMAL piston (3,-2) firing into an EMPTY cell (3,-3) watched by an observer (2,-3): a
   wire-free signal across the bottom (right side -> observer (4,0) -> dust (4,-1) -> (3,-1)/(4,-2)
   -> piston (3,-2) -> observer (2,-3) -> block (1,-3) -> bottom puller (1,-2) + dropper (0,-3) ->
   observer (-1,-3) -> block (-2,-3) -> dust (-2,-2) -> observer (-2,-1) -> BL's puller, and
   observer (-1,-2) -> corner puller (-1,-1), observer (0,-2) -> bottom puller again).
5. Hidden dust over the top: the observer (0,5) watching the elevator's top piston pulses the dust
   line (-1,5) -> (-2,4) (on (-2,3): TL's puller directly, ML's by QC) and -> (0,4) (into the
   elevator's lower piston at (1,4), which then pulls TM up into (1,3)).
6. The center goes DOWN by being pushed: the elevator (1,5)/(1,4) pushes TM+C+BM down one, the
   bottom puller (1,-2) takes C into (1,-1), the elevator pulls TM back up two cells; BM is pulled
   sideways into the floor corner (0,-1) by the corner piston (-1,-1); BR goes down (2,-2).
11 pistons, 7 wide x 9 tall x 1 = 63.
