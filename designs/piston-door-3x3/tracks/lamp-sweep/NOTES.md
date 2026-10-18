# Track lamp-sweep: exact re-synthesis of every board leader with time-shaping blocks

Direction (from the coordinator): two of the night's last gains came from `door synth` circuits
with a redstone lamp (lights at once, goes dark 4 gt after its power ends): wire21's 41 (lamp-timed
P_R) and wire19sf's 42 (a 4-block lamp trigger for M). The earlier `door resynth` sweeps used the
default palette (block, dust, dot, repeaters, observers, torches): no lamps, copper bulbs, note
blocks or targets. This track re-runs `door resynth` (and targeted `door synth` on 4-7 block
control groups) on every board leader with those blocks in the palette, keeping each design in its
league.

## Summary (verified with `door verify --extra-offsets 28`, archived in `archive/`)
| file | blocks | board | was | open / close |
|---|---|---|---|---|
| `flat43.txt` (= `BEST.txt`) | **43** | flat (one z layer, slime-free): best on the board | 45 (flat-b-v6), 49 (flat-v8, its source) | 46 / 44 |
| `a1-107.txt` | **107** | speed: the fastest-open solid door, 16 blocks smaller | 123 (speed-2-a1m) | 10 / 7 |
| `o6-107.txt` | **107** | speed: open 11 / close 7, 12 blocks smaller | 119 (speed-wire-o6) | 11 / 7 |
| `w41.txt` | 41 | slime-free (best at 06:28; sf-holdA reached 40 and 38 since) | 42 (wire19sf-w42) | 28 / 26 |
| `fb44.txt` | 44 | flat (superseded by flat43) | 45 (flat-b-v6) | 30 / 26 |

All five re-verified at 16:25Z with main's verifier (main as of 15:51Z merged in): 32 positions x 3
cycles, and rotation-safe (they also work turned 90, 180 and 270 degrees).

