# fresh-a: a 3x3 door from first principles, minimum block count

## How each door block can leave the opening
* The walkway is 3 deep on both sides, so z moves are useless: every block leaves in the
  door plane. Edge blocks need 1 cell outward; the four middle-edge blocks need 2 cells if
  they go sideways; the center needs 2 cells whatever it does, and its exit cell (the cell
  just outside a middle-edge block) must be vacated by that middle-edge block.
* The center can only be touched through its 4 in-plane neighbours (door cells) or the
  walkway: a push needs the pusher to end inside the opening, so it must be PULLED twice by
  one sticky piston that moves one cell between the pulls (A riding B), or by two pistons.
* A middle-edge block can vacate the center's exit cell by being captured by a side group:
  after it is pulled out (BM down to (1,-1)), a slime of the right group sits next to it and
  drags it away sideways when the group moves. That replaces a dedicated escape piston (P_T
  in v2) with 2 slime and puts the timing on the group piston.

## Design "C-down" (first build)
```
          T            T  sticky, facing down, extended: lifts TM 1 (NOT lever)
   s  PL  h  PR s      PL/PR sticky pushers (lever ON = extend): push the side groups out
   s  #  #  #  s
   s  #  C  #  s
   s  #  #  #  s
         a  s  s       a = A's head; s at (2,-1) captures BM for the right group
         A             A  sticky, facing up, extended: pulls BM down, then pulls C down twice
         B             B  sticky, facing up: lifts A between the two pulls
```
Pushers instead of pullers: the side pistons are powered by the lever itself (no inverter).

## Block budget (before building)
| part | blocks |
|---|---|
| door | 9 |
| left group slime (-1,0..3) | 4 |
| right group slime (3,-1..3) + capture (2,-1) | 6 |
| pistons PL PR T A B | 5 |
| mechanism | 24 |
| lever + control (torch for T, NOR for A, late signal for PR, B pulse + A pulse observer) | ~25 |
| target | ~50 |

## Log
* 22:31 C-down mechanism validated with ideal redstone-block inputs (wip/cdown-drive2.txt):
  opens and closes. Lesson: a QC-only source next to A does not update A; A's real source
  must be direct or a dust (second-order updates).
* Key idea: PULSE-DRIVEN pistons. A 2gt observer pulse that ends in the scheduled-tick phase
  makes a sticky piston SPIT what it pushes (type-2 retract), while the same pulse makes it
  PULL when its head extends into air. So every piston that is retracted at rest becomes a
  toggle: open pulse = push out & leave (PL, PR) / reach & pull (T, A); close pulse = reach &
  pull back (PL, PR) / push & leave (T, A). No steady power, no inverters, no late-off lines.
  Open = a, p, B^, a, Bv, a ; close = a, p, B^, Bv (B lifts A+BM+C together into place).
* 22:45 Pulse-toggle trains break on ordering (open needs A,PR,dance; close needs dance,PR,A;
  a single fixed train cannot serve both). Back to sustained logic, but two tricks to cut it:
  - C-up geometry (A above, B above A) so A's sources never quasi-connect B.
  - "K trick": A's rest power reaches it through a conductor K glued to the capture group
    (K = (0,4) on the left group's slime (0,3)), fed by a repeater. A = NOT(L) AND group-home:
    off early at open (torch), on late at close (group back) with no late line for A.
  - BM puller Q powered by a redstone block carried by the left group (no wire to the bottom).
* 23:05 sculk sensor gadget (designs/gadgets/sculk-hold.*): sensor ON TOP of the lever's block
  holds it 30gt after any flip (strong power goes to the block BELOW the sensor). Sculk doors are
  ranked separately, so parked. rb2+sculk (wip/rb2-sculk.txt, 45 blocks) opens but fails to
  close: the lever-off update reaches P_L through its head; not pursued.
* 23:20 PIVOT: "solid, no slime or honey" leaderboard is empty. Fresh slime-free design SF1:
  6 side pullers (sticky, extended at rest), Q pulls BM, A/B dance lifts C, P_T pulls TM west
  at y=3. Validate with redstone-block placeholders first.
* 23:30 SF1 slime-free with placeholders works end to end (wip/sf1-mech.drive). Stage w1:
  NOT-L torches T_N/T_N2 on the lever's floor + router dust to the side pullers (dust on a K
  block: dust points into the top puller, K powers the middle one directly and the bottom one by
  QC) and to Q: verified rest state + cycle with only P_T and A placeholders left.
* 23:48 SF1 fully wired and VERIFIED: 126 blocks, slime-free (BEST-slime-free.txt,
  archive/fresh-a-sf1-w2-126b.txt, builder `fresh-a-sf1-w2` in designs_fresh_a.rs).
  Control: T_N/T_N2 (NOT L) on the lever floor feed the side pullers and Q; P_T's torch T_P and
  A's torch T_A sit north, fed by a fast-L route over the top, R6 (L_2), net M = OR(L_4, L_26)
  -> R_pin -> Y_P; A late = M -> R_al (8gt) -> dust into Y_A.
  Pitfalls found: (1) a dust on top of a block that a repeater strongly powers reads that power
  back: with the late repeater pointing into Y_A, the fast-L dust on Y_A latched the whole chain
  on (fixed: late input as dust = weak power); (2) router routes computed one after another do
  not see each other (a later step-down was cut by an earlier floor) and ignore signal decay;
  (3) `b.dust` twice in a column overwrites the upper dust's floor (chain dust vanished).
  Not competitive: pulse-driven side pistons (logic-min, 53-55) beat steady pullers + torches.
