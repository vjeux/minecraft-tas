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
