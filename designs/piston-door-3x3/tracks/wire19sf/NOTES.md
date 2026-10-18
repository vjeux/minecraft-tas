# Track wire19sf: wiring mech-synth's 19-block slime-free mechanism (pist19r)

Goal: a slime-free door below 44 (`archive/slime-free-s7q-44b.txt`) from mech-synth's `pist19r`
(19 mechanism blocks: door 9, every edge block on its own pull-type sticky piston, M down, A/B
elevator, P_T), driven by ONE pulse train identical on both lever edges.

| file | blocks | note |
|---|---|---|
| w45.txt | 45 | first wired pist19r: open 28 / close 26, 32 positions |
| w44.txt | 44 | P_T chain off O_a (3 observers) instead of off the lever (4); regrow --size 3 finds it too |
| **w42.txt = BEST.txt** | **42** | w43 with M's trigger found by `door synth` (4 blocks instead of 5): observer G (-1,-1,0) watching L0's head/gap cell (-1,0,0) -> redstone lamp (-1,-2,0) under it; observer (0,-1,0) watching G -> M's quasi cell (1,-1,0); observer (0,-2,0) watching the lamp -> M's piston. Close: the t16 pulses are blocked (C is being spat into (1,0,0) by A2), the lamp's fall (4 gt late) gives the t22 pulse. Open: three toggles (t16, t21, t26), M ends down. Open 28 / close 24, 104 positions, buildable |
| w42e.txt | 42 | w43e with w42's M trigger mirrored (observer (3,-1,0) on RP0's head cell, lamp (3,-2,0), observers (2,-1,0) -> M's quasi cell and (2,-2,0) -> M's piston) |
| w43.txt | 43 | M's chain ends in M's QUASI cell (1,-1,0) instead of M's piston: the other cell parity, so the left column fires at t12 (2 observers) and M at t22 on close with 4 observers after BL's watcher. Open 26 / close 24. `w43b.txt` = its mirror (chain south) |
| w43e.txt | 43 | the mirrored structure: P_T east at (3,3,0) (park (2,3,0)), TL pulled UP (puller (0,4,0), chain (0,4,-2) -> (0,4,-1) off O_a) + left pair fired by an observer (-1,2,0) watching TL + block (-2,2,0); P_T chain (2,4,-2) -> (3,4,-2) -> (3,4,-1) into its quasi cell (3,4,0); right column on a stack-feed (dust (4,2,-1), floor (4,1,-1)) fed by (4,4,-1) -> (4,3,-1) off the P_T chain (t12); M via BR's watcher (2,-1,0) + 4 observers into M's quasi cell (t22 on close) |

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

## w43 (43 blocks, slime-free): the wiring
19 mechanism (pist19r with TR pulled UP: door 9, left column (-2,0..2,0) on 3 pull-type sticky
pistons facing east, TR puller (2,4,0) facing down, right pair (4,0..1,0) facing west, M's
puller (1,-2,0) facing up, A (1,4,0) and B (1,5,0) facing down, P_T (-1,3,0) facing east) + 24
control:
* A/B (8, s7q's north cluster): lever (1,4,-3) hung on O_a (1,4,-2) facing north (A1 at t2
  through X dust (1,4,-1) on (1,3,-1): X powers A up top, its floor powers A one cell lower),
  O_b (1,5,-2) watching O_a -> block J (1,6,-2) -> repeater R_B d4 (1,6,-1) into B's quasi cell
  (B window t12-20), O (1,5,-1) watching R_B -> X (A2 t14, A3 t22).
* P_T (3): O_a -> (0,4,-2) -> (-1,4,-2) -> (-1,4,-1), whose back points into P_T's quasi cell
  (-1,4,0): t8.
* Right side (4, s7q's): (2,4,-2) watching O_a -> (2,4,-1) -> TR puller (t6); observer (3,2,0)
  watching TR's cell -> block (4,2,0) above the pair (direct + quasi).
* Left column (4): (-2,4,-1) watching (-1,4,-1) -> (-2,3,-1) -> cross dust (-2,2,-1) on
  (-2,1,-1) (stack-feed): t12.
* M (5): observer (0,-1,0) watching BL -> (0,-1,-1) -> (0,-1,-2) -> (1,-1,-2) -> (1,-1,-1),
  whose back points into M's quasi cell (1,-1,0) (= M's head cell, the piston faces up): t22 on
  close (BL is pushed home at t12, watched at t14), t24 on open.
