# Track bridge: the sculk bridge (m79s's trick) applied to every near-complete partial

Brief (coordinator 08:17Z): m79s finished a 7x9 by wiring its last input WIRELESSLY with a
calibrated sculk sensor. Apply the same bridge to the other near-complete partials: c2e's m88c
(8x8), ring56's t8xL / t8xL-b (8x8), q8seed (7x8) and p2 / jf (7x9), wire131's f131F-b2 (f131's).

## RESULT 1 (09:20Z): bridge-p2-sculk, 7x9 = 63b, 49 blocks, sculk, slime-free, flat, FULL
## RESULT 1b (09:27Z): bridge-p2-sculk-r = the same + OBSIDIAN at (-1,-1): ROTATION-SAFE

`jf/p2-sculk-r.txt` (= BEST.txt): `door verify` WORKS (4 positions x 3 cycles, open 49 / close 20)
and rotation-safe (90, 180, 270 at 4 positions each); redsim eval WORKS at 32 positions in all 4
orientations. Why p2-sculk broke at 180 degrees (traced in redsim on `door turn`'s output): the K
sensor's SECOND activation (it re-arms at t29 and hears A' high extend at t31: active t33-43)
leaves K powered but blocked (A' is moving) until the gate observer (2,-2) fires at t35 and updates
both A' (west of F2) and K (east of F2). Updates go west first, so as built A' queues its block
event first, extends and becomes immovable, and K's push fails; mirrored, K goes first and pushes
F2's BM, A', the frame stone, the trapdoor and the BL watcher one cell west. The iron trapdoor
(-1,-1) was unused (air works as built: 48 blocks, but only facing one way); obsidian there makes
K's stray push fail in every orientation. 162 variants of the free cells / sensor facings worked
as built and 0 turned: the push line, not the sensors' placement, was the cause.

`jf/p2-sculk.txt`: `door verify` WORKS (4 positions x 3 cycles, open 49 / close 20),
NOT rotation-safe (180 degrees); redsim: `cosearch eval` WORKS at 32 positions, FULL, hallway ok,
frame 12/12; `door sheet --fill`: frame 12/12 (the three static frame cells changed from smooth
stone to the door block, stone); `door properties`: FULL seamless, dentless, volume 63 (7x9x1).

ring56's p2 (layout J: top row UP, ML+BL left, MR+BR right, C down 2 by the bottom elevator A'/B',
BM down then right into F2 by the corner pull-type K (3,-1); every piston wired by cosearch except
K) + TWO calibrated sculk sensors, nothing else:
- (3,-2) calibrated sensor facing WEST replaces the delay-4 repeater that stood there. It hears the
  lever (distance 9.4: activation at t9 on both edges) and powers K from below for its 10-tick
  window [9, 19]: K as a 10 gt hold works at start 8..13 (open) and 8, 9, 14, 15 (close) (c2e's K
  windows were for 2 gt pulses). Facing west keeps its back on the lamp (4,-2) (no power into the
  lamp, whose observer would fire B' again) and still emits west into the gate observer's cell,
  so it QC-holds the dropper (2,-3) like the repeater did (its second activation).
- (0,-3) calibrated sensor facing EAST powers B' (1,-3) directly (its back on (-1,-3)): the
  repeater's job (holding the dropper over [28, 36] so the gate does not loop) is no longer needed
  as the bottom's timing now comes from the two sensors.
Found by `door ideal-rs --vary` (new, this track): 151,200 block variants of the bottom-right of
p2 with sculk sensors, observers, wool, note blocks... screened in 7 s with only the lever real;
4 work, all the same two sensors.

## RESULT 2 (09:50Z): bridge-e88-sculk, 8x8 = 64b, 52 blocks, sculk, FULL, frame 12/12, rotation-safe

`e88/t3k-sculk.txt` (archived by the coordinator as archive/bridge-e88-sculk-52b.txt, credit e88 +
bridge): agent e88's t3k (everything wired but the shuttle piston (-2,-3), whose push and pull
pair works as ONE hold starting at any tick >= 24) + a calibrated sensor (-3,-3) in a wool pocket
((-3,-2), (-2,-2), (-1,-2)) that hears the door only late and holds the shuttle for its 10 gt
window. Found by a 980-variant `ideal-rs --vary` screen in 0.1 s; `--drop 2`: only the static
frame stones can go (52 is minimal). door verify WORKS + rotation-safe; open 45 / close 27.

## The recipe behind both results
1. Turn the missing input into a HOLD and sweep it (`door ideal-rs --set/--sweep`): a corner puller
   (p2's K) or a push-then-pull pair (e88's shuttle) often works as one 10 gt (calibrated) or 30 gt
   (plain) hold at a free start; pistons that must spit (pull-type pulses on close) never do.
2. Put sensors on the piston's free neighbours and screen exhaustively (`--vary`), with wool cells
   around the sensor to delay what it hears first (e88) and to stop re-arm echoes.
3. Check every orientation: vibration order is direction-dependent (p2-sculk broke at 180 degrees).

## The other partials (why the bridge does not fit, all measured with ideal-rs)

Every count below is an exhaustive `door ideal-rs --vary` screen (only the lever real unless noted;
sensors = calibrated facing W/E/N + plain; ~10-20k combinations/s on 20 threads).

- c2e's m88c (8x8): A's hold must start 7..9 and last 3..5 gt (extend 8..10, retract 11..13);
  a sensor gives 10 (calibrated) or 30 gt windows, an observer on it 2 gt taps (a 2 gt pulse
  makes A spit). Why a longer hold fails (trace): B (1,5), lever-held through the two delay-4
  repeaters (3,5), (2,5) (the most the row has room for), retracts at t16 to lift A to (1,4) for
  its last pull; with A still extended at (1,3) that pull fails and TM stays at (1,2). A 4 gt pulse
  needs a delay-2 repeater on (0,3) or (-1,3), but (0,3)'s floor is TL (pulled away at t22: the
  repeater pops) and (-1,3)'s floor is L2's frame cell (air at rest). On close A sits at (1,3) from t18 and any update + power there breaks the close
  (ideal close pulse at (1,3) ok only if it ends by t17), so A's last pull must stay c2e's
  side-powered stone (0,4), which leaves (0,3) (fed only from an observer at (-1,3), 2 gt) for the
  hold. A's last pull itself: open only, start >= 15..17, ANY length (a 30 gt hold works); L2/L1:
  one 2 gt pulse, open any tick, close any but 13, 15..17 (holds fail on close: L2/L1 must spit).
  Screens: top-left 5 cells 117,600; top-right + top-left (the TR-at-(2,3) trick with every
  (3,5)/(2,5)/(3,3) delay) 33,600 + 80; sensors in 6 cells top + bottom 374,220; sensors replacing
  bottom-chain parts + the top 97,200: 0 work. (m79s has since made an 8x8 of this family with
  satwire's m79q top, whose hold is real.)
- t8xL-b (8x8): A at a1 4..6, a2 20..27, a3 34..42; B's two 2 gt pulses must be 12 apart (16 for
  b1 15..16), tied to the left side's parity chain. With that chain's repeater (-1,5) at delay 2
  B may be 10 apart (calibrated observer taps) and at delay 3 14 apart (sensor -> lamp -> observer),
  and B may even be ONE 10 gt hold (b 15..30, A2 = b+2, A3 >= b+12, delay 1 or 2). With W freed from
  its real chain the WHOLE elevator fits sensor outputs (`ring56/t8xLb-h4.ideal`, 264 combos): A =
  the 3 taps of an observer on a calibrated sensor (t2+2, t2+12, t2+22, any t2 0..21), W = ONE 2 gt
  pulse at t2+4..6 (a second W pulse 4, 6 or 14 later is harmless, 10 is not), B = a 10 gt hold at
  b = t2+7..9 (left repeater delay 1 or 2); U = the 3 taps of a calibrated sensor too, activated at
  15 or >= 35 on close (`t8xLb-u3.ideal`: 1647 of 3721 (open, close) pairs). The layout fails: A's
  only feed is an observer at (2,4) watching (3,4); a sensor there holds W (3,3) below it for 10 gt,
  a sensor at (4,4) behind an observer (3,4) QC-holds W through (3,4) (facing east fixes that), and
  every placement (4,4), (4,5) behind 2-3 observers re-arms at T+20 and re-hears the elevator (B's
  release, A's own taps) 2-3 cells away through the observer chain, which no wool can shield: an
  endless echo (A taps every ~27 gt). Screens: A real with W/B ideal 26,784 + 60,264 + 321,408,
  whole top-right 504,000 + 97,200 + 43,200: 0 work. Screens with only U ideal (A and B real), A+U
  ideal, B+U ideal (52,920 each), B with the left repeater varied (306,180), sensor/observer
  placements for A and B at once (40,000): 0 work. U (1,-2): any
  2 gt pulse on open, >= 36 on close; 10/30 gt holds fail on both edges (U must spit on close).
- q8seed (7x8): the same top elevator as t8xL (A x3 from (2,4) only, B x2 from (2,5) only) plus U
  and BR's puller: not attempted beyond the t8xL-b result.
- ring56 t10 (7x8): its elevator accepts A at a, a+10, a+20 (sculk track), but A's only feed is the
  observer (0,4) watching (-1,4), and a sensor there holds the corner puller E (-1,3) below it for
  10 gt (E must pull TM before B1 pushes A into (1,3)); a sensor at (-2,4) QC-holds TL's puller
  (the sculk track's t10-sk1 failure). Not wirable this way.
- ring56 j12 (jf with the dust top): does not work with K ideal at any tick (its right half needs
  re-timing), and 161,280 bottom-right sensor variants: 0.
- wire131's f131F-b2 (f131's): N needs a LEVEL while open plus a blip (off 12..22 on open, on 12..22
  on close); all 11 sensor/observer states of its one free neighbour (-2,5): 0.
- Seen while scanning every explore/* plan with <= 2 ideal pulses per edge: e88 t3k (8x8, only the
  shuttle ideal: finished here as RESULT 2), m56 p1 (7x8, A's hold + L0; the TR relay is 1 gt
  short), s79 c78m (BR's puller only, but U/K/L0 unwired), m79s s88f (K, L0, L1/L2; m79s finished an
  8x8 there), c88 s19 (A's push+hold 3..4 gt: not a sensor window; c88b on it).
- e78 (`e78/t3k7.txt`): bridge-e88-sculk without its x=-3 column is a 7x8 = 56 box that WORKS with
  P_M (-1,-1) and the shuttle ideal; the shuttle may be any 10 gt hold >= 24, but P_M must be ONE
  2 gt pulse at 17..22 (10/30 gt holds: 0; 3 calibrated taps: only pm 22 with the shuttle at 29),
  and in 7 columns its feeders are (-2,-1) (an observer there would watch outside the box; a
  conductor there needs an emitter at (-2,-2), whose only watched cell is the shuttle itself, >= 24)
  and (-1,-2) (an observer facing down watches the redstone block, which moves only with the
  shuttle). So P_M keeps e88's x=-3 observer column (the 8th column).
- ring56 s1 (7x8 single U) with K moved right and p2-sculk-r's bottom (`x/s1k.txt`): works with the
  top/side inputs ideal at (ML 0, BR 15..16) or (21..22, 37..39), but BR's puller has no feeder other
  than (4,-1), which the transplanted bottom needs as BR's watcher (sensors there: 0).
- Top pullers of s1 accept 3 calibrated taps at any tick (`x/s1-tl3.ideal`: 651 of 651).

## c077 (ceil's 8x7 = 56 mechanism, ceiling-held top) with sensors (10:35Z-, in progress)
- Why c077 and not c079: c079's BM puller (1,-2) sits between the BL and BR pullers, so its only
  feed would be (1,-3) below, where nothing can emit upward without a floor at y=-4 (8x9). In c077
  F0/F2 are static and BM's puller has free side cells (0,-2)/(2,-2).
- Schedule (plan `c077/c077v.ideal`, every event its own variable; windows with the others fixed):
  BL's puller (-2,0) open 1, 7, 31 / close 1, 13; ML's puller (-2,1) open 1, 31 / close 13 (7, 9,
  12, 15 only); TL corner (-1,3) 7 both (3..10 / 2..10); P3 (-3,1) open 13 + 25 (8..17, 21..26; or
  one 10 gt hold 13..16); P1' at (-1,1) open 19 (13..21); BM/MR/BR open 31, close 19. MR, BR and BM
  moved TOGETHER work at ANY common tick (61 of 61 starts, same on both edges).
- Wired: MR + BR at t5 on both edges with 3 blocks: observer (3,3) facing west on TR's park (fires
  once per edge, t3), note block (4,3) (basedrum: it sits on an observer; a harp note block flips
  instrument and breaks the rest check), observer (4,2) facing up (MR direct, BR by QC). With MR/BR
  there, BM must fire at >= 15 on both edges (`c077/c077-r1.txt` + `c077-r1.ideal`: WORKS with BM at
  20 and the left side ideal). Park watchers of the pulse-type R0/BR pullers fire twice (4 gt apart),
  so BM needs another single-pulse source (3 calibrated taps: 0).
- Left side open/close asymmetry is the crux: ML+BL fire together at open 1 and 31 but at close 13,
  BL alone at open 7 and close 1. An observer on TL's puller (t3 both edges) fits the open ML pulse
  but not the close side.
- Annealer runs: `door cosearch search --box=-3,-2:4,4 --assist c077/c077-r1.ideal --offplane
  --no-mech --palette all,sculk,calibrated,wool` (runs/c077-r1-anneal.log, finds in c077/finds/).
  After 8 min (23M screens): it wires P1' (the 18-tick input at (-1,1)) but keeps BL, ML, the TL
  corner, P3 and BM assisted; no find.
- Dead ends measured: P3 as a sensor hold (13..16): a sensor at (-3,0) is adjacent to BL's puller
  (hears it at t1), at (-3,2) it can only be shielded by wool at (-2,2), which is ML's only feed
  cell. BM as one pulse >= 15: the bottom-right has no floor for a comparator (no copper-bulb
  T flip-flop: an observer on a bulb also sees its `powered` flag, 2 pulses), park watchers of the
  pulse-type R0/F1 cells fire twice, a calibrated sensor at (4,-2) hears the lever at t7 and
  re-arms into the late door events (3 activations). The TL corner from TL's puller watcher (-1,4)
  is QC-only (no update reaches (-1,3) until TL lands at (0,3) at t3 = plan 2, open needs 3..10).
- Next ideas: a lamp turns a 2 gt pulse into two pulses 6 apart (on edge, off edge + 4), the exact
  rhythm of the left side (open 1, 7, 13, 19, 25, 31; close 1, 7, 13): a lamp ladder from TL's
  puller watcher could drive BL / TL corner / P3 / P1' in turn; BM from the left side (it may fire
  at any open tick and >= 15 on close).

## e78: q78's 7x8 (the jf/p2 mechanism with e88's lever-held top row), stone-door bottom (11:12Z-)
- q78's e78m (all ideal) / e78s (top + both sides wired, bottom ideal) work for any bottom start;
  the bottom = p2-sculk-r's rows -1..-3 (e78a) does not.
- `--holds` on e78s (d=8): B' (1,-3) = ONE 10 gt hold from 19..22 (both edges); A' lifted (1,-1) =
  hold 17..19 or a pulse 21..27 (open); K (3,-1) = a pulse 11..17 (2 taps 12..14, 3 taps 16..17);
  A' low (1,-2) has no sensor mode (two pulses on open, one on close).
- Why p2's bottom fails on the stone top: the bottom sensors hear the top at t0/t1 (K sensor (3,-2)
  at t7, B' sensor (0,-3) at t8), the sides fire BL/BR at ~t4 (TL/TR puller watchers), so the
  sensors come 3-4 ticks early relative to BL (p2: BL t2, sensors t9) and the right chain 14 ticks
  early (p2: BR t18). Wool cannot shield the sensors through a stone door (the lines cross TR/MR/BR):
  q78's wool-door variant fixes the K sensor this way. The K sensor also QC-triggers the dropper
  (2,-3) whose state change fires the gate observer (2,-2): replacing it with an ideal K hold breaks
  even p2 itself (0 of 169), so the two jobs cannot be separated in place.
- e78n (`e78q/e78n.txt`, sides ideal): 0 for every (left, right) side tick 0..40; bottom sensor,
  dropper and wool variants: 25,600 + 19,200 combos, 0.
- Annealer on e78a/e78s's bottom rows until ~12:02Z: runs/e78-anneal.log, finds in e78q/finds.

## Tools (this track)

- `door ideal-rs --vary x,y[,z]=state|state|keep|air` (repeatable: every combination, times the
  `--sweep` grid) and `--ok-only`: block variants applied in memory to the plan's design before each
  run (redsim, no JVM): the exhaustive screen that found p2-sculk (151,200 variants in 7 s).
- `door ideal-rs --drop N`: also every subset of up to N removable blocks (not the door cells, the
  lever or heads) taken out: a redsim block minimizer (p2-sculk-r: only its static frame stones and
  the orientation-only obsidian can go).
- Pitfalls: `--vary=-1,4=...` (with `=`) for negative x; `door patch` starts a JVM (~40 s), use
  `--vary` instead.
- `door ideal-rs <plan> --holds N`: for EVERY ideal input cell of a plan, what a sculk sensor can
  give it instead (10/30 gt hold, 3 taps 10 apart = observer on a calibrated sensor, 2 taps 14
  apart = sensor -> lamp -> observer, one pulse), every start 0..=N (or none) on each edge
  independently, other inputs as planned; prints the working starts per edge and those that work
  with the same tick on both edges (one sensor hearing the lever). ~1 s per plan. Reproduces p2's
  K (hold10 open 8..13 / close 8,9,14,15) and e88's shuttle (hold10 >= 24).

## Screens with --holds (10:15Z-10:35Z, for the coordinator)
- f131 pn31 (7x9, only P3 (-1,3) ideal): P3 = one pulse at plan 6..7 (both edges) or 2 taps 14
  apart from 6..7; holds and 3 taps fail. Unwirable as laid out: P3's only feed is the conductor
  (-2,3) powered from (-2,4); the column above is (-2,5) and the lever's support (-2,6) (a full
  block), so sensor -> lamp -> observer does not fit and every other stage is 2 gt off (the real
  chain fires P3 at t6 = plan 5). 2520 column variants: 0.
- s79 c78z (7x8, U's 1 gt first pulse + U2 + R0 ideal): R0 = any pulse or 3 calibrated taps (any
  start); U2 = 3 calibrated taps only, open 7..45 but close 9..10 exactly; plain taps, holds: 0. A
  sensor at (3,-2) fits the 4 free bottom-right cells with three observers but hears R1/R2 at t1
  (activation t4) and has no room for wool.
- ceil's 9 ceiling-held mechanisms (`ceil/holds/c*.txt`): 8-10 inputs each; side/bottom pullers
  take 3 taps at any start, the 8x7 family's two-pulse left input (-3,0) is one 10 gt hold at
  13..16, (-1,1) a hold at 5..13; (-2,-1) only a pulse 3..9 or 2 taps 4..8.
