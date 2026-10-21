# Study 14: combination locks (WR board)

ONE design: the item-frame combination lock players build. The player turns N item frames (the
dials); the door opens only when every dial shows the rotation of its hidden code frame. "Set
the code by rotating the hidden frame behind each dial."

## The judge (`circuit: lock N`, crates/circuit/src/lock.rs)
- Ports: `port input.d0..d<N-1>: x y z frame` (the dials) and `port input.k0..k<N-1>: x y z frame`
  (their code frames), each an `entity: minecraft:item_frame x.5 y.5 z.5 {Facing:..b,Item:{..},
  ItemRotation:..b}` of the design; one `port output:` (on while open).
- The input device is a real frame turned by a real right-click: the bridge's `use_entity` calls
  `ItemFrame.interact` with an empty hand, which runs `setRotation(r + 1)` and updates the
  comparators reading the frame (rewriting the frame's data would not). The summon updates them
  too, as a player putting the item in does (`setItem`).
- Every combination of the dials is tried (an odometer, one click per step, 16 gt per
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
| lock1-test (judge test) | 1 | 51 (+2 frames) | 8 / 8 gt | passes the judge (robust); not a candidate: routing by dust and repeaters |

## Target and gap
- Target: one dial module of about 10 blocks + the 2 frames (2 frame blocks, 4 comparators, the
  two fan-out dusts on strongly powered blocks underneath, one bus comparator), tiling dial by
  dial with a constant head (b0 source) and tail (the output). Gap: the core's two fan-outs cross
  (f must reach S1's rear and S2's side, k the opposite), so stacking dials at pitch 2 puts one
  dial's k on the next dial's comparator side; the layout search is the open work.
- Extendable: a `lock N` family (N = 1..6) from the module, every N judged.
