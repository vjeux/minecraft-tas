# Track w63top: finish the w63 top (7x8-014 in a 7x9 box, rows -2..6, bottom pockets real)

## Results
- ideal/s1.ideal (pure ideal) and ideal/t1.ideal on w63t-1.txt (A driver chain + right-stack
  observer REAL, (4,4) toggled ideally): **WORK at 32 positions x 2 cycles** (open/close 77).

## Key idea: share (4,4)
- Forced cells (by hand, cell by cell): A's only clean driver at both A positions is (2,4) =
  observer facing EAST (fires A at (1,4) directly, at (1,3) by QC through (1,4) WITH updates at
  both edges; a conductor at (2,4) QC-arms A at (1,3) without updating it: A2 never retracts
  cleanly). (2,4) watches (3,4); (3,4) cannot be powered by anything (any signal into (3,4)
  QC-fires P_T and updates it), so (3,4) = observer facing EAST watching (4,4). The right stack
  (TR puller direct + MR puller by QC) is fed only by (4,3) = observer facing UP, which also
  watches (4,4). So every A pulse comes 2 gt after a right-stack pulse.
- Fix: A1 single, A2 a TRIPLE (pull, push, pull: odd count = one net move, 6 gt apart), A3
  single -> the right stack gets runs (1)(3)(1) = 5 pulses; its bottom watcher sees 2+5+2 = 9
  (odd): BR still nets one toggle. Busy-window counts: a run of k stack pulses 6 gt apart gives
  2,3,5,6 watcher pulses for k = 1..4.
- t1 windows (A1 = 0, even values usable with observer timing): P_T 2..7, B1 9..11, A2 triple
  start 13..18, B2 28..33, A3 37..49, U any, left triple start >= 33.

## Open problem (where it stands)
- P_T's only drivers: (3,5) observer facing UP (QC through (3,4)), (3,4) facing up (ties A to
  P_T+2: fails), (4,3) (taken by the right stack / lever only), a conductor at (4,3) (fires the
  right stack with P_T). With (3,5) = P_T's observer, the right column (4,4)<-(4,5)<-(4,6) has
  only the lever and P_T-coupled cells ((3,6), (3,5)) as inputs, so every A pulse after A1 needs
  a P_T pulse 8 gt earlier: fine for A2a (P_T 6 -> A 14), the question is A2b/A2c/A3.
  P_T extra pulses are harmless before A1 (open: pulls from an empty (1,3); close: blocked if
  TL is at (0,3) and (-1,3) is immovable) and on close after B2+4 (pull from an empty (1,3)).