* Verified at 104 positions; `door buildable`: BUILDABLE (43 actions, lever off, 10 placements
  against a temporary dirt block).

## w45 (45 blocks): the first wiring
* A/B: s7q's north cluster, 8 blocks with the lever.
* P_T (-1,3,0): 4-observer chain off the lever into its quasi cell (-1,4,0): t8.
* Right side = s7q's: TR goes UP (puller (2,4,0), fed off O_a by (2,4,-2) -> (2,4,-1)), the right
  pair (4,0..1,0) fired by an observer (3,2,0) watching TR's cell + block (4,2,0). (So the
  mechanism is pist19r with TR pulled up instead of east: still 19.)
* Left column (3 pistons): stack-feed (cross dust (-2,2,-1) on (-2,1,-1)) fed by a 4-observer
  chain continuing the P_T chain: t16.
* M: observer (0,-1,0) watching BL + 3 observers into M's piston: t22 on close (t24 open).
* Budget: 19 mechanism + 26 control.

## Searches on w43 that found nothing smaller
* `door minimize --moves all --plateau` (436k swaps), a plateau walk with pairs (15 steps);
* `door regrow --size 2` (7k) and `--size 3` (2.1M candidates, obs/block/dust/rep1-4);
* `door route2 --max-rip 6` (278 rip sets) and `--pairs --tolerance 2` (1573 rip sets, 28
  candidates, none passes the judge);
* `door synth` for M's trigger alone (k <= 6, observers/blocks/dust/repeaters/comparators/
  torches/lamps/copper bulbs, box x -3..3 y -4..0 z -2..2): with the left column at t12 the
  smallest are 6 blocks aimed at M's piston, and with it at t16 exactly my 4-observer chain.
* Notes from the walk: X's floor (1,3,-1) is only a support: X alone powers A in its lower
  position (it points into (1,4,0), A-low's quasi cell, and dust updates two cells away), so the
  floor can be any full block (an observer works too).

## Ideas examined (no saving found)
* Odd pulse counts: a pull-type toggle piston accepts 1 or 3 pulses per edge (3 toggles end in
  the right place), so X (pulses at t2, t14, t22) or an observer watching X (t4, t16, t24) can
  fire any "anytime" piston (TR puller, side columns) -- but nothing anytime sits next to X, and
  the side M depends on must move exactly once.
* A3 earlier (B window fed at t2 instead of t4) would let M fire at t20 (one observer less) but
  needs a second observer on the lever plus a relay to B's quasi cell (+2).
* P_T + L2 + L1 from one block at (-2,3,0) (above L2, beside P_T): unreachable by t9; BL down
  instead of L0 loses the BL link M's timing relies on.

## Lessons
* **Door-block watchers are asymmetric**: a pull-type piston fired at t moves its block at t+2 on
  open (extend, then pull) but at t on close (push), so an observer watching the door block fires
  2 gt EARLIER on close. M (must be >= A3 on close) through BL's watcher needed 2 more observers
  than the open timing suggests (w43 attempt: M at t20 on close, A3 then lifted T).
* **Observer-chain parity**: along a chain, (cell parity + t/2) is invariant, so the time at
  which a chain can fire a given cell is fixed mod 4; a stack-feed dust reached at t12 cannot be
  reached at t14 by observers (a repeater or a different target cell flips it). **Fix used in
  w43: aim at the piston's quasi cell instead of the piston** (opposite parity); for a piston
  facing up the quasi cell is its own head cell.
* `door synth` with the door judge finds the same M triggers (5 observers + block, or lamp
  tricks) at 6 blocks when the left column fires at t12, and nothing of <= 5 in x -3..3,
  y -4..0, z -2..2 aimed at the piston itself (`synth-m.patch`, `synth-m16.patch` contexts).
* An observer pointing into the cell above A (from (1,4,-1)) powers A in both positions (A low
  through B's head, by quasi-connectivity, and it updates A); unused so far.
* Watching a pulse-driven piston double-fires (t+2 and t+6: extension, then the end of the
  retraction); watching a door block's rest cell fires once.

## Tools (this track)
* `door obs-path <design> --from x,y,z --to x,y,z [--len k] [--forbid ...]`: every shortest (or
  exact-length) observer chain from a changing cell to the cell the last observer's back points
  into; static safety rules (free cells, outside the walkway, outputs only into air that is not a
  piston's quasi cell, not watched by other observers).
