# Track fly: flying machines (contraption study 7)

Brief: /home/vjeux/agents/CONTRAPTIONS.md (study 7). Self-propelled slime/honey + observer +
piston structures. Kinds: one-way engines (fly until blocked), two-way (return stations /
bouncing), with payload; horizontal and vertical. Axis: kind + speed (gt per block moved);
metric: blocks (fewest first). Every design verified in the REAL game (4 positions x 4 rotations).

## Tools (crates/circuit-fly, `mctas circuit fly ...`)
- `sim <designs>... [--trace N] [--visits]`: redsim, milliseconds. Places like the clock judge,
  kicks (`port start:`), tracks the translation every gt (structural states, piston heads part of
  their piston, a frame with a moving block = in motion) and the steady state (frame n+P equals
  frame n shifted by d, held for whole periods). FLIES / SHUTTLES / MOVES / STILL / BROKEN, speed
  P/d gt per block, first move, end state (at rest intact at a shift, corridor end, horizon).
- `verify <designs>... [-v]`: the same tracker on the real game, every build in its own corridor
  sandbox (stops placed and updated first, then the machine like `clock::verify::place`), 4
  positions x 4 rotations, entities at the end = a drop = BROKEN; `robust` when every build
  agrees (direction turned back); then redsim's verdict on the same design (AGREES / DIFFERS).
- Format (clockinfra's): `circuit: flying-machine`, `port start:` kick, `travel: dx dy dz`,
  `distance: k`, `port payload[.n]:`, `stop: x y z <state>` (any number; not counted).

## Results (all real-game robust, 16 builds)
| design | kind | blocks | gt/block | dir | how |
|---|---|---|---|---|---|
| [fly-d1](fly-d1.txt) | one-way | 7 | 10 | horizontal | rear = normal piston X + 2 slimes + observer A; front = observer B + slime + sticky piston Y facing back. X pushes the front 1 cell; B, landing, fires Y, which reach-pulls the rear slime row (X and A dragged); A, landing, fires X. Stops intact against a wall in front of either row (`stop:`). |