No change on the solid 40s (fc2, fx1, m05), the new 38 (sf-holdA-b38), sculk s4 (36) or gravity
(g42; gravity-2's g33 has 5 removable blocks).

What the sweep says: the new blocks paid off where a design had never been re-synthesized at all
(flat-v8, flat-b-v6, wire19sf-w42, the two speed doors), and there they mostly enabled plain
dust/stone collapses: a redstone lamp started flat43's 6-step cascade, torch-on-target taps and a
lamp + observer shrank the speed doors. On the record designs every connected group of up to 5
wiring blocks is already minimal with lamps, bulbs, note blocks and targets too.

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
* `synth`: the enumeration stops (the group is skipped) after 200 x `--max-cands` visited sets, so
  a search tree that explodes while few sets are finished circuits no longer runs for hours.
* `grow`/`regrow --states`: `bulb` and `target` (lamp and note were there).

## Inputs (board leaders at 13:20Z, plus the ones that appeared later)
| board | design | blocks |
|---|---|---|
| solid | archive/fc2-40b, riders-fx1-40b, m05-21b-sym-40b; from 15:28Z sf-holdA-b38 (explore/sf-holdA BEST.txt) | 40, 38 |
| slime-free | archive/wire19sf-w42-42b | 42 |
| flat | explore/flat-b BEST.txt (flat-b-v6), explore/flat BEST.txt (flat-v8) | 45, 49 |
| sculk | explore/sculk-2 BEST.txt (sculk2-s4) | 36 |
| gravity | archive/synth-g42-42b; gravity-2's g33 (5 removable blocks: skipped) | 42, 33 |
| speed | archive/speed-wire-o6-119b (open 11 / close 7); explore/speed-2 BEST.txt (speed-2-a1m, open 10 / close 7) | 119, 123 |

(Copies used as run inputs were deleted so the leaderboard does not list them twice.)

Palette for every resynth: `block,dust,dot,rep,obs,torch,cmp,lamp,bulb,target,note` (size 5:
without `torch,cmp` unless `--require-any` limits the count).

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
| `a1-107.txt` | **107** | speed: the fastest solid door (open 10) | speed-2-a1m (123, open 10 / close 7) | resynth 2..3 with `--max-open 10 --max-close 7`: 16 replacements (the o6 target taps again, a REDSTONE LAMP + observer instead of a block + 2 observers, the note block of speed-2's odd-pulse trick folded into the lamp) | 10 / 7 |
| w41 | 41 | slime-free | - | resynth size 5 (52 groups, palette without torch/cmp, up to 274k circuits per group): nothing | |
| fc2-40b | 40 | solid | - | targeted `door synth`: the whole top cluster (O_a, O_b, J, R_B, O, X + floor: 7 blocks) cleared, k <= 6, `--require-any lamp,bulb,note,target`, box (-2,2,-1)..(1,7,1), sinks A (both positions), B and its quasi cell: 440k circuits, none passes the screen | |
| riders-fx1-40b | 40 | solid | - | the same top-cluster synth (sinks + P_L and its quasi cell, since X fires P_L in fx1): 701k circuits, none passes | |
| m05-21b-sym-40b | 40 | solid | - | wire21's top-left synth again (O_a, A driver, O_b, J, O_c, O_d, P_L observer: 7 blocks, k <= 6) with bulb/note/target added and `--require-any`: 835k circuits, none passes | |
| flat43 | 43 | flat | - | resynth size 5 (173 groups, full palette, up to 666k circuits per group, 70 min): nothing | |
| sculk2-s4 | 36 | sculk | - | only 6 removable blocks, no connected group of 5 or 6: resynth exhausted | |
| flat-b's fb44 | 44 | flat | - | resynth size 5 (30 groups): nothing | |
| m05-21b-sym-40b | 40 | solid | - | resynth size 5 (52 groups, full palette, `--require-any lamp,bulb,note,target`): nothing | |
| fc2, fx1, m05 | 40 | solid | - | resynth 2..4 with special floors (`--palette ...,gt,glass,obsidian --floors block,gt,glass,obsidian --require-any glazed,glass,obsidian`: floors slime never drags, non-conducting glass, immovable obsidian): nothing | |
| fc2-40b | 40 | solid | - | resynth size 5 (75 groups, full palette, `--require-any lamp,bulb,note,target`, up to 285k circuits per group, 45 min): nothing | |
| w41, flat43 | 41, 43 | slime-free, flat | - | resynth 2..4 with glass/obsidian floors (`--require-any glass,obsidian`): nothing | |
| sf-holdA-b38 (the new record, 08:06) | 38 | solid + slime-free | - | resynth 2..4 (115 groups, full palette): nothing; resynth size 5 (93 groups, palette without torch/cmp): nothing; `door minimize --moves all --plateau`: nothing (25 neutral designs, 700 removals after them); size 6 in the control cluster with `--require-any`: the k=5 enumeration explodes (no group finished in 3 min; hence the visit cap) | |
| flat43 | 43 | flat | - | `door minimize --moves all --plateau`: nothing (14 neutral designs, 462 removals + 30 reroutes after them) | |
| a1-107 | 107 | speed | - | `door minimize --moves all --plateau --max-open 10 --max-close 7`: nothing (1.95M swaps, 20.7k removals, 9.5k compounds; 218 neutral designs, then 21k removals + 7.8k reroutes) | |
| sf-holdA-b38 | 38 | solid + slime-free | - | `door regrow --size 2 --any` with lamp, bulb, note, target added to the states (cut any 2 control blocks, add 1 near either; 153 pairs, 82k candidates): nothing | |
| fc2, fx1, m05 | 40 | solid | - | the same `regrow --size 2 --any` with the new states (276 pairs each, 139k-162k candidates): nothing | |
| flat43 | 43 | flat | - | the same `regrow --size 2 --any` (253 pairs, 142k candidates): nothing | |
| sf-holdA-b38 | 38 | solid + slime-free | - | `door regrow --size 3` with the new states (21 connected triples re-grown with 2 blocks, 3.15M candidates): nothing | |
| fc2, m05, fx1 | 40 | solid | - | the same `regrow --size 3` (23 / 22 / 25 triples, 3.0M / 3.8M / 3.3M candidates): nothing | |

## What did not work, and why
* **The record designs have almost nothing left to re-synthesize.** fc2/fx1/m05 have 11-17
  removable blocks, b38 17, s4 6, g33 5; their connected groups up to size 5 are minimal with every
  palette tried (lamps, bulbs, note blocks, targets, comparators, torches, glass, obsidian, glazed
  terracotta floors). Their remaining slack is in the lever placement and the mechanism, which
  resynth cannot move (the lever, its support and every piston are fixed).
* **Copper bulbs never appeared in a winning circuit.** A bulb toggles on rising edges only; every
  door here needs the same response on both lever edges, which pulses already give. The bulb ->
  comparator level (now in the static model) was never needed either.
* **Size 6 does not scale**: with k = 5 the enumerator visits hundreds of millions of sets per
  group even when `--require-any` keeps few of them (the new visit cap skips such groups).
* **Size-4 groups on the 107-119 block speed doors**: ~1200 groups at 10-15 s each on a loaded box
  (stopped; both speed doors had every size-2/3 group searched).

## Next ideas
* A lever-move repair search: move the lever (and drop its support), then re-synthesize the
  groups around it with a budget of group size + saved blocks - 1 (fc2's lever block K and R_q's
  floor are its most likely savings; sf-holdA's b38 got its last block from a lever move).
* Size-4 resynth on a1-107 / o6-107 with `--require-any` or `--only` slices (each improvement
  there is a speed-board tie-break).
* A smarter enumerator for k >= 5: one sink at a time (a circuit per function), and symmetry
  reduction (dust vs dot, floor types that nothing watches).
