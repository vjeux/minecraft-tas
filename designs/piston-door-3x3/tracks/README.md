# Agent tracks

One folder per agent track: `NOTES.md` (the raw notebook: what was tried, what worked, what did not
and why), usually `BEST.txt` (the track's best verified design) and `archive/`. The story they add
up to is [docs/history.md](../../../docs/history.md); the rankings are
[LEADERBOARD.md](../LEADERBOARD.md). Add your track's line here when you start one.

| track | goal | result |
|---|---|---|
| [buildable](buildable/NOTES.md) | integrity check: can a survival player build the records? | `door buildable`; every board leader buildable at 32/32 positions, none in the naive order; found the unwaxed-copper hazard |
| [box56](box56/NOTES.md) | a flat FULL-seamless door in volume <= 56 (7x8 / 8x7) | no door: `door topology` (wiring components, piston links, pulse parity) finds 0 of the 99 7x8/8x7 mechanisms of agent families' catalog wirable (strict parity); 11 asymmetric ones survive only if 3+ pulses chain-merge; gadget merger-gap; tlup mechanism (19, ideal inputs) |
| [center-mech](center-mech/NOTES.md) | new mechanisms for the middle column | v2u2 96 (solid record: sides pushed out off one shared block), rb2-sculk 48 (sculk) |
| [entities](entities/) | doors that use entities (minecarts, armor stands, boats...) | the `entity` category and the "Entities allowed" board: entities summoned by the bridge, judged exactly back at rest after each close ([rules.md](../../../docs/rules.md#categories)) |
| [eligible](eligible/NOTES.md) | doors ELIGIBLE for records under the Squid regulation (section 3: repeater input, hallway, seamless) | `door eligible`, `door relever`, `--repeater-input` / `--eligible` on every search, `minimize --objective volume`; **fl81n volume 81 (9x9x1), FULL + hallway** (flat43-full + a note block in the lever's cell); b39n 39 blocks (b38 + note block, the first eligible solid door) |
| [exotic](exotic/NOTES.md) | unusual redstone components vs. the smallest door | trapdoors, fence gates, skulls, rails, dispensers, crafters, scaffolding: gadgets + search palettes; pm45-trap 45 (14 / 14: an observer watching an iron trapdoor) |
| [fast-clean](fast-clean/NOTES.md) | the fastest solid doors under the "nothing in front of or behind" rule | every fast door broke it in the same six cells (the lever net's staircase); re-laid over the top: a1c 107 (10 / 7), s4c 101 (12 / 7), n3c 116 (9 / 7), c1r 108 (9 / 7) |
| [flat](flat/NOTES.md) | the flat board (every block in the door's plane, no slime) | 60 -> 49 (flat-v8): observer chains, `synth` and `resynth` on the late chains |
| [flat-b](flat-b/NOTES.md) | the flat board with a different mechanism | the 19-block mechanism upside down: 48 -> 44 (flat-b-v7) |
| [flat-seam](flat-seam/NOTES.md) | the smallest-VOLUME flat + FULL seamless + hallway door (slime-free) | `resynth --fit` compaction, gadget `pulse-merge`; **e89b 8x9 = 72** (49 blocks, cd19 without row 5, the flat seamless record), h44 9x9 = 81 (pist19r floor), s55 9x10 = 90 |
| [families](families/NOTES.md) | the complete catalog of flat seamless MECHANISMS per box | `door families`: exit-plan generator + exact macro solve; [CATALOG.md](families/CATALOG.md) |
| [fresh-a](fresh-a/NOTES.md) | a door from first principles | slime-free 122 (a side group drags the top-middle out) |
| [fresh-b](fresh-b/NOTES.md) | a fresh design, minimum blocks | the mirrored pump family: mp6 46 (solid record), mp8 43, opens and closes in 16 |
| [fresh-c](fresh-c/NOTES.md) | symmetric control: the same signals on both lever edges | fc1 42, **fc2 40** (solid record 08:10-15:15), fc2f 41 (22 / 9) |
| [fresh-d](fresh-d/NOTES.md) | symmetric control on a slime-free mechanism | s4 45 slime-free (waxed bulb; the unwaxed one led to the random-tick rule), gadget bulb-pulse |
| [fresh-e](fresh-e/NOTES.md) | beat 40 with a different geometry | nothing below 40; gadgets head-qc and order-flip; leads: a free Q delay, a shared bottom block |
| [gravity](gravity/NOTES.md) | a gravity (sand) door, material tricks | the drop tower: g4 opens in 5, g8 48 (5 / 11), g10 45 |
| [gravity-2](gravity-2/NOTES.md) | a cheaper gravity door | g36 -> g33 33 (5 / 16), the gravity record |
| [lamp-sweep](lamp-sweep/NOTES.md) | re-synthesize every leader with lamps, bulbs, note blocks, targets | flat43 43 (flat record), w41 41 slime-free, a1-107 (fastest solid, 10 / 7 at 107), o6-107 |
| [logic-min](logic-min/NOTES.md) | logic minimization of v2's control | pulse-driven pistons (gadget pulse-toggle): 49 solid; slime-free 55 -> 50 |
| [mechanics](mechanics/NOTES.md) | gadgets from the 26.3 source, applied to the door | the gadget library, `door drive` expects, `door patch`; rb2-hold 53 and 51, rb5-bchain 50, rb5-hold 48 (solid records) |
| [mech-synth](mech-synth/NOTES.md) | mechanism synthesis around the door | `door mech-synth`; the 21-block pulse-only and 19-block slime-free mechanisms and their catalogs |
| [mech-wire](mech-wire/NOTES.md) | rank mechanisms by mechanism + wiring | `door mech-wire`; nothing below 40 (its router wires ~12 blocks worse than by hand) |
| [observer-seq](observer-seq/NOTES.md) | event-driven sequencing | the first slime-free doors: s1 89 -> s6 58 |
| [pareto-mid](pareto-mid/NOTES.md) | fill the middle of the solid Pareto frontier | `door frontier`; gadgets log-leaves and rb-lamp (asymmetric delays from a moving log or redstone block); mp8-leafU 43 (15 / 17), mp8-lampU 44 (14 / 16), mp7-leafC 46 (15 / 13), mp7-repU 46 (14 / 14) |
| [q-free](q-free/NOTES.md) | beat 40 with a free Q delay or a shared bottom pulse | both fail (Q's head forwards A's update; no 1-block shared source); a 40 variant |
| [redstone-block](redstone-block/NOTES.md) | movable power: shuttled redstone blocks | rb1 73, rb2 57, rb5 52, rb6 48 (solid records); rb7-rb9 fast (rb9 61 at 16 / 17) |
| [riders](riders/NOTES.md) | blocks riding moving structures as timing elements | fx1 40 (ties fc2), r3 43, r2 46; P_M cannot be freed |
| [router2](router2/NOTES.md) | a port-aware, timing-aware net router | `door route2`; no saving on any record |
| [sculk-2](sculk-2/NOTES.md) | sculk sensors replacing control wiring | s1 38, s4 36 (works facing north/south only); gadgets sculk-delay-line, sculk-wool-occlusion |
| [sculk-ew](sculk-ew/NOTES.md) | make sculk-2's doors work facing east/west | why they broke (gadget landing-race: the landing block's update order decides which piston fires first); s4r 36 = s4 with one repeater delay 1 -> 2: rotation-safe in all 8 orientations, vanilla-checked, buildable |
| [search-tool](search-tool/NOTES.md) | automated local search | `door minimize`, `harvest`, `transplant`; v2 109, center-mech 88, sculk 43, gravity 46 / 45 |
| [sf-holdA](sf-holdA/NOTES.md) | the 19-block slime-free mechanism with A held at rest | **b38 38**: the solid and slime-free record |
| [shapes](shapes/NOTES.md) | door shapes relative to the opening, shape boards | `door shapes`, swept shapes, the ground check, `--shape` on every search; boards "in a 3-thick wall", "on the ground", "smallest box" |
| [showcase](showcase/NOTES.md) | make the results easy to look at | `door gallery`, `door montage`, title cards, the timeline chart |
| [onerender](onerender/NOTES.md) | record every door once | one clip per design (`clips.rs`): videos and GIFs cut from it by ffmpeg, a new label in 1.5 s instead of a 60 s recording |
| [slime-free](slime-free/NOTES.md) | the "no slime or honey" board | combo 48, s7 45, s7q 44 |
| [speed](speed/NOTES.md) | the fastest door | w6 12 / 7, s4 12 / 7 at 108, o7 11 / 7 at 119 |
| [speed-2](speed-2/NOTES.md) | open in 10, smaller 11 / 7 | a1 125 opens in 10 (an odd-tick pulse, gadget odd-pulse), n2 113 the first to open in 9; c1 112 and c2 109 at 9 / 7 under the column rule; open 8 validated with scripted inputs |
| [synth](synth/NOTES.md) | circuit synthesis by exhaustive simulation | `door synth`, `door resynth`; gravity 45 -> 42 |
| [tick-door](tick-door/NOTES.md) | the fastest door with 0-tick and 1-tick mechanics | w14: **opens in 4 ticks** (the proven floor), closes in 15, 229 blocks: the first lever-wired open-4 |
| [tick-study](tick-study/NOTES.md) | 0-tick and 1-tick gadgets, floors for the door | floors open >= 4, close >= 1; an opening clear at 4 that closes in 3 (ideal inputs); 25 gadgets, all identical in vanilla ([mechanics.md](../../../docs/mechanics.md#9-floors-for-the-fastest-3x3-door)) |
| [tile](tile/NOTES.md) | tileable doors: one module copied N times, one lever | the tile judge (`door verify --tile`, `door tile-scan`); an opening every 8 blocks at 52 blocks per opening (b38h-bus8: b38 + a 14-block bus), then 46 (b38h-obs8: the bus became an observer chain); a wide mechanism proven with ideal inputs |
| [top-control](top-control/NOTES.md) | move all control near the top | rb6-bmobs 47 (solid record: observer-fired bottom-middle), st43 42 (sculk) |
| [vanilla-build](vanilla-build/NOTES.md) | replay the build scripts on the official server | `door vanilla-build`; every leader passes |
| [vanilla-check](vanilla-check/NOTES.md) | prove the simulator matches vanilla | `door vanilla-check`, `vanilla-fuzz`; no divergence |
| [wire19sf](wire19sf/NOTES.md) | wire the 19-block slime-free mechanism | w45 -> w42 42 (slime-free record at the time, buildable) |
| [wire21](wire21/NOTES.md) | wire the 21-block pulse-only mechanism | m05 40 (ties fc2) |
