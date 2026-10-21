# World-record piston extenders: the bar (study 6, track pext)

vjeux 2026-10-20 12:45Z: "world record kind of implementations for every single one of these...
quality over quantity". This page sets, for every reach N, the bar a top redstoner would expect,
derived from piston mechanics (26.3 source, our redsim), then tracks each N until it meets the bar
or the bar is shown impossible. Designs below the bar leave the catalog (folded or removed).

## What a piston extender is judged on

`circuit: piston-extender N`: one lever (or button) pushes a payload exactly N cells and brings it
back, everything back to rest, 3 cycles, 4 positions x 4 rotations in the real game. The record
qualities, in the order a builder cares about them:

1. **General**: any payload block (not only sand falling home), horizontal push (works in any of
   the 4 horizontal directions and up/down where noted). Gravity-only designs (sand payload,
   travel up) are a separate, labelled class.
2. **Compact**: blocks (the lever and piston heads not counted), then footprint.
3. **1-wide and tileable**: every block in the vertical plane of the push, nothing sticking out
   sideways, so copies stand side by side (doors, walls of extenders).
4. **Fast**: extend (lever on -> payload held at N) and retract (lever off -> all at rest), in gt.
5. **Single input**: one lever; a button version when the design takes one as is.

## Bounds from the mechanics

* **Blocks >= N + 2 (N >= 2)**. A stationary extender needs N pistons (an extended piston adds one
  cell, a piston cannot move itself; flying-machine "extenders" that crawl and come back are a
  different category) and the payload; and for N >= 2 at least one more block: on the off edge the
  front piston must act twice (pull, then reach-pull after the piston behind it has pulled it
  home), and the lever changes once, so something must re-trigger it (an observer, a carried
  redstone block, a delay).
* **Extend >= 2N gt, retract >= 2N gt**. Each push or pull moves the payload one cell and takes
  2 gt (a moving block for 2 ticks); a moving block cannot be pushed or pulled again, so the
  payload's N moves are sequential. Slime-free stacks need N(N+1)/2 single-cell pulse actions on
  the off edge (`pext trains --cells`, BFS-shortest), so their retraction grows like N^2.
* **1-wide footprint >= (2N + 1) x 2**: the push line spans the rest line (N pistons + payload)
  and the payload's far cell 2N; one more layer for the power.

## The bar per N

"Now" = the best archived design (catalog). Targets are the record bar: meet it, or show by search
that it cannot be met (then the bar becomes the proven optimum).

| N | now: smallest general | now: 1-wide | now: fastest | BAR (general, 1-wide) |
|---|---|---|---|---|
| 1 | n1-sticky 2 blocks, 3/3 gt (trivial) | same | same | not a record (a lone sticky piston): drop from the catalog's headline |
| 2 | sf7 7 blocks, 7/12 gt (5x2x2); rider7-flat 7, 9/9 (5x3x1) | rider7-flat 7, 9/9 | sf7 7/12 | **<= 6 blocks, 1-wide, extend <= 5 gt, retract <= 9 gt**; or prove 7 optimal (exhaustive 1-wide search, k-block additions to every A-B-payload-lever base) |
| 3 | flat-cascade16 16 blocks, 28/34 gt (9x5x1) | same | rider20 15/18 gt (20 blocks, 7x4x3; 10-21 anneal of rider24) | **<= 12 blocks, 1-wide, height <= 4, extend <= 12 gt, retract <= 18 gt** |
| 4 | flat-cascade26 26, 43/55 gt | same | cascade33 41/53 | **<= 16 blocks, 1-wide, extend <= 16 gt, retract <= 24 gt** |
| N >= 5 | flat cascade 10N - 5 - floor((N-1)/3), 14N-6 / 16N-8 gt | same | same | **one 1-wide family, <= 3N + 4 blocks, extend <= 4N gt, retract <= 6N gt**, judged up to N = 12 |

Gravity class (sand payload, travel up): sand6 (6 blocks, 7/12 gt) is the N = 2 entry, labelled
"sand only, pushes up"; it does not rank among general extenders.

Slime-free: kept only where it is close to the record (N = 2: sf7). The slime-free N >= 3 stacks
(sf38 ... sf-sweep356, quadratic retraction) are below the bar and leave the catalog; the
slime-free schedule results stay in NOTES.md.

## Ideas toward the bar (to try, in this order)

* N = 2 at 6 blocks or faster: exhaustive `pext grow` over 1-wide bases (A, B, payload, lever
  placements) with k = 3 (palette: stone, observer, slime, honey, redstone block, piston, repeater,
  dust); speed: B triggered on landing (powered destination, no observer delay).
