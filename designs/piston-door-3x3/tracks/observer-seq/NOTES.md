# observer-seq track: event-driven sequencing

## Finding 1: every piston can be a retracted-at-rest TOGGLE (verified with ideal pulses)
`wip/toggle-mech.txt` + `wip/toggle-mech-drive.txt`: all six pistons retracted at rest, each
action is ONE 2gt observer pulse. The pulse PULLS when the head extends into air, and PUSHES +
SPITS when it pushes a block (the observer turns off during the scheduled-tick phase while the
pushed block is still a moving_piston -> retract type 2). So no piston needs a held power state:
- sides P_L/P_R and P_BM: open pulse = pull, close pulse = push+spit;
- A (1,4,0) retracted at rest: A1 pulls T up to (1,3,0);
- P_T at (1,3,1) facing north, retracted: spits T forward to (1,3,-1); close pulse pulls it back;
- B: pulse 1 spits A down to (1,3,0); pulse 2 pulls it back up.
Opening = A, P_T, B, A, B, A. Closing = the EXACT reverse, each step a toggle of the same piston:
A (push C to (1,2,0)), B, A (push C to (1,1,0)), B, P_T, A (push T down).
Both lever edges start with {sides, M, A}. Full cycle returns to the exact rest state.

Cell change timeline per step (from `door drive` `changes`): a piston base changes 3 times
(ext, moving, retracted) -> an observer watching a base double-pulses; a watched moved-block
cell gives one clean pulse.

## Tools
- `door drive`: `box x0 y0 z0 x1 y1 z1`, `changes N` (per-tick block change log in 3D), `mark`.
