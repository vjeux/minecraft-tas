# clk-obs HANDOFF (block-update clocks)

Track clk-obs of the clock study: observers, pistons and sticky pistons (moved blocks, moving
observers), copper bulbs, note blocks, lamps, trapdoors and their hybrids. Branch
`explore/clk-obs`; crate `crates/clock-obs` (`mctas clock obs <cmd>`, see `--help`); notes and
results: [NOTES.md](NOTES.md); live board: Space memory `minecraft-tas/clocks/families/obs.md`.
The same branch also carries contraption study 1 (pulse circuits, crate `crates/circuit-pulse`):
[designs/circuits/tracks/pulse/NOTES.md](../../../circuits/tracks/pulse/NOTES.md).

## Headline results
- EXTENDABLE families, archived as cards (`extend/`; instance for any n: `mctas clock obs family
  <name> --n N --out FILE`; `--check K` confirms n = 0..K in redsim; `members <paths>` lists the
  designs that are instances): obs-ring P = 8 + 4n (4 + 2n blocks), note-ring P = 10 + 4n
  (6 + 2n), trap-ring P = 10 + 4n (6 + 2n), bulb-ring P = 20 + 8n (6 + 2n), odd-a P = 11 + 4n
  (8 + 2n), odd-b P = 13 + 4n (10 + 2n), divider P = 12 x 2^n (3 + 3n).
- EVERY period P >= 4 has a block-update clock: the census gives P4 (4 blocks), P5 (6), P6 (2),
  P7 (5), P9 (5); `mctas clock obs every` builds every P >= 8: observer rings (P = 0 mod 4),
  rings with one note block (P = 2 mod 4), and the ODD PISTON STAGE + observer tower (odd
  P >= 11; [oddring11.txt](oddring11.txt), 8 blocks; how it works: NOTES.md).
- P1, P2 impossible for this family: rows in `designs/clocks/impossible.tsv` (each component's
  own period, read in the 26.3 source). P3 OPEN: only pistons 0-ticking every 3 gt could do it;
  exhaustive hunts found none: every <= 5-block structure (all palettes), every 6-block polycube
  with 4 pistons + 2 redstone blocks (1.94 G sims), every 6-subset of a 3x2x2 / 2x3x2 box, and
  every 6-block polycube of piston/sticky/redstone block/slime/stone (21.9 G sims: periods 6, 9,
  12, 18, 24 only).
- Smallest clock per (period, family): exhaustive census k <= 5, grows to 6, 7, 8 and 9 blocks
  (50 M candidates, 258 K minimal), all archived by me: batch A (167, the smallest per period),
  B (719), C (552 second mechanisms, `mech2/`), D1 (61 new smallest per period P59..P444,
  `smallest9/`), D2 (the rest of `families9/`, ~1250), and the observer-free 6-block census
  (`pist6/`, 8).

## What exists
- `extend/`: the 7 family cards (archived; their 137 instances were removed from the archive).
- `families/`, `mech2/`, `smallest9/`, `families9/`, `pist6/` (+ the `.md` tables): the picks
  archived in batches A..D2, named `p<P>-<family>-<blocks>b[-a|-b]-<structure hash>`.
- `every/`: one construction per period 8..200 (`mctas clock obs every`).
- Hand designs: obs2-p6 (pair), ring4h (P8), ring6 (P12), lampring6 (P26), notering6 (P10),
  div3 (P48 divider), oddring11/13/23 (the odd piston stage); `drive/` = first real-game
  measurements with `door drive`.

## How to reproduce / continue
- Exhaustive census: `mctas clock obs census --k 5 --out-dir D` (18 min, 160 threads; k=4: 6 s).
  Palette with `--palette` (shorthands in NOTES.md). Only MINIMAL clocks are kept. Targeted hunts:
  `--max-period 3 --min-pistons N --min-rblocks N --max-rblocks N`; non-connected placements:
  `--box WxHxD`; observer-free: `--need-powered-piston` (lossless: nothing else starts a move).
- Grow: `mctas clock obs grow D --rounds 2 --write-per-family 2 --out-dir G`.
- Pick: `mctas clock obs pick D G ... --only-smaller-than designs/clocks/archive/verdicts-v1.tsv
  --skip-structures designs/clocks/archive [--mechanisms] --out-dir P`, then `mctas clock archive
  P/*.txt --workers 6`, commit designs/clocks/archive, push, one line to the coordinator.
- Family instances: `mctas clock obs family members <paths>` then `mctas clock obs prune
  --members M` (removes the instance files and both TSV rows); `prune --orphans` removes rows a
  union merge brought back for files that are gone.
- Relay rings: `mctas clock obs rings --max-len 10 --relay note,lamp,bulb,trap`; divider:
  `mctas clock obs divider <base> --bulbs K`; constructions: `mctas clock obs every --min A --max B`.
- Never rebuild `target/release/mctas` while a `clock archive` of family cards runs (it spawns
  the current binary to build members).

## Open
- P3: a 0-tick piston machine of period 3, or a proof (NOTES.md: necessary structure; a counting
  bound is NOT proved).
- Smaller clocks for the long periods only constructions reach: a 10-block grow or compact odd
  stages.
