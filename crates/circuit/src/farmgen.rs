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

/// THE OBVIOUS bamboo fuel farm (the bar for the fuel farm topic, 2026-10-22): `n` copies of the
/// archived robust single-stalk harvester bamboo-hc-12b side by side, 3 blocks apart, sharing two
/// hopper lines (under the mud, and beside the stalks at the third block's height) that end in one
/// chest (the super smelter's fuel chest). Raw bamboo is the fuel (50 gt each).
pub fn bamboo_rows(n: i32) -> String {
    let xe = 3 * (n - 1) + 1; // the lines' east end
    let lo = -1;
    let mut s = String::new();
    let _ = writeln!(s, "name: bamboo-rows{n}");
    let _ = writeln!(s, "# The obvious bamboo fuel farm: {n} copies of bamboo-hc-12b 3 blocks apart. In each, an observer");
    let _ = writeln!(s, "# watches the bamboo's third block and fires the piston below it, which breaks the second block so");
    let _ = writeln!(s, "# the third falls; a hopper minecart on the mud inside the stalk and one half over the third block's");
    let _ = writeln!(s, "# cell take the items. Two hopper lines (under the mud, and beside the stalks) carry them to one");
    let _ = writeln!(s, "# chest: the super smelter's fuel chest. Written by `circuit farm-gen --layout bamboo-rows`.");
    let _ = writeln!(s, "circuit: harvester minecraft:bamboo");
    for k in 0..n {
        let _ = writeln!(s, "port crop.s{k}: {} 1 0", 3 * k);
    }
    let _ = writeln!(s, "port output: {xe} -2 0");
    let _ = writeln!(s, "origin: {lo} -1 -1");
    let _ = writeln!(s, "palette:");
    let _ = writeln!(s, "  S minecraft:stone");
    let _ = writeln!(s, "  M minecraft:mud");
    let _ = writeln!(s, "  O minecraft:observer[facing=north,powered=false]");
    let _ = writeln!(s, "  P minecraft:piston[extended=false,facing=north]");
    let _ = writeln!(s, "  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]");
    let _ = writeln!(s, "  E minecraft:hopper[enabled=true,facing=east]");
    let _ = writeln!(s, "  H minecraft:hopper[enabled=true,facing=down]");
    let _ = writeln!(s, "  T minecraft:hopper[enabled=true,facing=south]");
    let stalk = |x: i32| x >= 0 && x.rem_euclid(3) == 0 && x <= 3 * (n - 1);
    // rows z = -1..=2, columns x = lo..=xe
    let layer = |s: &mut String, y: i32, f: &dyn Fn(i32, i32) -> char| {
        let _ = writeln!(s, "layer {}:", y + 1);
        for z in -1..=2 {
            let row: String = (lo..=xe).map(|x| f(x, z)).collect();
            let _ = writeln!(s, "  {row}");
        }
    };
    let line = |x: i32| if x == xe { 'H' } else if x >= 0 { 'E' } else { '.' };
    layer(&mut s, -1, &|x, z| match z {
        0 => line(x),
        -1 if x == xe => 'T',
        _ => '.',
    });
    layer(&mut s, 0, &|x, z| match z {
        0 if stalk(x) => 'M',
        -1 if x == xe => 'H',
        _ => '.',
    });
    layer(&mut s, 1, &|x, z| match z {
        -1 if stalk(x) => 'S',
        -1 if x == xe => 'H',
        _ => '.',
    });
    layer(&mut s, 2, &|x, z| match z {
        -1 => line(x),
        1 if stalk(x) => 'P',
        2 if stalk(x) => 'S',
        _ => '.',
    });
    layer(&mut s, 3, &|x, z| match z {
        0 if x >= -1 && x <= 3 * (n - 1) + 1 && x.rem_euclid(3) != 0 => 'S',
        1 if stalk(x) => 'O',
        2 if stalk(x) => 'd',
        _ => '.',
    });
    layer(&mut s, 4, &|x, z| if z == 0 && stalk(x) { 'S' } else { '.' });
    for k in 0..n {
        let x = 3 * k;
        let _ = writeln!(s, "entity: minecraft:hopper_minecart {:.1} 3.0 0.0 {{Rotation:[0f,0f]}}", x as f64 + 0.5);
        let _ = writeln!(s, "entity: minecraft:hopper_minecart {:.1} 0.875 0.5 {{Rotation:[0f,0f]}}", x as f64 + 0.5);
    }
    s
}

