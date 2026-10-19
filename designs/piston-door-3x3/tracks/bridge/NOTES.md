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

- c2e's m88c (8x8): A's hold must start 7..9 and last 3..5 gt (extend 8..10, retract 11..13);
  a sensor gives 10 (calibrated) or 30 gt windows, an observer on it 2 gt taps (a 2 gt pulse
  makes A spit). On close A sits at (1,3) from t18 and any update + power there breaks the close
  (ideal close pulse at (1,3) ok only if it ends by t17), so A's last pull must stay c2e's
  side-powered stone (0,4), which leaves (0,3) (fed only from an observer at (-1,3), 2 gt) for the
  hold. 117,600 top-left variants and 33,600 top-right + top-left variants (sensors, observers,
  repeaters, dust, wool, note blocks, lamps; the TR-at-(2,3) trick with every (3,5)/(2,5)/(3,3)
  delay): 0 work.
- t8xL-b (8x8): B's two pulses must be 12 apart (16 for b1 15..16), tied to the left side's parity
  chain; A at a1 4..6, a2 20..27, a3 34..42. A calibrated sensor's taps are 10 apart: no (a1, b1)
  pair fits A at a, a+10, a+20 (swept). B as ONE hold (push down + pull up) works only for 16 gt.
  U (1,-2): any 2 gt pulse on open, >= 36 on close (holds fail on close: U must spit).