* N >= 3 fast and small: per stage one sticky piston + one slime + its trigger (3 blocks): a
  self-timed cascade whose round starts come from the units themselves (no spine), and pistons
  powered on landing instead of observer pulses (QC rails that no update reaches at rest).
* Retraction in one front-to-back pass (linear time) by slime/honey couplings that never include a
  piston's own base in its push.

## Minimum volume (vjeux 2026-10-21: "minimum volume")

The box the game judge reports: every non-air cell over the whole run (moving blocks and extended
heads count), the lever left out. `mctas circuit pext anneal` searches it directly (redsim now
reports that run box: `rs::Report::run_box`): random edits (delete, re-state, add next to a block,
move one cell, swap), the volume in the energy (1000 per cell), `--random-pick` (a random passing
candidate when none improves, so the box can grow before it shrinks), `--free-lever`.

| N | before | now (game: robust, flat 1-wide) |
|---|---|---|
| 3 | cascade16 9x6 = 54, 28/34 gt; cascade19 9x6 = 54, 27/27 | **pext-n3-flat-cascade16-9x5**: 16 blocks, 9x5 = 45, 28/34 gt (replaces cascade16); **pext-n3-flat-cascade18-9x4**: 18 blocks, 9x4 = 36, 27/27 (replaces cascade19: fewer blocks, as fast) |
| 4 | cascade23 12x6 = 72, 35/47 | **pext-n4-flat-cascade23-12x4**: 23 blocks, 12x4 = 48, 39/39 (the old one stays: faster extend) |
| 5 | cascade31 15x6 = 90, 48/64 | **pext-n5-flat-cascade31-15x4**: 31 blocks, 15x4 = 60, 50/55 (the old one stays: faster extend) |
| 6 | cascade37 18x6 = 108, 59/75 | **pext-n6-flat-cascade37-18x4**: 37 blocks, 18x4 = 72, 59/63 (replaces it) |
| 7 | cascade46 21x6 = 126, 74/94 | **pext-n7-flat-cascade44-21x4**: 44 blocks, 21x4 = 84, 70/76 (replaces it) |
| 8 | cascade52 24x5 = 120, 85/107 | **pext-n8-flat-cascade52-24x4**: 52 blocks, 24x4 = 96, 87/95 (the old one stays: faster extend) |
| 9 | cascade59 27x6 = 162, 92/114 | **pext-n9-flat-cascade59-27x4**: 59 blocks, 27x4 = 108, 94/104 (the old one stays: faster extend) |
| 10 | cascade69 30x6 = 180, 109/135 | **pext-n10-flat-cascade69-30x4**: 69 blocks, 30x4 = 120, 107/119 (replaces it) |
| 11 | cascade76 33x6 = 198, 118/146 | **pext-n11-flat-cascade75-33x4**: 75 blocks, 33x4 = 132, 123/135 (the old one stays: faster extend) |
| 12 | cascade82 36x6 = 216, 129/157 | **pext-n12-flat-cascade82-36x4**: 82 blocks, 36x4 = 144, 127/145 (replaces it; three anneal runs: 36x6 -> 36x5 -> 36x4) |

What changed, every N: the row of observers under the line is gone (they ride on top of the
slimes instead), and the starters above were rearranged so nothing moves outside 4 rows (the
16-block N = 3 keeps 5).
The length stays 3N (the line of N pistons and N - 1 slimes plus the payload, then N cells of
travel); the bound is 2N + 1 with no slime between the pistons.

Any 3D shape (vjeux 10-21: "huuuge for a triple piston extender"), searched, nothing smaller yet:
* `pext anneal` without `--plane-z` from the 9x4 N = 3 (two seeds, 30 000 steps each): every
  3D move costs volume; nothing below 36.
* `pext grow` census: the 9x4 N = 3 with its top row removed (lever kept outside the box), up to 5
  blocks grown back inside 9x3: none works (11 min, 64 threads).
* From the 3D designs (anneal without `--plane-z`): pext-n3-rider24 (8x5x3 = 120, 25/25 gt) went
  to 20 blocks, 7x4x3 = 84, 15/18 gt: the fastest N = 3 (archived as pext-n3-rider20-7x4x3,
  rider24 removed as dominated), still more volume than the flat 36; pext-n3-cascade22
  (9x4x4 = 144) went to 18 blocks, 108.
* The 3D cascades for N = 4..8 compacted the same way (3 or 4 rows by 3 or 4 deep), each faster
  than its old self and archived in its place: N = 4 cascade27 12x3x4 36/42 gt, N = 5 cascade36
  15x3x3 48/48 gt (the fastest N = 5), N = 6 cascade44 18x3x3 56/76 gt (fastest extend), N = 7
  cascade55 21x4x3 68/90 gt (fastest extend), N = 8 cascade59 24x4x3 82/88 gt (the fastest N = 8).
  Their volume (144 .. 288) stays above the flat 3N x 4.
