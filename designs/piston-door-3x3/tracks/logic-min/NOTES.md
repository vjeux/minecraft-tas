# logic-min: logic minimization of v2's control

Results (verified `door verify --extra-offsets 28`, also with main's binary; BEST files also
pass `--extra-offsets 100` and `--cycles 6` with main's torch-burnout fix):
| file | blocks | note |
|---|---|---|
| lm1.txt | 60 | first pulse-driven v2 (`door gen lm1`, crates/piston-door/src/lm.rs) |
| lm1-59.txt | 59 | minimizer: P_L chain's first link watches the bottom-middle cell |
| lm2-54.txt | 54 | P_T NOR collapsed: OR = one dust sandwiched between R1's and R_P's output blocks |
| lm2-53.txt | 53 | P_T torch moved next to P_T (2-dust OR net) |
| lm3-52.txt | 52 | left column fired by a 5-observer chain watching P_T (P_L on the top row) |
| lm4-50.txt | 50 | R_P reads O_a's output block J_X and points straight into the torch's attachment |
| lm6-49.txt = **BEST.txt** | **49** | P_R fired through its QC cell by the lever's observer (no relay block) |
| lm7-50-clean.txt | 50 | 49 with P_L back on the bottom chain: the door does not re-move while closing |
| sf-55 / sf-53 / sf-51 | 55/53/51 | slime-free: 3 pulse-driven pistons per side column, same steps |
| sf-50.txt = **BEST-slime-free.txt** | **50** | slime-free, O_R fires straight into the right pistons' dust |
| sf-52-clean.txt | 52 | slime-free without the close-time re-move |
Each step is a `door patch` script in this folder (*.patch), applied to the previous file.

Caveat of 49 / 50 (and lm4-50, sf-51, sf-50): X's last pulse re-fires R_P (it reads J_X, which
X back-feeds), so on close P_T retracts and re-extends once more (t=32..40) and the left column,
watching P_T, opens and closes once more (t=42..50): final state exact, "closed after 52 ticks".
The *-clean variants avoid the visible re-move for +1 / +2 blocks.

## Tool: `door ideal` (crates/piston-door/src/ideal.rs)
Runs a mechanism-only design with ideal inputs (redstone blocks set/cleared by a schedule at
given ticks after each flip), judges it like the verifier, and sweeps schedule variables
(`--sweep name=lo..hi`, grid or `--independent`). Files: ideal-mech.txt + ideal.sched (v2 as is),
ideal-mech-pulsed.txt + ideal-pulsed.sched (pulse-driven variant).
Also `door drive`: `design <path> dx dy dz` (world offset) and `settle [max]` (until idle).

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

## The 49-block control (BEST.txt), t = game ticks after the flip
- Lever (5,4,0) on O_R (observer facing up): its back faces the air cell (5,2,0), P_R's
  quasi-connectivity cell: P_R (5,1,0) gets the t=2 pulse and the update through it.
- O_a (4,4,0) watches the lever (t=2) -> J_X stone (3,4,0) -> X dust (2,4,0): A's first pulse.
- O_b (4,5,0) watches O_a (t=4) -> block -> dust -> R_B (2,6,0, d4): B window 12..20 via QC
  (1,6,0); O (2,5,0) watches R_B -> X: A pulses at 14 and 22 (pull C twice / spit C, push T).
