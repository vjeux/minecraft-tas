# vanilla-check track

Goal: prove the simulator's verdicts match REAL vanilla Minecraft 26.3, not design doors.

## The tool: `mctas door vanilla-check <design>...`

```
./target/release/mctas door vanilla-check designs/piston-door-3x3/archive/rb6-bmobs-47b.txt \
    [--extra-offsets 4] [--positions N] [--cycles 3] [--verbose] [--keep-world]
```

* Starts the OFFICIAL server jar unmodified (`java -jar server-bundle-26.3.jar nogui`, the jar
  `mctas setup` downloads) in a fresh world under /dev/shm: void superflat
  (`{"layers":[{"block":"minecraft:air","height":1}],"biome":"minecraft:the_void"}`), peaceful,
  `spawn_mobs`/`spawn_monsters`/`advance_time`/`advance_weather` off (the judge's sandbox
  rules), RCON on a free port, online-mode off, compression off, `/tick freeze`, `/save-off`.
* A tiny offline game client written in Rust (`crates/mctas/src/vanilla/bot.rs`, protocol 777)
  logs in as `mctas_bot` (creative), stays flying out of the build, and RIGHT-CLICKS THE LEVER
  (`use_item_on`). Server side that is `LeverBlock.useWithoutItem` -> `pull(state, level, pos,
  null)`, the very call the judge makes (a `/setblock` of a powered lever would NOT be the same:
  it skips the attached block's neighbor updates and the vibration game event).
* World readback: `save-all flush` + a Rust region-file reader (`vanilla/mod.rs`, `nbt.rs`):
  every block state of the judge's watch box after every tick, plus the saved scheduled block
  ticks and block entities (moving pistons) for the judge's `idle` (falling blocks via
  `execute if entity @e[type=falling_block]`). 26.x quirks: dimensions live in
  `world/dimensions/minecraft/overworld/region`, palette entries are a bare name for a block's
  DEFAULT state (expanded through the simulator), `{id, properties}` otherwise, and strings in a
  mixed list are wrapped as `{"": name}`.
* Per position (the design translated by the judge's offsets; SAME absolute coordinates in both
  worlds, since dust update order is position dependent):
  1. simulator: the judge's exact sequence (raw set, wire shape pass, `self` update of every
     block, settle), recording the watch box every tick;
  2. vanilla: clean the box (`fill air strict`, kill entities, 80 ticks so pending ticks and the
     torch burnout memory expire), `forceload` + wait for `execute if loaded`, bot teleported
     near the lever BEFORE placement, then `/setblock <pos> <state> strict` of every block
     (strict = flags 2|816 = UPDATE_CLIENTS|KNOWN_SHAPE|SUPPRESS_DROPS|
     SKIP_BLOCK_ENTITY_SIDEEFFECTS|SKIP_ON_PLACE, bit for bit the judge's RAW_FLAGS), using the
     states after the judge's wire shape pass; then `tick step 1` + readback until idle, flip,
     step until idle, flip, ... 3 cycles;
  3. both runs go through the same rule checks (`vanilla_check::judge`, a copy of `verify_at`:
     closed at rest, lever port, open + walkway walkable, door restored, whole box back to rest,
     no entities) and every frame is compared cell by cell. Output: PASS (identical tick for
     tick), FAIL (vanilla fails the rules, frames identical to the simulator), or DIVERGES
     (first differing tick and cells).
* Idle and flip schedule: vanilla steps exactly as many ticks as the judge did in each phase
  (when the judge settled), so every lever flip happens at the same tick in both worlds; at that
  tick vanilla must be quiet on its OWN observable state (saved block/fluid ticks, moving
  pistons, falling blocks), else it keeps stepping and the extra frames are reported. Block
  events are not saved to disk; a hidden one would surface as a frame difference afterwards.
* Real-time cycle (first position, default on): `/tick unfreeze` at vanilla's 20 tps, the bot
  clicks while the game runs, the world is frozen and read back only after the judge's settle
  time + 20 ticks; the open and closed states must equal the simulator's.
* Known gap: vanilla has no command for the judge's `self` update (neighborChanged on every
  block after placement). Canonical designs are already settled, so it is a no-op for them; a
  design that depends on it shows up as a DIVERGES at `settle` (see `selftest-bud.txt`: a
  quasi-powered retracted piston extends in the judge, stays retracted in vanilla; the tool
  reports `settle tick 1: [7, 0, 0] sim sticky_piston[extended=true] / vanilla
  sticky_piston[extended=false]; [7, 1, 0] sim moving_piston / vanilla air`).

## Harness pitfalls found on the way (fixed; each one first looked like a game divergence)

* `save-all` writes only chunks marked unsaved, and scheduling or running a block tick does not
  mark a chunk unsaved: a chunk whose blocks did not change keeps a STALE `block_ticks` list on
  disk. v2-115b looked "idle" 3 ticks early in vanilla (door not restored at 2/8 positions)
  until every snapshot first flips a marker block at y=-64 in each chunk (strict, no updates)
  so every chunk is re-saved. With fresh tick lists v2-115b matches tick for tick.
* The last position of a design was not cleared before the next design (positions overlap):
  now the previous box (plus a 6-block margin) is wiped, ticks/entities/block entities checked.
* 26.x world layout and palette encoding (see above); the server's default `white-list`.

## Results

(see below, filled in as runs finish; logs in logs/)
