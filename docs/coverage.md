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
| mechanism catalog (`door families`) | every flat seamless mechanism per box | **~** macro piston model (PistonStructureResolver port: push limit 12, sticky pull of the block two ahead, retracted pistons movable, extended pistons / heads / obsidian immovable; one settled action at a time, a 2 gt pulse = push-and-spit or extend-into-gap-and-pull; quasi-connectivity coupling since v3; no redstone), then each entry replayed with ideal inputs in the real simulator (`sim` column; v3: 85 oc + 44 sym schedules WORK, 68 entries have no ideal plan) | in-plane; <= 11 pistons facing E/W/U/D (sticky/normal, retracted/extended at rest); no helpers; no slime; the center leaves straight in exactly 2 moves, the edge on its line in 2, the 7 others in 1 (`--two N`: N of them in 2); a mover = a sticky puller 2 ahead or a pusher 1-2 behind; a piston reused at rest or lifted 1 cell along its axis by one lift. Run only on some boxes: `--carry N` (a reused piston displaced 1-2 cells in ANY direction by a carrier, carriers carried up to depth N: 9x6/6x9 with N = 1, P <= 12; 7x7 / 7x8 generate 1.9M / 2.6M candidates, not solved) and `--vmid any` (ML or MR leaves vertically: 7x8, P <= 10: 0 found, 13032 unresolved at a 50k cap); minimal mechanisms only | within its bounds, except "unresolved" candidates (pulse graph over 300k states and A* found nothing; 7x7: 1639, of which a 5M-state re-check proved 93 have no open state, 503 reach an open state with no close found, 1043 still over the cap) |
| fullmech (`door fullmech`, pistmodel) | the catalog re-solved exactly | **✓** redsim with ideal inputs (1/3-tick pulses, levels, same-tick pairs) | families' generator (+ `--reduce`: an input replaced by a carried/static redstone block or observer; `--anneal`) | within the generator's space, state cap 30k |
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
| scheduled ticks: priorities, FIFO, dedup (`willTickThisTick`) | ✓ | ✓ | — | ✓ | ~ phases (repeaters, observers, torches); no repeater-priority races; dedup exact | ✓ | ? |
| block events, same-tick (0-tick) transients | ✓ | ✓ | ✗ | ✓ (pairs) | ✗ observers miss intra-tick transients | ✓ | ? |
| dust update order (BlockPos hash, position dependent) | ✓ | ✓ | — | ✓ | ✗ (the judge's 32 positions catch it) | ✓ | ? |
| quasi-connectivity as BUD (armed, fires on the next update) | ✓ | ✓ | ~ coupling only (v3): a retracted unpowered piston right below a powered one follows it (pulse while the upper one is out, level held / released); QC from the wiring, and arming without an update, not modeled | ✓ | ~ the firing update is assumed | ✓ | ? |
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
| iron / oak trapdoor, doors, fence gates | ✓ | ✓ (trapdoors in the default resynth/regrow palette) | — | — | iron trapdoor ✓ | ✗ | ? |
| dropper / dispenser (immovable, QC power sensor) | ✓ | ✓ (synth palette) | — | — | dropper ✓ | ✗ | ? |
| hopper (enabled state), crafter | ✓ (no items) | ✓ (judge) | — | — | ✗ | ✗ | ? |
| powered / activator rails | ✓ | ✗ (not in palettes) | — | — | ✗ | ✗ | ? |
| slabs (top/bottom), non-conducting full blocks (glass...) | ✓ | glass ✓, slabs ✗ | — | — | glass ✓, slabs ✗ | glass ✓, slabs ✗ | ? |
| leaves + log (the only 1 gt delay) | ✓ | ✓ (grow palette) | — | — | ✗ | ✗ | ? |
| buttons; lever | ✓ | lever | — | — | lever | lever | ? |
| redstone block as static wiring | ✓ | ✓ | — | ✓ (`--reduce`) | ✗ | ✓ | ? |
| **Pistons and moving blocks** | | | | | | | |
| pistons in the mechanism | ✓ | ✓ | ✓ (macro, settled actions; replayed in the simulator) | ✓ | fixed input | ✓ | ? |
| update pistons (push nothing; block updates / observer triggers) | ✓ | ✓ (judge, if a move adds one) | ✗ (minimal mechanisms drop them) | ✗ | ✗ (requested) | ✓ | ? |
| pistons moving pistons | ✓ | ✓ | ~ solver: any push line or sticky pull may move a retracted piston, along or across its axis (the "rider shuffles"); generator: lifts 1 cell along the axis; `--carry N` (1-2 cells any direction, carriers carried to depth N) ran on 9x6/6x9 only | same | — | ✓ | ? |
| spitting / block dropping on short pulses | ✓ | ✓ | ~ 2 gt pulse action (push and spit; no block dropping) | ✓ | recorded | ✓ | ? |
| 0-tick pistons | ✓ | ✓ | ✗ | ~ same-tick pairs | ✗ | ✓ | ? |
| helper blocks (stoppers, line fillers) | ✓ | ✓ | ✗ (the model has stone / obsidian pieces; the generator places none) | ✗ | — | ✓ | ? |
| carried power sources (redstone block, observer) | ✓ | ✓ | ✗ (2D catalog); rows: `--rblocks` (strip agent) | ✓ (`--reduce`) | ✗ | ✓ | ? |
| immovable blocks (obsidian, crying obsidian, block entities) | ✓ | ✓ | obsidian, extended pistons, heads (no other immovable in the palette) | obsidian | obsidian | ✗ (not in the palette) | ? |
| push limit 12, glazed terracotta | ✓ | ✓ | ✓ / ✗ (mech-synth has it, families' grid model not) | ✓ | — | ✓ / ✗ | ? |
| slime / honey (excluded from the flat seamless target by vjeux) | ✓ | ✓ | ✗ (not in families' grid model) | ✗ | ✗ | ✗ | — |
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
2. **The catalog's mechanism space**: no helpers, lifts of 1 cell along the axis (general
   `--carry` ran on 9x6/6x9 only), no 0-ticks, <= 2 moves per block (non-center blocks 1 unless
   `--two`), no carried sources, state-capped candidates (7x8: 1861 unresolved; 7x7: 1639, a
   5M-state re-check proved only 93 of them). The community 7x8 may sit outside it.
3. **cosearch's palette** lacks obsidian, trapdoors, droppers, slabs, leaves/log, glazed
   terracotta (cheap to add).
4. **topology** is a heuristic: never cite it as a reason to skip a mechanism.
5. **redsim** item transfer (container flip-flops): in progress.

## The rule for claims

* A FOUND design counts only when the real judge passes (`door verify --extra-offsets 28`).
* A negative claim names the tool, its bounds (box, palette, piston count, moves, timing) and the
  rows above it depends on. "satwire UNSAT" never stands alone: it is "no wiring with palette P
  in box B under satwire's encoding".
* Only an exhaustive tool whose semantics are ✓ on every row the claim touches can state
  "impossible" (today: none for wiring; boxproof is the candidate).
