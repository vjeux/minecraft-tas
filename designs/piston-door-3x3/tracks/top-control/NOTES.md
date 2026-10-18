# Track top-control

Direction (from the coordinator): move all control near the top. Side pistons up at y=2 or
wherever the backbones allow, power the bottom-middle piston by the cheapest path, re-derive
the wiring with the router and the minimizer.

## Results (all verified with `door verify <file> --extra-offsets 28`, 32 positions x 3 cycles)
| design | blocks | category | file | idea |
|---|---|---|---|---|
| **rb6o** | **47** | **solid (record)** | `rb6o.txt` = `BEST.txt` = `BEST-solid.txt` | redstone-block's rb6 (48) with the bottom-middle piston fired by an observer watching the bottom-right door block |
| m48o | 47 | solid | `m48o.txt` | the same trick on mechanics' rb5-hold (48) |
| st43o | 42 | sculk (record) | `st43o.txt` | the same trick on search-tool's 43 sculk |
| v3c | 87 | solid | `v3c.txt` | side pistons high; redstone blocks riding the backbones power A, P_T and P_BM; only the side pistons are wired |
| rb2s2 | 50 | sculk | `rb2s2.txt` | redstone-block's rb2 (57) with its 14-block late hold replaced by a sculk sensor + one repeater |
| m51s | 48 | sculk | `m51s.txt` | mechanics' 51 with its 10-block hold replaced the same way |
| sf1 / sf1-min | 128 / 126 | solid, slime-free | `sf1.txt`, `sf1-min.txt` | first slime-free door: v2-min with each backbone replaced by a stack of 3 sticky pistons on one torch |

## rb6o (47, solid record): observer-fired bottom-middle piston
* rb6 powers P_BM (extended at rest) with a redstone block riding the left structure, which
  needs the left backbone extended 2 cells down (2 slimes + the redstone block).
* Instead: P_BM retracted at rest (face at (1,-1,0), M two cells away), an observer at (2,-1,0)
  facing up watching the bottom-right door block (2,0,0), its back strongly powering a block at
  (2,-2,0) next to P_BM. When the right structure opens, BR leaves -> 2gt pulse -> the sticky
  piston extends to M and pulls it down (a 2gt pulse pulls a block from 2 away). When the right
  structure comes back, BR returns -> 2gt pulse -> P_BM pushes M up and the pulse ends while M
  is moving, so it spits M in place. -3 blocks +2 = 47. Same on mechanics' rb5-hold: m48o 47.
* `door minimize --moves all --plateau` finds nothing more on rb6o.

When v3c was found (87) the record was 109; rb2s2 was the first sculk door. All three were
overtaken during the night (solid 48, sculk 46, slime-free 55 by other tracks).

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
* The same idea (redstone blocks moved by the structures instead of wires) was pushed much
  further by the redstone-block track (pushed side structures, one shuttled redstone block).

## Sculk sensor as a one-block 30gt hold (rb2s2 50, m51s 48; gadget `designs/gadgets/sculk-hold`)
* The 26.3 sculk sensor works in the simulator: a lever click (or any piston move) within 8
  blocks reaches it after floor(distance) ticks (block-entity phase); it is then active 30gt
  (power by distance, weak power on every side, strong power into the block below), then 10gt
  cooldown (deaf).
* rb2 / mechanics-51 hold their shuttle piston Q 24gt after the lever goes off with 3 repeaters
  plus dust and floors (10-14 blocks). A sculk sensor next to Q does the hold. It wakes 2-3
  ticks after the click, but Q's retract block event runs at tick 1, so one 4-tick repeater into
  Q fed by 2 dust from the lever covers the first ticks (6 blocks + sensor).
* The door's own late pistons (Q, P_R, A) move inside the sensor's cooldown, so it stays deaf.
* Ruled later: sculk doors rank in their own category (footsteps can trigger them).

## sf1 (slime-free, 128 -> 126)
* Each side column on a vertical stack of 3 sticky pistons (one per row). One wall torch on the
  middle row powers all three: side power to the middle piston, quasi power to the bottom one
  through the middle piston's cell, and the torch's strong power into the block above it for
  the top one. The torches are fed from v2's sides line.
* Pitfall: v2's lever drop (dust (5,4,0) on (5,3,0)) quasi-powered the new top-right stack
  piston (4,2,0) through (4,3,0); moved the drop to (6,5,0).
* logic-min's 55 is far better (retracted pistons fired by 2gt pulses pull a block from 2 away
  and spit it back, so no holds at all).

## Tools added (separate commits)
* `door route --swept-script <drive script>`: the router's reserved cells come from a drive
  script with ideal inputs, so a mechanism can be wired before it runs on its own
  (`drive::swept`).
* `canonicalize` (used by `gen`/`minimize`) retries with the settled redstone states until the
  machine stays put: dust written at power 0 no longer lets a piston fire during `gen`.
* Gadget `sculk-hold` (+ a note and table row in docs/mechanics.md).

## Failed / notes
* `door minimize --moves all` found nothing on v3c (87) nor on rb2s2 (50).
* v3b: the router's goal dust got an extra connection from a later route and stopped pointing
  into P_R (the router does not check final dust shapes); P_R needs OR(W_out, W_out+8), not a
  symmetric delay (open order P_L < P_R < B, close order B < P_R < P_L cannot come from
  symmetric delays).
* A sculk sensor alone next to Q (no covering repeater) fails: Q retracts at tick 1, before the
  sensor wakes at tick 2, and T is pushed back into C's cell.
* Gaps that stay: a lever-driven piston always reacts at tick 1, so any wireless/late holder
  needs a 2-tick cover; making Q quasi-powered by the lever (not updated by it) would remove the
  cover, but then P_L loses its quasi source and O2 its watched lever (no layout found).
