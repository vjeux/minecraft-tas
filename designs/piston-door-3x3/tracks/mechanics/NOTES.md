# Track mechanics: gadgets from the 26.3 source, applied to the door

## Gadget library (designs/gadgets/, each proven by a drive script)

`mctas door drive --quiet designs/gadgets/*.drive` runs them all (PASS/FAIL); the table and the
source facts behind them are in docs/mechanics.md. Ten gadgets: dual-edge, pulse-stretch, spit,
qc-bud, lamp-fall-delay, bulb-toggle, slime-nonstick, silent-hold (rb5's trick), pulse-bridge,
lever-tap.

Tools added: `door drive` probes/trace/expect assertions and a multi-script PASS/FAIL runner;
`door patch` (a design variant as a short edit script: set/clear/port). Every design below is a
patch file in this folder applied to its parent.

## Results

| blocks | category | file | parent + patch | idea |
|---|---|---|---|---|
| 91 | solid | mech-91.txt | v2-min 115 + ta.patch | A torch next to the observer column (lever on its attachment, A late = 1 repeater) |
| 90 | solid | mech-90.txt | 91 + `door minimize` | stray floor |
| 89 | solid | mech-89.txt | 90 + ta2.patch | A torch right next to X, observer pair |
| 53 | solid | mech-53.txt | rb2 57 + rb2-hold.patch | late hold as a short U, glazed terracotta floors beside the slime |
| 51 | solid | mech-51.txt | 53 + rb2-bchain.patch | pulse bridge (block, cross dust, block) from O2 to R_B |
| 50 | solid | mech-50.txt | rb5 52 + rb2-bchain.patch | same bridge on rb5's silent hold |
| **48** | solid | mech-48.txt | 50 + rb5-hold.patch | silent-hold block on the floor of the lever's repeater |
| **47** | solid | mech-47.txt = BEST.txt | 48 + bm-obs.patch | top-control's trick: P_BM pulse-driven by an observer on the bottom-right door block (no redstone block, backbone 2 shorter) |
| **42** | sculk | sculk-42.txt = BEST-sculk.txt | search-tool's sculk 43 + bm-obs.patch | same P_BM trick on the sculk-hold design (which already used the pulse bridge) |
| 104 -> 99 | solid, slime-free | sf-104.txt, sf-99.txt = BEST-slime-free.txt | mech-90 + sf2.patch, then `door minimize` | side columns on individual pullers |

### 91 / 89: the A torch (v2 family)

v2's A torch sat on the far left, fed by a 16-block climb from the sides line (A early) and a
12-block chain from the P_T net (A late). In mech-91 the lever sits on top of the A torch's
attachment K (A early for free), O2 watches the lever itself, the torch T_A feeds X (through a
dust X2), A late is ONE repeater from the P_T net into K (+4gt after P_T), and no dust touches K
(a strongly powered K would back-feed the lever net and latch the late chain; the lever net is
fed by a repeater reading the lever: gadget lever-tap). mech-89 puts T_A right next to X.

### 53 -> 48: redstone-block's rb2/rb5 control

* rb2's 24gt hold for Q (14 blocks) re-laid as a U west of the lever (10): the first repeater
  reads the lever itself; the two floors beside the left slime backbone are glazed terracotta.
* O2's pulse to R_B (6 blocks of dust) through a block strongly powered by O2's back, an
  isolated cross dust, and a block R_B reads (4): gadget pulse-bridge.
* On rb5 (silent hold): the hold block H moved onto the floor of the lever's repeater (0,5,-2),
  fed by lever -> repeater -> block -> dust -> repeater (9 -> 7).

### 104: slime-free

mech-90 with the slime backbones and P_L/P_R replaced by pullers: the bottom row is pulled down
by three pullers (torches under them on the sides line's floors; the line repeater moved to
(3,-2,1)), (0,1)/(0,2) by two pullers on ONE wall torch (direct + quasi-connectivity), same on
the right. The right top puller's quasi cell (4,3,0) must not touch a powered block: mech-89's K
(4,4,0) quasi-powers it (so mech-90 is the base), and X2's floor is glass (not a conductor).

## What did not work

* rb with pulse-driven A (logic-min style, no Q / RB_A / hold) keeps the T-drag working with
  ideal inputs (`door ideal tracks/mechanics/rb-pulsed-ideal.sched`), but only if P_R turns off
  EXACTLY when B's window ends (pr_off = b+bw; 17 and 19 fail, like logic-min's P_T), so P_R
  must share R_B's output with B while also getting a lever-delayed hold that B must not see.
  Counted wiring (p0 path into A, L_8 into P_R, R_B window into P_R) lands at 47: no gain.
