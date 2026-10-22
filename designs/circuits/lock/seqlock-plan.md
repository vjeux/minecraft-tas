# Button sequence lock: design plan (clk-item, work in progress)

Judge: `circuit: sequence-lock N L` (crate::seqlock). Mechanism being built:

- **One clock pulse per press, per button.** The button's dust lights a copper bulb (it toggles
  on rising edges only). A comparator reads the bulb, and an observer watches that comparator,
  giving one 2 gt pulse P_x per press 4 gt later. (An observer watching the bulb itself fires
  twice per press, because the bulb's power state changes on release.) A torch inverts it into
  NP_x (15 at rest, 0 during the pulse). NP_any comes from all buttons' pulses merged.
- **Token.** State s is held by copper bulbs. B_0 is LIT when the token is NOT at 0 (unlit at
  rest). B_1..B_L are UNLIT when the token IS at s (lit at rest). For every s, the reader R_s is
  therefore 15 exactly when the token is not at s: the same blocker polarity for every step.
- **Advance gate per (step s, button x).** G = subtract comparator:
  - rear: a composter at level 2 (constant 2);
  - side 1: a dust network carrying max(h_{s,x}, R_s);
  - side 2: NP_x.
  G outputs 1 only if the token is at s, the code of s is x, and x was pressed. h_{s,x} >= 2 iff
  c_s != x; with the frame reader's f = c + 1, h = max(f at dust distance x,
  composter(x+2) - f).
  - N=2, button a: h = f.
  - N=2, button b: h = 3 - f (a subtract comparator with a composter-3 rear).
  - G powers a solid block Q, which powers the next bulb B_{s+1} (toggle: the token arrives) and
    a repeater into the G-merge line.
- **Leaving step s** (s >= 1): TP_s = subtract(rear composter 2, sides R_s and NP_any) toggles
  B_s back to lit, through a block Q' that also feeds the TP-merge line.
- **B_0.** Toggled by the TP-merge at t (the token leaves s >= 1), and by the G-merge delayed
  4 gt (off in between). A correct press at s >= 1 toggles it twice (no change). A wrong press,
  or a press at L, toggles it once (the token is back at 0). The advance from 0 toggles it once
  (the token leaves 0).
- **Output.** A subtract comparator with a redstone block at its rear and R_L at its side: on
  while B_L is unlit (open).

Timing: a press at t; P at t+4..t+6; NP at t+6..t+8 (torch); gates out at t+8..t+10; bulbs
toggle at t+8. Readers lag 2 gt, so no cascade within one press. The next press comes at t+22
(stone button).
Primitive checked in redsim: bulb + reader + observer gives one pulse per press (/tmp p1b).

## Findings (redsim, 2026-10-21)
- A 2 gt pulse into a comparator (rear or side), or into a torch, is MISSED: the comparator
  evaluates 2 gt after the change, when the pulse has just ended (the observer's own off-tick
  runs first). Every pulse that a comparator must see is stretched to 4 gt by a delay-2 repeater
  right after the observer.
- Primitive p3 (/tmp/dsp/seq/p3.txt) passes: with code a only the a presses toggle bulb A,
  with code b only the b presses toggle bulb B. This checks the pulse, the inverter (a subtract
  comparator with a redstone block behind it) and the gates (composter-2 rear, f / 3 - f sides).

## Revised netlist (no B_0)
- Bulbs B_1..B_L, lit at rest; B_s is unlit while the token is at s; state 0 = all lit.
- The blocker of step s (s >= 1) is R_s, the reader of B_s (15 = no token at s). It reaches
  each gate's side-1 net through a one-way repeater, so the gates' h nets stay separate.
- TP_s (s >= 1): subtract comparator, rear composter 6, sides R_s and NP_any. Its block Q'_s
  toggles B_s (the token leaves s) and feeds the TP-bus through a repeater.
- The blocker of step 0 is the TP-bus (some token was at s >= 1), again through one-way
  repeaters. Step 0's NP inputs are delayed 4 gt so that they line up with the TP-bus.
- Output: B_L unlit.

## Generator status (2026-10-21 evening)

`circuit display seqlock` (crates/circuit-display/src/seqgen.rs) builds tiles joined by routed dust.
Routing fails for good topological reasons: chained buses (NP_a, NP_b, NP_any, R, TE) leaving
every tile in the same places cross each other in any single-layer channel. Fixes found on the
way (kept): settle the design without its gates and write it at rest; a button's dust must be
a cross; no dust may point into a block next to a bulb; multi-source nets start from one
source; a search path never stacks on itself; every target within 14 dust.

Next layout (no crossing by construction):
- Straight lanes through all tiles carry each button's toggle level (the reader of its pulse
  bulb), one booster per tile: lane A (y 1, z 3), lane B (y 1, z 5).
- Each tile taps a lane with an observer (one pulse per press, no electrical tap): B from its
  south side, A from above, and stretches it to 4 gt (delay-2 repeater). That gives PA and PB.
