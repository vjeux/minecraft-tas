# props-dyn: dynamic door properties

Track for vjeux's 2026-10-18 ask: check every WORKING door (leaderboard verdict cache `ok`, main's
archive + HEAD + every `explore/*` branch) against the community regulation's dynamic properties.
The cheap/static ones are agent `props` (`properties.rs`); these are in
`crates/piston-door/src/properties_dyn.rs`, command:

```
mctas door properties-dyn [designs...] [--archive-dir designs/piston-door-3x3/archive] [--branches]
    [--verdicts designs/piston-door-3x3/archive/verdicts-v5.tsv] [--uncached] [--workers 24]
    [--out-tsv designs/piston-door-3x3/archive/properties-dyn-v1.tsv]
```

Writes the TSV (key: `hash` = the leaderboard's design hash, 16 hex digits, same as
`verdicts-v5.tsv`, so it joins `properties.rs`'s columns) and the markdown table next to it (`.md`).

Every check runs ONCE: the design as written (offset 0), as built (no turn), 1 open/close cycle
(snow 2, endurance 20). The judge's own pass criteria are reused (`closed_problems`,
`open_problems`, rest state restored over the whole watch box, no entities); the surroundings are
part of the rest state (a hallway or shell block that gets dragged away is a failure).

| column | what | pass |
|---|---|---|
| hallway | smooth stone in every empty cell face-adjacent to the 3x3 tunnel (columns left/right/below/above the opening, door plane included) over the build's depth extent + 3 each way, raw-placed before the build settles | the judge's cycle checks pass |
| contained | a shell flush around the swept bounding box (every cell held at rest or at any tick of a cycle, plus opening and walkway), the tunnel's 3x3 columns left open: air, smooth stone, slime, honey, obsidian, one seeded random mix | works with all 6 (the `gap` variant puts the same shells one block further out) |
| snow | a snow layer (setBlock with updates, as weather: observers see it) on every sky-exposed top face that can hold one (`canSurvive`), in each stable state, 2 cycles | opens and closes, no entity appears, nothing but snow ever changes; `flammable_exposed` lists burnable blocks with a sky-exposed top |
| repeater | lever left off; a delay-1 repeater points into the lever's support block, a redstone block placed/removed behind it (outside the build, its swept cells and the tunnel); input changes in the scheduled-tick phase | opens and closes; ticks counted from the tick before the repeater's output changes (= a lever flip's tick 0) |
| steps | `GameEvent.STEP` (new bridge op `gameevent`, `Worker::game_event`) at every walkable feet cell of the tunnel within 3 of the door, in both stable states | nothing changes for 40 ticks and until idle |
| 20 cycles | `crate::verify` with 20 cycles at offset 0 (walkway-clutter rule off, as older verdicts) | passes |

## Findings so far
- First full run (2026-10-18 19:22Z, 240 working designs = every `ok` in verdicts-v5 across the
  archive, HEAD and the explore/* branches; 126 s on 24 workers): hallway 4 yes / 236 no;
  contained (flush) 146 yes / 94 no (all failures: smooth stone + slime (+ mix)), with a 1-block gap
  240 yes; snow 206 yes / 34 no (13 designs have a flammable block on top: targets, leaves);
  repeater input 14 yes / 148 no / 78 no site; footsteps 220 yes / 20 no (exactly the sculk
  designs); 20 cycles 240 yes. The air shell (= no surroundings) passes for all 240, so the
  harness agrees with the judge.
- Repeater input fails for every door that reads the LEVER BLOCK itself (observers watching the
  lever: fc2's O_a / P_L pair, logic-min's pulse observer) or the lever's weak power into a
  non-support neighbor: the lever never changes state, only its support block gets powered.
- `n/a (no site)`: the lever's support has no free horizontal side whose repeater could stand
  outside the build and the walkway (sculk-ew s4r: lever above the opening, the repeater's floor
  would be in the walkway).
- Contained (flush): slime doors whose structure ends on the swept box's face drag the shell
  (smooth stone, slime; the mix) and jam; obsidian and honey shells usually pass; with a one-block
  gap everything tried so far passes.
- Hallway: slime touching the tunnel's side cells sticks to the hallway wall (fc2, sculk-ew), and
  doors that pull blocks below or beside the opening (fc2's M dent) fail.
- Steps: sculk doors trigger on footsteps in the walkway (sculk-ew s4r: the calibrated sensor
  hears them from the closed side).
