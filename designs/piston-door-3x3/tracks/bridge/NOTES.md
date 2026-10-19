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

## The other partials (why the bridge does not fit, all measured with ideal-rs)

Every count below is an exhaustive `door ideal-rs --vary` screen (only the lever real unless noted;
sensors = calibrated facing W/E/N + plain; ~10-20k combinations/s on 20 threads).

- c2e's m88c (8x8): A's hold must start 7..9 and last 3..5 gt (extend 8..10, retract 11..13);
  a sensor gives 10 (calibrated) or 30 gt windows, an observer on it 2 gt taps (a 2 gt pulse
  makes A spit). On close A sits at (1,3) from t18 and any update + power there breaks the close
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
  shuttle ideal; agent e88 active), m56 p1 (7x8, A's hold + L0; the TR relay is 1 gt short), s79
  c78m (BR's puller only, but U/K/L0 unwired), m79s s88f (K, L0, L1/L2; m79s finished an 8x8 there).

## Tools (this track)

- `door ideal-rs --vary x,y[,z]=state|state|keep|air` (repeatable: every combination, times the
  `--sweep` grid) and `--ok-only`: block variants applied in memory to the plan's design before each
  run (redsim, no JVM): the exhaustive screen that found p2-sculk (151,200 variants in 7 s).
- `door ideal-rs --drop N`: also every subset of up to N removable blocks (not the door cells, the
  lever or heads) taken out: a redsim block minimizer (p2-sculk-r: only its static frame stones and
  the orientation-only obsidian can go).
- Pitfalls: `--vary=-1,4=...` (with `=`) for negative x; `door patch` starts a JVM (~40 s), use
  `--vary` instead.
