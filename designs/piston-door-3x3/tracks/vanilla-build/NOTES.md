# vanilla-build track

Goal: close the last gap in the correctness story. `door buildable` proved (in the simulator) that a
survival player can build every board leader; `door vanilla-check` proved the simulator matches the
unmodified 26.3 server for raw-pasted designs. Nobody had replayed a BUILD SCRIPT on the real
server. `mctas door vanilla-build` does: a bot player executes the script's placements in the
official server, exactly as a player would, and the result is compared with the simulator after
every action, with the judge's rest state at the end, then through the judge's open/close cycles.

## The tool: `mctas door vanilla-build <script.build.txt>...`

```
./target/release/mctas door vanilla-build designs/piston-door-3x3/tracks/buildable/scripts/fc2-40b-2.build.txt \
    [--design <file|@builtin|ref:path>] [--extra-offsets N | --positions N | --offset x,y,z] \
    [--cycles 3] [--until-step K] [--real-time] [--slow N] [--stay] [--verbose] [--keep-world] [--tick-rate 100]
```

The design is the script's `# source:` line unless `--design` is given. Default positions: the
judge's four (`--extra-offsets 28` gives the judge's 32). Exit code 1 on any FAIL.

Per position (script and design translated by the judge's offset; same absolute coordinates in
both worlds):

* **Simulator.** The script's actions go through the bridge's `build` op ONE action per call (the
  same code path `door buildable` used, so the same clicks), and the judge's watch box is read
  after every action. The op now also reports the exact click it used (yaw, pitch, sneak, hit
  block, face, hit point) and the ticks it let pass (after the temporary dirt, after the block,
  before each right-click, after the action): additive fields in `Sandbox.placeLikePlayer`, no
  behaviour change. The script's click line is parsed and must equal the simulator's click (the
  script omits the yaw when looking straight up/down: it is taken from the simulator then, only
  when the rest of the click agrees). Then the post-build settle (20 ticks + until idle, as `door
  buildable`), the judge's own rest state (raw paste + shape + self updates, separate sandbox),
  and the judge's cycles from the built state.
