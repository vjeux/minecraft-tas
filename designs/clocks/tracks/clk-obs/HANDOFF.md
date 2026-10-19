# clk-obs HANDOFF (block-update clocks)

Track clk-obs of the clock study: observers, pistons and sticky pistons (moved blocks, moving
observers), copper bulbs, note blocks, lamps, trapdoors and their hybrids. Branch
`explore/clk-obs`; crate `crates/clock-obs` (`mctas clock obs <cmd>`, see `--help`); notes and
results: [NOTES.md](NOTES.md); verified clocks: [VERIFIED.md](VERIFIED.md) (+ `logs/`); captions:
[captions.tsv](captions.tsv); live board: Space memory `minecraft-tas/clocks/families/obs.md`.

## Headline results
- EVERY period P >= 4 has a block-update clock: the census gives P4 (4 blocks), P5 (6), P6 (2),
  P7 (5), P9 (5); `mctas clock obs every` builds every P >= 8: observer rings (P = 0 mod 4),
  rings with one note block (P = 2 mod 4), and the ODD PISTON STAGE + observer tower (odd
  P >= 11; [oddring11.txt](oddring11.txt), 8 blocks; how it works: NOTES.md). `every/` holds
  P8..P200, all confirmed by redsim; the census gaps and samples up to P200 verified robust in
  the game (`logs/verify-every-gaps.log`).
- P1, P2 impossible for this family: rows in `designs/clocks/impossible.tsv` (each component's
  own period, read in the 26.3 source). P3 open: only pistons 0-ticking every 3 gt could do it;
  exhaustive targeted hunts found none (every <= 5-block structure; every 6-block polycube with 4
  pistons + 2 redstone blocks: 1.94 G sims; boxes next).
- Smallest clock per period: every P <= 49 except 1-3 and 50+ gaps (`best/`, archived by the
  coordinator; the 7-block grow added P25 at 6 blocks and P29..P156 at 7, archived by me).

## What exists
- `best/`: the smallest clock of each period found by 16:30Z (38 periods + note ring + divider),
  reported 16:33Z and archived by the coordinator.
- `families/` (+ `families.md`): the smallest clock of each (period, family) under the in-place
  family rule (or the next structure when the coordinator's batch has the smallest), 941 designs,
  named `p<P>-<family>-<blocks>b-<structure hash>`; I archive them with `mctas clock archive`
  (batch A: the smallest per period, 167; batch B: the rest, 774).
- `every/`: one construction per period 8..200 (`mctas clock obs every`).
- Hand designs: obs2-p6 (pair), ring4h (P8), ring6 (P12), lampring6 (P26), notering6 (P10),
  div3 (P48 divider), oddring11/13/23 (the odd piston stage); `drive/` = first real-game
  measurements with `door drive`.

## How to reproduce / continue
- Exhaustive census: `mctas clock obs census --k 5 --out-dir D` (18 min, 160 threads; k=4: 6 s).
  Palette with `--palette` (shorthands in NOTES.md). Only MINIMAL clocks are kept. Targeted hunts:
  `--max-period 3 --min-pistons N --min-rblocks N --max-rblocks N`; non-connected placements:
  `--box WxHxD`.
- Grow: `mctas clock obs grow D --rounds 2 --write-per-family 2 --out-dir G` (round 1 from the k=5
  census: 8 min; round 2: ~1.5 h, 181 M candidates).
- Pick: `mctas clock obs pick D G ... [--by-period] [--skip-structures DIR] --out-dir P` then
  `mctas clock archive P/*.txt --workers 8` (the judge + archive), or `mctas clock verify` and
  `mctas clock obs report --logs LOG --designs P --out VERIFIED.md`.
- Relay rings: `mctas clock obs rings --max-len 10 --relay note,lamp,bulb,trap`; divider:
  `mctas clock obs divider <base> --bulbs K`; constructions: `mctas clock obs every --min A --max B`.

## Open
- P3: a 0-tick piston machine of period 3, or a proof (NOTES.md: necessary structure).
- Smaller clocks for the periods only constructions reach (50, 51, 53, 57, 59, 61, 63, 67-69, 71,
  73, 75, 77, 81-83, 85, 88, 89, 91, 93-95, 97-103, ...): an 8-block grow round, or compact
  odd stages.
- Up to 2 structurally different mechanisms per (period, family): `cores` groups structures by
  their steady-state core; pick a second core per group.