- P_T torch (2,4,2) on K (3,4,2), powering P_T through its QC cell (1,4,2):
  R_P (3,4,1, d4) reads J_X (t=2 pulse): window 10..18 straight into K;
  R1 (5,4,1, d4) reads the lever: block (5,4,2) on at 8 lights dust (4,4,2) pointing into K.
  Open: torch off at 10 (< B at 12). Close: R1 falls at 8, R_P rises at 10 (the repeater's tick
  outranks the torch's), R_P falls at 18 -> torch on at 20 = the tick B retracts.
- P_BM: O_m (2,-1,0) watches the right column's bottom stone -> block (2,-2,0) -> P_BM.
- P_L (-3,2,0): observers (0,3,2) [watches P_T] -> (-1,3,2) -> (-2,3,2) -> (-3,3,2) -> (-3,3,1),
  whose back gives signal into P_L's QC cell (-3,3,0) (with the update P_L needs).
Block budget: mechanism 21 (9 stone, 6 slime, 6 pistons) + lever/O_R 2 + X/floor/J_X 3 +
B chain 6 (O_a counted in X) ... = 21 + 28.

## Timing chain constraints (why q = 4, R1 = d4, R_P fed at 2)
B window [q+8, q+16] (R_B d4 fed by a pulse at q); P_T on must equal B off -> R_P fed at q-2;
P_T off on open (R1+2) must precede B (q+8); close gap R_P rise - R1 fall <= 2. With one
repeater R1 <= 8, so R1 = 8, q = 4, R_P fed at 2.

## Failed / notes
- Observer watching a piston (or its head cell) fires TWICE per motion (extend, retract, land
  2 ticks apart) -> a slime-free column toggled twice. Use single-change sources.
- Isolated dust placed as plain `redstone_wire` canonicalizes to a DOT, which does not power
  sideways: always write `redstone_wire[north=side,east=side,south=side,west=side]`.
- Torch output block next to its own OR dust = clock (lm2a).
- R_B pointing into a block (instead of air above B) leaves B without the update it needs.
- P_R right under O_R with the lever on top: the lever QC-powers P_R (level) -> stuck extended.
- P_T cannot be pulse-driven: any symmetric pulse source fires at the wrong edge too.
- Lever at (4,4,0) (O_a straight into X, O_b straight into R_B's block: -3) needs O_R at (4,3,0),
  which the right backbone would drag; slime-free: R0 and R_P then cost 4 more (net 0).
- Silent (QC, no-update) P_T: no neighbor of P_T changes at close+20, so it never re-extends.
- Simulator bug found and reported (fixed on main): Sandbox.clear() reset game time but kept
  RedstoneTorchBlock.RECENT_TOGGLES, so torches burned out in long multi-offset runs.
- Minimizer (`--moves all --plateau --plateau-pairs`, `--walk 200`) finds nothing below these.

## Next ideas
- B chain (6-7 blocks): a 4-tick pulse source adjacent to R_B's back would save 2.
- A layout search for the lever / O_R / O_a / R1 / R_P cluster (the parity argument shows
  where relays are unavoidable; the simulator judges candidates).
- Apply pulse-driven pistons, QC-through-air firing and the P_T-watching chain to the rb family.

## rb family (rb6-bmobs-47), measured with `door ideal` (rb-ideal-mech.txt + rb-ideal.sched)
Ideal ticks (event after t ticks = piston acts at rel t+1), one variable at a time:
- H (left pusher): any time on both edges (h1 0..12, h2 0..30): free; in rb47 it already costs 0
  (QC from the lever block, closed by A's update at rel 10).
- P_BM pulse: open 4..12, close 16..30: free.
- P_R: open p1 3..9 (needs A released >= 3 ticks before), close p2 17..19 or 21..23 only.
- A: release a1 = 0 (with p1 = 3); re-power a2 >= 18.
So rb has the same asymmetric requirement as v2's P_T: P_R early on open, at B-off on close.
rb47 pays for it with F + R (shared with A) + the 7-block hold; the hold must keep F powered
through A's updates at close rel 10 and 12 (F's head touches A) and release before rel 18, and
it must stay silent (its output is not F's neighbour) so F retracts on A's landing update.
Tried / ruled out:
- Hold with D pointing into the lever's block (6 blocks): D would be Rb's floor (a repeater
  cannot carry a repeater) -> Rb pops. Every placement of "Rb reads the lever, D points at F's
  QC cell" collides the same way or needs a longer route.
- One-repeater hold (lever+8): ends at rel 8, F then retracts on A's rel-10 update.
- Pulse-toggled P_R / F: needs one pulse per edge, early on open and at B-off on close; every
  cheap pulse source (lever observer, X observer, B, the centre cell) is symmetric.
- P_R = R1 OR R_P without a torch: a 2-tick gap on close (R1 falls at 8, R_P rises at 10) and
  P_R is A's neighbour, so it is updated inside the gap.
- Bridge to C (O -> block -> dust -> block): C's input cell is never adjacent to a cell an
  observer of the lever can output into without moving the lever, which feeds F and H.
No rb variant below 47 found.
