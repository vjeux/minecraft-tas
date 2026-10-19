# cosearch partials: doors wired except for a few inputs still driven by their ideal plan

Each file is a candidate from `door cosearch search --assist <plan>`: the door works in redsim's
screen (one position, 2 cycles; points shown) while the listed plan inputs stay plan-driven. They
are NOT doors on their own. Continue one with the same plan and only those inputs assisted, e.g.

    mctas door cosearch search designs/piston-door-3x3/tracks/cosearch/partials/w63-2as.txt \
      --assist designs/piston-door-3x3/tracks/cosearch/seeds/w63.ideal "--assist-cells=-2,2;4,2" \
      --reference-ideal designs/piston-door-3x3/tracks/cosearch/seeds/w63.ideal --lock-pistons \
      --lever-moves --box=-2,-2:4,6 --focus=-2,2:4,6 --threads 24 --secs 1800 --confirm

| file | mechanism, box | plan (seeds/) | screen | inputs still assisted (door coords) |
|---|---|---|---|---|
| w63-2as.txt | 7x8-014 (wire64b w63), 7x9 = 63, x -2..4 y -2..6 | w63.ideal | 77/79 | (-2,2) left stack, (4,2) right stack (each 3 pulses 6-7 gt apart) |
| fm-7x8-000-2as.txt | pistmodel fm-7x8-000, 7x8, x -2..4 y -2..5 | fm-7x8-000-groups.ideal | 77/79 | (-2,0), (-2,2) (left members of the puller group) |
| fm-7x8-000-leftlever-3as.txt | same, lever pinned left | fm-7x8-000-groups.ideal | 77/79 | (-2,0), (2,4), (4,1) |
| vm1-5as.txt | vmid vm1, 7x8, x -2..4 y -2..5 | vm1b.ideal | 77/79 | (-2,2), (4,2), (1,4), (0,5), (1,5) |
| vc1-6as.txt | vmid vc1, 7x8, x -2..4 y -3..4 | vc1.ideal | 77/79 | (1,4), (4,2), (-2,3), (1,-2), (3,-1), (1,-3) |
| t1-2as.txt | wire56 t1 (7x8-028), 7x8 | t1.ideal | 75/79 | (1,5) P5, (4,2) PTR |
| e89b-8x8-77of79.txt | e89b without row -4, 8x8 = 64 (no plan: e89b's schedule as shaping) | - | 77/79 | (no assist) the center stays: the elevator's lower piston needs an 8 gt window |
