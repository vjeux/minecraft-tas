# Track search-tool: automated local search (Rust)

Goal: turn `door minimize` into a real local search usable for any contraption, run it on
v2-min and on the other tracks' bests, archive every improvement here with its source.

## The tools

### `mctas door minimize <design> --out <file> --moves all [--plateau] ...` (search.rs)
- Move families (`--moves`, comma list or `all`):
  - `singles`, `pairs`, `triples`: removals (triples are local: two of the three distances
    within `--radius`);
  - `delays`: another repeater delay;
  - `moves`: a block to a free face-neighbor cell (an extended piston moves with its head;
    the lever moves or re-attaches nearby and its port follows);
  - `subs`: dust<->repeater (all facings), torch<->observer and facings, slime<->honey,
    sticky<->normal piston (head follows), floors<->glass / glazed terracotta;
  - `reroute`: rip up a dust segment between two anchors with its floors, re-lay it with the
    router from anywhere on one side of the net to the other end;
  - `renet`: net re-synthesis: rip every non-terminal dust cell of a net (terminals = dust that
    touches a component, directly or through a plain block) and reconnect the terminals as a
    greedy Steiner tree (each terminal as the root), also with one terminal dropped, also with
    each repeater turned into dust (merging its two nets), strict and relaxed adjacency;
  - `swap`: a local pair or a face-connected triple of blocks out, one block of a vocabulary
    (floors, glazed terracotta, obsidian, redstone block, dust, repeaters, torches, observers)
    in, in any free cell within `--radius` (supported placements only). This is what finds
    "dust + floor -> one powered block" hops.
- `--objective open|close|total`: minimize worst-cycle ticks (then blocks) instead of blocks,
  within `--slack` extra blocks; feeds the speed leaderboards.
- Neutral edits are also tried with one removal within `--radius` (compounds).
- `--plateau`: when nothing improves, every neutral edit that passes the judge is followed by
  every single removal (`--plateau-pairs`: pairs too) and its own reroutes/renets.
  `--depth 2`: then a second neutral edit after each verified first one, then removals.
  `--walk N`: random walk over verified neutral designs (drifts into fragile designs; not
  recommended).
- The router knows "drag" cells (next to moving slime, never occupied): new floors there are
  glazed terracotta (slime never drags it), no dust. It treats every plain non-door block as a
  floor it can stand dust on.
- Screening: first `--screen-offsets` judge positions (default 2), 1 cycle, 200-tick cap, with a
  frame-less copy of the judge's checks (`--screen-frames` uses the judge's own code): ~8000
  candidates/s with 24 workers. Survivors get the full judge (4 + `--extra-offsets` 28 positions
  x 3 cycles), and every accepted step is re-verified in a fresh sandbox (the exact `door
  verify` path). Failed designs are cached by hash; sandboxes are reused per worker. Candidates
  stay in their league (same category; no slime/honey added to a slime-free design).
- `--max-open/--max-close` (from redstone-block): speed constraints for the speed boards.
- `--archive` archives each improvement next to `--out`; `--name` renames the design.

### `mctas door harvest --out-dir <dir> --top N --within W --minutes M --moves all --plateau [--every 15]` (harvest.rs)
Gathers every design on HEAD and `origin/explore/*` (like `door leaderboard`), judges them, and
runs the search on the `--top` smallest working designs of each league (categories +
slime-free) within `--within` blocks of the league best that were not harvested yet
(`<dir>/harvested.txt`), writing `<dir>/from-<label>.txt` and archiving every improvement in
`<dir>/archive/`. `--every M --hours H` keeps fetching and harvesting.

### `mctas door transplant --out-dir <dir> --top N --within W --max-cells 12` (harvest.rs)
Learns an edit from every pair of designs (a -> b with the same door port, b smaller, at most
`--max-cells` cells apart) and tries each on the best designs of every league where the touched
cells match; improvements are archived and polished by the local search.

### `mctas door reuse-check <design> [--times N] [--fresh]`
Verifies a design N times in one reused sandbox and counts passes (found the torch leak below).

## Results (all verified at 32 positions x 3 cycles)

