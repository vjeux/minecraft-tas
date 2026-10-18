# logic-min: logic minimization of v2's control

Results (all verified `door verify --extra-offsets 28`; BEST and BEST-slime-free also pass
`--extra-offsets 100` and with main's binary):
| file | blocks | note |
|---|---|---|
| lm1.txt | 60 | first pulse-driven v2 (`door gen lm1`, crates/piston-door/src/lm.rs) |
| lm1-59.txt | 59 | minimizer: P_L chain's first link watches the bottom-middle cell |
| lm2-54.txt | 54 | P_T NOR collapsed: OR = one dust sandwiched between R1's and R_P's output blocks |
| lm2-53.txt | 53 | P_T torch moved next to P_T (2-dust OR net) |
| lm3-52.txt = **BEST.txt** | **52** | left column fired by a 5-observer chain watching P_T (P_L on the top row) |
| sf-55.txt | 55 | slime-free: 3 pulse-driven pistons per side column |
| sf-53.txt = **BEST-slime-free.txt** | **53** | slime-free + the P_T-watching chain (leads the slime-free board) |
Each step is a `door patch` script in this folder (*.patch), applied to the previous file.

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
on close (block 2 ahead is moving) - verified for P_L, P_R, P_BM (whole slime column) and A
(gadget designs/gadgets/pulse-toggle).
- S pistons: one 2gt pulse per edge, any time (s 0..14, s2 0..30) -> no torches, no steady line.
- A: no steady power at all. Pulses: p0 at 0..4 after each edge (lifts T on open; on close it
  pre-pushes C one cell, harmless) + the two B-edge pulses. On close the B-off pulse is the one
  that pushes T into the door (so A's late chain disappears).
- P_T: the only steady signal: off at 6..11 on open, on EXACTLY when B retracts on close
  (pt_on = b+bw; +1 with a 3-tick X pulse).
=> one observer on the lever drives S, A's p0 and the B chain; everything that changes state
once per edge (the lever, P_T, a moving door stone, another observer) can fire the S pistons.

## The 52-block control (BEST.txt), t = game ticks after the flip
- Lever (5,4,0) sits on O_R (observer facing up): pulse t=2 into K_R obsidian (5,2,0) -> P_R
  (5,1,0) below it; obsidian because it touches the right backbone's open cell.
- O_a (4,4,0) watches the lever (t=2) -> stone (3,4,0) -> X dust (2,4,0): A's first pulse (lifts T).
- O_b (4,5,0) watches O_a (t=4) -> block -> dust -> R_B (2,6,0, d4): B window 12..20 via QC
  (1,6,0); O (2,5,0) watches R_B -> X: A pulses at 14 and 22 (pull C twice / spit C, push T).
- P_BM: O_m (2,-1,0) watches the right column's bottom stone -> block (2,-2,0) -> P_BM.
- P_L (-3,2,0): observers (0,3,2) [watches P_T] -> (-1,3,2) -> (-2,3,2) -> (-3,3,2) -> (-3,3,1),
  whose back gives signal into P_L's QC cell (-3,3,0) (with the update P_L needs).
- P_T torch (2,3,2) = NOT(OR net): R1 (lever + d4: on at 8) -> block (5,4,2) above the OR dust
  (5,3,2); R_P (K_R pulse + d4: window 10..18) -> block (5,2,2) under it; OR dust -> dust (4,3,2)
  -> torch attachment (3,3,2). Open: torch off at 10 (< B at 12). Close: R1 falls at 8, R_P rises
  at 10 (2-tick gap survives because the repeater's tick has priority over the torch's), R_P
  falls at 18 -> torch on at 20 = the tick B retracts.
Block budget: mechanism 21 (9 stone, 6 slime, 6 pistons) + lever/O_R/K_R 3 + X/floor/J_X 3 +
B chain 7 + P_T 11 + P_BM 2 + P_L chain 5.

## Timing chain constraints (why q = 4, R1 = d4, R_P fed at 2)
B window [q+8, q+16] (R_B d4 fed by a pulse at q); P_T on must equal B off -> R_P fed at q-2;
P_T off on open (R1+2) must precede B (q+8); close gap R_P rise - R1 fall <= 2. With one
repeater R1 <= 8, so R1 = 8, q = 4, R_P fed at 2.

## Failed / notes
- Observer watching a piston (or its head cell) fires TWICE per motion (extend, retract, land
  2 ticks apart) -> a slime-free column toggled twice. Use single-change sources (an observer
  watching a stone that moves, the lever, another observer, P_T).
- Isolated dust placed as plain `redstone_wire` canonicalizes to a DOT, which does not power
  sideways: always write `redstone_wire[north=side,east=side,south=side,west=side]`.
- Torch output block next to its own OR dust = clock (lm2a).
- R_B pointing into a block (instead of air above B) leaves B without the update it needs.
- P_T cannot be pulse-driven: any symmetric pulse source fires at the wrong edge too (the open
  pull must come before B, the close push exactly at B-off).
- Lever at (4,4,0) (O_a's back straight into X, O_b straight into R_B's block: -3) needs O_R at
  (4,3,0), which the right backbone would drag; in the slime-free door it works for R2/R1 but R0
  and R_P then need 4 more blocks (net 0).
- Silent (QC, no-update) P_T: no neighbor of P_T changes at close+20, so it never re-extends.
- Minimizer (`--moves all --plateau --plateau-pairs`, `--walk 200`) finds nothing below 52/53.

## Next ideas
- A layout search (place lever/O_R/R1/R_P/K/torch with adjacency + parity constraints, judged
  by the simulator) for the P_T cluster: the OR wiring is 5 blocks, 2 is the parity minimum.
- Apply the pulse-driven pistons and the P_T-watching chain to the rb family (record 47).
