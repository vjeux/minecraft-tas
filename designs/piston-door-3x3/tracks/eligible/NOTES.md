# eligible: doors eligible for records under the Squid regulation

Track for vjeux's 2026-10-18 ask: the smallest (then the fastest) solid 3x3 doors that pass the
judge AND the regulation's record-eligibility rules (section 3 of the Squid rules,
~/agents/squid-door-rules.txt): repeater input (3.1.3), lever on the surface with a fixed support
(3.1.2, 3.1.5), a buildable hallway around the tunnel, isolated components (3.9, footsteps), at
least QUART SEAMLESS (3.10). Static checks: track [props](../props/NOTES.md) (`door properties`);
dynamic ones: track [props-dyn](../props-dyn/NOTES.md) (`door properties-dyn`).

## Tools (this track)

- `door eligible <design>... [--extra-offsets 28]`: one line per design: the judge + rotation
  check, repeater input at EVERY judge position (props-dyn's rig: the lever left off, a delay-1
  repeater into its support block fed by a redstone block placed/removed behind it, both edges),
  then at the design's own position the hallway check, footsteps (sculk designs), the SEAMLESS
  tier and the lever's place. Prints ELIGIBLE or what fails.
- `--repeater-input` on `minimize`, `regrow`, `grow`, `harvest` (SearchArgs) and `resynth`:
  candidates must also pass props-dyn's repeater-input check (screened at the first judge
  position, checked at all 32 after the full judge; the canonical form too).
- `--eligible`: `--repeater-input` + props-dyn's hallway check + footsteps (sculk) + QUART
  SEAMLESS or better + lever on the surface + fixed lever support.

## Why almost every door failed repeater input

With repeater input the lever never changes state and powers nothing by itself: only its support
block gets (strongly) powered, in the scheduled-tick phase. Every leader read the LEVER: an
observer watching the lever cell (b38's O above the lever, w42's support observer A watching it,
fc2's O_a), or the lever's own weak power into a neighbor (b38's note block N below the lever).

## The fix: a note block in the lever's cell, the lever moved onto it (+1 block)

Replace the lever by a note block and hang the lever on that note block. A note block changes
state (`powered`) the moment its power changes, on both edges, so the observer that watched the
lever sees the note block change at exactly the same time (same tick, same phase): the timing of
the whole machine is unchanged. The lever (or the repeater) strongly powers the note block, which
conducts to its neighbors: b38's N below it still toggles (it reads the strongly powered note
block above), and N itself is still only weakly powered, so the QC piston E that reads N's cell
still does not see it.

Dust in the lever's cell does NOT work for b38 (b38-a1..a4, 40 blocks): dust strongly powers the
block below it, so N became a conductor and quasi-connected E stayed powered, the top-center
block was never pulled back. The note block must carry b38's instrument for what is below it
(`instrument=bass`: a note block below; with `harp` the judge fails "not back to its rest state").

| design | blocks | lever open/close | repeater open/close | seamless | notes |
|---|---|---|---|---|---|
| [b39n](b39n.txt) | 39 | 36/32 | 37/33 | QUART z+ | sf-holdA b38 + note block at (1,5,-3), lever on its west face; slime-free, rotation-safe, props-dyn: hallway/contained/snow/repeater/steps/20 cycles all yes |
| w42-n1..n5 (work/) | 43 | 28/24 | 29/25 | QUART z-,z+ | wire19sf w42 + note block in the lever cell; every lever face works |

## Log
- 19:30Z merged explore/props + explore/props-dyn on top of the coordinator's local main.
- 19:52Z b39n: ELIGIBLE at 32 positions (judge + repeater input), rotation-safe.
