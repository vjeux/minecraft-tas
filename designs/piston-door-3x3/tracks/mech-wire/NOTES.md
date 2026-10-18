# Track mech-wire: searching mechanism + wiring jointly

Goal: beat the solid record 40 (fc2 25 + 15, fx1, m05 21 + 19) by ranking mechanisms by their
TOTAL cost (mechanism + wiring), since every wiring search says each record is a local minimum
of its own wiring.

**Result: no design below 40.** The tool (below) wires a mechanism from its ideal-input schedule
end to end, automatically, into designs that pass the judge at 32 positions, but its wiring is
~12-14 blocks worse than the hand wirings of the same mechanisms (53 vs m05's 40), so its
ranking is an upper bound, not a record finder (yet). Details and what would close the gap:
"Why the router loses to hand wiring".

## Tool: `mctas door mech-wire` (crates/piston-door/src/mechwire.rs)

The missing link between `door mech-synth` (mechanisms + ideal schedules) and `door route2`
(the port/timing-aware router):

1. **Measure the mechanism with ideal inputs.** The schedule (redstone blocks set/cleared by
   script, `door ideal` format) is replayed for one open and one close phase; the frames go
   through route2's analysis (`route2::analyze`, split out of `route2::measure`): piston
   occupancy per tick, swept cells, cells next to moving slime, and what an observer watching any
   changing cell would emit (motion triggers). The input cells are ignored (free for wiring).
2. **Wanted waves**: every piston cell must get exactly what its ideal inputs gave it (directly
   or through its quasi-connectivity cell), every other piston cell nothing (route2 receivers,
   masked by occupancy).
3. **Lever spots**: free cells within `--lever-radius` of the `--lever-near` receivers that need
   power first, not touching a piston or the cell above one, hanging on an unmoving block of the
   mechanism, on a new block, or on a new OBSERVER watching the lever (the lever-observer gadget:
   lever + its block + the first pulse in 2 blocks). `--screen-top N`: each spot is screened by
   routing every receiver alone (no sharing); only the N best spots get the full routing.
4. **Route** every receiver from the lever with route2's router: A* (distance heuristic),
   `--partial` (a receiver fed by several sources, like the elevator piston pulsed by the lever
   and twice by the elevator's window, is connected in steps and goes to the back of the order),
   `--order-limit` (0 = beam over every order), served receivers re-checked every round.
5. **Judge** each schedule's best candidates (`--verify N`, the full judge: 32 positions x 3
   cycles), print a ranking of verified totals.

Timing (a mech-synth schedule fires its steps every 6 gt from tick 0, which no wiring does):
* `--retime --gaps 2,4,6 --max-tick T --first 1`: every assignment of step gaps (set ticks odd =
  observer pulses at even ticks), replayed with ideal inputs; the fastest working ones are wired.
  `--compress`: greedy shrinking of each gap instead (no slack left: poor wiring, see below).
* `--windows`: a cell pulsed twice per edge (a pulse-toggled piston pushed then pulled, the
  elevator's B) becomes ONE level window (one repeater) when the door still works that way.
* `--auto-free W`: an ideal-input sweep moves each event alone (-12..+24 gt). Events with a
  window of >= W ticks are NO-OPS (the door works without them: dropped, their ticks become
  don't-care for the cells they touched) or FREE (side and bottom pistons: the receiver accepts
  any 2 gt pulse per edge in the window; `--odd`: or 3, a pulse-toggled piston toggling 3 times
  = one net move timed by the last pulse, wire21's trick).
* `--timing`: every receiver also accepts any combination of its events' own windows.
* `--catalog-ids 355,356` reads mech-synth catalog entries (base + added blocks + SYM sequence)
  and builds their ideal schedules (mech-synth's `ideal_plan`); several schedule files at once.

route2 additions (opt-in, `door route2` behavior unchanged, fc2 calibration still 10/10):
`Receiver.alts / .free / progress`, `RouteOptions.partial / order_limit / threads / astar`,
parallel beam rounds, memoized transfer and a poison pre-check (~4x faster searches), and
`Problem::model` gives router-placed observers on moving cells their measured wave (before, a
candidate using one was judged with an OFF wave by the router's own check).

Example (wire21's abpt21r timing, ~1 min):
```
mctas door mech-wire designs/piston-door-3x3/tracks/wire21/ideal/chain.ideal --auto-free 8 \
  --timing --odd --partial --order-limit 0 --beam 8 --max-states 30000 --budget 36 \
  --lever-radius 2 --lever-near 3 --screen-top 4 --verify 20 --out-dir /tmp/mw
mctas door mech-wire designs/piston-door-3x3/tracks/mech-synth/catalog-21/m05-21b-sym.ideal \
  --windows --retime --gaps 2,4,6 --max-tick 27 --retime-max 1 ... (same routing flags)
```

## Results (all verified by the judge, 32 positions x 3 cycles)
| mechanism | schedule | wired | after polish | file |
|---|---|---|---|---|
| abpt21r (wire21 chain.ideal) | natural chain ticks | 53-56 (depends on the causality rule) | 53 (`--polish-secs`: route2 rip-up 56 -> 53), 51 (`door minimize --moves all` from 54) | `wired/abpt21r-chain-polished-53b.txt`, `wired/abpt21r-chain-min.txt` |
| m05 (catalog-21) | --windows --retime: steps [1,5,11,13,19,21,23] = wire21's hand timing, found by the grid | 53 | | `wired/m05-natural-53b.txt` |
| m02 (catalog-21) | --windows --retime: [1,5,9,11,17,19,21] (open 24 / close 24) | 52 | | `wired/m02-52b.txt` |

For comparison the hand wirings of these mechanisms: m05 = 40 (wire21), abpt21r ~41-43.

Not wired: m00 (T-mover extended at rest: a NOT-lever level, no torch in the default palette: no
lever spot reaches every receiver); m01 (lever spots and candidates exist, none of 16 passes
the judge: not diagnosed further); catalog-22 #355/#356 (no working retiming with gaps 2/4/6
up to tick 25: they need level inputs on extended pistons), #357 (one working retiming, no
lever spot reaches every receiver). Compressed schedules (`--compress`, no slack): 0 of 8-20
candidates passed the judge for m01/m02/#1-#4; natural timings (`--windows --retime`) pass.

## Why the router loses to hand wiring (m05: 53 vs 40)
* **No sharing by construction.** Each receiver is connected by its own cheapest path from the
  powered nodes; the hand designs make one block serve several receivers (m05's smooth stone at
  (0,4,0) takes O_a's pulse and O's two pulses and powers A in both positions; the (2,4,0)
  observer is a P_R chain link AND A's retract update).
* **Its model is conservative where the hand designs are clever**: quasi-connectivity through
  powered blocks is a "latent" pseudo-receiver that must stay off (m05's A-low is QC-powered
  through (0,4,0) and woken by the P_R chain); no lamps in the palette (m05's P_R driver turns 2
  pulses into 3 with a lamp); BUD/update tricks are invisible.
* **Exact timing.** Compressed schedules leave no slack, so the router must hit every tick and
  the judge then rejects most candidates (catalog-21 compressed: 0 of 8-20 pass); the
  `--retime` grid with `--windows` finds the natural timing (m05: wire21's exact hand timing).
* **Free events and motion triggers.** A free event (P_R) can be routed at a time different
  from the measured one; a motion trigger watching it (O_m for M) then fires at another time
  than the router assumed. Worse, the router fired free side pistons by watching their OWN
  column (a causality loop the static model accepts). Fixed in two steps: `own_motion` replays
  the schedule without a free receiver's events and collects every cell whose motion changes;
  no receiver may watch those cells, tap a node fed by an observer on them
  (`Model::downstream`), or pick one up as context (m02: 0 of 16 candidates passed the judge
  before, the first one after). The price: fewer cheap triggers (abpt21r 53 -> 56 before
  polish). A second measure-and-route pass with the realized times would give them back.

## Next ideas
* Sharing-aware routing: after the router's first full routing, rip each net in the context of
  all the others (route2's rip-up) and run `door minimize --moves all` (renet): 54 -> 51 so far.
* Lamps/note blocks/copper bulbs in the router palette (odd pulse counts, wire21's P_R driver).
* Second pass with realized event times for free events and motion triggers.
* q-free's constraint as a mechanism filter: anything that moves once per edge next to
  (0,-2,0) in fc2's family gives a shared P_L + P_M pulse (38).

## Log
- 05:10 read the brief and the track notes (mech-synth, router2, wire21, fresh-c).
- 05:20-06:30 tool: ideal replay -> Measured -> wanted waves -> lever spots -> route_net ->
  judge; compression, event windows, no-op/free events, catalogs, A*, partial routing, model
  fix for router-placed observers. First automatic door: abpt21r 54 (verified), minimize 51.
- 06:30-07:20 lever-spot screen, per-schedule ranking, --windows + --retime (m05 53 with the
  hand timing found by the grid). Batches over catalog-21 and catalog-22 M-first entries.
- 07:20-08:00 the judge rejected almost every candidate: free side pistons watched their own
  column. Causality rule for free receivers; m02 52 verified. --polish-secs (route2 rip-up +
  local search). Final batches: catalog-21 m01/m03/m04/m06/m10, catalog-22 #357/#358/#359/#361.
- 08:00-08:25 exact causality (`own_motion` + taint through `Model::downstream`); re-runs:
  m02 52, abpt21r 56 -> 53 with `--polish-secs`. Ran out of time for the catalog batches
  (each mechanism costs ~3 min of retime grid + 1-2 min of routing on a loaded box).
