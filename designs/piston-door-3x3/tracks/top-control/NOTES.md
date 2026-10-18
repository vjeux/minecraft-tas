# Track top-control

Direction (from the coordinator): move all control near the top. Side pistons up at y=2 or
wherever the backbones allow, power the bottom-middle piston by the cheapest path, re-derive
the wiring with the router and the minimizer.

## Results
| design | blocks | file | idea |
|---|---|---|---|
| v3c | 87 | `v3c.txt` (archive/) | side pistons high; redstone blocks riding the backbones power A, P_T and P_BM; only the side pistons are wired |
| rb2s2 | **50** | `rb2s2.txt` = `BEST.txt` | redstone-block's rb2 (57) with its 14-block late hold replaced by a sculk sensor + one repeater |

All verified with `door verify <file> --extra-offsets 28` (32 positions x 3 cycles).

## v3c (87): what it is
* Pull-type side pistons high: P_L (-3,4,0) grabs the left backbone by its top slime (the
  backbone runs y=-2..4), P_R (5,2,0).
* The left backbone carries two redstone blocks: (0,4,0) powers A while the left column is
  closed, (0,-2,0) powers the bottom-middle piston P_BM (1,-2,0) (the left structure is 12 blocks,
  exactly the push limit). The right backbone has a slime arm (3,2,1),(3,2,2),(3,3,2) carrying a
  redstone block at (2,3,2) next to P_T. So A, P_T, P_BM have no wiring: the left column's window
  (opens first, closes last) drives A, the right column's window (inner) drives P_T.
* Control: TL, a torch on the lever block LF, gives W_out = NOT(L0 or L32) (L32 = the lever
  through four 4-tick repeaters into LF). A dust line at y=6, z=-1 (one repeater above B's
  quasi-connected cell, so the dust floor there never powers B) carries W_out over the door to
  P_L. P_R takes OR(W_out, W_out+8gt) right at the piston (dust on one face, a repeater on
  another), which opens it 6gt after P_L and closes it 4gt before.
* Timing rules found (B at beta=14): the right column must open >= 2 ticks after T lands in
  (1,3,0) and before B; at close it must push T back only after A's second (useless) observer
  pulse is completely over (beta+15); the left column must come back after T lands.

## rb2s2 (50): sculk sensor as a one-block 30gt hold
* The 26.3 sculk sensor works in the simulator: a lever click (or any piston move) within 8
  blocks reaches it after floor(distance) ticks; it is then active 30gt (power = by distance,
  weak power on every side, strong power into the block below), then 10gt cooldown (deaf).
  Test: `sculk-test.txt` + `sculk-test.drive`.
* rb2 holds Q (the redstone-block shuttle) 24gt after the lever goes off with 3 repeaters + 4
  dust + 7 floors. Here a sculk sensor next to Q does the hold. It wakes 2 ticks after the
  click (block-entity phase), but Q's retract block event runs at tick 1, so something must
  cover the first ticks: one 4-tick repeater into Q fed by 2 dust from the lever (6 blocks).
  57 - 14 + 7 = 50.
* Re-trigger risk: the door's own late pistons (Q, P_R, A) move inside the sensor's cooldown,
  so it stays deaf; verified at 32 positions.

## Tools added (separate commits)
* `door route --swept-script <drive script>`: the router's reserved cells come from a drive
  script with ideal inputs, so a mechanism can be wired before it runs on its own.
* `canonicalize` (used by `gen`/`minimize`) retries with the settled redstone states until the
  machine stays put: dust written at power 0 no longer lets a piston fire during `gen`.

## Failed / notes
* `door minimize --moves all` found nothing on v3c (87).
* First v3b: the router's goal dust got an extra connection from a later route and stopped
  pointing into P_R (the router does not check final dust shapes).
* Sculk sensor alone next to Q (no repeater) fails: Q retracts at tick 1, before the sensor
  wakes at tick 2, T is pushed back into C's cell.
