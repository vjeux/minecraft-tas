# Track riders: blocks riding moving structures as timing elements

Direction: beat the rb solid record (47) by giving P_R / Q their asymmetric timing with "riders"
(blocks carried by a moving structure) instead of the 7-block silent hold. Then (coordinator,
08:15Z) re-target the ideas at fresh-c's fc2 (40).

## Results (all `door verify --extra-offsets 28`, 32 positions x 3 cycles)
| file | blocks | open/close | idea |
|---|---|---|---|
| r2.txt = BEST.txt | 46 | 22 / 23 | rb6-bmobs-47 without the hold: Q held by a redstone block riding the LEFT structure; P_L held by the B window |

## r2 (46): the left structure carries Q's hold
* rb6's Q (shuttle of RB_A) needs power from open t1 until the close dance ends (t18-20); rb6
  pays 7 blocks for a 16 gt silent hold.
* Rider: slimes (0,3,0) and (0,3,-1) glue a redstone block RID (1,3,-1) to the left backbone.
  When the left structure opens (t1-3), RID lands at (0,3,-1), right under Q: Q stays powered
  while the left door column is open, whatever updates it gets (the lever's at close t1, the
  B-push and X updates through its head at t10-14).
* For that the left structure must come back only AFTER the close dance: P_L (QC from the lever
  block, released by A's motion) is also powered by the B window: a second B repeater R_B2
  (0,5,1) (d4) reads the cross dust's floor (0,5,2) and points into a block K (0,5,0) right above
  P_L; K also powers B (harmless: same window as R_B). Glass floor (0,4,1) because X points into it.
* Close: t10 B pushes A (P_L updated, K powered: stays); t18 R_B/R_B2 fall, B lifts A, P_L
  retracts, RID leaves Q, Q retracts, RB_A leaves P_R, P_R pulls T into (1,3,0) (block events in
  that order, all at t18); A, T, RB_A land at t20; A pushes T down. Closed at 23 (rb6: 25).
* Failed first try (r1.txt, broken): with P_L released at t10 (rb6's timing), RID comes back to
  (1,3,-1) during the close dance and powers A at (1,3,0) (A stays extended, B cannot lift it).
  Ideal check that led to r2: a redstone block at (0,5,1) from close t9 to t17 (`door drive`).
