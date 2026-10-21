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
| lock3-lines | 3 | 102 (+6 frames) | 8 / 8 gt | ARCHIVED: every combination of 2 codes, robust |
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

## Target and gap
- Target: one dial module of about 10 blocks + the 2 frames (2 frame blocks, 4 comparators, the
  two fan-out dusts on strongly powered blocks underneath, one bus comparator), tiling dial by
  dial with a constant head (b0 source) and tail (the output). Gap: the core's two fan-outs cross
  (f must reach S1's rear and S2's side, k the opposite), so stacking dials at pitch 2 puts one
  dial's k on the next dial's comparator side; the layout search is the open work.
- Extendable: a `lock N` family (N = 1..6) from the module, every N judged.
