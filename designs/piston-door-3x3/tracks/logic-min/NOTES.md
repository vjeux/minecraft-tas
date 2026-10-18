# logic-min: logic minimization of v2's control

## Tool: `door ideal` (crates/piston-door/src/ideal.rs)
Runs a mechanism-only design with ideal inputs (redstone blocks set/cleared by a schedule at
given ticks after each flip), judges it like the verifier, and sweeps schedule variables
(`--sweep name=lo..hi`, grid or `--independent`). Files: ideal-mech.txt + ideal.sched (v2 as is),
ideal-mech-pulsed.txt + ideal-pulsed.sched (pulse-driven variant).

## Measured requirements (ideal ticks: event applied after t ticks = piston acts in frame t+1)
v2 as built (S, A, P_T steady; B window b..b+8; X pulses at B edges +2):
- S (P_L, P_R, P_BM): ANY time on both edges (s_off 0..12, s_on 0..40 all pass).
- open: a_off <= pt_off-3, pt_off <= b (same tick OK only if P_T's event is queued first).
- B: bw in {7,8,9}; X pulse at B edge + {2,3}, width {2,3}.
- close: pt_on in [b+bw, b+bw+2] or [b+bw+5, a_on]; a_on >= b+bw+5.

## Key finding: every piston can be PULSE-driven (retracted at rest)
A 2gt pulse on a retracted sticky piston pulls on open (block 2 ahead is still) and pushes+spits
on close (block 2 ahead is moving) - verified for P_L, P_R, P_BM (whole slime column) and A.
- S pistons: one 2gt pulse per edge, any time (s 0..14, s2 0..30) -> no torches, no steady line.
- A: no steady power at all. Pulses: p0 at 0..4 after each edge (lifts T on open; on close it
  pre-pushes C one cell, harmless) + the two B-edge pulses. On close the B-off pulse is the one
  that pushes T into the door.
- P_T: the only steady signal: off at 6..11 on open, on EXACTLY when B retracts on close
  (pt_on = b+bw; +1 with a 3-tick X pulse).
=> one observer on the lever can drive S, A's p0 and the B chain.
