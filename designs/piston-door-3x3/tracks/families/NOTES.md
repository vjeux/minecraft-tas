# Track families: the complete catalog of flat (1-wide) seamless mechanisms per box

Brief (vjeux via the coordinator, 2026-10-18 21:09Z): "Can we search exhaustively all the 'families'
and see if we can wire them?" A family = a door mechanism: the pistons that move the 9 door blocks
and the paths the blocks take (wiring and timing aside). Records at the start: flat-b-v7-full 80b
(8x10x1, center exits DOWN via an elevator below) and flat43-full / fl81m 81b (9x9x1, center exits
UP via an elevator above).

## Stage 1: `mctas door families` (crates/piston-door/src/families.rs)

Output: [CATALOG.md](CATALOG.md) (+ [CATALOG.tsv](CATALOG.tsv)): per box (volume order) every minimal
mechanism that fits, with its pistons, schedules, symmetric train, free cells (room for wiring) and
the simulator verdict with ideal inputs.

### Model
* A fast grid port of mech-synth's macro piston model (`PistonStructureResolver`, push limit 12,
  sticky pull of the block two ahead, retracted pistons movable, extended pistons / heads / obsidian
  immovable), restricted to the door plane: pistons face E/W/U/D. `P` = 2 gt pulse (push and spit,
  or extend into a gap and pull the block beyond), `L` = level toggle. A level-on that cannot extend
  is refused; a move that would leave the box is refused (the box holds every state).
* Open test = FULL seamless for a flat door (`door properties`): the 3x3 empty; the 12 ring cells
  hold only air, door blocks or plain blocks (no piston body or head, no obsidian); every ring cell
  that some state of the cycle fills but the open state leaves empty has air or a door block right
  behind it (the hole rule). Closed: the door plane's ring is hidden behind the door blocks, so a
  flat door is QUART on both sides by construction; the `rest-ring` column says whether the rest
  state also keeps pistons out of the ring (the coordinator's stricter reading).
* Solve = the WHOLE state graph of the pulse model (pulses on every piston, levels only on pistons
  extended at rest), capped; cheapest open path + cheapest close path back to the exact rest
  state (door blocks interchangeable). `--levels` adds an A* pass with level events.
* Symmetric train: pistons used once per edge and touching no cell another piston changes are
  "edge" toggles (fired as a block at the start or at the end, every split tried); the core gets a
  product BFS of the run from rest and the run from open under the same events.

### Generator (the bounds)
* Box W x H, the door at every offset (left-right mirror offsets skipped: mirror images merged).
* The CORE: the center leaves straight in exactly 2 moves; the edge block on its line leaves in 2
  moves (any path not ending on the center's park cell).
* The 7 other perimeter blocks: 1 move out by a pull-type sticky puller (retracted, 2 cells away),
  or (`--two N`) up to N of them by any 2-move path.
* Every move gets every provider: a PULL by a sticky piston 2 cells ahead (retracted or extended at
  rest), a PUSH by a piston 1-2 cells behind (sticky or normal), or a piston already placed reused
  from its rest cell or LIFTED one cell along its own axis by one sticky lift piston (the elevator).
* At most P pistons (`--pistons`), no helpers, no slime/honey.
* Each candidate is solved; working ones are reduced to MINIMAL mechanisms (greedy: no piston can
  be dropped), canonicalized under the mirror, and described in every box they fit.
* The solver may find solutions outside the plan that generated a candidate (riders moved
  sideways, block shuffles): those are real macro-model solutions and are kept.

## Log
* Both records' mechanisms (wiring dropped) solve in 7x8 (`--check`): flat-b-v7-full (cd19
  family) and flat43-full, each with a 13-pulse symmetric train (6 core + 7 toggles).