| blocks | category | source | how |
|---|---|---|---|
| 113 | solid | v2-min (main, 115) | reroute: the sides line drop re-laid from (6,4,2) to (5,4,0), -2 (15 s) |
| 109 | solid | 113 above | renet: the lever net (7 terminals) re-synthesized, -4 |
| **88** | solid | center-mech BEST (96, v2u2) | repeater (3,4,4) turned into dust + renet of the merged X net (-6), lever drop reroute (-2); 16 s. Was the record for a while. |
| 85 | solid | top-control BEST (87) | plateau: neutral renet of the lever net, then a reroute (-2) |
| 67 | solid | redstone-block rb1 (73) | renet (same result as rb1-min) |
| 59 | solid | logic-min BEST (60) | observer (1,-1,1) turned east->north, observer (2,-1,1) removed |
| 58 | gravity | gravity g3 (60) | smooth stone -> glass then renet without a terminal (-1), then a floor removed (-1) |
| 67 | gravity | gravity g1 (69) | reroute (-2) |
| 46 | sculk | main archive rb2-sculk-48 | swap twice: a dust + its floor became one smooth stone (observer/repeater fed through a powered block), -1 each |
| **43** | sculk | top-control sculk BEST (48) | swap (-1), then a hold repeater with both its floors removed, plus a merge (-4); was the sculk record until st43-bmobs (42) was built on it |
| **46** | gravity | fresh-b gravity-g6 (49) | swap: obsidian + repeater -> one observer, then removals (-3); was the gravity record until g10 (45) was built on it |
| 46 | gravity | gravity g4 (51), gravity BEST (49), g8 (48) | wall torch moved + its dust/floor dropped, dust+floor pairs |
| 43 | sculk | m51-sculk (48) | swap + removals |
| 48 | gravity | gravity g4 (51) with `--max-open 5 --max-close 11` | speed boards: same 5/11 ticks as g4 with 3 fewer blocks |

Speed (`--objective total`): the 88 gets (4,4,3) delay 4->1: close 31 -> 25 ticks at 88 blocks.
The speed leaders (gravity g4: open 5, close 11; fc2: close 9) are local minima for speed too.

Local minima for every family (plateau depth 1-2, swaps): solid fc2 40, rb6-bmobs/m48-bmobs 47,
mechanics 48/51/53, rb2 57, rb5 52, logic-min 49/50/52/53, sculk st43-bmobs 42, gravity g10 45.
The records are hand-tight: their remaining slack is in mechanism choices, not in wiring or
components the local moves can see. Transplanting the tracks' own patches between lineages
re-derived known designs only.

## Log

- Bug found and fixed (bridge, 9e8c60d): reused sandboxes leaked redstone torch burnout memory
  (`RedstoneTorchBlock.RECENT_TOGGLES` is per level and only forgets toggles 60 game ticks later;
  `clear()` rewinds the clock to 0). On logic-min's slime-free 50, a torch at a reused position
  burnt out after 8 runs: every later screen/judge in that worker's sandbox failed (`door
  reuse-check`: 8/30 before, 30/30 after). The official judge is not affected (distinct position
  per offset), but every search reusing sandboxes could miss improvements on torch designs. All
  designs are re-harvested after the fix (old list in harvest/harvested-before-torch-fix.txt).
- Frame-less screening (screen_in): the judge's checks on the final state of each phase without
  per-tick regions; identical pass counts, 1.5-3x faster (~8000 candidates/s with 24 workers).

- v2-min: singles and pairs: no removal passes even the 2-position screen (115 is pair-minimal).
- v2-min, all families: round 1 found the reroute above (113). Round 2 at 113: nothing improving
  among 14k removals + 4.5k compounds; plateau: 59 of 281 neutral edits pass the judge, none of
  them followed by a single or pair removal (316k candidates, 160 s) improves.
- `--walk` drifts into fragile designs: verified neutral neighbors fell 59 -> 27 -> 9 -> ... -> 0
  in 6 steps.
- renet on 113: -4 at once (109); 109 is a local minimum for everything incl. plateau pairs.
- center-mech's 96: -8 in 16 s (88).
- Harvests (other tracks' bests): 87 -> 85, 73 -> 67, 60 -> 59, gravity 60 -> 58 and 69 -> 67.
- rb2's 14-block hold (lever -> dust out -> 3 repeaters back into Q) looked like a U the router
  cannot shorten; mechanics then found the 10-block version by hand (repeater straight off the
  lever, glazed terracotta floors next to the slime): that trick is now in the router.
