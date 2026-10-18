# Track search-tool: automated local search (Rust)

Goal: turn `door minimize` into a real local search usable for any contraption, run it on
v2-min and on the other tracks' bests, archive every improvement here with its source.

## The tool: `mctas door minimize --moves ...` (crates/piston-door/src/search.rs)

- Move families (`--moves`, comma list or `all`): `singles`, `pairs`, `triples` (removals;
  triples are local: two of the three distances within `--radius`), `delays` (repeater delay),
  `moves` (a block to a free face-neighbor cell), `subs` (dust<->repeater, torch<->observer and
  facings, smooth stone<->glass floors), `reroute` (rip up a dust segment between two anchors
  with its floors, re-lay it with the router from anywhere on one side of the net to the other
  end).
- Neutral edits (delays/moves/subs/neutral reroutes) are also tried with one removal within
  `--radius` (compounds). `--plateau`: when nothing improves, every neutral edit that passes the
  judge is followed by every single and pair removal. `--walk N`: then step to a random verified
  neutral design (at most N steps without improvement).
- Screening: first `--screen-offsets` judge positions (default 2), `--screen-cycles` (1), cap
  `--screen-ticks` (200). ~2500 candidates/s with 24 workers (a candidate that fails costs one
  position). Survivors get the full judge (4 + `--extra-offsets` 28 positions x 3 cycles), so
  every accepted step passes `door verify --extra-offsets 28`. Failed designs are cached by hash;
  sandboxes are reused per worker.
- `--archive` archives each improvement next to `--out`; `--name` renames the design;
  `--minutes` bounds the run.

## Results

| blocks | source | how |
|---|---|---|
| 113 | v2-min (main, 115) | reroute: the sides line drop re-laid from (6,4,2) to (5,4,0), -2 (first round, 15 s) |
| 109 | 113 above | renet: the lever net (7 terminals) re-synthesized from (6,4,2) as a greedy Steiner tree, -4 |
| **88** | center-mech BEST (96, v2u2) | repeater (3,4,4) turned into dust + renet of the merged X net (-6), then the lever drop reroute (6,4,2) -> (5,4,0) (-2); 16 s |

## Log

- v2-min: singles and pairs: no removal passes even the 2-position screen (115 is pair-minimal).
- v2-min, all families: round 1 found the reroute above (113). Round 2 at 113: nothing improving
  among 14k removals + 4.5k compounds; plateau: 59 of 281 neutral edits pass the judge, none of
  them followed by a single or pair removal (316k candidates, 160 s) improves.
- `--walk` (random walk over verified neutral designs) drifts into fragile designs: verified
  neutral neighbors fell 59 -> 27 -> 9 -> ... -> 0 in 6 steps. Not useful as is.
- renet (new family, net re-synthesis) on 113: -4 at once (109).
- 109 is a local minimum for every family incl. plateau depth 1 with pairs (297k candidates)
  and relaxed renet.
- center-mech's 96 (v2u2): -8 in 16 s (88). BEST.txt is now this 88 (v2u2 mechanism: sides
  pushed out by two pistons sharing an obsidian block, honey carrier for M, redstone blocks
  riding the side groups). Plateau depth 1 after it: nothing.
