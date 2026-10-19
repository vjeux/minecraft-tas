searched in 868.8 s
| key | blocks | note | inputs | output |
|---|---|---|---|---|
| AND | 5 | function and, latency 4 gt | [[0, 1, 0], [1, 1, 0]] | [0, 1, 1] |
| AND fastest | 5 | function and, latency 4 gt | [[0, 1, 0], [1, 1, 0]] | [0, 1, 1] |
| IMPLY (a -> b) | 3 | function imply, latency 2 gt, glitches 2 | [[0, 1, 0], [1, 1, 0]] | [1, 0, 0] |
| IMPLY (a -> b) fastest | 4 | function imply, latency 0 gt, class instant | [[0, 1, 1], [2, 1, 1]] | [2, 1, 2] |
| IMPLY (b -> a) | 3 | function imply-ba, latency 0 gt, class instant | [[0, 1, 0], [1, 1, 0]] | [0, 1, 1] |
| IMPLY (b -> a) fastest | 3 | function imply-ba, latency 0 gt, class instant | [[0, 1, 0], [1, 1, 0]] | [0, 1, 1] |
| NIMPLY (a & !b) | 2 | function nimply, latency 0 gt, class instant | [[0, 1, 1], [1, 1, 0]] | [0, 1, -1] |
| NIMPLY (a & !b) fastest | 2 | function nimply, latency 0 gt, class instant | [[0, 1, 1], [1, 1, 0]] | [0, 1, -1] |
| NIMPLY (b & !a) | 2 | function nimply-ba, latency 0 gt, class instant | [[0, 1, 1], [1, 1, 0]] | [-1, 1, 0] |
| NIMPLY (b & !a) fastest | 2 | function nimply-ba, latency 0 gt, class instant | [[0, 1, 1], [1, 1, 0]] | [-1, 1, 0] |
| NOR | 3 | function nor, latency 0 gt, class instant | [[0, 1, 0], [2, 1, 0]] | [1, 1, -1] |
| NOR fastest | 3 | function nor, latency 0 gt, class instant | [[0, 1, 0], [2, 1, 0]] | [1, 1, -1] |
| OR | 2 | function or, latency 0 gt, class instant | [[0, 1, 0], [2, 1, 0]] | [1, -1, 0] |
| OR fastest | 2 | function or, latency 0 gt, class instant | [[0, 1, 0], [2, 1, 0]] | [1, -1, 0] |
| XOR | 4 | function xor, latency 0 gt, class instant | [[0, 1, 1], [1, 1, 0]] | [2, 1, 2] |
| XOR fastest | 4 | function xor, latency 0 gt, class instant | [[0, 1, 1], [1, 1, 0]] | [2, 1, 2] |
