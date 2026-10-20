# Study 10, BUD switches: world-record targets

vjeux (2026-10-20 12:45Z): "world record kind of implementations ... quality over quantity". The
critic's C32 adds: per variation, one design that wins it, and its card leads with the reason a
builder picks it; anything that wins nothing folds. Judge: `mctas circuit sim|verify` with kind
`bud` (docs/circuits.md); census proofs: NOTES.md "Census" (redsim palette, connected designs, no
fences: redsim doesn't model them).

## Why each class set is its own variation (the cards' headline)
- **place remove state shape** (observer class): the general change detector.
- **place remove** (presence detector): ignores the target's own state changes (a crop growing, a
  door opening, a dust's power). That is why a builder takes it over the observer.
- **place** only / **remove** only: one edge of presence (an item arriving, a block mined).
- **place remove state update** (the classic BUD): sees pure block updates, which no observer can.
  Variants: never jams (two updates close together), latched (holds until reset).
- **all five**: everything an observer or a BUD sees, on one output.

## Record board

| variation | target | holder | status / proof |
|---|---|---|---|
| observer class, smallest | 1 block (bound) | bud-observer | **record** (k = 1 census) |
| observer class, tileable every 1 | 1 block (bound) | bud-observer, its output read from the observer itself (no judge's lamp, so neighbors' lamps can't light each other) | **record** (was bud-observer-stone, 2 blocks: folded) |
| observer class, fastest | latency 1 gt | none (observers: 2 gt) | target: a QC piston path, as bud-slime-qc-fast does for its classes |
| presence, smallest | 2 blocks | bud-fence-observer (footprint 3) | holds; the k = 2 census (no fences) has no other |
| presence, footprint 1 | 3 blocks | bud-torch-dropper / bud-torch-lamp (silent), both tile every 1 reading the observer | k = 2 census: only observer-class sets; needs a fence case analysis |
| presence, tileable every 1 | 2 blocks | bud-fence-observer, output read from the observer: copies stack every block up a column | **record** (was bud-fence-stone, 3 blocks: folded) |
| place only / remove only | 4 blocks | bud-place-only, bud-remove-only (latency 5, tile every 1) | k = 3 census: neither set exists in 3 (no fences); target: lower latency |
| classic BUD, smallest | 3 blocks | bud-slime-qc (jams on 2 gt pairs) | **record** (k <= 2 census: none) |
| classic BUD, never jams | 5 blocks | bud-qc-nojam (latency 3, re-arm 8) | target 4: the k = 4 census covered sticky pistons only (686 designs, all jam) |
| classic BUD, latched | 2 blocks | bud-latched-qc | **record** (one block can't latch) |
| classic BUD, latched, latency 1 | 7 blocks | bud-latched-qc-fast | target: fewer blocks |
| classic BUD, latency 1 | 9 blocks | bud-slime-qc-fast | target: fewer blocks |
| all five | 13 blocks | bud-all-five | target <= 8, open. No 5-block design from the slime QC BUD + an observer + 1 block (census k = 1, observer east, south or below the target). Every observer watching the target has its back 3 blocks (Manhattan) from the piston, so its pulse needs 2 more blocks (a dust pair) to reach it; an observer next to the slime block is dragged along. The 8-block dust-pair version meets the slime QC's jam (a second update 2 gt after the first): the record's step-cut avoids it. The k = 2 census (31 states) costs ~15 h at the BUD judge's ~1 s per design: not run |
| any class set, instant (latency 0) | 0 gt | none | target: a place seen in its own tick (a placed block powered at once by a torch under it), or prove impossible for a pulse |

## Work list
1. all five <= 8 blocks (observer + QC BUD sharing the target cell, outputs merged; <= 5 has no
   design from the slime QC BUD + observer + 1 block).
2. latency 1 designs in fewer blocks (classic, latched, observer class).
3. Card captions: each design's first comment line names the variation it wins.
4. Fold whatever wins nothing; the catalog's `beats_on_variation` has the BUD metrics.
