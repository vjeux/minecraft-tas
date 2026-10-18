# Track mechanics: gadgets from the 26.3 source, applied to the door

## Gadget library (designs/gadgets/, each proven by a drive script)

`mctas door drive --quiet designs/gadgets/*.drive` runs them all (PASS/FAIL). See docs/mechanics.md.

## Results

| blocks | file | how |
|---|---|---|
| 91 | mech-91.txt (from v2-min 115 via ta.patch) | A torch moved next to the observer column (T_A) |

### 91: T_A (ta.patch on v2-min)

v2's A torch sat on the far left, fed by a 16-block climb from the sides line (A early) and a
12-block chain from the P_T net (A late). Now:

* The lever sits on top of K=(4,5,0); O2 watches the lever itself (W gone).
* T_A, a wall torch on K's west face at (3,5,0), feeds a dust X2 under it at (3,4,0) that joins
  X, so A gets T_A's steady power through X in both of its positions (the observer pulses still
  arrive through X). X2's floor (3,3,0) is glazed terracotta: it sits on the right backbone's top
  slime and the slime does not drag it (gadget slime-nonstick).
* A late: one repeater (4,5,1) from the P_T net into K, +4gt after P_T. No dust touches K: a
  strongly powered K would back-feed the lever net (and latch the late chain).
* The lever net is fed by a repeater reading the lever (one-way), then 3 dust down to (7,4,2).
* P_T's L_8 feed moved to (5,4,2) (delay 2) into a block (4,4,2) whose dust (4,5,2) is the A-late
  repeater's input.

Timing: B window starts at 10 (O2 watches the lever directly), P_T retracts at 6.
