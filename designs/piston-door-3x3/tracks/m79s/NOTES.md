# Track m79s: finish m79q (7x9 = 63, u7-m10 mechanism, B pull-type) into a working door

## RESULTS
1. **m79s-sculk-63b-50.txt (= BEST.txt), archived m79s-sculk-63b-50-50b**: 63b (7x9x1), 50 blocks, sculk,
   slime-free, flat, FULL seamless + hallway, frame 12/12, open 43 / close 35, verify 32 positions x 3
   cycles, rotation-safe. satwire's m79q-sat (B and A's last pull follow TL's arrival) + its last ideal
   input (L1/L2) wired by a calibrated sculk sensor (-2,5) + observer (-2,4) -> wool (-2,3); the sensor's
   window also holds B (dust (-1,5) -> wool (0,5)); wool (-2,3), (0,4), (0,5) occlude the re-arm echo.
2. **m79s-sculk-64b.txt, archived m79s-sculk-8x8-64b-50b (the first 8x8 door)**: 64b (8x8x1, x -3..4,
   y -2..5), 50 blocks, sculk, FULL + hallway, frame 12/12, open 41 / close 33, verify 32 positions x 3
   cycles, rotation-safe, NO ideal input: s78b's mechanism (below) with
   - U delayed +2 gt (observer (2,-2) facing east instead of the stone), which moves K's windows to open
     end <= plan 12 / close start 9..11;
   - K: PLAIN sensor (-3,-2) (lever out of its range 8, hears R0 at t1, d 7.3 -> t8) -> lamp (-3,-1) ->
     observer (-2,-1) facing west -> K at t10 on both edges; the lamp gives only 2 edges (lit, dark 34 gt
     later), both before the sensor re-arms (t48), so no echo; K's second pulse is blocked on open (BM, C,
     (2,-1), obsidian (3,-1)) and pulls nothing on close;
   - L0 + L1 + L2: calibrated sensor (-3,5) (the lever is occluded by wool (0,5); hears R1 at t1 -> t9) ->
     observer (-3,4) up -> wool (-3,3) -> dust (-3,2) (points into L2) on stone (-3,1) (L1 directly, L0 by
     QC through L1's cell); 3 taps 10 gt apart (pull/push/pull on open, push/pull/push on close);
   - B held during the sensor window: dust (-2,5) on GLASS (-2,4) -> dust (-1,5) -> wool (0,5) (a stone at
     (-2,4) QC-holds L2 through (-2,3) and breaks it);
   - wool (-3,3), (-2,3), (0,4), (0,5) occlude L0/L1/L2, K, A, B from the top sensor; the top cascade
     ends (t43) before the bottom sensor re-arms (t48).
## State (07:50Z)
- satwire's m79q-sat-topright (copied: sat-topright.txt, sat.ideal): its top-right does NOT drive B. B and
  A's last pull are driven by the still-ideal L1/L2 input: observer (-1,3) watches TL's wall cell (-1,2)
  -> stone (-1,4) -> dust (-1,5) -> lamp (0,5): B directly (L2's head arriving, tap a), A by QC through
  (1,5) (TL landing 4 gt later, tap b: B and A land in the same tick, B's push fails, A fires).
  sat-noA.ideal (A's ideal removed) WORKS in redsim: the ONLY input left is L1/L2: one 2 gt pulse per edge,
  open plan tick >= 6 (any later), close any. The pulse must be exactly ~2 gt on BOTH edges (open: the
  B/A coincidence needs TL to land 4 gt after the head; close: spit).
- The top-left is sealed from the lever: nothing it can see moves on open before L2. Only sound (sculk)
  crosses.

## Sculk bridge (cand/)
- s1/s2: sensor on (-2,3) (window into L2): open pulls TL but the long window kills the B/A coincidence;
  close re-pulls. Window = dead.
- s3/s4/v1/v2: sensor (-2,5) + observer (-2,4) up: 3 taps per activation, re-arm echo loops forever.
- v3/v4: calibrated (-2,5) facing west (back to the dust, so no power into the B/A chain) + observer
  (-2,4) + WOOL at (-2,3) (L2/L1 occluded), (0,5) (wool replaces the lamp as B's conductor: B occluded),
  (0,4) (A occluded): the echo is GONE (settles), sensor hears R0 at ~t1 (activation t11). Fails because
  calibrated taps are 10 apart: tap b's L2 push comes at P+10 and its B push at P+12 while A's last
  pull is still landing (needs P+13): one tick short again. A plain sensor (taps 30/10 apart: fits) has
  no legal cell: (-2,5) powers the dust (-1,5) (B for 30 gt), (-2,4) QC-powers L2, (0,4) powers A.
- Copper-bulb T flip-flop (1 tap per activation) needs sensor + bulb + comparator + observer: no room
  next to the fixed B/A chain ((-1,3),(-1,4),(-1,5),(0,5)); the comparator can only read through (-1,4).

## 7x8: m10top's m10f (A's hold + L0 ideal) with sculk (08:05Z-)
- Windows (ideal-rs): A's hold start plan 7..9 for 3..5 gt (start 7: 3-5, 8: 3-4, 9: 3); L0 any 0..40.
- Only 2 free cells ((-1,3), (3,3)): pockets k=3 with the sculk palette: no filling drives the hold or L0.
- cosearch search (sculk + wool palette, pistons locked, 56 threads, 25 min): running.
- m10f annealer (25 min, 70M screens): nothing (2 assisted left). The 7x8 box is full.

## 7x8: s78b = satwire's m79q top on c2e's 2-row 7x8 bottom (s78/, 08:20Z)
- s78b.txt + s78b.ideal: ideal-rs WORKS (4 positions x 2) with only K (-1,-1), L0 (-2,0), L1/L2 (-2,2)
  ideal (U real: R0's watcher (4,-1) -> lamp (4,-2) -> observer (3,-2) -> stone (2,-2): U at t5 (blocked by
  the hold's column on open) and t11). Windows: K open = a pulse ENDING at plan 6..10 (any start), close =
  START 7..8 (any length 1..12); L0 any; L1/L2 open >= 6, close any.
- The m79s top-left sensor does not carry over: in m79s it heard the bottom chain's signal piston (3,-2)
  at t3 (d 8.6 -> t11, power 7); with this bottom it hears nothing (s78a).
- pockets k=3 (sculk palette): the bottom-left (4 cells) drives L0 with 3 blocks: dropper (-1,-2) (QC-senses
  power INTO K's cell) -> observer (-2,-2) east -> observer (-2,-1) down -> L0, i.e. L0 follows K's feed.
- Why K is the wall (by hand): K must pull BM before U's head enters (1,-1) at t11, so it cannot follow U
  on open; its feeds are (-2,-1) (east) and (-1,-2) (up): an observer there would look outside the box,
  (-1,-2) can only be strongly powered from (0,-2) (an observer watching U, too late), (-2,-1) only by dust
  (then L0, whose only feed is (-2,-1) upward, is dead) or by a source emitting both ways (a sensor: a 10 gt
  window ends too late on open: it would need to start at t1). So K + L0 by sound do not fit the 4-cell
  bottom-left. Annealers running anyway (pistons locked, and a second run with piston moves).
- Solid 7x9 (no sculk) annealer, 30 min, top focus, lever moves: nothing (the top-left is sealed: it sees
  only L2's head cell, B's head cell (1,4) and the static (0,3), none of which moves before L2/B).
- s78-a1 (pistons locked, 40 threads): L1/L2 real in 7x8 (calibrated sensor (-2,5) + observer (-2,4) +
  wool, the 63's trick; finds/s78-a1/partial-full-live.txt): K and L0 left. s78-a2 (piston moves allowed,
  37 min, 94M screens): no release.
- Eligible variant of the 63 (`door relever --seamless full+hall`): none: a note block in the lever cell
  (4,5) no longer powers the log (4,4) -> dust (4,3) -> the R pair's dropper (not closed at rest); repeater
  input into (4,4) would miss the dust (3,5) (TRp's hold).
- Solid 7x9 with this mechanism: impossible by the case analysis above (the top-left sees only L2's and
  B's head cells and the static (0,3); B and A's last pull need the pull-type head pattern (open 2 taps
  4 gt apart, close 1 tap) with a master piston that the lever must reach first; the elevator column
  (1,3..5) is the only element shared by the two top halves).
- Next runs (sculk + WOOL palette, which the sculk track's runs lacked: wool occlusion kills re-arm echoes):
  ring56's t8L and t10s 7x8 seeds (sk/), all inputs assisted.

## cosearch2's f131F-b2 (7x9, wire131's mechanism, ONE input left: N's train) (09:13Z)
- N's input is the iron trapdoor (-2,4) watched by the observer (-1,4): 3 toggles per edge (open on/off/on,
  close off/on/off). f131/p131F-shift.ideal: the train WORKS for EVERY common shift d 0..12 and spacings
  g, h 8..14 (all 637 combinations), so a calibrated sensor IN (-2,4) (state changes T, T+10, T+20, seen by
  the same observer) is exactly N's train.
- But f131/c-*.txt / p.txt (sensor in (-2,4)) never settle: the design's own settle motion (TRp (2,4)
  retracts at t1 and re-extends at t4 during placement) wakes the sensor, its taps move the left pullers
  next to it, and the calibrated re-arm (T+20) hears them: an echo loop. Needs wool around the sensor
  ((-2,3) and (-1,4) are the observers that must stay) or a different N watcher. cosearch (sculk + wool,
  20 threads) running on it.
- f131F is agent f131's (coordinator 09:45Z): left to them. My finding for them: the comparator (3,4) that
  relays the redstone block (4,4) to TRp makes TRp retract and re-extend during the settle (a comparator's
  block entity starts at 0); a delay-1 repeater there (f131/v3.txt) does the same job with no settle motion
  (p-v3.ideal WORKS), after which a calibrated sensor in (-2,4) settles at rest but echoes after the open.

## 7x8 with this mechanism: the bottom-left wall (09:47Z)
- s78c (7x8, U delayed, K and L0 EXTENDED at rest, both held by a redstone torch (-2,-1) standing on
  (-2,-2): powering (-2,-2) while open turns the torch off, K pulls BM and L0 pulls BL; on close the torch
  relights and both push back): s78c.ideal WORKS with the level at (-2,-2) set at plan 7..10 on open and
  cleared at 5..9 on close (L1/L2 ideal). So the 7x8 needs ONE lever-like level at (-2,-2).
- But nothing in the 7x8 bottom row can supply it: row -2 holds no dust, repeater, comparator or torch (no
  floor), so (-2,-2) can only be strongly powered by an observer at (-1,-2) (a 2 gt pulse) or be a source
  itself (a sensor: a 10/30 gt window; the torch cannot stand on it); a window ends inside the open state
  and the relit torch makes L0 push BL back (K's re-push is blocked by the obsidian line, L0's is not).
  The pulse-type alternatives fail as before (K and L0 share (-2,-1); an observer there feeds only one of
  them). So the m79 mechanism needs the 8th column (the 64) or a different bottom.
- Pockets/annealers in 7x8 (sculk + wool, 2.5 h total): s78b L1/L2 real (2 left: K, L0), m10f nothing,
  ring56 t8L/t10s nothing.

## Next ideas
- Solid versions: none with this mechanism (the top-left only sees L2's and B's head cells, which the lever
  cannot reach first). A solid 7x9/8x8 needs a mechanism whose sealed region contains a lever-reachable mover.
- 7x8: a bottom whose corner piston and BL puller are fed from two different cells, or BL leaving down;
  mechspace's c78-* (the community 7x9 squeezed into 7x8: the bottom is sealed from the lever) with the
  bottom woken by a plain sensor + lamp + observer (this track's K trick) and the top as in c79.
- Tricks worth reusing (all proven here): wool occludes sculk echoes (a wool static or conductor block on
  every line from the late movers); a plain sensor + lamp + observer = 2 clean taps, both inside the
  sensor's 40 gt dead time (no echo); a calibrated sensor + observer = 3 taps 10 gt apart (an odd count for
  a pull-type piston); a sensor's distance sets the parity (activation = event tick + floor(d)); out-of-range
  (plain 8) or wool-occluded levers make a sensor hear the t1 pistons instead.
