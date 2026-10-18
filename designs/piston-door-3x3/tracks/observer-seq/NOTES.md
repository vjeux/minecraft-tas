# observer-seq track: event-driven sequencing

Best verified design: none below the record (51 at the time of writing). This track produced
mechanisms and sequencing rules validated with ideal pulses, a drive change log, and a set of
traps; see "Next ideas" for what to build on them.

## Finding 1: every piston can be a retracted-at-rest TOGGLE (verified, wip/toggle-mech*)
All six pistons retracted at rest; each action is ONE 2gt observer pulse. The pulse PULLS when
the head extends into air, and PUSHES + SPITS when it pushes a block (the observer turns off in
the scheduled-tick phase while the pushed block is still a moving_piston -> retract type 2).
No piston needs a held power state, so no torches, no late holds:
- sides (outer P_L/P_R or inner pushers) and P_BM: open pulse / close pulse;
- A retracted at rest at (1,4,0): A1 = pull T up to (1,3,0);
- P_T at (1,3,1) facing north, retracted: spits T forward to (1,3,-1); close pulse pulls it back;
- B: pulse 1 spits A down to (1,3,0), pulse 2 pulls it back up.
Opening = A, P_T, B, A, B, A. Closing = the EXACT reverse, every step a toggle of the same
piston (A pushes C to (1,2,0), B, A pushes C to (1,1,0), B, P_T, A pushes T down).

## Finding 2: the CLOSING needs no B at all (verified, wip/toggle-window*, wip/e1m-drive.txt)
A pushes C from (1,3,0) to (1,2,0) (spit), T comes back into (1,3,0), then ONE more A pulse
pushes T and C down together (a push moves 2 blocks, a pull only 1). Closing = A, T-in, A.
Only the opening needs the UP dance (B window).

## Finding 3: e1 mechanism, toggles + the rb tricks (verified with ideal pulses, @obs-e1m)
`crates/piston-door/src/observer_seq.rs::e1_mech` (26 blocks of mechanism):
- P_L (0,3,0) faces west into the left structure's top slime (-1,3,0) and SHARES (0,4,0) with A:
  one lever-edge pulse into (0,4,0) does {P_L, A} on both edges (A1 opening, A' closing);
- the right structure's slime (2,3,0) drags T out of (1,3,0) when P_R (2,4,0) pushes (no P_T);
- P_BM is a toggle pulsed by the left column's bottom observer (0,-1,0) through (0,-2,0)
  (no extra slimes, no redstone block, no wiring);
- rb2's window (R_B (1,6,1), O (1,5,1) on R_B, X dust (1,4,1) on (1,3,1)).
Ideal timeline that opens and closes it cleanly (`wip/e1m-drive.txt`):
open: t0 pulse (0,4,0); t8 pulse P_R; t16 window. close: t0 pulse (0,4,0); t8 pulse P_R;
t16 pulse A (pushes T+C home). Note the opening's t16 A pulse is harmless, so ONE source can
fire "A" at t16 in both directions; only the window must be opening-only.

## Event triggers that work (in the simulator)
- An observer watching a door column's top or bottom stone ((0,3,0) down on (0,2,0), (0,-1,0)
  up on (0,0,0)) fires exactly once per column move, in both directions; it is not dragged.
- A "park" observer watching the cell T is parked in fires once per direction (T in, T out).
- An observer watching a TOGGLED piston base double-pulses (ext at t, moving at t+2, placed at
  t+4 after the observer already turned off): fatal for toggles, but usable when the first
  pulse is blocked by the still-moving block (e.g. A blocked by a moving T) and the second one
  does the job. A HELD piston base (window >= 4gt) gives exactly one pulse per edge.
- Blocking as gating: a piston pulse is harmless when its push line holds a moving_piston at
  the block-event phase (B against a T still sliding in), or when it extends into air with
  nothing to pull.

## What failed and why
- Window lock with a repeater on R_B: O watching R_B fires on lock/unlock (state change). Lock
  an upstream repeater, or let O watch B instead.
- X dust fed from the north by a repeater: the dust connects to the repeater and stops pointing
  into A. X must connect only east/west (or be a dot).
- Inner side pushers on top of the door (P_L (0,3,0), P_R (2,3,0)) take the cells v2's X uses;
  inner pushers below the door need a long lever net to the bottom. Mixing (one top, one
  bottom) leaves the bottom observer able to reach only one shared conductor.
- Repeater delay >= 2 stretches a 2gt pulse: no push+spit after it. Delay chains for toggles
  must be delay-1 repeaters or observer chains (2gt each).
- The only direction discriminators are the lever level and the positions of moved parts
  (structures, T, M). Every event fires in both directions, so each opening-only step (the B
  window) needs a gate; with a torch lock the lever must sit next to R_B, which fights the
  lever-edge observer that must output into (0,4,0). That conflict is why no full e1 was wired.

## Next ideas
- Wire e1 with a structure gate instead of a lock: the window's feed passes through a block
  glued to the right structure that is in place only when the structure is out (opening).
  Estimated 46-49 blocks if the geometry closes.
- rb2 family: the closing does not need its B window if A gets one closing-only spit pulse before
  Q returns RB_A; then Q's 24gt late hold becomes ~4gt (and the window in the closing is harmless
  because A is extended and immovable). Needs a falling-edge-only pulse into A.

## Tools
- `door drive`: `box x0 y0 z0 x1 y1 z1`, `changes N` (per-tick 3D block change log), `mark`.
