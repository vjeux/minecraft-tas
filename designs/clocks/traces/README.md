# Clock traces (recorded in the game)

Real-game recordings of the clocks whose motion redsim does not model (design entities: boats, minecarts; fluids), for
the in-browser viewer (webrender). One JSON Lines file per archived clock, `<design stem>.trace`, written by
`mctas clock misc trace-export designs/clocks/archive/<name>.txt` (track clk-misc; format agreed with webrender):

- line 1, the header: `{"trace":1,"name":..,"design":"designs/clocks/archive/<file>","period":P,"warmup":W,"ticks":N,
  "box":[[x0,y0,z0],[x1,y1,z1]],"loop":[W,W+P],"output":[x,y,z]}` (`output` only when the design has one); ticks =
  warmup + 2 periods, from the judge's verdict row;
- then one line per game tick t = 0..=ticks: `{"t":t,"blocks":[[x,y,z,"state"],..],"entities":[{"id":0,
  "type":"minecraft:oak_boat","pos":[x,y,z],"yaw":a,"pitch":b}]}`: at t = 0 every non-air cell of the watched box (the
  design plus a 3-cell margin), later only the cells that changed (air included); every entity every tick with a stable
  id; positions rounded to 4 decimals, yaw / pitch to 2.

Coordinates are the design's own: the build is the judge's first one (rotation 0, position 0), placed and kicked exactly
like `clock verify`. Re-export after re-archiving a clock.
