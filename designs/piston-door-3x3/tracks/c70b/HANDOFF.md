# HANDOFF: track c70b, 2026-10-19 (three tasks, newest first)

## 3. Generator: extended-at-rest pullers by default (coordinator, after the community 7x8 audit)
- families / mechspace / fullmech: pullers EXTENDED at rest are allowed for 1-move exits by default
  (`--no-ext-pullers` = the old catalog), bounded: `--ext-max 3` exits by such pullers per mechanism,
  each within `--ext-dist 1` of an earlier one (`--ext-apart` drops it). The generation runs the 4
  center exit directions on 4 threads. Merged to main up to e426d787a; 40cf01ba4, abf205372,
  d26db2ae4, 6254dcf3e sent for merge. Costs and the archive recount: docs/audit.md section 2.
- door audit: g1 = --no-ext-pullers, g2..g5 the defaults, g6 every flag incl. --ext-max 12
  --ext-apart. Selftest audit:generator-membership expects c79 out,IN,IN,IN,IN,IN.
- Open: the default bound leaves out the u7-m10 family (m79s, s88i, cosearch-63b-50: exits 3 apart;
  IN with --ext-dist 3, +52% candidates). Bounds (a) / (b) of docs/audit.md section 6 still open.

## 2. The community 7x8 (SacredRedstone) from vjeux's screenshot (community/NOTES.md)
- `community-7x8.txt`: 48 blocks, solid, FULL, rotation-safe, open 42 / close 38, verify
  --extra-offsets 28 WORKS. Every block read from the screenshot except (3,5) (hidden dust: lever ->
  comparator) and (3,3) (a delay-4 repeater from c77's door fill). The screenshot shows (3,3) EMPTY;
  every invisible option there (dust, rails) closes but cannot reopen in redsim and the JVM (the lever
  block feeds both the elevator top and TR's puller; only 2 re-pulses leave TR up, dust gives 1).
  The real wiring at (3,3) is the open question (a version difference or a misread not found).
- Mechanism (wiggle door: TL/ML/BL/TR move 3 times, TM 5 on open), tricks and tool audit:
  community/NOTES.md + community/audit.md. c77 shrinks it (47, record-eligible).
- Tool fixes from it: `door cycle` checks every flip + `--vary` (merged); cosearch quartz_like
  (pockets3's same fix is on main).

## 1. Parked: t7b in 8 rows (partials/t7b8.md) and c070x (partials/c070x.md)
- t7b8 (NOTES.md 6-9, files t7b/): a hybrid B works with ideal inputs (east lift pulse + a redstone
  block shuttle the gate watches); a real right-side lift chain exists (t8t.txt); P_M (pm = 12
  exactly) and the shuttle's two pulses are missing on the left. pockets3 runs the left-cell fills
  (runs/t8t-f1 on explore/pockets, 19:40Z).
- c070x: negative result (NOTES.md 1-5): U needs a one-shot at exactly t11 and its only one-shot takes
  BL's only feed; every screen 0.

## Running processes
None of mine.
