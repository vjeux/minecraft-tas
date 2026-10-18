# Track wire21: wiring mech-synth's 21-block pulse-only mechanism (abpt21r family)

Mechanism: mech-synth's catalog m05 (= abpt21r with the T-mover at (1,3,1) facing north, so T
leaves into z=-1 and (0,3,0)/(0,4,0) stay free for fc2's A driver). 21 blocks, driven by 2gt
pulses only: P_L, A1, P_T, B window, A2, A3, M (>= A3), P_R, identical on both lever edges.

## Best: 40 blocks (ties the record), open 40 / close 38 (`BEST.txt`)
21 mechanism + 19 control:

| part | blocks | how |
|---|---|---|
| lever + P_L | 3 | lever on observer o (-3,4,0) watching it; observer (-3,3,0) watching o fires P_L's quasi cell (-3,2,0) at t4 (fc2) |
| A1 | 2 | O_a (-1,4,0) watches the lever, t2 into smooth stone (0,4,0) next to A |
| B window | 3 | O_b (-1,5,0) watches O_a (t4) -> stone J -> repeater delay 4 (0,6,0): window t12..20 into B's quasi cell (1,6,0) (fc2) |
| A2/A3 | 1 | O (0,5,0) watches the repeater: t14, t22 into (0,4,0) |
| P_T | 2 | O_c (-1,4,1) watches O_a (t4), O_d (0,4,1) watches O_c (t6) into P_T's quasi cell (1,4,1) |
| P_R | 6 | observer (2,5,0) watching B (t14, t22) -> observer (2,4,0) -> observer (3,4,0) -> smooth stone (4,4,0) -> LAMP (5,4,0) -> observer (5,3,0) into P_R (5,2,0) |
| M | 2 | O_m (2,-1,0) watches the right column's bottom block, fires P_M through smooth stone (2,-2,0) (fc2) |

### The two ideas that made it fit
* **Odd pulse counts.** A pull-type pulse piston toggles on every pulse, so 3 pulses per edge =
  one net move, timed by the LAST pulse. The lamp (lights at once, goes dark 4 gt after its
  power ends) turns the 2 B-watcher pulses into 3 P_R pulses, the last one late (~t34). O_m then
  fires M three times too, all after A3 on close: push M,C,T home, pull M, push M. This is how the
  "single late pulse at the bottom" (the crux of every abpt21r wiring) is avoided.
* **The A driver needs no dust.** `door minimize` replaced fc2's X dust + floor by one smooth
  stone at (0,4,0): the observer (2,4,0) of the P_R chain fires into (2,3,0), which updates A in
  its low position.

## Chain-tick windows (ideal/chain.ideal: a variable = tick an observer turns on; observer k of a
chain from the lever = 2k)
a1 1..2, pt 6..7 (>= a1+4), bon 11..12 (>= pt+5), a2 14..15, boff 19..20, a3 22..24 (<= m),
m >= 22 (= a3 allowed), pl/pr anywhere (single pulses).

## History
| design | blocks | open/close | how |
|---|---|---|---|
| top1.txt (+ideal M, P_R) | 34 | - | fc2's top control on m05 + O_c/O_d + fc2's P_L observers |
| full1.txt | 43 | 26 / 24 | + 7-observer chain to P_R (t20) + O_m (M at t22) |
| full1-min.txt | 42 | 26 / 24 | minimize: lever hung on the P_L observer |
| synth/synth-6-*.txt | 41 | 40 / 38 | `door synth` (k=6) replaced the 7-chain by a lamp-based 6-block P_R driver |
| BEST.txt (s60-min) | 40 | 40 / 38 | minimize: X dust + floor -> one smooth stone |

## Searches that found nothing smaller (door synth, door judge as the target)
* P_R driver with the rest fixed (40 context minus the 6 driver blocks): k <= 5, palette
  obs,block,dust,rep1-4,cmp,torch,lamp,target,gt,rblock,obsidian (+ copper bulb, added to synth's
  palette as `bulb`), box x -1..7, y 0..8, z -3..3: none. Keeping the B-watcher and (2,4,0):
  only 4-block completions (= 40).
* 42-context (X dust) P_R driver: obs-only k <= 6 none; wide palette: first hits at k = 6 (lamp).

## What did not work / why
* m10/m11 (M first, T-mover riding a side structure): the riding T-mover needs both of its
  positions powered by BOTH of its pulses (positions swap between open and close).
* A single late pulse for M (>= t22 on close): every cheap late source near the elevator fires
  twice per edge (observer on the window repeater or on B: t14, t22); a repeater lock or
  comparator gate needs the window repeater's front, which is B's quasi cell; a lever-level delay
  line costs 3 repeaters. The odd-count trick sidesteps it.

## Tools
* `door synth --palette bulb`: waxed copper bulb (T flip-flop), modeled like a lamp.
EOF
