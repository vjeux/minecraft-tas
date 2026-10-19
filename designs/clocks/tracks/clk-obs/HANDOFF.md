# clk-obs HANDOFF (block-update clocks)

Track clk-obs of the clock study: observers, pistons and sticky pistons (moved blocks, moving
observers), copper bulbs, note blocks, lamps, trapdoors and their hybrids. Branch
`explore/clk-obs`; crate `crates/clock-obs` (`mctas clock obs <cmd>`, see `--help`); notes and
results: [NOTES.md](NOTES.md); verified clocks: [VERIFIED.md](VERIFIED.md) (+ `logs/`); captions:
[captions.tsv](captions.tsv); live board: Space memory `minecraft-tas/clocks/families/obs.md`.

## What exists
- `best/`: the smallest clock of each period found (38 periods + note ring + divider), all CLOCK
  under `mctas clock verify` (8 positions x 4 rotations; robust except P21, rotation-dependent).
  Reported to the coordinator 2026-10-19 16:33Z.
- `families/`: the smallest clock of each (period, family) under the in-place family rule
  (verification: `logs/verify-families.log`).
- Hand designs: obs2-p6 (pair), ring4h (P8), ring6 (P12), lampring6 (P26), notering6 (P10),
  div3 (P48 divider); `drive/` = their first real-game measurements with `door drive`.

## How to reproduce / continue
- Exhaustive census: `mctas clock obs census --k 5 --out-dir D` (18 min, 160 threads; k=4: 6 s).
  Palette with `--palette` (shorthands in NOTES.md). Only MINIMAL clocks are kept.
- Grow: `mctas clock obs grow D --rounds 2 --write-per-family 2 --out-dir G` (round 1 from the k=5
  census: 8 min; round 2: ~1.5 h).
- Pick: `mctas clock obs pick D G ... [--by-period] --out-dir P` then `mctas clock verify P/*.txt`
  then `mctas clock obs report --logs LOG --designs P --out VERIFIED.md`.
- Relay rings: `mctas clock obs rings --max-len 10 --relay note,lamp,bulb,trap`; divider:
  `mctas clock obs divider <base> --bulbs K`.

## Open
- Missing periods (<= 60): 3, 25, 29, 31, 33, 34, 39, 41, 44, 45, 47, 49-51, 53, 55-57, 59 (the
  7-block grow was running).
- P3: impossible? (necessary: only pistons 0-ticking every 3 gt; none <= 5 blocks).
- A constructive odd family (every odd P): a piston stage that adds an odd delay needs a power
  pulse longer than an observer's 2 gt (else the sticky piston spits); see NOTES.md.
