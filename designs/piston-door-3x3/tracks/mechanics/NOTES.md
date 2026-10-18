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
| 104 | solid, slime-free | sf-104.txt = BEST-slime-free.txt | mech-90 + sf2.patch | side columns on individual pullers |

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
