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
- Screening: first `--screen-offsets` judge positions (default 2), 1 cycle, 200-tick cap:
  ~2500 candidates/s with 24 workers. Survivors get the full judge (4 + `--extra-offsets` 28
  positions x 3 cycles), so every accepted step passes `door verify --extra-offsets 28`.
  Failed designs are cached by hash; sandboxes are reused per worker. Candidates stay in their
  league (same category; no slime/honey added to a slime-free design).
- `--archive` archives each improvement next to `--out`; `--name` renames the design.

### `mctas door harvest --out-dir <dir> --top N --minutes M --moves all --plateau` (harvest.rs)
Gathers every design on HEAD and `origin/explore/*` (like `door leaderboard`), judges them, and
runs the search on the `--top` smallest working designs of each category that were not
harvested yet (`<dir>/harvested.txt`), writing `<dir>/from-<label>.txt` and archiving every
improvement in `<dir>/archive/`.

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
| **43** | sculk | top-control sculk BEST (48) | swap (-1), then a hold repeater with both its floors removed, plus a merge (-4) |

Speed (`--objective total`): the 88 gets (4,4,3) delay 4->1: close 31 -> 25 ticks at 88 blocks.
The speed leaders (gravity g4: open 5, close 11) are local minima for speed too.

Local minima for every family (plateau depth 1-2): rb2 57, mechanics 53/51/50, rb5 52,
center-mech 88, sculk 50s, gravity g4 51. The current records are hand-tight: their remaining
slack is in mechanism choices, not in wiring the local moves can see.

## Log

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