* **Vanilla** (official jar, unmodified; vanilla-check's server, RCON, region-file readback). The
  bot (`vanilla/bot.rs`, protocol 777) does what the script says:
  * select the item: `/item replace entity mctas_bot weapon.mainhand with <item>` (the selected
    hotbar slot; `redstone_wire` -> `redstone` etc.);
  * look the script's way: `/tp mctas_bot <feet> <yaw> <pitch>` sets body AND head rotation
    (`teleportSetPosition`), which is what `getStateForPlacement` reads (`getHorizontalDirection`,
    `getNearestLookingDirections`); the bot answers the teleport before clicking;
  * sneak when the script says so: `ServerboundPlayerInputPacket` with the shift bit
    (`setShiftKeyDown` = `isSecondaryUseActive`: a sneaking player holding an item places against
    a repeater instead of cycling it);
  * right-click the given face of the given block at the face's centre
    (`ServerboundUseItemOnPacket`): server side `ServerPlayerGameMode.useItemOn` ->
    `BlockItem.place` (flags 11, `setPlacedBy`, BLOCK_PLACE game event), a player's click;
  * barrier: the server acks every click/break with `ClientboundBlockChangedAckPacket(sequence)`;
    once seen, the server has handled it and everything the bot sent before (one connection is
    handled in order), so RCON reads afterwards see the result;
  * click-set properties (repeater delay, the lever ON while building) and `right-click` steps:
    right-clicks with an EMPTY hand (`useWithoutItem`: `RepeaterBlock` cycles, `LeverBlock.pull`),
    face UP at the block centre like the simulator's `use`;
  * `break` steps and the temporary dirt: `ServerboundPlayerActionPacket(START_DESTROY_BLOCK)`, a
    creative player breaks at once = `playerWillDestroy` + `removeBlock` + `destroy`, no drops:
    the simulator's `breakLikePlayer`;
  * the temporary dirt is PLACED BY A CLICK too, sneaking, against a neighbouring block of its cell
    when there is one; in the void there often is none (the first block of a build), then an
    invisible anchor (glass) is pasted next to the dirt cell with `setblock ... strict` (no
    updates, no game event) where nothing else touches it, the bot clicks it, and it is removed
    the same way right after the click, before any tick. The dirt itself is a real player
    placement (flags 11 + BLOCK_PLACE), the same as the simulator's dirt.
  * where it stands: ON THE VIEW RAY when possible: eye = hit point minus 0.8..4.2 blocks along the
    look direction, the ray from the eye to the face crossing only air, the 0.6x1.8 body (plus a
    0.2 margin) clear of every block there before and after the action (and of every cell the
    machine ever sweeps when the action sets something in motion), within survival reach 4.5 of
    the clicked block and the placed one. Otherwise any clear spot within reach with the same
    look (the server checks reach, not sight); these are counted and listed with the reason.
  * while the world ticks after an action it waits far above the build (12 blocks over the watch
    box: out of every sculk sensor's 8-block range, touching nothing), since the simulator has no
    player; `--stay` keeps it at its click spot instead (a player standing by, see findings).
  * time: frozen, stepped exactly as the simulator's build did (`tick step n` for each of the
    reported waits); after every action vanilla must be quiet on its own observable state (saved
    block/fluid ticks, moving pistons, falling blocks) and its watch box must equal the
    simulator's, cell by cell, full block states.
* **Then:** the built vanilla world vs the judge's rest state (every block state of the watch box),
  entities (drops, or items the bot picked up), and the judge's cycles with the bot clicking the
  lever (vanilla-check's `flip`, `vanilla_record`, `judge`, `compare_frames`): tick for tick
  against the simulator's cycles from its built state.
* `--real-time`: the bot builds with the game RUNNING at 20 tps (where the simulator let the world
  run n > 0 ticks it waits n + 10; where it acted at once, the bot acts as fast as it can); every
  action is still compared once the world is quiet, the final state with the judge's rest; the
  cycles run frozen, tick for tick.
* `--until-step K`: replay the first K actions only and compare (minimal repros).
* `--slow N`: a slower player: N extra idle ticks after every action in BOTH worlds (then until
  idle if that woke something up; noted when the simulator itself wakes up).
* `MCTAS_VB_PROBE="x,y,z;bot"`: when vanilla is busy where the simulator is idle, print those
  blocks' NBT (a sculk sensor's listener and `last_vibration_frequency`) and the bot's position.

## Results (2026-10-18, official server jar sha1 33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c)

**No simulator divergence.** Every board leader's build script, replayed by the bot in the
unmodified server, leaves vanilla identical to the simulator after EVERY action (full block states
of the judge's watch box, and quiet when the simulator is idle), ends exactly in the judge's rest
state, and the judge's 3 open/close cycles from the built world match the simulator tick for tick.

Stepped, the judge's 4 default positions (32 for fc2-40b-2), logs/A, B, C, E, G:

| board | script | blocks | actions | positions | result | open / close |
|---|---|---|---|---|---|---|
| solid #1 | buildable/scripts/fc2-40b-2 | 40 | 40 | **32** | PASS | 24 / 9 |
| solid #2 | buildable/scripts/fc2-40b | 40 | 40 | 4 | PASS | 24 / 9 |
| solid (riders) | buildable/scripts/riders-fx1-40b | 40 | 40 | 4 | PASS | 24 / 26 |
| solid | buildable/scripts/piston-door-3x3-fresh-b-mp8-43b | 43 | 43 | 4 | PASS | 16 / 16 |
| slime-free #1 (now) | scripts/wire19sf-w43-43b (new, `door buildable`) | 43 | 43 | 4 | PASS | 26 / 24 |
| slime-free | buildable/scripts/slime-free-s7q-44b | 44 | 44 | 4 | PASS | 24 / 14 |
| sculk #1 (now) | scripts/sculk2-s1-38b (new; lever ON while building) | 38 | 39 | 4 | PASS | 19 / 38 |
| sculk | buildable/scripts/st43-bmobs-42b | 42 | 42 | 4 | PASS | 22 / 38 |
| gravity #1 | buildable/scripts/synth-g42-42b | 42 | 42 | 4 | PASS | 5 / 17 |
| gravity #2 | buildable/scripts/gravity-g10-45b | 45 | 45 | 4 | PASS | 5 / 19 |
| fastest close (solid) | buildable/scripts/speed-wire-w6-109b | 109 | 109 | 4 | PASS | 12 / 7 |
| fastest open (solid) | buildable/scripts/speed-wire-o6-119b | 119 | 119 | 4 | PASS | 11 / 7 |
| flat #1 (now) | scripts/flat-v2-54b (new) | 54 | 54 | 4 | PASS | 36 / 34 |
| flat | buildable/scripts/flat-v2-60b, design 07bdbd0 (55 b, see findings) | 55 | 55 | 4 | PASS | 36 / 34 |
| lever ON while building | buildable/scripts/riders-r3-43b | 43 | 44 | 4 | PASS | 22 / 33 |
| lever ON while building | buildable/scripts/fresh-d-s4-waxed | 45 | 46 | 4 | PASS | 26 / 14 |

Real time (the bot builds while the server runs at 20 tps; logs/D, H, L): fc2-40b, mp8, s7q, g10,
synth-g42, w6, o6, riders-fx1, riders-r3 (bot standing by, logs/D), st43, sculk2-s1, wire19sf-w43,
flat-v2-54b (bot parked, logs/H, L): all PASS. (logs/D's st43 FAIL is the player standing by
its sensor, finding 1; logs/H's sculk2-s1 FAIL came from the first real-time version waiting 10
ticks even where the simulator acts at once, finding 5; fixed, logs/L.)

Clicks: 2900 of 3300 clicks of the final-binary 4-position runs (logs/J, G, K: 88%) were made
from a spot ON the script's view ray; the others (85% "body", 15% "sight") from elsewhere within
reach (finding 3). Temporary dirt: placed by a click against a neighbouring block, or (the first
blocks of a build, in the void) against an anchor.

Final binary regression (logs/J): the 12 buildable-track scripts above again, all PASS at 4
positions with the bot parked while the world ticks.

Runs A-E kept the bot at its click spot while the world ticked (a player standing by); the final
default (runs G, H) parks it out of every sensor's range while the world ticks, as the simulator
has no player. Both pass stepped; only a slower player standing by a sculk door differs (below).

## Self-test (the tool does catch a wrong build)

`selftest-fc2-no-sneak.build.txt` is fc2-40b-2's script with ", sneaking" removed from step 32
(the support at -1 4 1 is a repeater). Expected: the bot right-clicks the repeater (delay 2 -> 3)
instead of placing the sticky piston. Result (logs/selftest-no-sneak.log): FAIL, first divergence
after step 32: `[-1, 4, 1] sim repeater[delay=2] / vanilla repeater[delay=3]; [0, 4, 1] sim
sticky_piston / vanilla air`, the built world differs from the judge's rest state, the vanilla door
fails cycle 0, and the tool notes that the script's click differs from the simulator's.

## Findings

1. **A player standing by a sculk door is a vibration source the simulator does not have**
   (real vanilla, not a simulator bug; the sculk board's known caveat, now with a concrete case).
   st43-bmobs-42b step 26 places a slime block at -1 4 0, the player looking straight down at it
   from just above (feet 0.38 above the slime top), 2.3 blocks from the sensor at 0 3 -1. The
   sensor runs its 41-tick cycle in both worlds; the moment it is receptive again, vanilla's sensor
   hears a frequency-2 vibration (hit_ground/bounce class) at power 11, i.e. from the player's
   position, and fires once more (41 more ticks of activity; nothing moves; the build still ends in
   the rest state and the door passes). A player who acts right away (the script's pace) masks it
   with the next placement; a slower one sees it. Repro (logs/I):
   `mctas door vanilla-build designs/piston-door-3x3/tracks/buildable/scripts/st43-bmobs-42b.build.txt --positions 1 --stay --slow 10 --until-step 26`
   (FAIL: "vanilla needed 33 more ticks to be quiet after the simulator's 41 (pending: [0, 3, -1]
   sculk_sensor in 23)"; `MCTAS_VB_PROBE="0,3,-1;bot"` prints the sensor's `last_vibration_frequency: 2`
   and the bot's position). The same command without `--stay` (bot out of range) PASSES, as does
   `--real-time` without `--stay`; the 38-block sculk leader sculk2-s1 passes even with `--stay
   --slow 10`.
2. **BUILDABILITY.md's flat row is mislabelled** (BUILDABILITY.md is now the archive catalog's
   buildable column, generated from the scripts). `flat-v2-60b.build.txt` was written at 04:56
   from `origin/explore/flat:.../BEST.txt`, which was by then the 55-block revision 07bdbd0 (04:53):
   55 actions = 55 blocks. Against `archive/flat-v2-60b.txt` the build ends without that design's
   bottom observer row in BOTH worlds (logs/B-flat-4pos.log: every action identical, built !=
   rest); against `07bdbd0:designs/piston-door-3x3/tracks/flat/BEST.txt` it PASSES. The 60-block
   flat was never shown buildable by that script; scripts should name a commit or an archive file,
   not a moving `BEST.txt`. The current flat leader (54 b) now has a script here (PASS).
3. **12% of script clicks have no standing spot on their exact view ray** within reach 4.5:
   mostly "body" (the ray is open but a 1.8-tall player does not fit around that eye, typically a
   pitch-0 click on a face whose row below is built), some "sight" (every point of the ray sees a
   block before the face: e.g. fc2 step 28, "click the west face of 2 -2 0, looking down 60", seen
   only through the cell under the door). `door buildable`'s visibility rule checks only the first
   cell of the ray. The server checks reach, not sight, so vanilla's result is unaffected; but for
   every script line to be literally doable, `visible()` should demand the whole ray open and a
   body-sized standing spot (orientation-free blocks like stone can use any other look). The off-
   ray steps are listed per position in the logs ("clicked off the view ray ... at steps ...").
4. **The scripts omit the yaw when looking straight up or down**, yet it decides the facing of
   repeaters, floor levers and the like (fc2 step 25: a repeater facing west placed "looking down"
   needs yaw 270). vanilla-build takes it from the simulator (only when the rest of the click
   matches the script); printing it ("looking down, facing east") would make scripts complete.
5. **Click-set properties happen in the same tick as the placement** in `door buildable`'s build
   op (repeater delays, the lever ON while building): it right-clicks as soon as the world is idle,
   0 ticks after placing. sculk2-s1-38b step 37 places the door lever and switches it ON at once;
   its sculk sensor (placed at step 35) hears only one of the two. The bot manages the same at
   network speed (both packets land between the same two ticks: PASS in real time, logs/L), but a
   player switching it on ~10 ticks later, mid sensor cycle, gets a different mid-build state for
   2 actions (the door's left column pulled one block, logs/H) that converges back to the rest
   state; the door still works. The variant with the switch-on as its own action
   (`sculk2-s1-38b-lever-split.build.txt`: lever placed OFF, then right-clicked after the world
   is idle) PASSES in both worlds, stepped at 4 positions (logs/K) and in real time (logs/L): the
   simulator agrees with vanilla when both do the same thing. `door buildable` could model these
   clicks as separate actions with a player's click rate (>= 4 ticks).

## Harness notes

* Offsets translate the coordinates in the simulator's click text: clicks are compared parsed.
* The bot's body was first kept clear of every cell the build EVER occupies, which put 21 of fc2's
  40 clicks off the ray (future blocks are not there yet); now: the cells occupied before and after
  the action, plus every cell the machine sweeps only when the action sets something in motion.
* Never `git stash -u` while runs write logs into the tree: the files are replaced and the runs
  keep writing to deleted inodes (recovered with `tail --pid=<pid> -f /proc/<pid>/fd/1`).

## Next ideas

* Run `door vanilla-build` from the leaderboard cycle on every new record's build script (about
  20 s per position; `door buildable` first, then this) so "buildable" means "built by a player in
  the real game".
* Tighten `door buildable`: whole view ray open + a body-sized standing spot (finding 3), the yaw
  printed for straight up/down looks (finding 4), click-set properties as separate clicks at a
  player's rate (finding 5), sources pinned to a commit or archive file (finding 2).
* Sculk doors: search build orders that keep the player out of the sensors' range while they are
  receptive, or flag steps like st43's step 26 in the script (finding 1).
* A survival bot (inventory, slot switching, timed breaking) instead of creative: same placement
  code path, more realism in the transcript.

## Not modelled

* The bot is a creative player (infinite items; `/item replace` instead of picking from an
  inventory). Survival placement runs the same `useItemOn` -> `BlockItem.place` (only the stack
  count differs); breaking in survival takes time but ends in the same `destroyBlock`.
* No ground: the judge's world is a void, so the temporary dirt (and, for it, an invisible anchor)
  stand in for the pillars and floor a player would use.
* The bot floats (never sends movement): no footsteps, no landing, touching no block.
* Stepped on the simulator's schedule (or real time with margins): a faster player acting while
  pistons are still moving is not tried (neither does `door buildable`).
