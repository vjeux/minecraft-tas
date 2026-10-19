# satwire fixtures (known answers for `door selftest`)

* `bud48-pk3-s0.txt`: a 48-block re-wiring candidate of pockets' 7x9 mechanism
  (`archive/pockets-63b-50-0-50b.txt`) found by the first-shift 48b hunt with `--sound` (no update
  order). The game rejects it: the right pullers hang under a ceiling lever on a note block that a
  dust on top also powers; on the lever flip the upper puller reads its power while the dust still
  powers the note block (QC through it), the dust then turns off without updating the piston: a BUD,
  the R pair stays extended. With `--update-order` the model must REJECT it on the target recording:

      mctas door satwire designs/piston-door-3x3/archive/pockets-63b-50-0-50b.txt --from-wired \
        --full-frame --forbid-ring --sound --lever-oop --log-doors --one-warp --dmax 0 --skip 0 \
        --palette block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,glass,leaves,upiston \
        --explain designs/piston-door-3x3/tracks/satwire/fixtures/bud48-pk3-s0.txt --update-order

  expected line: `explain: the model REJECTS the candidate on the target recording; core: open
  piston (4,1); update-order clauses: open t1 piston (4,1) retraction` (~40 s). Without
  `--update-order` the model accepts it (no REJECTS line: it prints where the game's run differs).
