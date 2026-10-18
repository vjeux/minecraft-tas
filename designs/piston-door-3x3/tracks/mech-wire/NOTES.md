# Track mech-wire: searching mechanism + wiring jointly

Goal: beat the solid record 40 (fc2 25 + 15, fx1, m05 21 + 19) by ranking mechanisms by their
TOTAL cost (mechanism + wiring), since every wiring search says each record is a local minimum
of its own wiring.

## Tool: `mctas door mech-wire` (crates/piston-door/src/mechwire.rs)

The missing link between `door mech-synth` (mechanisms + ideal schedules) and `door route2`
(the port/timing-aware router):

1. **Measure the mechanism with ideal inputs.** The schedule (redstone blocks set/cleared by
   script) is replayed for one open and one close phase; the frames go through route2's analysis
   (`route2::analyze`, split out of `route2::measure`): piston occupancy per tick, swept cells,
   cells next to moving slime, and what an observer watching any changing cell would emit
   (motion triggers). The input cells are ignored (free cells for wiring).
2. **Wanted waves**: every piston cell must get exactly what its ideal inputs gave it (direct or
   quasi-connected), every other piston cell nothing (route2 receivers, masked by occupancy).
3. **Lever spots**: every free cell within `--lever-radius` of the receivers that need power
   first, hanging on an unmoving block of the mechanism, a new block, or a new OBSERVER watching
   the lever (the lever-observer gadget: lever + its block + the first pulse in 2 blocks).
4. **Route** every receiver from the lever with route2's router (A* now, `--partial` for
   receivers fed by several sources, `--order-limit` to route the earliest receivers first),
   rank mechanism + lever + wiring by blocks, judge the best (`--verify N`).

Timing (a mech-synth schedule fires its steps every 6 gt from tick 0, which no wiring does):
* `--compress`: steps re-timed to chain ticks (step 0 at --first, set at odd ticks = an observer
  pulse at even ticks), each gap shrunk to the smallest of `--gaps` that still works with ideal
  inputs (2 positions x 2 cycles).
* `--retime`: every assignment of gaps (grid), the working ones wired.
* `--auto-free W`: an ideal-input sweep moves each event alone (-12..+24 gt). Events with a
  window of >= W ticks are either NO-OPS (the door works without them: dropped, and their ticks
  become don't-care for the cells they touched) or FREE (a side or bottom piston that may fire
  any time: its receiver accepts any 2 gt pulse per edge in the window, `--odd`: or 3, a
  pulse-driven piston toggling 3 times = one net move timed by the last pulse, wire21's trick).
* `--catalog-ids 355,356` reads mech-synth catalog entries (base + added blocks + SYM sequence)
  and builds their ideal schedules (mech-synth's `ideal_plan`).

route2 additions (opt-in, `door route2` unchanged): `Receiver.alts` / `.free` (accepted
alternatives), `RouteOptions.partial / order_limit / threads / astar`, transfer memoization and
a cheap poison pre-check (~4x faster searches).

## Log
- 05:10 read the brief and the track notes (mech-synth, router2, wire21, fresh-c).
- 05:20-06:15 tool: ideal replay -> Measured -> wanted waves -> lever spots -> route_net ->
  judge; compression, event windows, no-op/free events, catalogs, A*.
