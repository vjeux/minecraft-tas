# eligible: doors eligible for records under the Squid regulation

Track for vjeux's 2026-10-18 ask: the smallest (then the fastest) solid 3x3 doors that pass the
judge AND the regulation's record-eligibility rules (section 3 of the Squid rules,
~/agents/squid-door-rules.txt): repeater input (3.1.3), lever on the surface with a fixed support
(3.1.2, 3.1.5), a buildable hallway around the tunnel, isolated components (3.9, footsteps), a
SEAMLESS tier (3.10). Static checks: track [props](../props/NOTES.md) (`door properties`);
dynamic ones: track [props-dyn](../props-dyn/NOTES.md) (`door properties-dyn`). Since 20:20Z size
= the community VOLUME (`door properties` volume: circuitry bounding box over a cycle, lever, door
blocks and hallway-ring quartz excluded), blocks only as the tie-break; since 20:25Z every door
must be SEAMLESS, target FULL + hallway.

## Results (volume first)

| design | volume | blocks | seamless | lever open/close | repeater open/close | what |
|---|---|---|---|---|---|---|
| **[fl81n](fl81n.txt)** (BEST) | **81 (9x9x1)** | 45 | FULL + hallway | 52/48 | 53/49 | seamless's flat43-full + note block in the lever cell, lever on its east face (in-plane): flat, slime-free, rotation-safe |
| s55 + note (work/relever-flat) | 90 (9x10x1) | 56 | FULL + hallway | 40/40 | 41/41 | flat-seam's s55 the same way |
| [fv58n](fv58n.txt) | 100 (10x10x1) | 58 | FULL + hallway | 42/42 | 43/43 | flat-v2-57b + note block, lever on its east face |
| [f45n](f45n.txt) | 280 (7x10x4) | 45 | FULL + hallway | 36/28 | 37/29 | seamless's e41-full + note block |
| [b39n](b39n.txt) | 252 (7x9x4) | 39 | QUART z+ (no longer counts) | 36/32 | 37/33 | sf-holdA b38 + note block: the fewest blocks |
| [w43n](w43n.txt) | 252 (7x9x4) | 43 | QUART | 28/24 | 29/25 | wire19sf w42 + note block |
| hA43-full + note, s4-full + note (work/relever-seam) | | 49 | FULL | 24/24, 26/20 | 25/25, 27/21 | the fastest eligible FULL doors so far |

Every row passes, at all 32 judge positions: the judge, the rotation check, props-dyn's
repeater-input rig on both edges, props-dyn's hallway, the lever's place, and props-dyn's other
columns (contained, snow, steps, 20 cycles).

## Tools (this track)

- `door eligible <design>... [--extra-offsets 28]`: one line per design: the judge + rotation
  check, repeater input at EVERY judge position (props-dyn's rig: the lever left off, a delay-1
  repeater into its support block fed by a redstone block placed/removed behind it, both edges),
  then at the design's own position the hallway check, footsteps (sculk designs), the SEAMLESS
  tier, the volume and the lever's place. Prints ELIGIBLE or what fails.
- `door relever <design>... [--branches] [--seamless full] [--subs note,dust] [--why]`: makes
  working doors repeater-input eligible (below): every variant judged like a record and checked
  with `--eligible` (and `--seamless`); table with ticks, tier and volume, variants written out.
- `--repeater-input` on `minimize`, `regrow`, `grow`, `harvest` (SearchArgs) and `resynth`:
  candidates must also pass props-dyn's repeater-input check (screened at the first judge
  position, checked at all 32 after the full judge; the canonical form too).
- `--eligible`: `--repeater-input` + props-dyn's hallway check + footsteps (sculk) + a SEAMLESS
  tier + lever on the surface + fixed lever support. Combine with `--seamless full+hall`.
- `minimize --objective volume [--slack N]`: optimize the community volume (one recorded cycle
  per screened candidate), blocks as the tie-break; `--slack` lets blocks grow.

## Why almost every door failed repeater input

With repeater input the lever never changes state and powers nothing by itself: only its support
block gets (strongly) powered, in the scheduled-tick phase. Every leader read the LEVER: an
observer watching the lever cell (b38's O above the lever, w42's and flat-b-v7's support observer
watching it, fc2's O_a), or the lever's own weak power into a neighbor (b38's note block N below
the lever).

## The fix: a note block in the lever's cell, the lever moved onto it (+1 block, same volume)

Replace the lever by a note block and hang the lever on that note block. A note block changes
state (`powered`) the moment its power changes, on both edges, so the observer that watched the
lever sees the note block change at exactly the same time (same tick, same phase): the timing of
the whole machine is unchanged (repeater input adds the usual one tick). The lever (or the
repeater) strongly powers the note block, which conducts to its neighbors: b38's N below it still
toggles (it reads the strongly powered note block above), and N itself is still only weakly
powered, so the QC piston E that reads N's cell still does not see it. The lever does not count in
the volume and the note block sits where the lever was, so the volume does not change.

`door relever` tries that on every free face of the note block (`note-<face>`), and two families
that keep the lever on its old support: a note block in the lever cell next to the strongly
powered support (`note@support-<face>`: it toggles but powers nothing) and dust there
(`dust@support-<face>`, with a floor if needed: it also powers the lever's old neighbors).
Over the archive (242 designs): nearly every leader converts with `note-<face>`; none needed the
`@support` families so far.

What does not convert: fc2 and its family (fc2-seamL, riders fx1, m05-40, pareto mp8, exotic
pm45), whose lever cell feeds several readers with different timings, and the fast-clean/speed
doors (9/7 at 93-125 blocks), whose floor lever both strongly powers a support read by dust AND
weakly powers a piston next to it: neither family reproduces both (`--why`: "not open").

Dust in the lever's cell does NOT work for b38 (40 blocks): dust strongly powers the block below
it, so N became a conductor and quasi-connected E stayed powered, the top-center block was never
pulled back. A note block takes the instrument of what is below it (`relever` recomputes it: with
the wrong one the judge fails "not back to its rest state").

## Searches that found nothing

- b39n: `minimize --moves all --plateau --eligible` (39 is a local minimum), `resynth --eligible
  --max-size 4` (nothing), `regrow --size 3 --eligible` (stopped: volume became the target).
- f45n: `minimize --moves all --plateau --eligible --seamless full+hall`: 45 is a local minimum.

## Log
- 19:30Z merged explore/props + explore/props-dyn on top of the coordinator's local main.
- 19:52Z b39n (39, QUART): the first eligible solid door.
- 20:13Z merged origin/main 9728b29 (seamless's --seamless filter).
- 20:17Z f45n (45, FULL + hallway). 20:29Z fv58n (volume 100, FULL). 20:33Z fl81n (volume 81, FULL).
