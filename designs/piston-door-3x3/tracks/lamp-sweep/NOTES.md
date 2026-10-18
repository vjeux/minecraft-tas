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
| `w41.txt` | **41** | slime-free (was 42) | wire19sf-w42 | resynth 2..4: A1's X dust + floor and the right-side feed (lamp (1,4,-1) on (1,3,-1), observers (2,4,-2) watching O_a and (2,4,-1)) -> smooth stone (1,4,-1) + smooth stone (2,3,-1) + cross dust (2,4,-1): the block next to A is powered by the dust, which also feeds the TR puller | 28 / 26 |
| `fb44.txt` | **44** | flat (was 45) | flat-b-v6 | resynth 2..4: observers (-1,4,0), (-1,5,0), (0,5,0) + smooth stone (1,5,0) -> observer (0,5,0) facing down, observer (1,5,0) facing west, smooth stone (2,5,0) | 30 / 26 |
| fc2-40b | 40 | solid | - | resynth 2..4 (115 groups, up to 39k circuits each): nothing | |
