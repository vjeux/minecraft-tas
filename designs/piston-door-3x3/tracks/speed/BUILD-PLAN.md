# Build plan: fastest solid door (track speed)

Read NOTES.md first (theory + ideal results). Working ideal: `ideal-6.txt` + `ideal-6.sched`
(`mctas door ideal designs/piston-door-3x3/tracks/speed/ideal-6.sched` -> WORKS, open 12 /
close 7, with scripted redstone blocks). A' is already driven by a real observer chain.

Ticks below are after the lever flip (t1 = first tick). Coordinates: door cells x 0..2, y 0..2,
z=0; E=(1,3,0); N1=(1,2,0); walkway x 0..2, y 0..2, z -3..3 (keep it collision free).

## Parts (as in ideal-6)
* F (1,4,0) sticky facing down, extended at rest, head in E. Pulls T into E at t1 (open).
* A' (0,3,0) sticky facing down, retracted. Q (-1,3,0) sticky facing east, retracted.
* T-returner TR (3,3,0) sticky facing west, retracted. T rests at (2,3,0) when open.
* Sides + M: sticky pistons extended at rest that pull their door block out (ideal-6 uses one
  piston per block: x=-2 and x=4 facing in, M piston (1,-2,0) facing up).
* Observer chain O_L (1,3,3) -> O_B (1,3,2) -> O_A (1,3,1), lever at (1,3,4): O_A powers E
  directly at t6..t8 on both edges (A' extends at t7, retracts t8 pulling C into N1).

## Signals still scripted, to wire (open = lever on)
| piston | open | close |
|---|---|---|
| F | unpowered from t1 (instant: redstone-block shuttle, a lever-powered piston shoves the block that powers F/sides/M) ; a reach-through pulse after Q pulled A' out (extend when E clears, retract 1+ tick later: pulls C into E) | push C from E into N1 and spit it (extend, retract while C still moves), later steady on again once T is in E (pushes [T, C] down) |
| Q | on t4 (push [A', T]), off at t11/t12 (pull A' back once A' landed at t10) | must stay retracted (a push on close is blocked if E holds a moving block or a piston head) |
| TR | off | push T from (2,3,0) into E after F's head left E, spit it |
| sides, M | off at t1 | on (push back) |

## Hard-won rules
* A piston queues an extend only if its push resolves at that moment; no retry until updated.
* A' at E is quasi-powered through F's cell: anything signalling INTO (1,4,0) powers A' at E.
  Power F through its own QC cell instead: keep (1,5,0) AIR and point a repeater/comparator into
  (1,5,0) (F updated on both edges, A' at E not powered).
* Block events re-check power when they run: a power gap inside one scheduled-tick phase is
  harmless.
* Q's head forwards neighbor updates: A's retract (t8) and landing (t10) both update Q.
* Pulse generator without dust order issues: comparator in subtract mode, back = lever via
  repeaters (delay a), side = lever via repeaters (delay b > a): output on [2a+2, 2b+2) on the
  rising edge only; swap the roles for a falling-edge (close-only) pulse.
* Observers fire 2gt after a shape change, for 2gt; a chain of side-watching observers is a
  position-independent delay line on both edges.
* Redstone-block shuttle: moving it switches its old neighbors off in the same tick (t1);
  its new neighbors switch on when it lands (events at t4).