* N = 2 in 5x2 (the 2N + 1 length; the second row below, above or beside; lever on the back
  piston, outside the box): up to 5 blocks, none works. rider-flat 6x2 = 12 stays.
* A shorter N = 3 needs a line without slimes between the pistons (2N + 1 = 7 long); its
  retraction needs the 6 single-cell actions of the slime-free stacks, whose logic has never fit
  a small box (sf43: 10x5x5).

## Status

| N | status | record design |
|---|---|---|
| 1 | to drop (trivial) | |
| 2 | 6 blocks 1-wide: NONE exists in the searched space (`pext topo --k 3`: the lever on the back piston, the second piston and the payload at all 11 placements of a 3x4 window, every 1-wide placement of 3 more blocks within 2 cells, 22 block states; `--free-lever --k 4`: the same with the lever anywhere in the plane, on any block). 7 blocks 1-wide with the lever on the back piston: only the rider and its mirror (frame under or over, slime or honey frame), all 9/9 gt (`--k 4`); 7 blocks with the lever elsewhere not searched (`--free-lever --k 5`). | pext-n2-rider7-flat (7, 9/9 gt, 6x3x1); sf7-1 (7, 7/12 gt, slime-free, 2 wide) |
| 3 | 16 blocks, 28/34 gt, 9x6x1 (was 19, 27/27): `pext lns` (remove 3, grow 2) on cascade19 gave 18; `pext lns --free-lever` (the lever may move) gave 17 then 16 (faster too); from 16 nothing smaller (remove 3 / grow 2, remove 4 / grow 2). Bar (12) not met. | pext-n3-flat-cascade16 (robust; cascade19 stays as the 27/27 variant) |
| 4 | 23 blocks, 35/47 gt (was 26, 43/55): `pext lns --free-lever --repeat` 26 -> 25 -> 24 -> 23; from 23 nothing smaller (remove 3, grow 2) | pext-n4-flat-cascade23 (robust) |
| 5, 6 | N=5: 31 blocks, 48/64 gt (was 33, 50/66; `lns --free-lever`, nothing smaller from 31). N=6: 37, 59/75 (was 39, 61/77; `lns --free-lever --repeat`: remove 2 / grow 1 gave 38, remove 3 / grow 2 gave 37, nothing smaller from 37) | pext-n5-flat-cascade31, pext-n6-flat-cascade37 (robust) |
| 7..12 | `lns --free-lever --remove 2 --add 1 --repeat` (seconds each): N=7 46 (was 48, 74/94 gt; remove 3 / grow 2: nothing smaller), N=8 53 then 52 with remove 3 / grow 2 `--local 2` (85/107 gt, was 54, 87/109; nothing smaller from 52), N=9 59 (was 61, 92/114 gt; remove 3 `--local 2`: nothing smaller); N=10 69, N=11 76, N=12 82: nothing with remove 2; N=10 remove 3 `--local 2`: nothing smaller (2.75 h); N=11 the same (4.2 h); N=12 the same (5.9 h) | pext-n7-flat-cascade46, pext-n8-flat-cascade52, pext-n9-flat-cascade59 (robust); n10..n12 unchanged |

### Why one-frame / staircase riders fail at N = 3 (redsim traces, 10-20)
- A moved observer that is still powered when a second move catches it lands powered and never
  fires again (Java `onPlace`): two stages carried by one push both fire at once, and the block event
  order decides which one fails.
- A slime block drags its neighbours only when it moves itself; a block pushed alone slides off slime
  (that is what lets the rider's payload slide along its strip). A piston is never powered through its
  own face, so a unit's observer powers only the piston behind its slime.
- A rear push moves every unit in front, so every carried trigger fires at once: rounds (front first)
  are forced, and every round start costs a pulser plus spine.
- One frame carrying both stage pistons (`frame3`: the lever's piston shoves a slime strip, the middle
  and front pistons and the block ride on it and slide when pushed) runs the same 3-step train
  (front, middle, front) on both edges, but the front piston slides, so nothing it carries can fire
  it: `grow` adds no 3 blocks that make it work (12 blocks), 4 not searched to the end.
- Idea not built: hold every piston extended with the lever (the extension ripples in ~8 gt instead
  of 27) and release them front first with delays (each release is a round start); the holds need
  quasi-connectivity sources two above the line that are levels (a pushed redstone block or an
  inverted torch), about 21 blocks for N=3.
