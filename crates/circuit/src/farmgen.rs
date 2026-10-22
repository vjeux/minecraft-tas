//! Full-size farm generators (coordinator 2026-10-21, vjeux: "you really need to make the farms
//! bigger"): `mctas circuit farm-gen` writes a whole farm as one design, judged like any harvester.

use std::collections::BTreeMap;
use std::fmt::Write;

/// THE OBVIOUS stem farm (the bar, no search): a double row of stems on wet farmland, two blocks
/// apart along the row (stems side by side along a row would grow fruit on each other's farmland),
/// each stem with one fruit cell (on mud) on its outer side; waterlogged grates in the row every 8
/// blocks keep the farmland wet. An observer above each stem fires when the stem attaches to a
/// fruit; the dust on it QC-powers the piston above the fruit cell, which pushes down and breaks the
/// fruit. A hopper minecart under each fruit cell's mud takes the squeezed drop, and a hopper line
/// under the carts carries it to a chest at the row's end. `n` stems a row (2n in all).
pub fn stem_rows(crop: &str, n: i32) -> String {
    let item = if crop.ends_with("melon") { "melon" } else { "pumpkin" };
    let name = format!("{item}-rows{}", 2 * n);
    let len = 2 * n - 1;
    let stem = |x: i32| x.rem_euclid(2) == 0;
    let grate = |x: i32| x.rem_euclid(8) == 3 || (x == len - 2 && (len - 2).rem_euclid(8) > 3 + 4);
    let mut s = String::new();
    let _ = writeln!(s, "name: {name}");
    let _ = writeln!(s, "# The obvious {item} farm: a double row of {} stems on wet farmland, two blocks apart along the", 2 * n);
    let _ = writeln!(s, "# row, each with one fruit cell (on mud) on its outer side; waterlogged grates every 8 blocks keep");
    let _ = writeln!(s, "# the farmland wet. An observer above each stem fires when it attaches to a {item}; the dust on it");
    let _ = writeln!(s, "# powers the piston above the fruit cell, which pushes down and breaks the {item}. A hopper minecart");
    let _ = writeln!(s, "# under each fruit cell's mud takes the squeezed drop; a hopper line under the carts carries it to");
    let _ = writeln!(s, "# a chest at the row's end. Written by `circuit farm-gen --layout rows`.");
    let _ = writeln!(s, "circuit: harvester {crop}");
    for x in (0..len).filter(|x| stem(*x)) {
        let _ = writeln!(s, "port crop.a{x}: {x} 1 1");
        let _ = writeln!(s, "port crop.b{x}: {x} 1 2");
    }
    let _ = writeln!(s, "port output.a: {} -3 0", len - 1);
    let _ = writeln!(s, "port output.b: {} -3 3", len - 1);
    let _ = writeln!(s, "origin: 0 -2 0");
    let _ = writeln!(s, "palette:");
    let _ = writeln!(s, "  F minecraft:farmland[moisture=7]");
    let _ = writeln!(s, "  M minecraft:mud");
    let _ = writeln!(s, "  G minecraft:waxed_copper_grate[waterlogged=true]");
    let _ = writeln!(s, "  O minecraft:observer[facing=down,powered=false]");
    let _ = writeln!(s, "  P minecraft:piston[extended=false,facing=down]");
    let _ = writeln!(s, "  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]");
    let _ = writeln!(s, "  E minecraft:hopper[enabled=true,facing=east]");
    let _ = writeln!(s, "  H minecraft:hopper[enabled=true,facing=down]");
    let row = |f: &dyn Fn(i32) -> char| -> String { (0..len).map(f).collect() };
    let hoppers = row(&|x| if x == len - 1 { 'H' } else { 'E' });
    let blank = row(&|_| '.');
    let mud = row(&|x| if stem(x) { 'M' } else { '.' });
    let soil_a = row(&|x| if stem(x) { 'F' } else if grate(x) { 'G' } else { '.' });
    let soil_b = row(&|x| if stem(x) { 'F' } else { '.' });
    let pist = row(&|x| if stem(x) { 'P' } else { '.' });
    let obs = row(&|x| if stem(x) { 'O' } else { '.' });
    let dust = row(&|x| if stem(x) { 'd' } else { '.' });
    let _ = writeln!(s, "layer 0:\n  {hoppers}\n  {blank}\n  {blank}\n  {hoppers}");
    let _ = writeln!(s, "layer 2:\n  {mud}\n  {soil_a}\n  {soil_b}\n  {mud}");
    let _ = writeln!(s, "layer 4:\n  {pist}\n  {obs}\n  {obs}\n  {pist}");
    let _ = writeln!(s, "layer 5:\n  {blank}\n  {dust}\n  {dust}\n  {blank}");
    for x in (0..len).filter(|x| stem(*x)) {
        for z in [0, 3] {
            let _ = writeln!(s, "entity: minecraft:hopper_minecart {:.1} -1.0 {:.1} {{Rotation:[0f,0f]}}", x as f64 + 0.5, z as f64 + 0.5);
        }
    }
    s
}

