# Track wire21: wiring mech-synth's 21-block pulse-only mechanism (abpt21r family)

Mechanism: mech-synth's catalog m05 (= abpt21r with the T-mover at (1,3,1) facing north, so T
leaves into z=-1 and (0,3,0) is free for fc2's X floor). 21 blocks, driven by 2gt pulses only:
P_L, A1, P_T, B window, A2, A3, M (>= A3), P_R, identical on both lever edges.

## Chain-tick windows (ideal/chain.ideal: a variable = tick an observer turns on; observer k of a
chain from the lever = 2k)
a1 1..2, pt 6..7 (>= a1+4), bon 11..12 (>= pt+5), a2 14..15, boff 19..20, a3 22..24 (<= m),
m >= 22 (= a3 allowed), pl/pr anywhere. fc2's control gives a1 = 2, B window 12..20 (O_b at 4 ->
delay-4 repeater), a2/a3 = 14/22 (observer on the window repeater) exactly.

## Results
| design | blocks | open/close | how |
|---|---|---|---|
| top1.txt (+ideal M, P_R) | 34 | - | fc2's top control on m05 + O_c/O_d chain for P_T (t6) + fc2's 2 P_L observers |
| full1.txt | 43 | 26 / 24 | + 7-observer chain O_d -> P_R (moved to (5,2,0)) firing at t20, O_m (fc2's motion trigger on the right column) fires M at t22 |

## The crux: M's single late pulse (>= 22 on close) at the bottom
Every cheap late source near the elevator fires twice per edge (observer on the window repeater:
14 and 22). Notes on what was considered are below in "Ideas".
