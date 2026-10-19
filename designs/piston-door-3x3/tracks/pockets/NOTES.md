# Track pockets: the pocket-search workhorse (door cosearch pockets, broadly)

Brief (coordinator for vjeux, 2026-10-19 05:47Z): run cosearch2's `door cosearch pockets` broadly on every
mechanism we have (7x8 = 56, 7x9 = 63, 8x8 = 64) and keep every working door; first the tool items
(periodic rest rule, off-plane lever, region closure). Box: devvm49255 (dedicated, 166 cores).

**pockets3 (15:09Z..): THE FIRST 7x8 = BEST.txt = `finds/e78-56b-40.txt`, 56b (7x8x1), 40 blocks, SCULK, FULL,
rotation-safe, open 98 / close 24, verify --extra-offsets 28 WORKS** (e78k's r1 made a door by `door fill --keep`
across regions, then shrunk 45 -> 40; section 9).

**Result (pockets2, 2026-10-19 09:24Z-12:30Z): `finds/t7b-rwfast-43.txt`, 63b (7x9x1), 43 blocks,
solid, FULL seamless, rotation-safe, open 39 / close 27, verify at 32 positions** (e88's 43-block record t7b,
43/31, with its right wall re-wired: same size, 8 ticks faster, so it ranks first by the tie-break), and
**`finds/s27-bls49.txt`, SCULK 8x8 = 64b, 49 blocks** (archived on main; not the sculk 8x8 leader: c77's
c074-64b-42 is) and
**`finds/s27-trbl51.txt`, solid 8x8 = 64b, 51 blocks, FULL, rotation-safe, verify at 32 positions** (c88b's s27
+ a 6-block bottom-left and a piston-popping top-right found by pockets; shrink took it to solid 50; also
`finds/s27-bl52.txt` 52, `finds/s26-bl53.txt` 53); 40 sculk 7x9 doors at 51 (c79 + a
3-block sculk TL/ML), 64 t7b wall variants at 43, 3 s26 bottom-lefts at 53. Section 8.
Earlier (pockets): `finds/c79-49-bl5-br3.txt`,
63b (7x9x1), 49 blocks, solid, slime-free, FULL seamless,
15/15, verify at 32 positions** (ties satwire-63b-49-49b; section 6). Plus two more 49s (`finds/c79-49-tl1`,
`-tl12`), two 50s, and 127 more 51-block wirings of the community 7x9. No 7x8 or 8x8 door: every
7x8/8x8 mechanism and hot seed tried stays a few inputs short (sections 2, 4, 5, 7).

