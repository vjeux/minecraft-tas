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
| 3 | flat-cascade19 19 blocks, 27/27 gt (9x6x1) | same | rider24 25/25 | **<= 12 blocks, 1-wide, height <= 4, extend <= 12 gt, retract <= 18 gt** |
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

## Status

| N | status | record design |
|---|---|---|
| 1 | to drop (trivial) | |
| 2 | 6 blocks 1-wide: NONE exists in the searched space (`pext topo --k 3`: the lever on the back piston, the second piston and the payload at all 11 placements of a 3x4 window, every 1-wide placement of 3 more blocks within 2 cells, 22 block states; `--free-lever --k 4`: the same with the lever anywhere in the plane, on any block). 7 blocks 1-wide with the lever on the back piston: only the rider and its mirror (frame under or over, slime or honey frame), all 9/9 gt (`--k 4`); 7 blocks with the lever elsewhere not searched (`--free-lever --k 5`). | pext-n2-rider7-flat (7, 9/9 gt, 6x3x1); sf7-1 (7, 7/12 gt, slime-free, 2 wide) |
| 3 | 16 blocks, 28/34 gt, 9x6x1 (was 19, 27/27): `pext lns` (remove 3, grow 2) on cascade19 gave 18; `pext lns --free-lever` (the lever may move) gave 17 then 16 (faster too); from 16 nothing smaller (remove 3 / grow 2, remove 4 / grow 2). Bar (12) not met. | pext-n3-flat-cascade16 (robust; cascade19 stays as the 27/27 variant) |
| 4 | 23 blocks, 35/47 gt (was 26, 43/55): `pext lns --free-lever --repeat` 26 -> 25 -> 24 -> 23; from 23 nothing smaller (remove 3, grow 2) | pext-n4-flat-cascade23 (robust) |
| 5, 6 | 33, 39 (lns remove 3 / grow 2: nothing smaller) | pext-n5-flat-cascade33, pext-n6-flat-cascade39 |
| >= 7 | open (the minimized flat cascades) | |

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
