# Rules: the judge, the categories and the boards

The single source of truth for what counts as a working 3x3 piston door and how designs are
ranked. Everything else (README, AGENTS.md, the leaderboard, the history) links here. The checks
live in `crates/piston-door/src/lib.rs`; the boards in `crates/piston-door/src/leaderboard.rs`.

## The door

A design file (format: [tools.md](tools.md#design-files)) names two ports:

* `door`: the minimum corner of the 3x3 opening. The opening lies in the plane `z = door.z` and
  spans x and y from that corner to +2.
* `lever`: a lever, unpowered at rest. Switching it on opens the door, switching it off closes it.

The **walkway** is the opening extended 3 blocks in front of and 3 behind the door (54 cells).

## The judge: `mctas door verify <design> --extra-offsets 28`

A door counts only if this passes when built from **main**: 3 open/close cycles at 32 world
positions. The coordinator re-verifies every record that way. The checks may never be weakened or
bypassed, and how designs are placed (`piston-door/src/video.rs::place`) may never change: a
design that only passes with a modified verifier does not count.

1. **Legal blocks.** Nothing a survival player cannot place: no command or structure blocks,
   barriers, light, bedrock, spawners, portals, `moving_piston`, fire, infested blocks... (the
   `FORBIDDEN` list), and no piston head without its extended piston. Every other block and
   mechanic in the game is allowed.
2. **It settles.** The design is pasted as written (raw, no updates), wire shapes are computed,
   every block gets one self update, and the build must then go idle.
3. **Every block can stand where it is** (since 2026-10-18 09:48Z). In the settled rest state every
   block passes the game's own `canSurvive`: no lever, torch, dust or repeater on air. (The
   minimizer once produced a floating lever.)
4. **Nothing changes by itself** (since 12:03Z). No block may take random ticks
   (`isRandomlyTicking`): unwaxed copper oxidizes, leaves decay, crops grow. Use waxed copper:
   fresh-d's unwaxed copper bulb oxidized, the observer watching it saw the change, and the door
   fired by itself at cycle 20 of a 40-cycle run.
5. **Closed at rest:**
   * the 9 door cells hold 9 identical plain full blocks; none is a mechanism block (slime, honey,
     pistons, observers, redstone blocks, lamps, targets, repeaters, comparators, dust, torches,
     levers, dispensers, droppers, hoppers, note blocks, TNT);
   * the walkway has no collision;
   * the `lever` port holds an unpowered lever.
6. **Nothing in front of or behind the door** (vjeux, since 2026-10-18 16:40Z): at rest the 3x3
   columns in front of and behind the opening, at any distance, are air: no machine parts, dust,
   torches, rails or levers. Designs judged before the rule keep their cached verdicts for now.
7. **Open.** The lever is switched on and the build runs until idle. Then the 9 door cells and the
   walkway have no collision and hold no moving block, and there are no entities.
8. **Close.** The lever is switched off and the build runs until idle. Then the door cells hold
   their rest blocks, the whole watched box (the design plus the walkway plus a 2-block margin) is
   identical to the rest state, and there are no entities (nothing broke or dropped).
9. **Bouncing is allowed** (vjeux, 15:07Z): only the settled states after each flip are judged, so
   the doorway may close and reopen mid-cycle (sf-holdA's 38 does).
10. **3 cycles at every position.** Dust updates its neighbors in a position-dependent order, so a
    door must work wherever it stands. Plain `verify` uses 4 world offsets; a record needs
    `--extra-offsets 28` (32 positions).

**Score**: the number of non-air blocks, piston heads excluded.

## Categories

`door verify` prints `category solid|gravity|sculk`, plus `, slime-free` and `, flat` when they
apply, and `rotation-safe` or `NOT rotation-safe`.

* **solid**: the main ranking; everything that is neither gravity nor sculk.
* **gravity**: the 9 door blocks are sand, red sand, gravel, suspicious sand or gravel, or concrete
  powder. Such a door can fall, so it ranks separately.
* **sculk**: the design contains a sculk sensor. Footsteps or mobs nearby can trigger one (in real
  play a player standing on a sensor breaks some sculk doors), so it ranks separately. Gravity wins
  if both apply.

## Rotations

Every working design is also run turned 90, 180 and 270 degrees at the same positions (the game's
own `BlockState.rotate` turns every block). Redstone updates neighbors in a fixed direction order,
so a door can work facing one way only. This is not part of the judge: the boards rank
**rotation-safe** designs, and the others are listed on their own board, "Not rotation-safe", with
the first turn that breaks them (sculk-2's 36-block doors work facing north or south only).

## The boards (`mctas door leaderboard`, LEADERBOARD.md)

`door leaderboard` judges every design on main and on the agents' `explore/*` branches with main's
verifier and writes [LEADERBOARD.md](../designs/piston-door-3x3/LEADERBOARD.md) (generated: never
edit it by hand). Each board shows its leader with a GIF, a video and how it works, the rest
collapsed.

| board | who is on it | ranked by |
|---|---|---|
| **Solid** (main ranking) | category solid, rotation-safe | blocks |
| Solid, no slime or honey | solid, no slime or honey block anywhere (`, slime-free`) | blocks |
| Solid, flat (one z layer), no slime or honey | slime-free solid designs whose every block (door, pistons, wiring, lever) lies in the door's plane, one block thick (`, flat`) | blocks |
| Sculk | category sculk, rotation-safe | blocks |
| Gravity | category gravity, rotation-safe | blocks |
| Not rotation-safe | working designs of any category that break when turned | blocks |
| Pareto frontier: open, close, blocks (any category / solid only) | every door that no other door beats on open ticks, close ticks and blocks at once | - |
| Fastest open / close / open + close (any category / solid only) | rotation-safe | ticks, ties by blocks |

* **Ticks** are game ticks after the lever flip, the worst of the 3 cycles at the first position
  (the design as written). Other positions can differ by a few ticks (gravity g10 opens in 3 to 7
  depending on where it stands).
  * open = the first tick from which the door cells and the walkway stay walkable (a moving block
    counts as blocking);
  * close = the first tick from which the 9 door cells hold their rest blocks.
* **Walkway clutter**: before the 16:40Z rule, blocks without collision (dust, torches, levers,
  rails) could stand in the walkway; the boards show how many do at rest, "clean" means none.

## Checks outside the judge

Not required for a record, run by the coordinator on board leaders ([tools.md](tools.md#correctness)):

* `door buildable`: a survival player can build it (a placement order that ends exactly in the
  judge's rest state, then the judge's cycles from the built state). Results in the
  [archive catalog](../designs/piston-door-3x3/archive/README.md).
* `door vanilla-check`: the unmodified official 26.3 server behaves the same, tick for tick.
* `door vanilla-build`: a bot player replays the build script on the official server.
