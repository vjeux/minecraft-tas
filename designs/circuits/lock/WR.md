# Study 14: combination locks (WR board)

ONE design: the item-frame combination lock players build. The player turns N item frames (the
dials); the door opens only when every dial shows the same position as its own code frame. "Set
the code by turning the code frames on the other side of the build."

## The judge (`circuit: lock N`, crates/circuit/src/lock.rs)
- Ports: `port input.d0..d<N-1>: x y z frame` (the dials) and `port input.k0..k<N-1>: x y z frame`
  (their code frames), each an `entity: minecraft:item_frame x.5 y.5 z.5 {Facing:..b,Item:{..},
  ItemRotation:..b}` of the design; one `port output:` (on while open).
- The input device is a real frame turned by a real right-click: the bridge's `use_entity` calls
  `ItemFrame.interact` with an empty hand, which runs `setRotation(r + 1)` and updates the
  comparators reading the frame (rewriting the frame's data would not). The summon updates them
  too, as a player putting the item in does (`setItem`).
- Every combination of the dials is tried (an odometer, one click per step, 40 gt per
  combination), for the built code AND for a second code set by turning every code frame 3 steps:
  2 x 8^N combinations. Per combination the output may change once, from the previous
  combination's value to this one's, and must have settled before the next click: no false open,
  no flicker while turning. Metrics: opens within / closes within (gt). 4 rotations x positions,
  the game only (redsim has no entities).

## Mechanics that bound the design (from the source)
- A comparator reads a frame only through a solid block: comparator -> block -> frame, the frame
  hanging on the block's far face and facing away (`ComparatorBlock.getInputSignal` /
  `getItemFrame`: the frame's direction must equal the comparator's facing). So each frame has
  exactly ONE reader. A frame gives rotation + 1 (1..8; `ItemFrame.getAnalogOutput`).
- Equality needs both directions, f > k and f < k, and each of f and k must reach two
  comparators. With one reader per frame, the dial's reader gives raw f (compare mode), and its
  value must fan out: a dust cell (adjacent readers get it undecayed), a solid block it strongly
  powers (comparators read it at the rear; dust beside or ON TOP of it gets f undecayed), or a
  second code frame on the code side.
- A comparator's side reads only dust, diodes and redstone blocks, never a powered solid block;
  its rear reads a powered block. Side inputs take the max of both sides.
- Mismatch signals are 1..7 (weak). A chain of SUBTRACT comparators carrying strength 1 (b0 = 1)
  ANDs any number of dials losslessly: each dial's two differences go into a bus comparator's two
  sides (max), and any mismatch >= 1 drops the bus to 0. Comparators do not decay, so the bus
  carries its 1 through every dial.

