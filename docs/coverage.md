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
| redsim (`door redsim-verify`, `cosearch eval`) | checks one design, ~1000x faster | **exact model**: 0/100,000 random redstone circuits, 0/100,000 random piston setups (`door pistfuzz`: every block, moving block entity, queue size, drop and the idle flag after every tick), 0/20,000 random container circuits (`redsim-fuzz fuzz --gen containers`: droppers/hoppers with items, comparators), 328/328 archived doors (2 cycles) and 72/72 gadgets identical tick by tick, 0 verdict disagreements on 287 designs, 2049/2049 with near-miss mutants (cosearch agree); block equivalence classes (`redsim::classes`) proven in the game (`redsim-fuzz classfuzz`: 3000 circuits vs class-swapped variants, 0 differ); falling blocks only **~** (row below); item entities ✗ (excluded) | — | — |
| local search (`minimize`, `harvest`, `grow`, `regrow`, `synth`, `resynth`, `route2`) | improve a working design | candidates judged by the **real judge** | edits of a few blocks near the design; palettes below | no (local) |
| mechanism catalog (`door families`) | every flat seamless mechanism per box | **~** macro piston model (PistonStructureResolver port; quasi-connectivity coupling since v3; no redstone), then each entry replayed with ideal inputs in the real simulator (`sim` column) | in-plane; <= 11 pistons facing E/W/U/D; no helpers; no slime; each door block 1-2 unit moves (center 2); a mover = a sticky puller 2 ahead or a pusher 1-2 behind; a piston reused at rest or lifted 1 cell along its axis (`--carry N`: 1-2 cells any direction, depth 2, in progress); minimal mechanisms only | within its bounds, except "unresolved" (state-capped) candidates |
| fullmech (`door fullmech`, pistmodel) | mechanisms solved exactly, exact `door ideal` plans exported | **✓** every transition is redsim; inputs are ideal but physical: a redstone block behind the piston's cell when switched on (it powers that piston and, by QC, the piston right below; it stays put when the piston is carried), exactly what `door ideal` replays (32/32 exported 7x8 plans pass it); actions: k-tick pulses (`--pulses`, default 1 = spit, 3 = push-pull), level toggles (pistons extended at rest; all with `--levels`), overlapping pulse pairs of pistons <= 3 cells apart (`--pairs`); every other action starts on a settled state | families' generator (the catalog's bounds, <= 10 pistons by default); `--reduce`: greedy input minimization, then one helper (redstone block, or observer facing E/W/U/D) in each free cell; `--anneal`: random mechanisms anywhere in the box (`--helpers`, `--slime`); `--check`: the mechanism of a design | no: A* per candidate with a 30k-state cap (7x8: ~1500 of 3448 capped), the close tried from the first 6 open states found; an uncapped "none" covers only the action set; `--anneal` samples |
| topology (`door topology`) | rank mechanisms by wirability | **~ heuristic, NOT sound**: wiring regions, piston links, pulse parity, memory links >= 7 cells, no-feed filter; its parity premise is false (observer busy window, mechanics.md section 8) | — | no: a ranking only |
| satwire (`door satwire`) | wiring for a FIXED mechanism | **~** time-unrolled SAT encoding (details in tracks/satwire/NOTES.md "COVERAGE"); every SAT model is screened by redsim and judged by the real judge (CEGAR), so a FOUND door is real; an UNSAT is only relative to the encoding | free cells of a box; palette below; one lever among candidates (outside the box or off-plane allowed); recorded piston power per tick with a time warp (stalls/skips) | yes within palette + box + encoding |
| cosearch (`door cosearch search`) | joint mechanism + wiring search | **✓** redsim (exact), then the real judge | any cell of the box: palette below; moves: set/remove/re-face/move/swap, repeater delay, comparator mode, carried redstone block or observer in front of a piston, piston add/move/re-face/extend toggle | no (annealing + exhaustive k <= 3 edits) |
| boxproof (whole-box SAT) | prove or find a door in a box | **?** (target: redsim's semantics, validated by differential tests) | **?** | **?** |

## Game behaviors (rows from redsim's source inventory, Minecraft 26.3)

Flat doors only below; "flat-impossible" marks behaviors that need a block outside the door plane.

| behavior | redsim | local search | catalog | fullmech | satwire | cosearch | boxproof |
|---|---|---|---|---|---|---|---|
| **Engine** | | | | | | | |
| neighbor updates (order W,E,D,U,N,S; who updates whom) | ✓ | ✓ (judge) | ✗ | ✓ | ~ every receiver re-reads its input each tick | ✓ | ? |
| shape updates (observers, dust shapes, note instrument) | ✓ | ✓ | ✗ | ✓ | ~ observers see recorded mechanism changes and wiring states per tick | ✓ | ? |
| changes made BY shape updates notify no neighbors (`updateFlags & -34`); flags 2 vs 3 per component | ✓ (fixture shape-update-no-neighbor-updates) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ? |
| tick phases: scheduled block ticks, then block events, then entities, then block entities (moving pistons, hoppers) | ✓ (`World::tick` = ServerLevel.tick) | ✓ (judge) | ? | ✓ | ? | ✓ | ? |
| scheduled ticks: priorities, FIFO, dedup (`willTickThisTick`) | ✓ (per-chunk queues merged by trigger, priority, sub-tick like LevelTicks) | ✓ | — | ✓ | ~ phases (repeaters, observers, torches); no repeater-priority races; dedup exact | ✓ | ? |
| block events, same-tick (0-tick) transients | ✓ (FIFO + dedup; events queued during the phase run in it) | ✓ | ✗ | ✓ inside an action; two inputs in one tick only with `--pairs` | ✗ observers miss intra-tick transients | ✓ | ? |
| block-entity ticker order (`blockEntityTickers` + pending; a rebound wrapper keeps its slot) | ✓ moving pistons; hoppers tick before moving blocks (exact while a design's hoppers exist before its first moving block) | ✓ (judge) | — | ✓ | ? | ✓ | ? |
| dust update order (BlockPos hash, position dependent) | ✓ | ✓ | — | — (no dust: no wiring) | ✗ (the judge's 32 positions catch it) | ✓ | ? |
| quasi-connectivity as BUD (armed, fires on the next update) | ✓ | ✓ | ~ coupling only (v3) | ✓ | ~ the firing update is assumed | ✓ | ? |
| comparator updates from changed containers / analog sources (`updateNeighbourForOutputSignal`: horizontal comparators, and through one conductor) | ✓ | ✓ (judge) | — | — | ? | ✓ | ? |
| item drops of popped / destroyed blocks (none for piston heads and moving pistons) | ✓ (recorded and compared by pistfuzz) | ✓ (judge) | — | ✓ | ? | ✓ | ? |
| **Components** | | | | | | | |
| dust (levels, shapes, steps, strong/weak) | ✓ | ✓ | — | — | ✓ (cross dust only) | ✓ | ? |
| repeater delays 1-4, pulse stretching | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| repeater locking, comparator side inputs | ✓ | ✓ | — | — | — flat-impossible | — flat-impossible | — |
| comparator (rear input, 2 gt, reads bulbs) | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| comparator reading container fill levels (dropper/hopper flip-flops) | ✓ (fill formula with the game's float math; 0/20,000 container circuits; fixture hopper-t-flip-flop) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| other analog sources: cake bites, composter level, end portal frame eye, respawn anchor charges, copper bulb (15/0), crafter (empty) | ✓ (fuzz family `analog`) | ✓ (judge) | — | — | ? | ? | ? |
| container item transfer without item entities: dropper -> facing container (4 gt, its single stack), hopper push + pull above (8 gt cooldown, the wasEmpty 8/7 rule, locked while powered) | ✓ (0/20,000; hopper removeItem/setItem do not notify: fixture hopper-remove-no-setchanged); a dropper with 2+ stacks (random slot) and item entities ✗ (flagged) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| observer (2+2 gt, on any watched change incl. moving blocks) | ✓ | ✓ | — | ✓ as a helper (`--reduce`, `--anneal --helpers`) | ✓ (watches wiring and mechanism cells) | ✓ | ? |
| torch / wall torch, burnout | ✓ | ✓ | — | — | ✓ / burnout ✗ (CEGAR catches) | ✓ | ? |
| redstone lamp (4 gt off) | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| waxed copper bulb (rising-edge toggle) | ✓ | ✓ | — | — | ✓ | ✓ | ? |
| note block | ✓ (instrument from the block below, mob head above: fixture note-under-wall-skull) | ✓ | — | — | ✓ | ✓ | ? |
| target block | ✓ | ✓ | — | — | ✗ | ✓ | ? |
| iron / oak trapdoor, doors, fence gates | ✓ | ✓ (trapdoors in the default resynth/regrow palette) | — | — | iron trapdoor ✓ | ✗ | ? |
| dropper / dispenser (immovable, QC power sensor) | ✓ (dispensers' item behaviors spawn entities: ✗, flagged) | ✓ (synth palette) | — | — | dropper ✓ | ✗ | ? |
| hopper (enabled state, items), crafter | ✓ hopper items (row above); crafter: empty grid only | ✓ (judge) | — | — | ✗ | ✗ | ? |
| powered / activator rails | ✓ | ✗ (not in palettes) | — | — | ✗ | ✗ | ? |
| slabs (top/bottom), non-conducting full blocks (glass...) | ✓ (slime conducts, honey/leaves hold nothing: fixtures slime-is-a-conductor, leaves-honey-hold-nothing) | glass ✓, slabs ✗ | — | — | glass ✓, slabs ✗ | glass ✓, slabs ✗ | ? |
| cake (needs a solid block below: pops off diodes, slabs...) | ✓ (fixture cake-on-unsupported-repeater) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| leaves + log (a 1 gt delay) | ✓ | ✓ (grow palette) | — | — | ✗ | ✗ | ? |
| scaffolding (a 1 gt delay per block: distance recomputed 1 gt after any shape update; breaks at 7, falls when placed at 7) | ✓ (0/30,000 scaffold circuits; with pistons 0/10,000; falling scaffolds 0/10,000; fixture scaffold-delay = designs/gadgets/scaffold-delay; link table rows); a FALLING scaffold pushed or lifted by a moving piston ✗ (entities moved by pistons, pistmodel's open item: 13/10,000 circuits that have both) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| buttons; lever | ✓ | lever | — | — | lever | lever | ? |
| redstone block as static wiring | ✓ | ✓ | — | ✓ (`--reduce`, `--anneal --helpers`) | ✗ | ✓ | ? |
| **Pistons and moving blocks** | | | | | | | |
| pistons in the mechanism | ✓ | ✓ | ✓ (macro) | ✓ | fixed input | ✓ | ? |
| piston heads forward neighbor updates to their base; a head without its base pops (no drop) | ✓ (pistfuzz) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ? |
| a retracting base / moving piston keeps the extended box as support (a lever or repeater on its back survives) | ✓ (`piston::sturdy_at`; found by the archived-door replay) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ? |
| attachments popping when their support moves or breaks (torches, levers, buttons, dust, diodes, rails, doors, cake) | ✓ (fuzz Set steps, pistfuzz) | ✓ (judge) | ✗ | ✓ | ? | ✓ | ? |
| update pistons (push nothing; block updates / observer triggers) | ✓ | ✓ (judge, if a move adds one) | ✗ (minimal mechanisms drop them) | ~ only `--anneal` places one | ✗ (requested) | ✓ | ? |
| pistons moving pistons | ✓ | ✓ | ~ lifts 1 cell (`--carry` in progress) | ✓ semantics: any carry the candidate's pistons reach (the carried piston keeps its identity and input, lands and fires exactly); candidates: the catalog's | — | ✓ | ? |
| spitting / block dropping on short pulses | ✓ | ✓ | ~ 2 gt pulse action | ✓ (1 gt pulse; exact event and finalTick rules) | recorded | ✓ | ? |
| 0-tick pistons | ✓ | ✓ | ✗ | ~ only through helpers moved by pistons (`--reduce`, `--anneal`): ideal inputs switch between ticks; `--pairs` starts two pistons in one tick, which is not a 0-tick | ✗ | ✓ | ? |
| helper blocks (stoppers, line fillers) | ✓ | ✓ | ✗ (next) | ~ `--anneal --helpers` (stone, obsidian, redstone block, observer); not in the generator | — | ✓ | ? |
| carried power sources (redstone block, observer) | ✓ | ✓ | ✗ | ✓ semantics; searched: one helper per mechanism (`--reduce`), `--anneal --helpers` | ✗ | ✓ | ? |
| immovable blocks (obsidian, crying obsidian, block entities) | ✓ | ✓ | obsidian | obsidian | obsidian | ✗ (not in the palette) | ? |
| push limit 12, glazed terracotta | ✓ | ✓ | ✓ / ✗ | ✓ / ✗ (not in the palette) | — | ✓ / ✗ | ? |
| slime / honey (excluded from the flat seamless target by vjeux) | ✓ | ✓ | ✗ (off) | ✗ (off; `--anneal --slime` only) | ✗ | ✗ | — |
| falling blocks (sand, gravel, concrete powder: gravity category) | ~ falls and landings; falling entities pushed by moving pistons or landing on moving blocks ✗ (`pistfuzz --gravity`: 2960/3000 before the failure cap) | ✓ (judge) | ✗ | ✗ | ? | ? | ? |
| **Search-space features** | | | | | | | |
| plain blocks searched by equivalence class (`redsim::classes`: 11 plain symbols, instrument splits only next to a note block) | ✓ (classes derived from every property redsim reads; game-proven by classfuzz) | ✗ | — | — | ? | ? | ? |
| lever outside the box (not counted in the volume) | — | ✗ | — | — | ✓ | ✓ | ? |
| lever off the door plane | — | ✓ | — | — | ✓ (`--lever-oop`) | ✗ | ? |
| timing chosen by the search | — | ✓ | sym/oc trains only | oc only (open and close sequences of actions on settled states, no symmetric train); pulse widths from `--pulses` | ~ time warp (pauses stretch/shrink, pulse widths fixed) | ✓ | ? |
| different behavior on open and close | — | ✓ | ✓ (oc plans) | ✓ | ✓ (two traces) | ✓ | ? |

Excluded on purpose everywhere (rules or categories): random ticks (unwaxed copper, crops, leaves
decay), weather and daylight, entities and player triggers (pressure plates, tripwire, item frames,
item entities: a dropper spitting into the world, hoppers catching drops), sculk (its own category),
fluids; analog sources that depend on them (jukebox, lectern, chiseled bookshelf, cauldrons,
beehives, daylight detectors).

redsim evidence: every ✓ in its column is exercised by `redsim-fuzz fuzz` (random circuits from the
row's palette family compared with the game block by block after every tick, 0/100,000 with
`--palette all`; `--gen containers` 0/20,000) and, for pistons, by `door pistfuzz` (0/100,000);
a named fixture is also a regression test in `crates/redsim/tests/fixtures/` (replayed by
`cargo test -p redsim`). Each component's source references are in its module
(`crates/redsim/src/redstone/*.rs`, `crates/redsim/src/piston/*.rs`).

## Gaps that could change a 7x8 / 7x9 verdict, by priority

1. **satwire UNSATs are not proofs** while the encoding approximates block updates (re-read every
   tick), QC-as-BUD, observer transients and repeater races, and lacks update pistons, redstone
   blocks, targets, slabs and leaves. Fix: encode updates exactly or use boxproof; until then an
   UNSAT means "no wiring in this encoding".
2. **The catalog's mechanism space**: no helpers, carry 1 cell, no 0-ticks, <= 2 moves per block,
   state-capped candidates (7x8: 1861 unresolved). The community 7x8 may sit outside it. fullmech
   inherits this space (it re-solves the same candidates exactly, ~1500 of 3448 7x8 candidates
   still state-capped); only its `--anneal` samples outside it.
3. **cosearch's palette** lacks obsidian, trapdoors, droppers, slabs, leaves/log, glazed
   terracotta (cheap to add).
4. **topology** is a heuristic: never cite it as a reason to skip a mechanism.
5. **redsim** container item transfer is in (droppers, hoppers, fill levels); a dropper holding
   2+ stacks (random slot), dispensers' item behaviors and item entities stay out (flagged when a
   circuit reaches them, never silently wrong).

## The rule for claims

* A FOUND design counts only when the real judge passes (`door verify --extra-offsets 28`).
* A negative claim names the tool, its bounds (box, palette, piston count, moves, timing) and the
  rows above it depends on. "satwire UNSAT" never stands alone: it is "no wiring with palette P
  in box B under satwire's encoding".
* Only an exhaustive tool whose semantics are ✓ on every row the claim touches can state
  "impossible" (today: none for wiring; boxproof is the candidate).
