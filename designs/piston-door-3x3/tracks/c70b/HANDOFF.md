# HANDOFF: track c70b (c070x, 8x7 = 56), 2026-10-19

## State
- No working door. c70's m2 (explore/c70 tracks/c70/wip/m2.txt, only BL ideal) is still the best partial.
- `wip/m2x.txt` + `ideal/m2x.ideal`: m2 with U's chain removed and U driven by an off-plane test observer X
  (plan toggles (-1,-1,2) at gu/guc). Windows: open gu = 9 EXACTLY (U at t11), close guc 6..32 (gaps 18, 20,
  21, 30); BL (0,-2) open 4..40 (not 31), close 4..~40. BL is wide once the bottom-left sensor loop is gone.
- `wip/m2w.txt`: m2 with WOOL door blocks + frame (the door occludes the lever for the whole bottom-left).
- `wip/w1b.txt` (+ ideal/w1b.ideal, no ideal input): wool; plain sensor (-2,-2) hears E1 at t8 -> normal piston
  (-3,-2) up -> its head into (-3,-1) at t9 -> observer (-2,-1) facing west fires U at t11; observer (-1,-2)
  facing west on the sensor fires BL at t10. Open works to t39; then the sensor's cooldown/inactive taps move BL
  twice more and it re-arms on BL's puller (d=2): endless loop; on close the piston's retraction gives a second
  U pulse after E2 (fatal, see NOTES 1).

## Why it is stuck (NOTES.md 1)
- M is lever-locked at 4/14/24 -> U must be exactly 11 (or 21).
- BL's puller is fed only from (-1,-2); U only from (-2,-1) or (-1,-2). The only ONE-SHOT source for U in the
  bottom-left is c70's moving observer landing at (-1,-2) (facing down). Every sensor-based source changes 2-3
  times and an extra U pulse on close after E2 makes the watcher fire D once (E pushed alone into (-1,1), spit).
- So U (one-shot, needs (-1,-2)) and BL (needs a static east-pointing block at (-1,-2)) cannot both be wired in
  the 8x7 bottom-left with this mechanism, unless a new trick gives a one-shot from (-2,-1).

## Searches (all 0)
- mine: m2 KEEP k<=3 over 10 cells (4.6M); m2w k<=5 on the 5 bottom-left cells (147M, 42 states).
- pockets3 (runs on explore/pockets): m2 cross-region KEEP k<=3 over all 23 wiring cells (47.1M): 0;
  m2w same and m2 bottom-left k<=5 with 50 states (255M): running at 16:10Z.

## Next ideas (not tried)
- A one-shot at (-3,-1) for the (-2,-1) observer: a normal piston pushing a block INTO (-3,-1) (the block stays)
  has no room (the push line comes from (-3,-3) or D), a pulled block needs (-3,0) = D's feed.
- D driven from the top-left as a HOLD (instead of the blocked-push watcher) widens M's window and frees (-2,0)/(-3,0),
  but the hold sensor hears E1 first, and on close E2 must be swallowed by the hold (q > 37) while on open E2
  must come after it.
- c077w: same exact 3-tick coupling; its top-left one-shot for S (moving observer (-2,4) -> (-1,4) pushed by a
  piston at (-3,4)) can only be fed from (-3,3), where any sensor powers D's feed (-3,2) (strongly, or by QC
  through an air cell).

## Running processes
None of mine.