## Board
| design | dials | blocks | opens / closes | status |
|---|---|---|---|---|
| lock3-q | 3 | 86, 6x3x9 = 162 | 8 / 10 gt | ARCHIVED (the one design): lock3-p3s with a delay-1 repeater as the low route's first cell (a repeater cannot climb onto D2: no cutter) and the high repeater at delay 3 (it ends 2 gt after the low path starts at a wrap) |
| lock3-p3s | 3 | 89, 6x3x9 = 162 | 6 / 8 gt | replaced by lock3-q: lock3-p3 with the output torch on the bus's south end support (no empty row) |
| lock3-p3 | 3 | 89, 6x3x10 = 180 | 6 / 8 gt | replaced by lock3-p3s: lock3-tern at pitch 3, the code lines one level down (their k dust feeds the k-1 dust diagonally, so the next T's side cell stays air) |
| lock3-tern | 3 | 93, 6x3x12 = 216 | 6 / 8 gt | replaced by lock3-p3: ternary dials, one shared OR dust bus, output read at its torch; redsim tick-exact against the game |
| lock3-lines | 3 | 102 (+6 frames), 6x5x10 = 300 | 8 / 8 gt | replaced by lock3-tern |
| lock1-tern (ternary dial) | 1 | 32 (+2 frames), 7x3x5 | 6 / 8 gt | passes the judge at 4 positions (robust); the same dial in the two-subtractor core costs 51 |
| lock1-test (judge test) | 1 | 51 (+2 frames) | 8 / 8 gt | passes the judge (robust); not a candidate: routing by dust and repeaters |

Measured shrink options for lock3-lines (28 blocks per dial: the core 14, the OR machinery 14):
- A strength-1 subtract-comparator bus instead of the repeater lines: about 79 blocks for 3 dials,
  but 14 comparators in series = about 32 gt from a click to the lamp (within the judge's 40 gt
  step since the coordinator's call: compactness beats speed for a lock).
- A one-dial check needing no fan-out of f: P = compare(dial frame, side k), Q = sub(P, side k-1)
  (k-1 = the second dust of the code line) gives Q = 0 (f < k), 1 (f = k), >= 2 (f > k) from 3
  comparators and 2 dusts; but testing "Q == 1" takes two more comparators or a torch per dial.

## Layout notes (clk-item, 2026-10-21)
- Fan-out without decay, worked out from the source (`RedstoneWireBlock` reads all 6 neighbours):
  a block strongly powered by a comparator gives its exact value to a comparator reading it at
  the rear, and to dust beside it, ON TOP of it or UNDER it. So the frame readers can sit one
  level below (or above) the core and power the blocks under the core's dust: the core's level
  stays free for the bus.
- The 2x2 core (dust f, S1 = sub(f, k) facing it; S2 = sub(k, f); dust k on the other diagonal)
  sends S1 and S2 out in OPPOSITE directions. Two dials stacked at pitch 2 put one dial's dust
  next to the other's comparator side (or dust next to dust), so the pitch is 3, or alternate
  levels.
- A strength-1 bus must be comparators end to end (dust kills it), so at pitch 3 it costs extra
  comparators; cheaper: OR every mismatch into ONE block X (S outputs into its sides at its level,
  and into a dust ON TOP of it from the level above, up to 4) with one wall torch on X = the
  output: 7 inputs = 3 dials per X, no bus at all. Searching packings of 2-3 dials around one X
  is the next step.
- Ruled out by geometry (hand check): S2's output dust on top of S1's output block when f's dust
  sits on top of its block (always 2 cells apart).

## Compaction analysis (2026-10-21, after the coordinator's "compactness beats speed")
- Why 28 blocks per dial: each frame has ONE reader cell (`getItemFrame`: comparator, block, frame
  in a line, the frame facing the comparator's direction), and the two-subtractor test needs each
  value twice (a rear and a side), so both frames need a reader + a hub block + a dust (the 2x2
  core [F dust, S1; S2, K dust] over hubs F and K). The core's two outputs always point in
  OPPOSITE directions (both orientations), so a bus or an OR block collects them only through
  extra relays, and a comparator bus needs a comparator (+ support) in every cell next to the
  cores' dusts. A U-shaped comparator bus at pitch 3 counts 28 per dial too: no gain.
- TERNARY DIAL (the way down): T = subtract(rear = the dial frame itself, side = K - 1), where
  K - 1 is the code reader's value one dust away. T = max(0, f + 1 - k): 0 if the dial is LOW,
  exactly 1 at the code, >= 2 if HIGH. One comparator reads the dial; no dial reader, no hub,
  and no second subtractor. "T == 1" from T's own dust line: the 1st dust D1 = T powers the
  block under it (a torch there is ON iff low), the 2nd dust D2 = T - 1 >= 1 iff high. All
  mismatches go into one block X with the output's wall torch: 3 dials' D2 into three faces, the
  low torches' dust network onto X's top. Estimate: about 20 blocks per dial, about 62 for 3.
- Its hazard: at the dial's wrap (rotation 7 -> 0) T can jump from HIGH straight to LOW. D2 drops
  at once and the low torch lights 2 gt later, so X would be unpowered for one torch delay
  (order-dependent: a flicker in some rotations). Fix: the high path through a repeater set to
  delay 2 (4 gt) into X, so the low torch lights before the high signal ends.
- Measured on lock1-tern: of its 32 blocks, the low torch's dust route to X costs 9 (4 dusts, 4
  supports, a cutter block so it does not climb onto the high dust). That route is the waste. A
  merged 3-dial version must share one route: every repeater output and low torch is a 15-strength
  source, so all of them may join ONE dust network into X (isolated from the T lines by the
  repeaters and the torches). Placement rule found by hand: a block strongly powered by a torch or
  repeater must never touch a T-line dust (the dust reads its 15).
- The comparator-forcing trick (from `ComparatorBlock.getInputSignal`): if the block between a
  comparator and its frame is powered at 15, the comparator reads 15 instead of the frame, so a
  previous dial's mismatch (15) can force the next dial HIGH and travel down the chain with no
  inverter; only lows need a torch.

- `circuit shrink` (game-judged, 2 positions x 4 rotations) on lock3-lines: no single block can go
  (96 candidate blocks): the 102 is a layout minimum, not a padding one.
- lock3-tern (93): redsim local census `--remove 1`: none of its 87 removable blocks can go. Where
  its blocks go: per dial 22 (Bd, T, D1, D2, R, the code stub + K dust + K + Bk, the cutter above
  the low route; level 1: 7 supports, the low torch A, 2 route dusts; level 0: 2 route supports),
  the bus 20 (10 dusts + 10 supports), the output torch 1. Next targets: (a) the bus: dials closer
  along it, or the R outputs and routes entering it at fewer cells; (b) pitch 4 -> 3 (now T of the
  next dial reads the code dust on its side: mirror alternate dials so two code lines share no T
  side); (c) the low route (6 per dial): A directly beside the bus.
- FORCING CHAIN (next to try): dial i's mismatch, high (its delay-2 repeater) or low (its torch),
  powers dial i+1's frame block Bd at 15 (a dust ON TOP of Bd fed by both: dust powers the block
  under it at its level, and a torch next to the dust lifts it to 15). Dial i+1 then reads 15,
  goes HIGH and passes the mismatch on; only the last dial needs the output inverter (its high
  repeater and low torch into X, X's wall torch = output). No shared network and no OR block per
  dial: per dial about Bd, T, D1/Y, D2, R, A and the K line (about 16 blocks). A repeater whose
  input drops and returns within its delay stays on (it re-reads its input at the tick), so the
  repeaters also filter the 2 gt wrap glitch.
- A single OR block H for all highs is safe next to any dust: a block powered only by dust is not
  seen by other dust (wire reads run with `shouldSignal` off), unlike a block a torch or a
  repeater powers.
- Hand packing of the forcing chain (2026-10-21): every placement tried collides. The forcing dust
  must be at 15, so the high repeater and the low torch must both touch it; the low torch hangs on
  T's output block, on the face toward that dust; T's two side cells are then next to the torch or
  to D1, so the code dust (K - 1) at T's side reads 15 or merges with D1. Ways out to test:
  T's code input from a comparator pointing into its side with the decay done before it (code
  reader -> dust -> dust -> a second comparator in compare mode -> T's side), or the low path
  through its own repeater (any distance) into the frame block's free face. A packing search
  (Rust, game-judged candidates) is the systematic route.

## Packing plan (2026-10-21, toward a big cut; redsim screens, trace-compare gates)
- lock3-tern spends 13 of its 29 blocks per dial on the OR: the shared dust bus (4 dusts + 4
  supports per dial) and the low torch's route to it (2 dusts, 2 supports, 1 cutter).
- COMPARATOR-CHAIN BUS (next candidate): one subtract comparator C_i per dial in a straight chain
  (C_i's output into C_{i+1}'s rear, diode to diode, no decay), carrying 15 from a source block
  powered by a torch under it. Dial i's two mismatch signals go straight into C_i's two SIDES:
  R_i (delay-2 repeater) pointing in, and the low torch's dust (15) at the other side; any 15 kills
  the bus (15 - 15 = 0; subtract mode, since compare passes 15 >= 15). No bus dusts, no route: per
  dial about 20 blocks (Bd, T + support, Bk, K + support, 2 code dusts, D1, its support with the
  low torch, D2 + support, R + support, C + support, the torch's dust), 3 dials about 63 + 6
  frames. The output is the last C's front (on = open).
- COMPARATOR-CHAIN VERDICT (2026-10-21, before building it): it glitches unless its stages
  are ADJACENT comparators. A diode feeding a diode ticks at HIGH priority
  (`checkTickOnNeighbor` / `shouldPrioritize`), so an upstream kill and a downstream release in
  the same tick resolve upstream first. Any delay between stages (a dust, a repeater) lets a
  carry click (dial 0 matched -> low while dial 1 low -> matched) open the lock for 2-4 gt. The
  delay-2 high path breaks the same tie the other way, unless the release is slower. Adjacent
  stages mean dials at pitch 1 along the chain, which the dial machinery (6 x 3 cells) cannot
  give. At pitch 4 with spacers it costs about what the dust bus costs (2 per C + 2 per spacer).
  Not a big cut: dropped. The OR bus is glitch-free because the output torch re-reads X at its
  tick, so a gap shorter than 2 gt never shows.
- Where a big cut can come from: (1) pitch 4 -> 3 with alternate dials mirrored (the code line
  north instead of south), so two code lines never share a T side: -2 blocks per dial and -1
  row each; (2) the low route (5 per dial) folded into the bus row, with the torch on D1's
  support facing the bus directly. This needs D2 + R turned 90 degrees so D1's support's east
  face is free; (3) the code dusts on existing blocks (a dust on Bd or Bk at power < 15 does not
  change the comparator reading through it: the frame's value replaces weak power, per
  `getInputSignal`). These interact, so they need a packing search (redsim screen in ms,
  trace-compare on the winner). (dial i's mismatch forcing dial i+1's frame block to 15) has a ripple
  hazard: a mismatch moving upstream (dial 0 matched -> low at its wrap while dial 1 becomes
  matched) leaves a gap of one stage (about 6 gt) at the output: a flicker. The OR (and the
  comparator chain, whose kills are all side inputs of one line) does not.
- The torch's block-above is always next to D1 (the torch hangs on D1's support), so the low
  signal must leave sideways through a dust, never through the block above the torch.

## Records log (each tick-identical in redsim and the game, archived robust)
| step | blocks | box | what changed |
|---|---|---|---|
| lock3-lines | 102 | 6x5x10 = 300 | two-subtractor cores, repeater OR lines |
| lock3-tern | 93 | 6x3x12 = 216 | ternary dials (one comparator per dial), one shared dust bus |
| lock3-p3 | 89 | 6x3x10 = 180 | pitch 3: the code lines one level down |
| lock3-p3s | 89 | 6x3x9 = 162 | the output torch on the bus's south end |
| lock3-q | 86 | 6x3x9 = 162 | a delay-1 repeater starts each low route (no cutter); high repeater delay 3 |
| lock3-q85 | 85 | 6x3x9 = 162 | (census) the first dial's high repeater feeds a block that the bus reads, instead of a bus dust on its own support |
| lock3-alt | 64 | 6x4x6 = 144 | lock3-two with the middle dial one level lower: each comparator side next to the other dial faces a plain support block, so no spacer rows; the bus steps down and up diagonally |
| lock3-two | 68 | 6x3x8 = 144 | TWO COMPARATORS PER DIAL: T = compare(dial, code), W = compare(code, T's dust); a torch on T's dust support (low) and one on W's front block (high), both directly on one dust bus under them (no routes, no repeaters); code frames read from the west, each on the block south of its dial |
Census results: the 2-dial cut (remove 2 / add 1, stone, wire, repeaters: 588,147 candidates,
25 s) found the lock3-q85 cut; the 3-dial lock3-q85 (remove 2 / add 1, stone, wire, torches x5,
repeaters x4, comparators x8: 11,430,940 candidates, 70 min on 64 threads) finds NOTHING: 85 is a
minimum for every 2-block change. A big cut needs a new structure.
- Next structure (two comparators per dial, no D2 and no high repeater): K's output dust (k) is
  adjacent to T's side AND to W's back. T = compare(f, k) is f when f >= k, else 0. W =
  compare(k, T's dust) is nonzero iff k >= T. Then match = (T > 0) AND (W > 0): low has T = 0,
  high has W = 0. Each dial: a torch on T's front block and a torch on W's front block, OR'd
  into the bus. Estimate 16 + routes per dial (the ternary dial is 21 with its route), if the
  torches can sit next to the bus.

## Target and gap
- Target: one dial module of about 10 blocks + the 2 frames (2 frame blocks, 4 comparators, the
  two fan-out dusts on strongly powered blocks underneath, one bus comparator), tiling dial by
  dial with a constant head (b0 source) and tail (the output). Gap: the core's two fan-outs cross
  (f must reach S1's rear and S2's side, k the opposite), so stacking dials at pitch 2 puts one
  dial's k on the next dial's comparator side; the layout search is the open work.
- Extendable: a `lock N` family (N = 1..6) from the module, every N judged.