* rb1 with an observer chain instead of the lever->O2 dust route (5 observers): +10gt on the B
  window, the close dance then collides with P_R's return (needs a longer hold).
* Removing the B pulse stretch or shortening the 16gt silent hold: the hold must outlast the
  2nd-order updates X sends through Q's head at the first A pulse (L+12..14) and end before A
  lands back (L+18).

## Next ideas

* The silent hold (7 blocks) exists only because the lever's off-update and X's 2nd-order updates
  (through Q's head, L+12..14) reach Q. If Q were quasi-powered by the lever's block from two
  cells away (silent at the off edge) and P_L were not Q's neighbor, R_B's own window (10..18)
  could cover the X updates through a silent path: no delay line at all (needs a P_L
  relocation; not tried).
* B chain (9 in the rb family): R_B, O and X are pinned by A's neighbors; only O2's 4-block
  bridge is negotiable and needs the lever (pinned by X_L) closer to R_B's back.
* Slime-free: logic-min's pulse-driven side columns (53) beat individual torches (104 here) by
  50 blocks; the remaining cost there is P_T's asymmetric net.

## Hold removal study (round 2, mech-47): measured, then shown not to pay

* Only the full 16gt hold passes: all 15 shorter combinations of the two repeater delays fail
  (`door patch` sweep of (0,6,-2) and (0,5,-3) delays 1..4).
* Q's updates during the close (drive trace): t=0 the lever (Q sits under the lever block X_L),
  t=10 P_L released by B's push on A, t=12 and t=14 X's second-order updates (X -> B's head at
  (1,4,0) -> Q's head at (1,4,-1) -> Q), t=20 A lands back next to Q's head = the release.
* R_B is on t=10..17, so one extra repeater R_Q at (0,5,1) reading the bridge floor (0,5,2),
  pointing into a block F=(0,5,0), would cover t=10..14 silently (F touches Q's quasi cell
  (0,5,-1) but not Q; F also powers B in the same window and holds P_L until t=18): 3 blocks.
* What breaks it is t=0. Every source that gives Q its open update (the lever, O2, anything the
  lever moves) fires at close t=0..4 too, so Q needs a delayed copy of the lever there. A silent
  delayed copy is a repeater whose front H touches Q's quasi cell (0,5,-1) but not Q
  (H in {(-1,5,-1), (0,6,-1), (0,5,-2)}; (1,5,-1) and (0,5,0) touch B) and whose input is the lever
  or its block. That input is always 2 cells from H in a straight line, i.e. 4 away from Q, while
  the lever must be within 2 of Q to update it: no single-repeater geometry exists. With a relay
  dust the short hold costs 4-5 blocks, plus 3 for R_Q: 7-8 >= the current 7.
* Moving power from a structure (a redstone block riding the left structure, "left open" =
  open t=2 .. close t=10) cannot reach Q: the left structure moves away from Q.

## Fast solid (speed boards)

* **fast-60** (`fast-60.txt` = `BEST-fast.txt`, open 16 / close 17, 60 blocks): redstone-block's
  rb9 (61, the fastest solid) + `bm-obs.patch`. fast-60 is exactly mech-47 + rb9's 13-block
  second-pulse route (an observer on the lever, 5 dust + 5 floors, a delay-2 repeater into R_B's
  input), which splits B's window into t4..6 + t8..12.
* Tried: a repeater R2 on top of B (in B's quasi cell) reading R_B, delay 2, pointing into a
  block next to that cell (`rb9-r2.patch`, 50 blocks): B does get its second window t8..12
  for 2 blocks, but B is not updated when R2 turns off (R2's off only updates its front block's
  neighbors) and O (watching R_B) never sees the t12 edge, so neither B's retraction at t12 nor
  A's second pulse at t14 happen: the door stays half open.
