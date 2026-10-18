# Track boxtall: narrow flat doors (5 or 6 wide, 1 thick), FULL seamless + hallway, smallest volume

Brief (vjeux, 2026-10-18 20:24Z): size = the community VOLUME (`door properties` volume, lever
excluded); goal: flat (`--shape thick<=1`) doors 5 or 6 wide (at most 1-2 columns beside the
opening on each side over the whole cycle), as short as possible (6x9 = 54, 5x10 = 50, 6x8 = 48).
Coordinator (20:27Z): every door must be FULL seamless + hallway-compatible (SEMI+hall interim).

## Geometry of a FULL seamless flat door (why narrow is hard)

Door plane z = 0, opening x 0..2, y 0..2. The hallway ring S at z = 0 is the 12 cells
face-adjacent to the opening: side cells (-1,y), (3,y), floor (x,-1), ceiling (x,3), y/x 0..2.
* Open state: each of the 12 ring cells holds a door block, a quartz-like block (stone) or air; an
  air ring cell that something passed through is a hole and the cell right behind it (one further
  out) must not hold circuitry. Closed: the door plane's ring is hidden (any circuitry is fine).
* Hallway (props-dyn, ring-aware): every S cell at depth +-1..3 is filled with smooth stone (all of
  them, for a flat door). A slime/honey block that moves while it sits in a door-plane ring cell
  drags those walls (z +-1): no slime/honey in the 12 ring cells when it moves.
* So all STATIC circuitry (wiring, pistons) lives outside the ring and the opening: in the 4
  corners (-1,3), (3,3), (-1,-1), (3,-1) and rows y >= 4 / y <= -2, plus columns beyond the ring
  (x <= -2, x >= 4: only in a 6/7-wide box).

### 5 wide (x -1..3)
* Every side ring cell is in S, so no static piston can stand in rows 0..2 at all: no horizontal
  piston can push or pull a door row (a pull needs a base at x -2 / 4, a push a base in the ring).
  Every door block leaves vertically: each lane's middle block needs a 2-cell move (an elevator,
  fc2's A on B) and the ceiling/floor rings hold only 3 + 3 blocks (+4 corners after row shifts):
  three elevators.
* Top and bottom halves cannot share a wire: the only cells between them are ring cells. A signal
  crosses only by motion (an observer watching a block that moves, e.g. a stone column in a side
  ring pushed by a corner piston: stones in S are hallway wall, not circuitry).

### 6 wide (x -2..3)
* The wide side's column leaves sideways on 3 pull-type sticky pistons at x -2 (heads into the
  ring, gaps at x -1): FULL-clean (the door blocks become the wall, the pistons hide behind).
* The narrow side's middle block (2,1) has no 1-cell exit: (3,1) needs a pull from x 4 or a push
  from (1,1) (inside the opening); up/down needs a 2-cell move (its own elevator), because the
  ceiling/floor ring cells above/below it hold TR/BR.
* The wide column x -2 is full of pullers in rows 0..2: top/bottom wiring cannot pass it either
  (an observer can watch one puller across, if only one puller stands there).

## Tools added
* `door mech-synth --seamless-open`: the macro model's open test also requires the door plane's
  ring to be FULL-clean (door blocks, stone or air; no heads; no circuitry behind an empty ring
  cell; no slime/honey in the ring). Use with `--lo/--hi` box limits.

## Log
* 20:41Z `bases/six-a.txt` (6 wide: left 3-stack, fc2 elevator A/B + P_T at the corner (-1,3), T/TR
  pullers, M/BR pullers): does not open (MR has no exit), as expected; `--add 2` running.