/// A stem lattice (pumpkin or melon): n x n stems two blocks apart on wet farmland, wet farmland
/// on every diagonal, and a fruit cell (on mud) on every side of every stem, each shared by the two
/// stems beside it (a cell holds a fruit only for the ticks before its piston breaks it, so sharing
/// costs no fruit). Wet farmland under the stem and on its four diagonals makes it roll 1 in 4
/// (CropBlock.getGrowthSpeed 7), and with four free sides every roll grows a fruit. Above each stem
/// an observer fires when it attaches; the dust on its top powers the cells above the four pistons
/// around it (quasi-connectivity), which push down into the fruit cells. The squeezed drop goes down
/// through the mud into a hopper minecart under it, which a hopper empties into a chest. Waterlogged
/// grates on diagonals every 8 blocks keep the farmland wet (each wets 4 blocks around).
pub fn stem_lattice(crop: &str, n: i32) -> String {
    let span = 2 * n - 1; // cells -1..=span on x and z
    let lo = -1;
    // grates on diagonals (odd, odd) every 8 blocks, each wetting 4 blocks around (FarmBlock)
    let mut lines: Vec<i32> = (0..).map(|k| 3 + 8 * k).take_while(|&v| v <= span).collect();
    if lines.is_empty() || lines[lines.len() - 1] + 4 < span {
        lines.push(if span.rem_euclid(2) == 1 { span } else { span - 1 });
    }
    let mut soil: BTreeMap<(i32, i32), char> = BTreeMap::new();
    for x in lo..=span {
        for z in lo..=span {
            let (ex, ez) = (x.rem_euclid(2) == 0, z.rem_euclid(2) == 0);
            let c = if ex == ez {
                // stems (even, even) and diagonals (odd, odd): wet farmland; grates on some diagonals
                if !ex && lines.contains(&x) && lines.contains(&z) { 'G' } else { 'F' }
            } else if x == lo || z == lo || x == span || z == span {
                // no fruit cell on the rim: every cell is then shared by two stems, so a fruit that
                // grows in an observer's 2 gt blind spot (a stem re-attaching within the pulse of its
                // last harvest) is still broken by the other stem's next harvest
                '.'
            } else {
                'M'
            };
            soil.insert((x, z), c);
        }
    }
    let stems: Vec<(i32, i32)> = (0..n).flat_map(|i| (0..n).map(move |j| (2 * i, 2 * j))).collect();
    let cells: Vec<(i32, i32)> = soil.iter().filter(|(_, c)| **c == 'M').map(|(p, _)| *p).collect();
    let name = format!("{}-lattice{n}", crop.trim_start_matches("minecraft:"));
    let item = if crop.ends_with("melon") { "melon" } else { "pumpkin" };
    let mut s = String::new();
    let _ = writeln!(s, "name: {name}");
    let _ = writeln!(s, "# A full-size {item} farm: {n} x {n} stems two blocks apart on wet farmland, wet farmland on every");
    let _ = writeln!(s, "# diagonal and a fruit cell (on mud) on every side of every stem but the rim's, each shared by the two");
    let _ = writeln!(s, "# stems beside it (a fruit that grows in an observer's 2 gt blind spot is broken by the other stem's");
    let _ = writeln!(s, "# next harvest). Wet farmland around makes a stem roll 1 in 4 (CropBlock.getGrowthSpeed), and every");
    let _ = writeln!(s, "# roll grows a {item} on a free side. An observer above each stem fires when it attaches; the dust on");
    let _ = writeln!(s, "# its top powers the cells above the four pistons around it, which push down into the fruit cells.");
    let _ = writeln!(s, "# The squeezed drop goes down through the mud into a hopper minecart, which a hopper empties into a");
    let _ = writeln!(s, "# chest. Waterlogged grates on diagonals keep the farmland wet. Written by `circuit farm-gen`.");
    let _ = writeln!(s, "circuit: harvester {}", crop);
    // copies side by side, rim to rim (a bigger farm: `farm-gen --n N`)
    let _ = writeln!(s, "tile: {} 0 0", span - lo + 1);
    for (k, (x, z)) in stems.iter().enumerate() {
        let _ = writeln!(s, "port crop.s{k}: {x} 1 {z}");
    }
    // the collection: a hopper line along x under every even row (the row's carts stand on it),
    // each ending in a chest at the east edge; an odd row's carts stand on hoppers facing north into
    // the line beside them (one line carries 2.5 items/s, two rows' output even at the judge's speed)
    for z in (0..span).filter(|z| z.rem_euclid(2) == 0) {
        let _ = writeln!(s, "port output.r{z}: {} -3 {z}", span - 1);
    }
    let _ = writeln!(s, "origin: {lo} -2 {lo}");
    let _ = writeln!(s, "palette:");
    let _ = writeln!(s, "  F minecraft:farmland[moisture=7]");
    let _ = writeln!(s, "  M minecraft:mud");
    let _ = writeln!(s, "  G minecraft:waxed_copper_grate[waterlogged=true]");
    let _ = writeln!(s, "  O minecraft:observer[facing=down,powered=false]");
    let _ = writeln!(s, "  P minecraft:piston[extended=false,facing=down]");
    let _ = writeln!(s, "  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]");
    let _ = writeln!(s, "  E minecraft:hopper[enabled=true,facing=east]");
    let _ = writeln!(s, "  N minecraft:hopper[enabled=true,facing=north]");
    let _ = writeln!(s, "  H minecraft:hopper[enabled=true,facing=down]");
    let layer = |s: &mut String, y: i32, f: &dyn Fn(i32, i32) -> char| {
        let _ = writeln!(s, "layer {}:", y + 2);
        for z in lo..=span {
            let row: String = (lo..=span).map(|x| f(x, z)).collect();
            let _ = writeln!(s, "  {row}");
        }
    };
    let is_cell = |x: i32, z: i32| soil.get(&(x, z)) == Some(&'M');
    let is_stem = |x: i32, z: i32| x >= 0 && z >= 0 && x.rem_euclid(2) == 0 && z.rem_euclid(2) == 0 && x <= span - 1 && z <= span - 1;
    let hopper = |x: i32, z: i32| -> char {
        if !(0..span).contains(&z) || !(0..span).contains(&x) {
            '.'
        } else if z.rem_euclid(2) == 1 {
            if is_cell(x, z) { 'N' } else { '.' }
        } else if x < span - 1 {
            'E'
        } else {
            'H'
        }
    };
    layer(&mut s, -2, &hopper);
    layer(&mut s, 0, &|x, z| soil.get(&(x, z)).copied().unwrap_or('.'));
    layer(&mut s, 2, &|x, z| if is_stem(x, z) { 'O' } else if is_cell(x, z) { 'P' } else { '.' });
    layer(&mut s, 3, &|x, z| if is_stem(x, z) { 'd' } else { '.' });
    for (x, z) in &cells {
        let _ = writeln!(s, "entity: minecraft:hopper_minecart {:.1} -1.0 {:.1} {{Rotation:[0f,0f]}}", *x as f64 + 0.5, *z as f64 + 0.5);
    }
    s
}
