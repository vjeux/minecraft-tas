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
| `entities` | `sandbox` | `entities: [{type, pos, uuid}]` |
| `update` | `sandbox`, `positions`, `kind: neighbors\|self\|shape` | `changed` |
| `exec` | `sandbox`, `command` | runs a vanilla command in the sandbox |
| `viewers` | | `count`, `names` (game clients connected; worker started with `--listen`) |
| `camera` | `sandbox`, `pos`, `yaw`, `pitch` | moves viewers there in spectator mode |
| `play` | `sandbox`, `ticks`, `tps` | steps at a wall-clock pace for viewers |
| `hold` | `ms` | serves viewers without ticking |

* `raw` placement writes exact states with no neighbor/shape updates and no `onPlace`
  (flags `UPDATE_CLIENTS|KNOWN_SHAPE|SUPPRESS_DROPS|SKIP_BLOCK_ENTITY_SIDEEFFECTS|SKIP_ON_PLACE`);
  `update` is `Block.UPDATE_ALL`, like a player.
* `idle` = no scheduled block/fluid ticks, no block events, no moving-piston block entities.
* `flags[i]` for palette entry `i`: `collides` (any collision shape) and `full` (full cube).
* Requests can be pipelined: write many, then read the responses.
