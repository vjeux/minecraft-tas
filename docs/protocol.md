# Bridge protocol

One JSON object per line on the worker's stdin; one response line per request on stdout, in
order, echoing `id`. Errors: `{"id":..,"ok":false,"error":"..."}`. On startup the worker prints
`{"event":"ready","version":"26.3"}`. Positions are `[x,y,z]`.

| op | request fields | response |
|---|---|---|
| `ping` | | `{}` |
| `info` | | `version`, `sandboxes`, `heap_used`, `heap_max` |
| `create` | `chunks: [minCX,minCZ,maxCX,maxCZ]` (default `[-1,-1,0,0]`), `seed` | `sandbox`, `min`, `max` |
| `clear` | `sandbox` | back to empty void, game time 0, reseeded |
| `drop` | `sandbox` | destroys the level |
| `set` | `sandbox`, `blocks: [[x,y,z,"state"],...]`, `mode: "raw"\|"update"\|<flags>` | `changed` |
| `fill` | `sandbox`, `from`, `to`, `state`, `mode` | `changed` |
| `get` | `sandbox`, `from`, `to`, `moving?` | `from`, `size`, `palette`, `flags`, `data` (base64 u16 LE, x fastest, then z, then y), `time`, `moving?` |
| `use` | `sandbox`, `pos`, `face?` | `result` (levers/buttons toggled directly; other blocks get a mock player's right click) |
| `step` | `sandbox` or `sandboxes`, `ticks`, `until_idle?`, `record?: {from,to}` | `results: [{sandbox, ticked, time, idle?, palette/flags/frames?}]` |
| `status` | `sandbox` | `time`, `block_ticks`, `fluid_ticks`, `block_events`, `moving_pistons`, `entities`, `idle` |
| `entities` | `sandbox` | `entities: [{type, pos, uuid, motion, yaw, pitch, design, still, collides, on_ground, box, state}]` |
| `summon` | `sandbox`, `entities: [{type, pos: [x,y,z] (decimals), nbt?}]` | `uuids` (a design's entities: deterministic UUIDs and entity randoms, tagged `mctas_design`; mobs refused) |
| `update` | `sandbox`, `positions`, `kind: neighbors\|self\|shape` | `changed` |
| `exec` | `sandbox`, `command` | runs a vanilla command in the sandbox |
| `viewers` | | `count`, `names` (game clients connected; worker started with `--listen`) |
| `camera` | `sandbox`, `pos`, `yaw`, `pitch` | moves viewers there in spectator mode |
| `play` | `sandbox`, `ticks`, `tps` | steps at a wall-clock pace for viewers |
| `hold` | `ms` | serves viewers without ticking |

* `raw` placement writes exact states with no neighbor/shape updates and no `onPlace`
  (flags `UPDATE_CLIENTS|KNOWN_SHAPE|SUPPRESS_DROPS|SKIP_BLOCK_ENTITY_SIDEEFFECTS|SKIP_ON_PLACE`);
  `update` is `Block.UPDATE_ALL`, like a player.
* `idle` = no scheduled block/fluid ticks, no block events, no moving-piston block entities, no
  falling blocks, and every design entity (`summon`) at rest: `still` = the last tick did not move
  it, horizontal motion exactly 0, vertical motion 0 unless it stands on the ground (a living
  entity carries gravity after its move every tick). A sandbox with no design entity is idle on
  the block conditions alone.
* `step` takes `entity_watch: {from, to, walk_from, walk_to}` (designs with entities): after every
  tick it records `entity_blocked` (a colliding or pushable entity overlaps the walk box), the
  first `entity_escape` (an entity box outside the watch box) and the first `entity_foreign` (an
  entity without the design tag: a drop).
* `rotate` takes `entities: [{type, nbt?}]` too and returns their data turned like structure
  templates turn entities (`Entity.rotate` / `mirror`): the new `Rotation` (and a hanging entity's
  facing), every other key as given.
* An entity's `state` is its saved data minus position, motion, UUID and the volatile fields
  (fall distance, fire, air, hurt time...), keys sorted, plus `mctas_rail` (the rail a minecart is
  on); a minecart's yaw and flip are left out (they record its last direction of travel).
* `flags[i]` for palette entry `i`: `collides` (any collision shape) and `full` (full cube).
* Requests can be pipelined: write many, then read the responses.