- Gates take the pulse at their rear: G_x = subtract(rear PA/PB, sides M_x, R_s'), TP_x =
  subtract(rear PA/PB, side R_{s+1}''). M_x = 15 when c_s != x (frame reader, then compare and a
  repeater). No composters or inverters at the gates.
- G_a and G_b (y 1) feed Q under B; TP_a and TP_b (y 3) feed Q' above it. R reads B from
  the north into K. The TP sides take K through delay-1 repeaters, and the next tile's
  blockers through delay-2 repeaters. TE = 15 - R chains west to tile 0's blocker.

## Compact architecture (coordinator 2026-10-21 19:58: "< 100 blocks for 2x3")

No per-tile taps, no boosters, no routing: three dust lanes from the button end, run straight
along the chain (L = 3, 4 keeps them under 15 dust).
- Per button, an edge detector: NP_x = max(torch on the button's block, button delayed 6 gt), which
  is 15 at rest and 0 for 4 gt after a press. P_any = OR over x of subtract(rear button, side
  button delayed): positive 4 gt pulses.
- Token: the unlit bulb B_s (s = 1..L); R_s reads B_s (15 = no token).
- Step s's code: F reads frame k_s into dust D (f = 1 for a, 2 for b). Za = subtract(rear
  composter 2, side D) = 2 - f into dust X. Zb = subtract(rear D, side X) = 2f - 2.
- G_a = subtract(rear X, sides NP_a, R_s) and G_b = subtract(rear Zb, sides NP_b, R_s) fire only
  in the window of the code's button with the token at s. They toggle B_{s+1}.
- TP_s = subtract(rear P_any, side R_s) fires on any press with the token at s and toggles B_s.
- Step 0's blocker: OR over s of TE_s = 15 - R_s (subtract, redstone block behind it).
- Output: R_L (15 = token at L) is the tap. Timing: a gate fires at w+4; the next step sees the
  token at w+8; the window ends at w+6.

### Compact layout: findings so far (paper)
- Best netlist: per step s and button x, T_x^s = subtract(rear composter 2, sides NP_x lane,
  R_s) = 2 iff (token at s, x's window). It powers block Q_x(s), which toggles B_s (departure) and
  is the rear of G_x^s = subtract(rear Q_x(s), side code_x) (code_a = f, code_b = 3 - f; f >= 1
  always, so G's arrival output of 1 into the next Q is harmless to the next G).
- B_s sits between Q_a(s) (y 1) and Q_b(s) (y 3). Chain along x per level: Q(s) G(s) [link]
  Q(s+1). The code for level b needs 4 - f: H with composter 4, then one climb from y 2 to y 3.
- Period 2 fails: G's side cells are also T's side cells. Period 3 with a dust link fails too: the
  link dust touches the next T's lane or R tap. Open: T on the far side of Q (z +1 vs -1
  alternating), or a comparator link whose sides are both inert.

### Chosen compact netlist (no T comparators, no constants but composter 2)
Per step s (frame f = 1 for a, 2 for b; lanes NP_a, NP_b: 15 at rest, 0 in the window; P_any:
positive window):
- F reads the frame into dust D. Za = subtract(rear composter 2, side D) = 2 - f into dust Zd.
- S_a = subtract(rear Zd, side NP_a) gives 1 in a's window when the code is a.
  S_b = subtract(rear D, sides NP_b and Zd) gives 2 in b's window when the code is b.
  Both feed dust SEL: positive only in the window of the code's button.
- G = subtract(rear SEL, side R_s) toggles B_{s+1} (R_s = 15 when the token is not at s).
- TP = subtract(rear P_any, side R_s) toggles B_s.
- Step 0's side: OR over s of TE_s = 15 - R_s.

## Sliding window: the inverted prefix automaton (2026-10-21, chosen)

Latch j holds T_j = NOT "the last j presses equal code[0..j)" (15 = inactive; written ON, locked
at rest). On each press all latches unlock for 6 gt (latch delay 4 gt: d < w < 2d, so no
inter-stage repeater) and take T_{j+1} = T_j OR mismatch_j, with T_0 = 0. Pure dust OR: no gates.
- mismatch_j = mA OR mB, from the frame comparator F -> dust D (f) -> dust D' (f - 1, free decay):
  - mA = subtract(rear D', side NBA) = code b and a held;
  - mB = subtract(rear composter 1, sides NBB, D') = code a and b held.
  NBA and NBB are torches on the button blocks (15 unless held). mA and mB output 1, so they point
  straight into latch j+1's input dust.
- Output = subtract(rear composter, side T_L).
- Clock: Ba -> R1 -> Kp <- R2 <- Bb; torch1 on Kp; torch2 on the block above torch1; a delay-2
  repeater after torch2; lock = OR(torch1, that repeater): low 6 gt, never on release.
- Estimate: about 10 parts per stage plus supports. Lanes are the cost driver, so lay them along x
  with mA and mB facing along x (sides on the lane rows).
- Open geometry issue: mA and mB must both point into the same input cell.
