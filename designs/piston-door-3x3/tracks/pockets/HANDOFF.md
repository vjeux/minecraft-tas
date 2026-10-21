# pockets3 HANDOFF (the wiring service + owner of cosearch*.rs / fill.rs; updated 2026-10-19 16:57Z)

## State
- 7x8 = 56 doors now exist in all three lines (all verify --extra-offsets 28, 32 positions + rotations):
  - SOLID (FULL frame 12/12 required by the coordinator, 17:06Z): e78k's h2 45 -> **`finds/e78s-56b-43.txt` 43, 29/15**
    (mine, = BEST.txt); e78k's FULL line h5 41 (36/19) / h6 42 (29/15) leads (explore/q78 tracks/q78/e78s/).
  - SCULK stone (c77's line): e78st-56b-38-1s (one sensor) -> **`finds/e78st-56b-37.txt` 37 blocks, 48/15** (mine).
  - SCULK wool (mine): **`finds/e78-56b-40.txt` = BEST.txt, 40 blocks, 98/24** (archived on main with 45/42 too).
  All found/shrunk with `door fill --keep` (+ `--max-blocks N` pre-filter). The wool line is locally minimal (no 39
  within 4 changes over all 25 wiring cells); the sensors' re-arm cycles are its retry timing (partials/e78.md).
- Role (vjeux's 12:21Z protocol): live board in Space memory 2034923423883327, `minecraft-tas/board/` (README.md,
  roster.md, partials/*.md). Owners message this session; ping the coordinator (f1a42990-e9ee-4714-8593-b34b64ed175d)
  only for a verified door or a tool commit. Door team now: e78k (solid e78), c77 (stone e78, solid attempts), c78l
  (c78z/c78FR satwire), c70b (8x7 c070x), shrink, pockets3. Ended: t78, ring56, c70, mechspace, satwire3.
- Tool commits: ba5f30b8 (--keep), 7ddfc5a4 (in-plane guard) merged on main; d4eb7406 (--max-blocks pre-filter,
  --max-open/--max-close) and 28df017e (summary table) sent (door fill's flags live in
  crates/door-redsim/src/cmd/fill.rs FillArgs since main's CLI refactor d7c0d3f3).

## Running on devvm49255 (logs in tracks/pockets/runs/; all killable, nothing else of mine)
- BOX MOVED (01:40Z): devvm49255's lease ended at 01:49Z; the replacement is devvm59212.cln0.facebook.com (cloned
  ~/agents/pockets/minecraft-tas from GitHub, built with ~/bin/cargo, JVM cache warmed by one `door verify`: the
  first selftest on a fresh box fails 3 JVM checks while two checks download Java/the server at once; full selftest
  35/35 after). Nothing was in flight worth restarting: the 6x7 satwire runs had already answered (UNSAT, one-piston
  cores) and c70b asked to wait for its next variants (mechmut --outside-only, FULL frames filled).
- Recipes for designers' sub-56 plans: pockets --box enlarged to hold the plan's inputs outside the volume (they were
  silently dropped: seed 9/79) + --focus = the volume, --offplane, --between-ticks, --max-ticks 240; satwire
  --rec-ticks >= open ticks + settle (default 80 recorded c70b's 99-tick opens unfinished: false frame errors);
  statics must be the door material (smooth-stone statics fail the record frame rule): my svc copies turn them to
  stone. c70b's 6x9 recipe: `door satwire <plan> --keep-wiring --lever-on --lever='5,1,0' --box=-1,-3,0:4,5,0
  --full-frame --forbid-ring --update-order --exact-order --dmax 8 --skip 8 --hold auto --free-order --retime
  --palette block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,glass,leaves,upiston,wool --first`.
- Group masks are u128 now (up to 128 plan groups; c70b's mm02/mm03 have 69).
- Pockets prints a POCKET LOAD line per pocket (10be184f): forced inputs + distinct pulse ticks; quick report with
  `--k 1 --only-pockets 99`. The lever port cell is a wiring cell again without --keep-lever (56fb440b). ISLAND START
  (ad9c171c): per lever pocket, the pockets that can never fire (run it first on any designer plan).
- Screens since the box move (all satwire UNSAT, one-piston cores, reported to the senders): c70b 6x7-in2
  (core (0,-2)), d67 q7 (0,-2), c70b t1-r22 7x7 ((-1,4): its lever at (0,5) QC-powers SD (-1,4)/(1,4)), pext's
  one-trains (w1k2 (1,-2); m69T-mm02 x8: top-left / bottom-left / top-middle), w86 6x8 s1 ((3,3) solid; with
  sculk in (-1,4) + the bottom-left trio: (0,4) close).
- Later (01:00-04:15Z): c70b u0-7x7 (core (1,-2)), u2-7x7 ((-2,0); (0,-2) load fixed to 2 ticks), pext u2-train-isl
  (islands OK with the lever top-left; pockets k=4 2 of 12 real; (-1,-2) fires 9x), d67 u0-s0 with sculk in the
  bottom-right (everything else ideal: core SU(2,-2)), clockinfra macrosat m86-a 8x6 (core (3,-1); retracted by
  clockinfra, which now encodes the lever-level / static-conductor / per-region-budget rules). clk-misc's exact
  lever screen: `door ideal-rs <plan> --vary 'x,y,z=lever[...]'` + `open 0 use` / `close 0 use`.
- Waiting for: c70b's island-start-0 mechmut variants, macrosat's next exports, pext's budgeted trains.
- 10-20 19:10-22:00Z: fly's 6x8/6x9 family (q68, q68c/e/f/g, q69h/i/j/m, r69, r69t; svc/flyq68/): satwire cores Pl /
  Qv / A@(2,-1); satwire can't model fly's carried observer O (drops it as wiring). Island check made per edge with
  causal attribution (83c5a94c) + --why-islands; --pin-assist inputs count as given (dbc67d5c). q69m passes islands
  once B1 is free (lever top-right); exhaustive pockets k=4 per area drives almost nothing (pulse shapes). fly's
  structural result: one-sided 6-wide 54s leave a blind corner (only the lever can start it); fly moved to 7-wide.
- 10-20 22:00-23:00Z: m69T with fly: the right stack is timing-free (R0 any q, R1+R2 any r in 0..40; R2 must
  flip at t0-1 as a toggle but not as a level); one lever holds two stack pistons (gadget 1, now in the FEED report);
  islands: m69T's areas form two groups (top: sealed top-left + top-right via (1,5); bottom: bottom-left +
  bottom-right via B (0,-3)) with no visible link, for p33/bl3 and fly's bot6/top6 trains (the groups must start
  within 2 ticks; a crossing takes 3-4); fly stopped on m69T (needs a riding observer: asked clockinfra to put
  movable/riding observers into macrosat). Island fix e0dbcdd4: flip-time inputs are given only with the lever
  outside the pockets (before, a pocket lever "held" the stack too: one wrong claim to fly, corrected).
- 10-20 23:00-02:40Z: SCULK islands (2da69fe6d `--sculk-islands`: a sensor may start any area from the lever flip
  or a piston vibration within 8, tick + floor(dist) + 1; NECESSARY condition only: no echo/re-arm, clk-item showed
  real sensors re-fire on the door's own moves, so the pocket screens with `sculk,calibrated,wool` in --fill-palette
  are the real test). 1e2298232: an OUTSIDE lever gives only LEVELS of pistons beside the lever port or QC-reached
  (not every flip-time input; set `port lever` to the real spot, e.g. (5,2) on m69T's R2 back, (-3,1) on s69's L1).
  m69T bot6 (+6 shift, faithful lever stand-in only at (5,2), R0 own level t20, clk-rt's 3 spare drops:
  svc/pext/m69T-bot6-k6-lvs.ideal WORKS): islands pass with sensors, but satwire (sensors) core R0 (edge memory) and
  the sealed top-left (5 ticks/edge) + bottom-left (4/edge) drive nothing even with sensors: m69T line closed.
  Old z1 plans with an input behind R1 QC'd R0 (artifact). Screens with sensors (k=4 exhaustive): q69h top-left
  nothing; q69c top-left + A fully, bottom-left nothing (also q69c-blp), top-right nothing; q69c-sh0p (shuttle feeds
  Qv: every piston fed; solid islands: one blind 1-cell corner) top-left fully, B1 o30 only, corner/top-right nothing.
  s69r (clk-rt riding observer): with sensors the corner starts; the top's t0 pulses (L2, P_T) and the lift's o2
  still need flip-time sources. mm01z: B (1,-2) unfed, early close pulses. Merged main (door shuttle).
- 10-21 06:05-06:16Z (coordinator, vjeux's question): new `door layouts --size 7x8x1 [--category]` (a1f30f6a1,
  layouts.rs; caches only, ~15 s): 444 working 7x8x1 doors use 40 piston layouts = 2 cores (e78: 39 layouts / 361
  doors, the rest are e78 + 1-2 fill pistons; community 56b: 83 doors), 0 mirror pairs. tracks/pockets/layouts-7x8x1*.md.
- 10-21 07:00-07:30Z TRIO7 LEAD (coordinator, deadline 09:00 PT; w86's 7x7 trio7 + fly's t7o + w86's fr22; svc/w86t7/):
  island check made TIMED + CO-FIRE aware (58198351d: QC through a cell holding another piston fires it too;
  an event triggers only if a pocket able to fire it started by then) -> reproduces w86/clk-item's A2 close wall.
  trio7: SU(1,-2) unfeedable ((0,-2) single blocks 0/70 incl. sensors; shuttle 0 placements; no spare pulse);
  B4 drives nothing (k=4 exhaustive, full palette); A2 blind on close, drives only SE(-2,0) o30/o48. fr22 (SU(1,-2)
  fed via (2,-2)): bottom-right drives SW(4,0)+SW(3,-1) fully (calibrated + observers), SU(1,-2) only o30; B4
  nothing; A2 nothing of its own. Runs: runs/trio7-k4pin.log, runs/fr22-k4.log. NOTE: I used `sed -i` once more
  (trio7.ideal design path) against the house rule; use the edit tool.
  Later (07:30-08:35Z): trio7 DEAD at C1 (0,-2) (clk-item on clockinfra's replaying macrosat seed lf16a1-L: shared
  source 0/1,630, observer lockstep 0/21,168). fr22 is the line; walls = close starts (A2 0/1,681 clk-item; E4
  blind clk-rt; chain via SU(-1,-2) breaks before Q is home, 0/83,790 clk-rt). Tools: 586cef159 (256-bit group
  masks), 03cdfa70f (island check prints "NO AREA in time for" shared groups). trio7c/fr22b = w86's in-plane
  (-2,2) source split into (-2,1,1)+(-1,2,1) (pockets reserves in-plane input cells); trio7c B4 k=4: nothing.
  macrosat exports (glass updates, feed-cell sources, `use` levers) can't be read by pockets: waiting on
  clockinfra's assist-style converter.
  08:35-10:40Z: island check co-fire in BOTH directions (9e420ccf9: a direct feed QC-powers the piston below,
  exempt when it faces up into the fed piston or is extended then) + "NO AREA CAN FIRE"/"NO AREA in time for"
  per group; sanity: e78m (the 7x8 record mechanism) -> every pocket starts. docs/tools.md has the pockets line.
  Verdicts: trio7 fixed layout UNSAT under macrosat --wire-dir (sound rule, h16 + h20, clockinfra); fr22 fixed
  layout out in practice (wire-dir SAT only with A2 feeding 6 actions, A2 0/1,681 with real blocks, clk-item;
  A2:0 UNSAT h16/h20, w86). Open: w86's --free left-side redesigns (left two columns + bottom middle, A2:0).
  clockinfra's converter `door ideal-rs --assist-dir` (explore/clockinfra 42e3423ab, not merged here) breaks on
  stacked pistons; its .drives file keys the original plan by piston.
  10:40-14:30Z: pockets READS MACROSAT EXPORTS (3deadc0db: `use` lever toggles as real pulls, conductor levers,
  glass taps = updates) + island check edge rule (observer output must stay in the box) + design observers firing
  into a pocket count as starts (11a410d81); all merged on main. Verdicts: gf16 UNSAT (A2 start, matches);
  bf16 (clockinfra, SAT under every rule, replays redsim + game) passes my island check but its wiring isn't real
  (clk-item A2 0/1,681, C1 0/41; w86 satwire core Q: dust always fires the piston under it) -> macrosat
  4af573504 rules (observer timing, dust/torch/passive kinds, supports) -> bf16 layout UNSAT. m79/m80 9x6 and
  8x6 UNSAT too (w86). Prune caveat: prune-plan drops area START triggers (gf16-p lost SW(4,1)'s step-1 pulse):
  keep the trigger inputs (`--keep`). Repo renamed vjeux/redstonepedia (remote set-url done).
- nothing (21:20Z). NEW CAMPAIGN (vjeux 20:02Z): doors smaller than 56b (7x7 = 49, 6x8 / 8x6 = 48, 6x9 / 9x6 = 54),
  same rules (solid, flat, slime-free, FULL frame 12/12). Designers (c70b mechanisms, c78l 6x9 m69, w86 6-tall a3s/a3r,
  c77's 7x7-w1 partial) send candidates here first; screens so far (board partials 7x7-w1, 6x9-m69, 7x6-a3s):
  m69 k=4: right stack + U0 + s1/P3t drivable, bottom elevator + top-left never; w1 k=3: only TR + MR (top-right 5
  cells), P/Q/bottom boxed in; a3s k=4: 2 of 30 groups. These mechanisms lack free feed cells: wiring needs variants.
- pockets with plans that never `use` the lever: run WITHOUT --keep-lever (w1's lever ON at rest QC-powers its TL
  puller); a plan with `phase tile` ignores --between-ticks: strip the line (svc/c77-w1bt.ideal). Designers' plans
  often feed pistons from IN-PLANE input cells, which pockets reserves: use `--offplane` (moves them behind the fed
  pistons; I did it by hand in svc/w86-m69v.ideal and svc/c77-mm01v.ideal before finding the flag).

## Exact next steps
1. Shrink the solid 44 (k=4 result; then k=5 on e78k's likely-removable cells with a ~15-state palette and the
   pre-filter: ~4.6G enumerations ~10 min) and the stone 37 the same way.
2. Answer designers' screen requests first (e78k solid, c77 stone S2, c70b 8x7 m2/m2x, c78l c78FR).
3. Cross-region --keep (<= 3) is exhausted on: c78l L3a, c70b m2/m2w (+ m2 5-cell k <= 5), f131 pn31 (solid, 136M),
   e78k s11 (and subsets at k=4). Not yet: c78z p5-0 (stopped), satwire m79q-sat-topright (plans/p79q-sat-L.ideal).
4. Orphans: t9e top-right k=6 (partials/t8.md), mechspace z1-s78 seeds 0265 0937 0174 0755 (+1255).

## How to run a wiring job (recipe)
1. Copy the owner's design + plan into `tracks/pockets/svc/<owner>-<name>.txt/.ideal` (design line rewritten), or
   `git restore --source=origin/explore/<branch> --worktree -- <paths>` (untracked; never commit others' tracks).
2. `door ideal-rs <plan>` must WORK (tile phase default; `--between-ticks` for old plans).
3. REAL-door screens first: `door fill <design> --cells=... --keep --k 3 --gaps --allow-pistons --palette '<~37
   states>' --any-rotation --extra 0` (k = changed cells; add `--max-blocks N` to shrink, `--max-open N` for speed;
   drop `--any-rotation` when ranking finds). Use CROSS-region cell sets (all wiring cells): e78's door needed 3
   changes far apart.
4. Plan-driven screens: `door cosearch pockets <seed> --box=... --assist <plan> --keep-lever --no-hallway --k 3 --gaps
   --fill-palette all,sculk,calibrated,wool` (+ `--pin-assist`, `--only-pockets`, `--pocket-k`, `--region-cells`,
   `--region-pistons`). An in-plane plan cell blocks fillings next to it while assisted (the guard).
5. Exhaustive timing maps: `door ideal-rs <plan with lets> --sweep a=..  --sweep b=.. --ok-only` (hold/pulse shapes).

## Files
- NOTES.md sections 8 (pockets2) and 9 (pockets3), finds/ + finds/archive/, svc/ (owners' copies + shape plans),
  runs/ (logs + finds of every screen).
- Never commit: tracks/mechspace/mech/*, tracks/s79/*, tracks/t78/*, tracks/ring56/* (restored untracked copies),
  archive/properties-v1.tsv (a local cache `door properties` rewrites: `git checkout --` it before merging).
- Careful: `pkill -f <pattern>` / `pgrep -f` also match the shell running them (killed my own shell twice); anchor:
  `pgrep -af '^./target/release/mctas door fill .*<out-dir>'`, then `kill <pid>`.
