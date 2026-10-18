# Track lamp-sweep: exact re-synthesis of every board leader with time-shaping blocks

Direction (from the coordinator): two of the night's last gains came from `door synth` circuits
with a redstone lamp (lights at once, goes dark 4 gt after its power ends): wire21's 41 (lamp-timed
P_R) and wire19sf's 42 (a 4-block lamp trigger for M). The earlier `door resynth` sweeps used the
default palette (block, dust, dot, repeaters, observers, torches): no lamps, copper bulbs, note
blocks or targets. This track re-runs `door resynth` (and targeted `door synth` on 4-7 block
control groups) on every board leader with those blocks in the palette, keeping each design in its
league.

## Tool changes (own commit)
* `synth`: note blocks in the static model (`note` palette entry): a conductor whose `powered`
  state follows its power in the same tick (NoteBlock.neighborChanged, setBlockAndUpdate: it also
  updates its neighbors), so an observer may watch it. Copper bulb -> comparator: a comparator reads
  a bulb behind it (analog 15 while lit), the T flip-flop state as a level.
* `resynth`: note blocks and targets are removable wiring; `--max-open/--max-close` (also on
  `synth`) reject candidates slower than the bound, at the first position as the leaderboard
  measures it (speed boards); every improvement is written to `--out` as soon as it is found.
* `resynth`: after an improvement only the groups whose neighborhood (blocks within margin + 3)
  changed are searched again (`--global-retry` restores the full restart). On the 119-block speed
  door the old loop rescanned all ~1800 groups after each of its 12 improvements.
* `synth`/`resynth --require-any lamp,bulb,note,target`: keep only circuits that use one of the
  new blocks, to sweep them cheaply on designs the old palette already exhausted (fc2's top
  cluster, 7 blocks -> k <= 6: 440k circuits instead of millions).
* Gadget `target-tap`: a target beside a dust line is powered by it (the dust bends into the
  signal source), a plain block is not.

## Inputs (board leaders at 13:20Z)
| board | design | blocks |
|---|---|---|
| solid | archive/fc2-40b, riders-fx1-40b, m05-21b-sym-40b | 40 |
| slime-free | archive/wire19sf-w42-42b | 42 |
| flat | inputs/flat-b-best.txt (flat-b-v6), inputs/flat-best.txt (flat-v8) | 45, 49 |
| sculk | inputs/sculk2-best.txt (sculk2-s4) | 36 |
| gravity | archive/synth-g42-42b | 42 |
| speed | archive/speed-wire-o6-119b (open 11 / close 7) | 119 |

Palette for every resynth: `block,dust,dot,rep,obs,torch,cmp,lamp,bulb,target,note`.

## Runs

| design | blocks | board | from | what changed | open / close |
|---|---|---|---|---|---|
| `w41.txt` | **41** | slime-free (was 42) | wire19sf-w42 | resynth 2..4: A1's X dust (1,4,-1) + its floor (1,3,-1) and the TR puller's 2-observer chain ((2,4,-2) watching O_a, (2,4,-1)) -> smooth stone (1,4,-1) (O_a powers it, it powers A), cross dust (2,4,-1) beside it on smooth stone (2,3,-1): O_a's own pulse now fires the TR puller at t2 instead of t6 | 28 / 26 |
| `fb44.txt` | **44** | flat (was 45) | flat-b-v6 | resynth 2..4: observers (-1,4,0), (-1,5,0), (0,5,0) + smooth stone (1,5,0) -> observer (0,5,0) facing down, observer (1,5,0) facing west, smooth stone (2,5,0) | 30 / 26 |
| `flat43.txt` | **43** | flat (was 45) | flat-v8 (49) | resynth 2..4, six steps in a row: (1) observers (-2,4,0), (-2,5,0), (-1,3,0), (-1,5,0) -> REDSTONE LAMP (-1,3,0) + cross dust (-1,4,0) + smooth stone (-1,5,0) (the lamp holds the signal 4 gt longer, which made the next five possible); (2-6) the observer/stone/repeater chain above (y=5..7) collapses into dust: a repeater d4 (1,6,0) fed by dust (0,6,0) | 46 / 44 |
| fc2-40b, riders-fx1-40b, m05-21b-sym-40b | 40 | solid | - | resynth 2..4 (114-115 groups each, up to 40k circuits per group): nothing | |
| sculk2-s4 | 36 | sculk | - | resynth 2..4: nothing | |
| synth-g42-42b | 42 | gravity | - | resynth 2..4: nothing | |
| `o6-107.txt` | **107** | speed (solid, slime-free; was 119 at the same 11 / 7) | speed-wire-o6 (119) | resynth 2..4 with `--max-open 11 --max-close 7`: twelve replacements, among them twice a wall torch on a TARGET block (gadget target-tap: the dust beside it bends into the target and powers it) instead of stone + dust + floor (stopped after all size-2/3 groups: the ~1200 size-4 groups take 10-15 s each on the loaded box) | 11 / 7 |
| w41 | 41 | slime-free | - | resynth size 5 (52 groups, palette without torch/cmp, up to 274k circuits per group): nothing | |
