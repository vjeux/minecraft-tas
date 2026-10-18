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
| [s55.txt](s55.txt) (archive/flat-seam-s55-55b.txt) | **9x10 = 90** | 55 | FULL | yes | 40 / 40 | flat-v2-57b compacted by `resynth --fit` (x >= -3): the M chain's 4 blocks in column x = -4 regrown inside, then one block less |

## Tool: `door resynth --fit x0,y0,z0:x1,y1,z1` (compaction)

Removable blocks outside the box count as offenders (like `--seamless` repair): only groups through
them are cut, each grows back INSIDE the box with up to `--extra` more blocks (every accepted
replacement leaves fewer blocks outside), and every replacement circuit is clamped to the box, so
the shrinking search that follows keeps the design inside. Pistons, the lever and its support are
never moved (they must already be inside). Combine with `--seamless full+hall` to keep the tier.
57b -> s55 took 78 s (4 out-of-box blocks: 57 -> 57 -> 58 -> 57 -> 56, then 55).

## Log
- `door minimize --moves all --plateau --shape flat --seamless full+hall` on 57b: nothing (48
  neutral designs, no removal after them). `door resynth --max-size 5` with lamps/notes/targets
  under `--seamless full+hall` on 57b: nothing in 7 min (stopped for the volume metric).
- 57b's 10 x 10: column x = -4 is the M chain (4 blocks), row y = 7 the top crossing (7 blocks),
  column x = 5 the right feed (4 blocks).