* Lessons (pockets2): run small pockets with `--gaps` (a disconnected filling, an observer feeding a QC
  cell through air, is invisible otherwise: c88 s26's BR driver); a plan built by `plan-from` can be off
  (s27's top-right close pulse: the erased door works only at c0..16, plan-from said c22: sweep it with
  ideal-rs before searching); compacting mech-synth plans does NOT help the pocket search.
* Next ideas (pockets2): t7b 42 needs its coupled bottom (BR merger + BL = 13 cells): a plan input on a
  NON-piston receiver (the note block (2,-4) that BL's observer watches) would split it; e78wsT's K needs
  its torch-off 1 gt later (a sensor hearing BR's puller from 3 blocks while still strongly powering the
  torch's support (4,-3)); c070x's U + M must come from one source 3 gt apart; s27-bl52's B line + support
  (k=6, `runs/b52-tlb-k6`) and top-right (k=4, `runs/b52-tr-k4`) for a 51.
* Lesson (pockets): run the region searches with the FULL palette (`--fill-palette all,sculk,calibrated,wool`) at
  small k first; the 49 came from a target block (4,-1) that the 12-state palettes never tried.
* Next ideas (pockets): a cross-region 48 (no single region of the 49 shrinks); 8x8 m10-family with the extra column
  on the RIGHT (room for the hold's delayed T chain and a torch-held B: m88a's hold at t7..8 has no source
  with the extra column on the left); compact the mech-synth "oc" plans (door windows) before pockets
  (inputs up to t66 need delay lines no pocket holds); a joint 5-block filling for e88 t3n's bottom-left
  (P_M + shuttle; 13 cells x 31 states at k=5 is ~4G screens: needs a smarter palette).

## 1. Tool items (explore/pockets)

* `d976c0f` PERIODIC rest in the screen (main 96a62d6): a closed state may differ from rest as long as the
  door cells are closed and some closed state repeats within cycles + 8 (extra cycles must open with
  frame 12/12 and close); `--exact-rest` keeps the old rule. On c79's bottom-right region the periodic
  screen keeps 116,943 fillings working vs 110,817 exact (bulbs, note blocks, lamps in the palette).
* `586d9e8` OFF-PLANE lever: the grid keeps a lever on a block's front/back face (seeds with one load,
  finds write one, `tracks/pockets/seeds/c79-front.txt` = c79 with the community's front-face lever: WORKS);
  lever candidates on the front face of a filling's own full block or of a static block next to the
  filling (`--no-lever-front`; `--lever-cells x,y,-1`); `--lever-polarity seed|off|on|both`.
  WARM retry (needed for every lever-ON-at-rest door): a filling whose blocks change state while the build
  settles (dust next to the lever, repeaters turning on, a dropper triggered) is screened again placed in
  its settled states (consistent rest, no settling transient). With it c79's lever-held top-right region
  (8 cells + the front-face lever, `--lever-cells 4,5,-1`) is RE-FOUND in full: cosearch2 matched only 4 of
  its 6 groups ((1,3)o2 and (1,4)c4 were lost to the settling transient of the unpowered fillings).
* `d545493` `plan-from --region R --close`: grows R until the door with R erased, driven by the plan,
  goes through the same states as the door (per plane cell, the sequence of states); every stateful
  wiring block next to R (or an observer watching it) that differs joins R. c79: bottom-middle (3 cells)
  closes to 8 (+ bottom-left's observers and dust: cosearch2's finding); the lever's 4 cells close to the
  top-right 8 (+ dropper, its observer, the elevator observers).
* Later tool commits: batch mode (several seeds / `.ideal` seeds / `--box auto`) with a summary table;
  work items split by the first cells' states (no k=6 tails) + a progress line (`--report`);
  `--lever-cells x,y,-1` on a pocket cell = the lever on the filling's own block there; audit item 5: a
  perfect screen the 4-position judge rejects is judged per position and kept as
  `pockets-repair-KofN-*.txt`; in-plane plan input cells hold the plan's blocks, not the build's (the halo
  and outside-the-box checks skip them; in-plane inputs ON at rest join the seed: mech-synth plans screen).

## 2. c2e seeds (coordinator top priority)

* m79a (c2e 1e02280, u7-m10 in 7x9, only A's hold ideal): NO PASS, bounded. The 5 free top cells (single
  cells) x 51 states drive nothing. Top-right re-wired too (2,5;3,5;3,4;3,3;4,3 + the dropper (4,2) by
  `--close`, lever kept ON at (4,5)), k=6 x 14 states (16M screens): two disjoint families, {R pair + TRp + B}
  or {R pair + A's hold + TRp} (dot dust (3,5) on (3,4) drops TRp at t0, TR lands in (2,3) at t3, a
  repeater (3,3) d3 on the dust (4,3) powers TR -> A), never the hold with B: B's level can only come in
  through (2,5), anything feeding (2,5) QC-holds TRp until B drops (>= t9), and the hold needs TR in (2,3)
  by t3 (the top is cut at x=1 by A, B and B's head: the right side is the only early source).
  Best true partial: 48 blocks, only B ideal (`runs/m79a-trA-k6s/pockets-partial-1as.txt`).
* m88a (c2e d58e481, 8x8, whole bottom real) + m10top's m10f top transplanted at the same cells
  (`seeds/m88a-top.txt` + `plans/m88a-hold.ideal`): `door ideal` WORKS with only A's hold ideal (t8, window
  t7..t8). Pockets k=4 x 35 states on its free cells (x=-3 column, (-1,3), (3,3)): nothing drives the hold.
  (-1,3) has no floor ((-1,2) is the left frame cell, air at rest), so (0,3) can only be fed from (0,4)
  (dust on it) or A by QC via (0,4) -> (1,4); m10f's observer (-1,4) gives one 2-gt pulse at L1/L2 time.

## 3. Mechanism inventory (frame at open, `door sheet --fill`)

FULL frame as given: w63top m10 / m10r / m10a, seq56 m4 / m5. 12/12 after `door sheet --fill` (static
stone = the door block in the 3 empty frame cells, written to `tracks/pockets/mech/*-ff.*`): w63top m8, m9,
ring56 t8, t10, jf, mechspace c78-001..006 (their smooth-stone statics were the wrong block for the frame
rule: frame cells must hold the door block itself), pistmodel fm-8x8-000..063 (`mech/fm8/`).

## 4. More c2e / m10top seeds (coordinator hot seeds)

* m79p (m10top bd34158: m79a with B pull-type, A's hold real = the hold family found in section 2):
  B's open pulse must start at t9..t10 (window [-1,0]); top-left (7 free cells, k=6 x 12 states,
  23.5M screens): L1/L2 (4 blocks) or A's last pull (5 blocks) separately, never together, close L1/L2
  never. B's pulse: by hand, every source B can see before it moves is at the right (the left sees
  nothing before B) and every element that fits there changes at t0..t4 or emits into (2,5) (QC on
  TRp, whose drop at t1 the hold needs).
* m79q (m10top a3871e5: B's chain real but 1 gt early): B's chain cells (2,5),(3,5),(4,5) x 55 states
  (`--fill-palette all`), k=3, B shifted +1: nothing. Every element that fits is even (observers 2,
  lamps 4, repeaters even, comparators/torches 2, bulbs 0); the one odd event (TRp's retraction at t1)
  is visible only from (2,5) facing down (output leaves the box) or from the lamp's cell.
  Tool note: m79q's real B trace (t8-9) fails the screen released while a plan pulse at the same
  ticks passes: an observer fires inside the tick, a plan block appears between ticks, so plan
  windows of observer-driven inputs can be 1 gt optimistic. m79q with sculk/calibrated sensors in
  the palette (60 states): nothing either.
* m79p top-right re-run to the end (k=6 x 19 states, 100M screens): B's open pulse in its window IS
  drivable, but never with the hold: (a) repeater d4 (3,5) on the lever -> observer (2,5) -> B at t10
  (TRp then QC-held until t8, so TR lands too late for the hold); (b) an odd-tick chain watching R1's
  retraction: observers (4,2)v, (4,3)v, dropper (3,3), observers (3,4)v, (3,5)v -> (2,5)e -> B.
* m79q-sat (satwire 56b8164, top-right re-synthesized): `door ideal` WORKS at 32 positions with ONLY
  L1/L2 (-2,2) ideal (`plans/p79q-sat-L.ideal`; A's last pull is real: B's pull lifts A to (1,4) and the
  lit lamp (0,5) QC-powers it). But it is circular: B's chain starts from the observer (-1,3) watching
  TL arrive in (-1,2), which only L2 does, and nothing the free top-left cells can see moves before
  that (60 states incl. sculk, k=3: nothing). L2 needs a trigger from the right or the bottom.

## 6. c79 other wirings (each region exhaustive, `finds/`)

* **49 blocks (`finds/c79-49-bl5-br3.txt` = `BEST.txt`, archived `finds/archive/c79-49-bl5-br3-49b.txt`):
  c79's mechanism + cosearch2's 5-block bottom-left + a 3-block bottom-right found with the FULL
  palette (`--fill-palette all,sculk,calibrated`, k=4): observer (4,0) up on R1 -> a TARGET block
  (4,-1) -> dust (3,-1) on the normal piston (3,-2). `verify --extra-offsets 28` WORKS (32 positions),
  rotation-safe at 32 positions, FULL seamless, frame 12/12, 15/15. Ties satwire-63b-49-49b (main,
  satwire's from-scratch re-wiring of the same mechanism: its bottom-right is the same 3-block shape
  with a note block). The bottom-right run also gave 92 more 51-block wirings (`runs/c79-br-all-k4`).
* Toward 48 (all on the 49, plan-from --close per region): bottom-left full palette k=4 (129M screens):
  no 4-block cover ({K + U} 3 blocks, L0 2: still 5); top-left k=5 x 16 states (incl. target, slab,
  trapdoor, glass; 32M screens): no 5-block cover; top-right with the lever on a block's front face
  ((4,3)/(4,4)/(3,4)) and (4,5) freed, k=7 x 12 states (572M screens, on c79-front): no cover with
  6 blocks + lever; bottom-middle + bottom-left (9 cells) k=7 x 8 states (38M screens): no 7-block cover
  ({U, K} 6 blocks, L0 3); top-right's (4,3),(4,4),(3,3) x 60 states (k=3, lever kept): 3 is the minimum;
  the whole top-right (7 cells, lever kept) k=6 x 16 states (incl. target, note block, lamp, trapdoor,
  comparator; 168M screens): no 6-block cover ({R, hold, A's close pull} 5 blocks, {B} 4: 7 is the min).
  So 48 needs a cross-region change (no single region of the 49 shrinks at these palettes).
* 50 blocks: `finds/c79bl50-tl1-63b-50.txt`, `c79bl50-tl12-63b-50.txt` (the 50-block door with the new
  observer-up top-left), verify WORKS 4 positions, rotation-safe.

* top-right (keep-lever, k=7 x 12 states, 85M screens): 5 wirings (`c79tr-63b-51-*`), delay/shape
  variants (dot dust (4,4), repeater pairs 4+1/3+2/2+3/1+4, a comparator at (3,5)).
* top-left (k=6 x 12 states, 73M screens): 30 wirings (`c79tl-63b-51-*`), the minimum is 6 blocks.
  A new family: an observer (-2,3) facing UP fires L2 (+ L1 by QC) from a state change at (-2,4)
  (a d1 repeater reading (-1,4), a note block, a dropper, an observer east), instead of c79's dust
  on (-2,3).
* bottom-middle + bottom-left closed region (8 cells, k=6 x 12 states): nothing new (best partial 49
  blocks with L0 left assisted).
* Region minima at these palettes: top-left 6, top-right 7 + the lever, bottom-right 4, bottom-left 5
  (the 50-block door), bottom-middle 3.

## 5. Coverage batches (k=3, ~37 states, lever both polarities + front face; `runs/b78-k3.log`, `runs/fm8-y2-k3.log`)

No mechanism is fully wired at k=3 (fillings of <= 3 blocks per pocket); what stays undriven is the
center train (A, B, U, K), as expected (c79's own regions need 4-8 blocks):
m4 8 of 18 groups undriven, m5 5/17, m8-ff 13/19, m9-ff 16/20, c78-002-ff 13/23, t8-ff 7/22, jf-ff 5/20
((1,4)o2 (1,-1)o16 (1,-2)o28 (1,-2)c0 (1,4)c2); fm-8x8 (24 flat ones): 11-17 undriven each.
Later batches (`runs/b78c-k3.log`, `runs/su8-k3.log`, `runs/su8-k4.log`, `runs/jf79-k3.log`): c78-001..006,
t10, the 7x8 refits of 8x8-131/135 (oc and sym): 10-20 groups undriven at k=3; jf in 7x8 is unwirable as
is (the middle top puller (1,4) is walled in by the other two pullers, the frame cell and the box top);
su8r-03 17/33 and su8r-05 23/36 undriven at k=4 (their oc plans run to t66), su8-d-sym 9/30 (the elevator
(1,3)/(0,4)/(0,5) and the right side on close).

## 7. Wiring service (coordinator, 09:08Z): the live 8x8 / 7x8 partials

* e88 t3n (8x8, P_M + the left redstone-block shuttle ideal): in 7x8 (x >= -2) the 3 bottom-left cells
  cannot drive the shuttle. In 8x8 (13-cell pocket incl. the x=-3 column) each is drivable ALONE: P_M with
  1 block (a sticky piston (-1,-2) facing west by the shuttle's redstone block: a BUD on P_M), the
  shuttle's 4 pulses with 3 (dropper (-3,-2) east, observer (-2,-2) east, note block (-1,-2) lit by the
  redstone block: the shuttle re-triggers itself); both need (-1,-2); no joint filling up to 4 blocks
  (31 states, 21M screens; `runs/t3n-k4.log`).
* c88 s11 (8x8 c79 mechanism, lever left): R (4,2) and BR (2,-2) with 3 blocks each (k=3); top-left k=5 x
  18 states (181M screens): B (1,5) with 4 blocks (observers (-1,3)v (-1,4)v (-1,5)v + (0,5) west, a chain
  started by TL's cell (-1,2)), A's top pull with 5 alone, never both within 5, A's t3 push/hold never.
* m79s s78b (7x8, seed `finds/s78-a1/partial-full-live.txt`, K + L0 assisted): the bottom-left pocket is 3
  cells; exhaustive (64 states, sculk/calibrated/wool): L0 with 2 blocks, K never.

## 8. pockets2 (2026-10-19 09:24Z ->, same box and branch)

Tools (explore/pockets, one commit each):
* `bc96adfe` `--pocket-k x,y=K` / `--pocket-fill x,y=<families>|states:<..>`: per-pocket depth and palette
  (cross-region runs: a k=7 pocket next to a k=6 one in one run).
* `4ad28287` finer work items for big palettes (a 4-cell pocket x 61 states had 61 items for 160 threads).
* `c6c4d7e2` a RELEASED input's real edges need no plan group: the released re-screen (the door running on
  the real wiring alone) already proves them. Before, a filling with an extra harmless pulse (c88 s11: A's
  push on close too) or ONE long pulse for a push/pull pair (e88 t3k's shuttle) was rejected.
* `0b6a8c43` `door ideal-rs --compact-dir DIR <schedules...>`: every input group as early as the mechanism
  allows (others fixed, 2 passes, merged pulses tidied), `<name>-cpt.ideal` with each group's window.

Results:
* 48 cross-region, TL + TR of the 49 (`plans/c79-49-tltr.ideal`, union plan; TL k=6 x 14 states, TR k=7 x
  14 states, 400M screens, `runs/c79-49-tltr-k67.log`): no 48. TR can take TL's elevator pulses (1,4)o12 +
  (1,3)c12 together with R, (1,3)o2, (1,4)c4 in 7 blocks, but never with B's level; TL alone then still
  needs 5 for TL/ML. With sculk: TL k=5 x 18 states incl. plain/calibrated sensors + wool
  (`runs/c79-49-tl-sculk-k5`): TL/ML both edges with 3 blocks (plain sensor (-2,5) -> observer (-2,4) up
  -> wool (-2,3), a conductor, -> TL's puller + ML by QC) but the elevator pulses still 4: no 5-block TL;
  40 combinations = 7x9 SCULK doors with 51 blocks (`pockets-63b-51-0` verify WORKS, rotation-safe,
  open/close 50): new wirings, no record (sculk board: 50). TR with sculk k=6: `runs/c79-49-tr-sculk-k6`.
* e88 t3k (8x8, only the shuttle ideal): ideal-rs: ONE long pulse (start 24..50, width >= 7) replaces the
  push + pull pair. The 4-cell pocket (full palette + sculk, k=4): plain sensor (-3,-3) next to the shuttle
  piston + wool (-3,-2) and (-1,-2): hears B' (1,-3) at ~t25 and holds the shuttle t30..t59; 24 such
  fillings screen perfect, the redsim judge passes 2 of 4 positions (at #2/#3 the wool's occlusion of P_M's
  t19 vibration flips: the line runs through a block corner) (`runs/t3k-k4/pockets-repair-2of4-*`).
  bridge finished t3k first (archive/bridge-e88-sculk-52b.txt, a calibrated sensor in a wool pocket).
* c88 s11 (top-left k=5 x 18 states, with the released-input rule, `runs/s11-tl-k5fix`): A's push
  (1,3)o3 real with 5 blocks: observer (-3,3) down on the lever's dust (-3,2) -> dust (-3,4) (-2,4) ->
  lamp (-2,3) -> observer (-1,3) west into the static (0,3) (true partial with B, A's top pull, R, BR
  assisted); B's 4-block chain also needs (-1,3): no A + B. s19 (B real): A has no path (the free
  top-left cells are cut off from (0,3)/(1,4) by B's x=-1 chain). ideal-rs: A's push on close too is
  harmless (s11: close push 0..19; s19: 0..3), window start 0..4, end <= 7, length >= 3.
* f131 pn26b (P3 + L2 ideal, `runs/pn26b-k4`): the only pocket is (-2,3..5); observers (-2,5) up, (-2,4)
  up + glazed terracotta (-2,3) (a conductor) give P3 and L2 at t6-8 and screen perfect with the plan
  double-driving, but fully real the door fails (center (1,1) never opens) although ideal-rs with both
  pulsed at t6..8 WORKS: the real drive differs from the plan (placement transient / in-tick timing).
* mechspace single-U oc plans compacted (`cpt/`, `runs/sl-compact.log`): 63 working plans, last input
  t62..66 -> t12 (sl-025, sl-076), t15 (sl-047), t18..30 for 30 more. pockets k=3 (default palette, lever
  both polarities) on the 12 most compact (`runs/slc-k3.log`): 8..22 of 25..33 groups undriven (sl-076 8,
  sl-025 9, sl-047 11), best partials 8..13 inputs left; the same plans UNCOMPACTED (`runs/slo-k3.log`):
  5..23 undriven (sl-025 5, sl-076 7, sl-089 7), best partials 9..12 left: compaction does NOT help the
  pocket search (every compacted group sits at the early edge of its window, many share t0).
* s79's c78z (7x8, only U and R0 ideal, coordinator 10:23Z; `runs/c78z-k4`, the 4 bottom-right cells,
  full palette + sculk, k=4): R0 real on both edges with 2 blocks (2000 variants); U's U2 open + U1 close
  with 3 blocks; never U's 1-gt open pulse nor the close U2: best true partial = only U assisted, 45
  blocks. ideal-rs: U's first pulse must be exactly 1 gt (2 gt: BM lands at (1,-1) one tick after K's
  pull starts, K and L0 are slaved to U through the observer (0,-2)).
* **e88-t7b-43b (the new 43-block record, a 10-piston mechanism) re-wired: `finds/t7b-rwfast-43.txt` =
  BEST.txt, 63b 43 blocks, solid, FULL, rotation-safe, open 39 / close 27 (t7b: 43 / 31), verify
  --extra-offsets 28 WORKS**: its right wall becomes observer (3,3) west -> dust (4,3) -> stone (4,2)
  (MR direct, BR by QC) instead of repeater + observer (4 gt faster). Both walls are minimal at 3
  blocks (k=3, full palette + sculk; 64 wall variants at 43 blocks in `runs/t7b-lw-k3`, `runs/t7b-rw-k3`).
  The bottom: BL (7 cells) is a closed region; BR's merger closes into BL (13 cells). `ideal-rs --drop 2`:
  only the 3 frame stones drop (the frame rule keeps them). BL k=6 x 14 states (813M screens, connected
  fillings, `runs/rf-bl-k6`): no 6-block BL; best true partial 41 blocks with only P_M assisted (BL in
  5: stone (0,-4) under the d4 repeater, note block (-1,-3), observer (-1,-2) UP watching P_M, the gate),
  and no 1- or 2-block P_M driver fits the free cells around it (`runs/rf-pm-k2`, full palette, --gaps).
* `--gaps` (tool bug found on c88 s25, coordinator 10:50Z: everything real but BR's puller (2,-2)): the
  4-cell bottom-right pocket at k=4 x 61 states found no driver because c88b's answer (s26: observer (4,0)
  up -> AIR (4,-1) -> normal piston (4,-2) QC'd -> observer (3,-2) east -> BR) is two pieces; with --gaps
  the run re-finds that 54-block door (`runs/s25-k4g`). s26 (`archive/c88-s26-54b.txt`, solid 8x8 54)
  regions (`s26/`): BR's driver 3 = minimal (k=4 gaps), B's level line (bulb, comparator, d2, d4) 4 =
  minimal (k=3); bottom-left k=6 x 16 states (`runs/s26-bl-k6`, 140M): **3 bottom-lefts with 6 blocks
  instead of 7 = `finds/s26-bl53.txt`, solid 8x8 = 64b, 53 blocks, FULL, rotation-safe, open/close
  34/34, verify --extra-offsets 28 WORKS** (lamp (-3,-2), observer (-3,-1) up, observers (-2,-2) (0,-2)
  west, note block (-1,-2), dust dot (-3,0); K rests retracted, no level repeater). Combined with c88b's
  s27 top-right (one observer (3,4) east instead of stone + dust): **`finds/s27-bl52.txt`, solid 8x8 =
  64b, 52 blocks, FULL, rotation-safe, verify --extra-offsets 28 WORKS** (coordinator 11:12Z).
  s27-bl52's top-right as a plan needs the close A pulse at c0..16 (plan-from said c22: the erased door
  only works at <= 16, ideal-rs sweep; `s26/b52-tr.ideal`): k=4 x 61 states (`runs/b52-tr-k4`, 197M):
  **`finds/s27-trbl51.txt`, solid 8x8 = 64b, 51 blocks, FULL, rotation-safe, verify --extra-offsets 28
  WORKS** (12:27Z): the observer (3,5) became a STICKY PISTON (3,5) facing east, fired by the observer
  (2,5) on B at t18; its head pops into (4,5) for 2 gt, the observer (4,4) facing up sees it and fires R2
  (+ R1 by QC) through the stone (4,3) at t20, and the observer (3,4) east sees (4,4) and pushes A through
  (2,4) at t22 (dust (4,4) + stone (4,5) gone): a piston as a 1-block "something moved" source for an
  observer. B line + support (7 cells) k=6 x 16 states on s27-bl52: `runs/b52-tlb-k6`; shrink takes the
  top-left A part with `door fill`.
* After shrink's **s27-tlp50** (solid 50: a sticky piston (-3,3) up replaces observer (-3,3) + stone
  (-3,4), on my s27-trbl51): bottom-left k=5 x 24 states incl. sticky/normal pistons + sculk (`runs/
  b50-bl-k5`, 168M): **`finds/s27-bls49.txt`, SCULK 8x8 = 64b, 49 blocks, FULL, rotation-safe, open 39 /
  close 45, verify --extra-offsets 28 WORKS** (12:40Z; a plain sculk sensor (-3,-1) + a dropper (-3,-2) up
  replace the observer (-3,-1), the lamp (-3,-2) and the dust dot (-3,0); 7 such 49s, all sculk). No solid
  5-block bottom-left. Minimal on s27-trbl51 / tlp50 with pistons + sculk in the palette: BR driver 3
  (k=2 x 61: none), B's bulb line 4 (k=3 x 61: none), top-right 6 (k=5 x 24 solid incl. pistons, k=4 x 30
  incl. sculk: none). Careful with `--region-pistons` + plan-from: plan-from lists the erased wiring piston
  as an input and its plan blocks at z=1 power fillings (fake covers): drop those events.
* Tile phase (main de6ecbce, vjeux/boxproof 13:20Z): `door cosearch` search/pockets assists now act in the
  TILE-TICK phase by default (f7e7023b: a wall torch on a support turned to stone at t-2, as
  `Schedule::realize`); `--between-ticks` = the old rule (old hand plans may need it or a +1 gt shift).
* q78's e78s (7x8, only the bottom ideal: A' x3, K, B' x2; `runs/e78s-k3`, `runs/e78s-k5`, --gaps):
  right pocket (x=2..4): B' all four pulses with 3 blocks (observer (3,-2) up on K -> lamp (3,-3) ->
  observer (2,-3) east), K with 2, never both within 5; left pocket: A' o8, (1,-1)o24, B' o30 alone;
  A' o36 never. The left pocket at k=4 x 30 states (`runs/e78s-l4`) drives ALL of A' + (1,-1) with 4
  blocks (calibrated sensor (-2,-2) east, observer (-1,-2) west, lamp (-2,-1), observer (-1,-1) west);
  with K from the right (5 blocks): **true partial with only B' ideal, 40 blocks** (`runs/e78s-kb/
  pockets-partial-1as.txt`); B' then has no filling in the 6 cells left (`runs/e78s-b4`, k=4, full
  palette). q78 then wired B', A1 and the late A'-low with sensors (e78wsK): ONLY K left, K's pulse must
  END at t12..13: exhaustive k=4 x 61 states + gaps over the 4 right cells and the 3 left ones: NO K
  driver (`runs/wsK-k4`); the wool (3,-2) is needed (the seed breaks without it), so K's feeds are (4,-1)
  (no west-pointing emitter fits) and its QC cell (3,0).
* ceil's c070x (8x7 = 56, plan with m = u+3; `runs/c070x-k5`, k=5 x 33 states, --gaps): BL-down and
  BM-down with 3 blocks each, E2's open pulse with 4; never U / M (coupled: windows [0,0]) nor E2 close.

## 9. pockets3 (2026-10-19 13:42Z ->, same box, branch and clone; the wiring service)

**THE FIRST 7x8 (15:09Z): `finds/e78-56b-45.txt`, 56b (7x8x1), 45 blocks, SCULK, FULL, open 105 / close 111,
verified on main at 32 positions x 4 orientations (archive/e78-56b-45-45b.txt)**: e78k's r1 (ring56's jf layout
in 7x8, wool door, K by a piston relay, one flaw at open t45) + 3 changes found by `door fill --keep` over the 14
bottom cells (<= 3 changed, 38 states): relay piston (3,-3) -> calibrated sensor E, its head cell (4,-3) ->
calibrated sensor E, (-2,-3) -> normal piston facing up. A retry machine (e78k: the sensors' re-arm cycles replay
the sequence until it converges). Every REGION-LOCAL search had failed (7, 11, 8-cell right cluster, left 9...);
the fix is CROSS-region. Then shrunk with `--keep --max-blocks N` (pre-filtered, ~20 s per step):
**`finds/e78-56b-42.txt`** 42 blocks, open 98 / close 24 (three independent 44-block edits combined: bottom observer
W (2,-3) + note (4,-3) - wool (-1,-1); top-right observer (3,3) W - (3,4) - wool (4,4); top-left observer (-1,3) E -
(-1,4) - wool (-2,4)); 41 (- obsidian (-2,-1)); **`finds/e78-56b-40.txt` 40 blocks, open 98 / close 24, verify 32
positions + rotations** (- wool shield (3,-2), right wall = observers (3,3) W -> (4,3) W -> (4,2) UP). 40 -> 39: none at
<= 3 changes. Speed (`--max-open`): 41 and 40 blocks at open 45 / close 56 exist (runs/e78s-fast3, e78s-fast40: the
note/bulb (-1,-3) + plain sensor (0,-3) + calibrated (-1,-1); or (-2,-3)/(-1,-3) out + piston (-2,-2)) but are NOT
rotation-safe (turned 90: still moving after closing). c77 has the stone line (e78st-56b-42, 51/15).

Tools (explore/pockets, merged on main):
* `28df017e` door fill prints a summary table at the end (finds by volume, blocks, open, close).
* `d4eb7406` door fill `--max-blocks` pruned before simulation (the judge's count per filling), `--max-open` /
  `--max-close` (speed search, checked on the 1-position screen first).
* `ba5f30b8` (main 6e74c92a) `door fill --keep`: region cells keep their seed blocks unless the filling
  changes them; `--k` counts the CHANGED cells (another palette state, or air where a block was) =
  `ideal-rs --vary` with a `keep` alternative and at most k cells off it, judged as the REAL door (no plan).
  Re-finds t7b-rwfast's observer (3,3) after removing it (k=1). Palette shorthands `amethyst`, `air`,
  `calib4` (calibrated sensors facing all 4 ways).
* `7ddfc5a4` (main b272d3c4) pockets IN-PLANE assist guard (t78's hs23-0008L via pockets2): a filling block
  4-adjacent to an in-plane plan cell of an input still assisted in a screen does not count there (the
  plan's redstone block powered the neighbor; real_drive removes it raw, the neighbor keeps its state: fake
  partials). The released re-screen counts when the filling feeds that input itself; combinations with
  such a filling are judged only once that input is released. hs23-0008 k=2: the 1-block "(2,-2) both
  edges" and the single-edge (-1,4) drives disappear. Conservative (`--offplane` frees those cells).

Screens (all real-door `door fill` unless "pockets"; 0 = nothing passes the 1-position screen):
* c77 kb1 (e78 stone, B' (1,-3) the only ideal input; 6 free cells (-2,-3) (-1,-3) (0,-3) (0,-2) (2,-3)
  (2,-2)): pockets kb1p3 (tile) k=4 x 62 states + gaps: no B' group driven; `door fill` k<=4 x 45 states
  (pistons, sculk, calibrated): 0 of 17.6M; k=5 x 46 (+ redstone block): 0 of 288M (948M pruned by the
  support rules); pockets kb1g (c77's pair + open-only gate) k=4: nothing. B' shapes (ideal-rs,
  svc/c77-kb1-*.ideal): ONE HOLD per edge works (10,062 of 2.17M): open = a hold ENDING AT EXACTLY 28
  (start 17..21; [18,28) = a calibrated sensor) or [23,46)/[24,44) or 17 gt from 27..45; close = any hold
  from 27..45 of 4..23 gt (not 17) or short ones at 18..22; TWO 2-gt pulses per edge work (open 17..22 then
  >= 28; close q1 25..30, q2 >= q1+4); one pulse, open-only holds and the same hold on both edges fail. B'
  feeds: only (0,-3) and (2,-3) (QC through (1,-2) = A' itself); an observer only from (0,-3) facing west.
* e78k r1 (K real by the piston relay; flaw at open t45): --keep over the 7 non-shrink cells <= 4 changed
  (23 states + air): 0 of 10.2M; all 11 cells <= 4: 0 of 94.4M; right cluster (2,-3) (2,-2) (3,-3) (3,-2)
  (4,-3) (4,-2) (4,-1) + (-2,-1) with repeaters/comparators/torches (41 states) <= 3: 0 of 1.45M, = 4: 0 of
  54M. r2 (observer (0,-3) on (-1,-3)): left 7 + top 2 <= 4: 0 of 36.3M. nb (note-block B' clock): (-2,-3)
  (-2,-2) (-2,-1) (-1,-1) (0,-1) (-1,3) (3,3) <= 4 x 29 states: 0 of 19.6M (shrink: k=5).
* c78l L3a (c78z with only TP (-1,3) + L1/L2 ideal): top-left 7 cells <= 3 changed x 49: 0 of 2.8M, = 4:
  0 of 119.6M; wool-door L3aw <= 3: 0 of 2.8M. Windows (svc/c78l-L3w.ideal, widths free): TP open = power
  OFF at exactly t13..14 (on from <= 12), TP close = on at 8..12 then off after 2..14 or >= 26; L open = on
  9..30 off by ~31; L close = EXACTLY one 2-gt pulse at 6..28. The sensor (-2,5) hears L0 at d 5 (t8 open /
  t6 close -> t13 / t11). M/s2 cannot be emulated by off-plane inputs: anything powering M's cell (1,5)
  QC-powers s (1,4) and a plan torch flips the update order (s extends first, M blocked).
* mechspace z1-s78 flat top 10 (pockets batch, between-ticks, k=3, lever both polarities, `runs/msz1-k3`):
  0873 / 1281 best partial 8 inputs left, 0667 7 left (the held stack (-2,1)/(-2,2) never real).
* t78 h8Q (hs23-0008 + real lever/A/BR, 6 inputs off-plane): k<=5 drives SU(0,-2) (2 blocks), SE(-1,3)
  both edges (3), B o6+o24 (1); top-right pocket 4 at k=5 = every filling (483M): SD(1,4) all groups (3 blocks),
  TR o30+c18 (4), TR+MR o30 (5), MR close c18 NEVER; best partial 4 inputs left (36 blocks). 24-plan hs23 batch
  k=3: 7..13 undriven groups each, best partials 7..10 left.
* ring56 t9e (7x9, B window + A2 + A3 ideal, left/BR/U pinned): pockets k=3 over the top-left 11 cells: none of
  B/A2/A3 (the top-left is blind until B moves); the top-right region (2,5) (2,6) (3,5) (3,6) cleared + (4,5) (4,6)
  at k=6 x 24 states started (runs/t9e-tr6) for the orphan partial t8, stopped at ~10% to free the box for the
  solid 7x8 work (no owner left; rerun when the box is idle).
* SOLID 7x8 FOUND by e78k (16:51Z, explore/q78 e755e4de tracks/q78/e78s/h2.txt, 45 blocks, 44/15: rr2 + a B'-watcher
  loop); shrunk here to **`finds/e78s-56b-44.txt` 44 blocks SOLID, 44/15, verify 32 positions + rotations** (top-left
  observer (-1,3) E into the row-3 dust, row-4 wiring gone); no 43 at <= 3 changes. c77's stone line: e78st-56b-38-1s
  (one sensor) minus obsidian (-2,-1) = **`finds/e78st-56b-37.txt` 37 blocks sculk, 48/15, verified** (75 finds at 37
  from --keep --max-blocks 37: the 46/14 ones turn (0,-3) back into a calibrated sensor); no 36 at <= 3.
* SOLID (sculk-less) 7x8, vjeux's ask (16:06Z): every sensor-to-solid swap fails: wool 40 with all 3 sensors forced
  non-sculk + <= 1 change 0 of 16.7M, each sensor alone + <= 2 changes 0 (8.3-8.6M each); c77's e78st-56b-40 with the
  bottom 15 cells cleared, k <= 3 non-sculk 0 of 2.9M, sensors removed + --keep <= 3 0 of 11.2M. The sensors'
  re-arm cycles ARE the retry timing (partials/e78.md). For c77: stone 40 minus S0 / S2 / S4 at --keep k <= 5 with a
  pruned 28-state solid palette (runs/st40-s*-k5; S0's screen passes are all NOT rotation-safe). For e78k's s11
  (solid from scratch, 39 blocks, only B2 = a 2-gt pulse into B' at t >= 29 ideal): --keep <= 3 over its 25 wiring
  cells, 42 solid states (runs/s11-keep3).
* Cross-region --keep on other one-input near-doors (the e78 recipe): c78l L3a (TP + L) 0 of 57.7M; c70b m2 (BL)
  0 of 47.1M, m2w 0 of 47.1M, m2 with the 5 bottom-left cells cleared at k <= 5 x 47 states 0 of 70.8M (255M
  fillings); c78z p5-0 (P2) started, stopped for s11 (rerun). Candidates still unscreened this way: f131 pn31 (7x9,
  only P3: svc/f131-pn31.txt, /tmp-free plan in tracks/f131/ppn31.ideal), satwire m79q-sat-topright (7x9, only
  L1/L2: plans/p79q-sat-L.ideal).
