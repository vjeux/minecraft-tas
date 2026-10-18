# Track wire19sf: wiring mech-synth's 19-block slime-free mechanism (pist19r)

Goal: a slime-free door below 44 (`archive/slime-free-s7q-44b.txt`) from mech-synth's `pist19r`
(19 mechanism blocks: door 9, every edge block on its own pull-type sticky piston, M down, A/B
elevator, P_T), driven by ONE pulse train identical on both lever edges.

| file | blocks | note |
|---|---|---|
| w45.txt = BEST.txt | 45 | first wired pist19r: open 28 / close 26, 32 positions |

## Timing windows of pist19r (ideal inputs, `door ideal --independent --sweep`)
`pist19r-window.ideal` (B on two pulses) and `pist19r-bwin.ideal` (B on one level window), ticks
after the first A pulse (a1 = 0):
* pulses: a1 0..3, pt 3..9, b1 9..16 (>= pt+3), a2 14..21 (>= b1+2), b2 21..26 (>= a2+3),
  a3 28..36 (>= b2+4), **m >= a3** (m earlier: M, C, T come home before A3, which then lifts T on
  close), left and right columns ANYWHERE (0..50).
* B as one window (what a repeater gives): window >= 7 (b_off >= a2+5) with a2 = b_on+2 and
  a3 = b_off+2 (`--sweep boff=13..17 --sweep a3=15..20`), pt = a1+4 with b_on = a1+8 works.
* With s7q's north cluster for A/B (`w19-cluster.txt`: lever on O_a, X dust + floor, O_b -> J ->
  R_B d4 -> B's quasi cell, O on R_B -> X): A1 t2, B window t12-20, A2 t14, A3 t22 (real ticks);
  P_T must fire at real t5..9 and **M at real t >= 22** (`w19-cluster.ideal` sweeps).

## w45 (45 blocks): the wiring
* A/B: s7q's north cluster, 8 blocks with the lever.
* P_T (-1,3,0): 4-observer chain off the lever into its quasi cell (-1,4,0): t8.
* Right side = s7q's: TR goes UP (puller (2,4,0), fed off O_a by (2,4,-2) -> (2,4,-1)), the right
  pair (4,0..1,0) fired by an observer (3,2,0) watching TR's cell + block (4,2,0). (So the
  mechanism is pist19r with TR pulled up instead of east: still 19.)
* Left column (3 pistons): stack-feed (cross dust (-2,2,-1) on (-2,1,-1)) fed by a 4-observer
  chain continuing the P_T chain: t16.
* M: observer (0,-1,0) watching BL + 3 observers into M's piston: t22 on close (t24 open).
* Budget: 19 mechanism + 26 control.

## Lessons
* **Door-block watchers are asymmetric**: a pull-type piston fired at t moves its block at t+2 on
  open (extend, then pull) but at t on close (push), so an observer watching the door block fires
  2 gt EARLIER on close. M (must be >= A3 on close) through BL's watcher needed 2 more observers
  than the open timing suggests (w43 attempt: M at t20 on close, A3 then lifted T).
* **Observer-chain parity**: along a chain, (cell parity + t/2) is invariant, so the time at
  which a chain can fire a given cell is fixed mod 4; a stack-feed dust reached at t12 cannot be
  reached at t14 by observers (a repeater or a different target cell flips it).
* An observer pointing into the cell above A (from (1,4,-1)) powers A in both positions (A low
  through B's head, by quasi-connectivity, and it updates A); unused so far.
* Watching a pulse-driven piston double-fires (t+2 and t+6: extension, then the end of the
  retraction); watching a door block's rest cell fires once.

## Tools (this track)
* `door obs-path <design> --from x,y,z --to x,y,z [--len k] [--forbid ...]`: every shortest (or
  exact-length) observer chain from a changing cell to the cell the last observer's back points
  into; static safety rules (free cells, outside the walkway, outputs only into air that is not a
  piston's quasi cell, not watched by other observers).
