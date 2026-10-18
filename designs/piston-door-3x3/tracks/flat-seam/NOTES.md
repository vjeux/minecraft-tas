# Track flat-seam: the smallest FLAT + FULL SEAMLESS + HALLWAY door (slime-free)

Brief (vjeux via the coordinator, 2026-10-18 ~19:50Z): flat (every block in the door plane,
`--shape flat`), no slime/honey, FULL seamless (`door properties`) and hallway-compatible
(`door properties-dyn`: every ring cell next to the 3x3 tunnel that is air at every tick of a cycle
filled with smooth stone, the door must still work), rotation-safe, judged at 32 positions.
**Metric change (20:13Z): VOLUME first** = the bounding box over all states (W x H for a flat
door), blocks as the tie-break. flat-v2-57b (the only FULL + hallway flat door at the start) is
10 x 10 = 100; target < 100, ideally <= 81 (flat43 is 9 x 9 but only QUART).

## Results

| file | volume | blocks | seamless | hallway | open/close | how |
|---|---|---|---|---|---|---|
| [h44.txt](h44.txt) (archive/flat-seam-h44-44b.txt) = BEST | **9x9 = 81** | 44 | FULL | yes | 52 / 48 | h45 + `door minimize --shape thick<=1,volume<=81 --seamless full+hall`: the delay-2 repeater (-3,3) -> dust, the dust (-1,4) gone |
| [h45.txt](h45.txt) (archive/flat-seam-h45-45b.txt) | 9x9 = 81 | 45 | FULL | yes | 56 / 54 | lamp-sweep-flat43 made FULL in its own box: BL flag -> corner watcher, X floor obsidian -> stone (early P_T pulse removed with s55's right cluster) |
| [s55.txt](s55.txt) (archive/flat-seam-s55-55b.txt) | 9x10 = 90 | 55 | FULL | yes | 40 / 40 | flat-v2-57b compacted by `resynth --fit` (x >= -3): the M chain's 4 blocks in column x = -4 regrown inside, then one block less |

## h45: flat43 made FULL (9 x 9) in three edits
flat43 (lamp-sweep, 43, QUART) had two open-state blockers: the BL flag observer (0,-1) (fires P_M
after the left column moves) and the X floor (0,3) = obsidian (immovable on purpose: an early
P_T pulse on close, from the a1 chain's first observer (4,4)E watching the lever and emitting into
P_T's quasi cell (3,4), pushes T, C and the floor west; obsidian blocks that push).
1. `resynth --seamless full+hall --extra 2 --fit <flat43's box>` replaced the BL flag + its stone
   by a corner watcher: observer (-1,-1) facing up watches BL's park cell (-1,0) (corners are not
   hallway cells), lamp (-1,-2) under it, observer (0,-2) facing west watching the lamp fires P_M.
   44 blocks (the run could not fix the obsidian with groups of <= 4).
2. By hand: s55's right cluster (lever (4,5) on observer (4,4) facing up, which fires the right top
   piston by QC through (4,3); (3,4)E watches (4,4), not the lever, so nothing emits into (3,4)
   early; (5,4)W, (5,3)U, (5,2)U -> stone (5,1) for the right middle + bottom), so P_T fires once,
   at pt, and the X floor can be plain stone (quartz-like: hallway wall when open). 45 blocks.
   The trick that keeps flat43 at 9 rows (no crossing over the top as in 57b/s55): R_B is the
   repeater ON TOP of B (1,6) facing east, fed from the right ((2,5) facing down watches the a1
   observer (2,4) -> dust (2,6)); it outputs west into the dust (0,6), which quasi-powers B and is
   watched by O (0,5) -> X dust (0,4) (a2, a3).

## Tool: `door resynth --fit x0,y0,z0:x1,y1,z1` (compaction)

Removable blocks outside the box count as offenders (like `--seamless` repair): only groups through
them are cut, each grows back INSIDE the box with up to `--extra` more blocks (every accepted
replacement leaves fewer blocks outside), and every replacement circuit is clamped to the box, so
the shrinking search that follows keeps the design inside. Pistons, the lever and its support are
never moved (they must already be inside). Combine with `--seamless full+hall` to keep the tier.
57b -> s55 took 78 s (4 out-of-box blocks: 57 -> 57 -> 58 -> 57 -> 56, then 55).

## Why the pist19r family (flat43, h44, s55, 57b) cannot go below 9 x 9
- **Height >= 9 (rows y -2..6).** M's puller sits at y = -2 (pull-type, gap (1,-1)). A (1,4) faces down
  and its quasi cell is B's own cell (1,5): every signal that powers B from its sides (0,5)/(2,5), or
  from (1,6) itself, is the same query that quasi-powers A (identical position and direction). So B
  can only be fed through the neighbours of (1,6): row y = 6 (flat43/h44: the repeater on top of B ->
  dust (0,6); 57b/s55: a repeater at (0,6) pointing into (1,6)).
- **Width >= 9.** Each side column's bottom piston ((-2,0), (4,0)) can only be fed from (x,-1) or
  from the outer column (x = -3 / x = 5, directly or by quasi-connectivity through the middle
  piston). With the side columns, their ring gaps and M's column in the way, the bottom region is cut
  off from the top region except through an outer column: one outer column reaches one bottom
  corner; the other corner is reachable only by watching M (an observer watching a pull-type piston
  fires twice per action: wrong parity) or by a repeater merger (needs a floor on row y = -3:
  outside the 9 rows). Tried in r1 (column x = 5 dropped, right bottom fed from M's watcher chain):
  BROKEN, the right bottom toggled twice per M pulse.
- So 9 x 9 = 81 (h44) is this family's floor. cd19 (center DOWN, flat-b) has neither constraint:
  B' (1,-3) can be fed from (0,-3)/(2,-3) without touching A's quasi cell (1,-1), and the 8-row box
  y -3..4 needs only: B' driven by two 2 gt pulses instead of an 8 gt window (no repeater floor on
  y = -4), the top pullers fed from (0,4)/(3,4).
- **B' with two pulses works (ideal inputs):** schedule
  [cd19-2p.ideal](cd19-2p.ideal) (on flat-b's cd19.txt): B' pulsed at b1 (push A' up; the pulse ends before the push does,
  so A' stays up) and b2 (head into the gap, then pull A' back); passes for b1 = 12, b2 = 17..20,
  a2 = 14..15, a3 >= b2 + 4 (A' lands back 4 gt after b2), top 24, pm 3..10. 4 gt X pulses all fail.
- Pulse merger (read in flat-b-v7-full): an observer watching a delay-2 repeater that a double pulse
  (t, t+4) feeds fires ONCE: the repeater is on for exactly 4 gt, and its off edge comes while the
  observer is still on (the repeater's tick has the higher priority).

## Log
- `door minimize --moves all --plateau --shape flat --seamless full+hall` on 57b: nothing (48
  neutral designs, no removal after them). `door resynth --max-size 5` with lamps/notes/targets
  under `--seamless full+hall` on 57b: nothing in 7 min (stopped for the volume metric).
- 57b's 10 x 10: column x = -4 is the M chain (4 blocks), row y = 7 the top crossing (7 blocks),
  column x = 5 the right feed (4 blocks).
