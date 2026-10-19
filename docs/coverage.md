# Coverage: what each tool can and cannot find

Every claim of the form "none found", "UNSAT", "impossible" or "blocked" is bounded by the tool
that made it. This page says, per tool, what it models EXACTLY, what it APPROXIMATES and what it
does NOT model, with the evidence. Cite the rows you rely on when you write such a claim
(see [the rule](#the-rule-for-claims)). Each tool's owner keeps its column true.

Legend: **✓** exact (with evidence) · **~** approximated (the approximation is named) · **✗** not
modeled / not searched · **—** not applicable · **?** not yet audited.

## The tools

| tool | what it does | semantics (how it predicts the game) | search space | exhaustive? |
|---|---|---|---|---|
| judge (`door verify`) | checks one design | **the real game** (26.3 server code in the bridge) | — | — |
| redsim (`door redsim-verify`, `cosearch eval`) | checks one design, ~1000x faster | **exact model**: 0/100,000 random redstone circuits, 0/100,000 random piston setups, 263/263 archived doors and 67/67 gadgets identical tick by tick, 0 verdict disagreements on 287 designs, 2049/2049 with near-miss mutants (cosearch agree) | — | — |
| local search (`minimize`, `harvest`, `grow`, `regrow`, `synth`, `resynth`, `route2`) | improve a working design | candidates judged by the **real judge** | edits of a few blocks near the design; palettes below | no (local) |
| mechanism catalog (`door families`) | every flat seamless mechanism per box | **~** macro piston model (PistonStructureResolver port; quasi-connectivity coupling since v3; no redstone), then each entry replayed with ideal inputs in the real simulator (`sim` column) | in-plane; <= 11 pistons facing E/W/U/D; no helpers; no slime; each door block 1-2 unit moves (center 2); a mover = a sticky puller 2 ahead or a pusher 1-2 behind; a piston reused at rest or lifted 1 cell along its axis (`--carry N`: 1-2 cells any direction, depth 2, in progress); minimal mechanisms only | within its bounds, except "unresolved" (state-capped) candidates |
| fullmech (`door fullmech`, pistmodel) | the catalog re-solved exactly | **✓** redsim with ideal inputs (1/3-tick pulses, levels, same-tick pairs) | families' generator (+ `--reduce`: an input replaced by a carried/static redstone block or observer; `--anneal`) | within the generator's space, state cap 30k |
| topology (`door topology`) | rank mechanisms by wirability | **~ heuristic, NOT sound**: wiring regions, piston links, pulse parity, memory links >= 7 cells, no-feed filter; its parity premise is false (observer busy window, mechanics.md section 8) | — | no: a ranking only |
| satwire (`door satwire`) | wiring for a FIXED mechanism | **~** time-unrolled SAT encoding (details in tracks/satwire/NOTES.md "COVERAGE"); every SAT model is screened by redsim and judged by the real judge (CEGAR), so a FOUND door is real; an UNSAT is only relative to the encoding | free cells of a box; palette below; one lever among candidates (outside the box or off-plane allowed); recorded piston power per tick with a time warp (stalls/skips) | yes within palette + box + encoding |
| cosearch (`door cosearch search`) | joint mechanism + wiring search | **✓** acceptance: the redsim judge (32 positions x 3 cycles, FULL seamless, hallway), then the real judge (`--confirm`); guidance: **~** a one-position, 2-cycle screen with partial credit per door cell and phase (a door slower than `--max-ticks` 160 per phase is screened out; the screen's ring check skips the hole rule, the final check has it) | the door plane only (pistons face E/W/U/D, nothing off-plane), every cell of the box (or `--focus` / `--focus-cells`): palette below; moves: set/remove/re-face/move/swap, repeater delay, comparator mode, carried redstone block or observer in front of a piston, piston add/move/re-face/extend toggle (`--lock-pistons` keeps the seed's, `--no-mech` forbids all), lever moves (`--lever-moves`) | no: simulated annealing + exhaustive enumeration of every edit of <= k focus cells (`--enum-k`, k <= 5; e.g. all 56^4 fillings of 4 cells in 103 s on 40 threads) |
| boxproof (whole-box SAT) | prove or find a door in a box | **?** (target: redsim's semantics, validated by differential tests) | **?** | **?** |

## Game behaviors (rows from redsim's source inventory, Minecraft 26.3)

Flat doors only below; "flat-impossible" marks behaviors that need a block outside the door plane.

| behavior | redsim | local search | catalog | fullmech | satwire | cosearch | boxproof |
|---|---|---|---|---|---|---|---|
| **Engine** | | | | | | | |
| neighbor updates (order W,E,D,U,N,S; who updates whom) | ✓ | ✓ (judge) | ✗ | ✓ | ~ every receiver re-reads its input each tick | ✓ | ? |
| shape updates (observers, dust shapes, note instrument) | ✓ | ✓ | ✗ | ✓ | ~ observers see recorded mechanism changes and wiring states per tick | ✓ | ? |
| scheduled ticks: priorities, FIFO, dedup (`willTickThisTick`) | ✓ | ✓ | — | ✓ | ~ phases (repeaters, observers, torches); no repeater-priority races; dedup exact | ✓ | ? |
| block events, same-tick (0-tick) transients | ✓ | ✓ | ✗ | ✓ (pairs) | ✗ observers miss intra-tick transients | ✓ | ? |
| dust update order (BlockPos hash, position dependent) | ✓ | ✓ | — | ✓ | ✗ (the judge's 32 positions catch it) | ✓ | ? |
| quasi-connectivity as BUD (armed, fires on the next update) | ✓ | ✓ | ~ coupling only (v3) | ✓ | ~ the firing update is assumed | ✓ | ? |
| **Components** | | | | | | | |
| dust (levels, shapes, steps, strong/weak) | ✓ | ✓ | — | — | ✓ (cross dust only) | ✓ | ? |
| repeater delays 1-4, pulse stretching | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| repeater locking, comparator side inputs | ✓ | ✓ | — | — | — flat-impossible | — flat-impossible | — |
| comparator (rear input, 2 gt, reads bulbs) | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| comparator reading container fill levels (dropper/hopper flip-flops) | ✗ (contents always empty; item transfer in progress) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| observer (2+2 gt, on any watched change incl. moving blocks) | ✓ | ✓ | — | — | ✓ (watches wiring and mechanism cells) | ✓ | ? |
| torch / wall torch, burnout | ✓ | ✓ | — | — | ✓ / burnout ✗ (CEGAR catches) | ✓ | ? |
| redstone lamp (4 gt off) | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| waxed copper bulb (rising-edge toggle) | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| note block | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| target block | ✓ | ✓ | — | — | ✗ | ✓ | ? |
| iron / oak trapdoor, doors, fence gates | ✓ | ✓ (trapdoors in the default resynth/regrow palette) | — | — | iron trapdoor ✓ | iron trapdoor ✓ (E/W, bottom/top); oak trapdoors, doors, gates ✗ | ? |
| dropper / dispenser (immovable, QC power sensor) | ✓ | ✓ (synth palette) | — | — | dropper ✓ | dropper ✓ (U/D/E/W); dispenser ✗ | ? |
| hopper (enabled state), crafter | ✓ (no items) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| powered / activator rails | ✓ | ✗ (not in palettes) | — | — | ✗ | ✗ | ? |
| slabs (top/bottom), non-conducting full blocks (glass...) | ✓ | glass ✓, slabs ✗ | — | — | glass ✓, slabs ✗ | glass ✓, smooth stone slabs ✓ (top/bottom) | ? |
| leaves + log (the only 1 gt delay) | ✓ | ✓ (grow palette) | — | — | ✗ | ✓ (persistent oak leaves, oak log x/y) | ? |
| buttons; lever | ✓ | lever | — | — | lever | lever ✓ (moved by `--lever` / `--lever-moves`); buttons ✗ | ? |
| redstone block as static wiring | ✓ | ✓ | — | ✓ (`--reduce`) | ✗ | ✓ | ? |
| **Pistons and moving blocks** | | | | | | | |
| pistons in the mechanism | ✓ | ✓ | ✓ (macro) | ✓ | fixed input | ✓ | ? |
| update pistons (push nothing; block updates / observer triggers) | ✓ | ✓ (judge, if a move adds one) | ✗ (minimal mechanisms drop them) | ✗ | ✗ (requested) | ✓ | ? |
| pistons moving pistons | ✓ | ✓ | ~ lifts 1 cell (`--carry` in progress) | same | — | ✓ | ? |
| spitting / block dropping on short pulses | ✓ | ✓ | ~ 2 gt pulse action | ✓ | recorded | ✓ | ? |
| 0-tick pistons | ✓ | ✓ | ✗ | ~ same-tick pairs | ✗ | ✓ | ? |
| helper blocks (stoppers, line fillers) | ✓ | ✓ | ✗ (next) | ✗ | — | ✓ | ? |
| carried power sources (redstone block, observer) | ✓ | ✓ | ✗ | ✓ (`--reduce`) | ✗ | ✓ | ? |
| immovable blocks (obsidian, crying obsidian, block entities) | ✓ | ✓ | obsidian | obsidian | obsidian | ✓ obsidian, crying obsidian, droppers | ? |
| push limit 12, glazed terracotta | ✓ | ✓ | ✓ / ✗ | ✓ | — | ✓ / ✓ | ? |
| slime / honey (excluded from the flat seamless target by vjeux) | ✓ | ✓ | ✗ (off) | ✗ | ✗ | ✗ | — |
| **Search-space features** | | | | | | | |
| lever outside the box (not counted in the volume) | — | ✗ | — | — | ✓ | ✓ | ? |
| lever off the door plane | — | ✓ | — | — | ✓ (`--lever-oop`) | ✗ | ? |
| timing chosen by the search | — | ✓ | sym/oc trains only | sym/oc | ~ time warp (pauses stretch/shrink, pulse widths fixed) | ✓ | ? |
| different behavior on open and close | — | ✓ | ✓ (oc plans) | ✓ | ✓ (two traces) | ✓ | ? |

Excluded on purpose everywhere (rules or categories): random ticks (unwaxed copper, crops, leaves
decay), weather and daylight, entities and player triggers (pressure plates, tripwire, item frames),
sculk (its own category), fluids.

## Gaps that could change a 7x8 / 7x9 verdict, by priority

1. **satwire UNSATs are not proofs** while the encoding approximates block updates (re-read every
   tick), QC-as-BUD, observer transients and repeater races, and lacks update pistons, redstone
   blocks, targets, slabs and leaves. Fix: encode updates exactly or use boxproof; until then an
   UNSAT means "no wiring in this encoding".
2. **The catalog's mechanism space**: no helpers, carry 1 cell, no 0-ticks, <= 2 moves per block,
   state-capped candidates (7x8: 1861 unresolved). The community 7x8 may sit outside it.
3. **cosearch** is not exhaustive (annealing; exhaustive only over <= 5 edited focus cells) and
   stays in the door plane; its palette gaps (obsidian, trapdoors, droppers, slabs, leaves/log,
   glazed terracotta) are closed (explore/cosearch 0dc82bf); still missing: oak trapdoors, doors,
   fence gates, dispensers, hoppers, rails, buttons, off-plane blocks.
4. **topology** is a heuristic: never cite it as a reason to skip a mechanism.
5. **redsim** item transfer (container flip-flops): in progress.

## The rule for claims

* A FOUND design counts only when the real judge passes (`door verify --extra-offsets 28`).
* A negative claim names the tool, its bounds (box, palette, piston count, moves, timing) and the
  rows above it depends on. "satwire UNSAT" never stands alone: it is "no wiring with palette P
  in box B under satwire's encoding".
* Only an exhaustive tool whose semantics are ✓ on every row the claim touches can state
  "impossible" (today: none for wiring; boxproof is the candidate).