/// THE OBVIOUS bamboo farm (reviewer, 2026-10-22): a row of `n` stalks side by side on mud over a
/// hopper line, a piston beside each stalk's second block and an observer above it watching the
/// third. The observers' dusts join into one line, so any stalk reaching three blocks fires every
/// piston (bamboo grows with no memory: cutting a short stalk early loses nothing). The second block
/// breaks, the third falls; the piston head shoves the items into a second hopper line in front of
/// the stalks at the second block's height, and what falls lands on the mud, whose hoppers take it.
/// Both lines end in one chest.
pub fn bamboo_line(n: i32) -> String {
    let xe = n; // the lines' east end
    let lo = -1;
    let mut s = String::new();
    let _ = writeln!(s, "name: bamboo-line{n}");
    let _ = writeln!(s, "# The obvious bamboo farm: {n} stalks side by side on mud over a hopper line. A piston beside each");
    let _ = writeln!(s, "# stalk's second block, an observer above it watching the third; the observers' dusts form one line,");
    let _ = writeln!(s, "# so any stalk reaching three blocks fires every piston (bamboo has no memory). The second block");
    let _ = writeln!(s, "# breaks and the third falls; the piston head shoves the items into a hopper line in front of the");
    let _ = writeln!(s, "# stalks, and the hoppers under the mud take what falls. Both lines end in one chest. Written by");
    let _ = writeln!(s, "# `circuit farm-gen --layout bamboo-line`.");
    let _ = writeln!(s, "circuit: harvester minecraft:bamboo");
    for x in 0..n {
        let _ = writeln!(s, "port crop.s{x}: {x} 1 0");
    }
    let _ = writeln!(s, "port output: {xe} -2 0");
    let _ = writeln!(s, "origin: {lo} -1 -1");
    let _ = writeln!(s, "palette:");
    let _ = writeln!(s, "  S minecraft:stone");
    let _ = writeln!(s, "  G minecraft:glass");
    let _ = writeln!(s, "  M minecraft:mud");
    let _ = writeln!(s, "  O minecraft:observer[facing=north,powered=false]");
    let _ = writeln!(s, "  P minecraft:piston[extended=false,facing=north]");
    let _ = writeln!(s, "  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]");
    let _ = writeln!(s, "  E minecraft:hopper[enabled=true,facing=east]");
    let _ = writeln!(s, "  H minecraft:hopper[enabled=true,facing=down]");
    let _ = writeln!(s, "  T minecraft:hopper[enabled=true,facing=south]");
    let stalk = |x: i32| (0..n).contains(&x);
    let layer = |s: &mut String, y: i32, f: &dyn Fn(i32, i32) -> char| {
        let _ = writeln!(s, "layer {}:", y + 1);
        for z in -1..=2 {
            let row: String = (lo..=xe).map(|x| f(x, z)).collect();
            let _ = writeln!(s, "  {row}");
        }
    };
    let line = |x: i32| if x == xe { 'H' } else { 'E' };
    // the floor: mud over one hopper line ending in the chest
    layer(&mut s, -1, &|x, z| match z {
        0 if x >= 0 => line(x),
        _ => '.',
    });
    layer(&mut s, 0, &|x, z| match z {
        0 if stalk(x) => 'M',
        0 if x == lo => 'S',
        // under the front wall: an item the head pushes down along the wall would slip out below it
        -1 if x >= lo && x <= xe => 'S',
        _ => '.',
    });
    // glass walls in front of the stalks up to the third block and at both ends, and a glass roof
    // over the stalks and the observers: the piston squeezes items against the front wall and they
    // shoot up (glass lets the daylight through: bamboo needs light 9)
    let wall = |x: i32, z: i32| (z == -1 && x >= lo && x <= xe) || ((x == lo || x == xe) && (0..=1).contains(&z));
    layer(&mut s, 1, &|x, z| if wall(x, z) { 'G' } else if z == 1 && stalk(x) { 'S' } else { '.' });
    layer(&mut s, 2, &|x, z| match z {
        1 if stalk(x) => 'P',
        2 if stalk(x) => 'S',
        _ if wall(x, z) => 'G',
        _ => '.',
    });
    layer(&mut s, 3, &|x, z| match z {
        1 if stalk(x) => 'O',
        2 if stalk(x) => 'd',
        _ if wall(x, z) => 'G',
        _ => '.',
    });
    layer(&mut s, 4, &|x, z| if (0..=1).contains(&z) && x >= lo && x <= xe { 'G' } else { '.' });
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
    // the collection: a hopper line along x under every even row (the row's carts stand on it); an
    // odd row's carts stand on hoppers facing north into the line beside them; every line ends in a
    // collector along z at the east edge, which ends in one chest (the farm makes about 1 item/s at
    // the game's speed; one hopper line carries 2.5)
    let _ = writeln!(s, "port output: {span} -3 {}", span - 1);
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
    let _ = writeln!(s, "  S minecraft:hopper[enabled=true,facing=south]");
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
        if !(0..span).contains(&z) || !(0..=span).contains(&x) {
            '.'
        } else if x == span {
            if z < span - 1 { 'S' } else { 'H' }
        } else if z.rem_euclid(2) == 1 {
            if is_cell(x, z) { 'N' } else { '.' }
        } else {
            'E'
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
