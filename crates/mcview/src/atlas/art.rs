//! Hand-drawn tiles in the look of the game's own textures (drawn here, not copied: no game asset
//! is shipped). Colors are measured from the game (`mctas web palette` on a local reference, never
//! committed); shapes follow its layout (an observer's face and bands, a piston's planks band...).
//! Each replaces the procedural tile of the same name.

use super::{CLEAR, Rgba, Tile, mix, rgb, rgba, scale};

type Painter = Box<dyn Fn(&mut Tile)>;

/// The stone of the game: 3 grays in their shares, in short horizontal streaks.
const STONE: &[(u32, f32)] = &[(0x727272, 0.34), (0x7f7f7f, 0.46), (0x8f8f8f, 0.20)];

/// A dark-seamed cobble pattern (the game's cobblestone and the stone faces of redstone parts):
/// mostly light stones (between `mid` and `light`), a highlight under the seam above, a shadow
/// over the seam below.
fn cobble(t: &mut Tile, x0: i32, y0: i32, x1: i32, y1: i32, light: u32, mid: u32, dark: u32, seam: u32, k: u32) {
    let mut c = t.clone();
    let between = mix(rgb(mid), rgb(light), 0.45);
    c.stones(rgb(light), between, rgb(seam), 11, k);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let p = c.get(x, y);
            let px = if p == rgb(seam) {
                p
            } else if c.get(x, y + 1) == rgb(seam) || c.get(x + 1, y) == rgb(seam) {
                mix(p, rgb(dark), 0.4)
            } else if c.get(x, y - 1) == rgb(seam) {
                mix(p, rgb(light), 0.3)
            } else {
                p
            };
            t.set(x, y, px);
        }
    }
}

pub fn tiles() -> Vec<(&'static str, Painter)> {
    let mut v: Vec<(&'static str, Painter)> = Vec::new();
    let mut add = |name: &'static str, f: Painter| v.push((name, f));

    // ---- stone, the most common block
    add("stone", Box::new(|t| {
        t.mottle(STONE, 4, 2, 0.55, 1);
    }));
    add("smooth_stone", Box::new(|t| {
        t.mottle(&[(0xa2a2a2, 0.35), (0xa8a8a8, 0.40), (0xb0b0b0, 0.25)], 4, 4, 0.4, 2).outline(0, 0, 15, 15, rgb(0x818181));
    }));
    add("smooth_stone_slab_side", Box::new(|t| {
        t.mottle(&[(0xa2a2a2, 0.35), (0xababab, 0.65)], 4, 4, 0.4, 3);
        t.rect(0, 0, 15, 0, rgb(0x838383)).rect(0, 15, 15, 15, rgb(0x838383)).rect(0, 7, 15, 7, rgb(0x838383));
        t.rect(0, 1, 15, 1, rgb(0xc9c9c9)).rect(0, 8, 15, 8, rgb(0xc9c9c9));
        t.rect(0, 0, 0, 15, rgb(0x838383)).rect(15, 0, 15, 15, rgb(0x838383));
    }));
    add("cobblestone", Box::new(|t| {
        cobble(t, 0, 0, 15, 15, 0xacacac, 0x888788, 0x6e6d6d, 0x5f5f5f, 4);
    }));
    add("stone_bricks", Box::new(|t| {
        t.mottle(&[(0x787678, 0.4), (0x7f7f7f, 0.6)], 4, 2, 0.5, 5);
        for y in [7, 15] {
            t.rect(0, y, 15, y, rgb(0x646564));
        }
        for (x, y0, y1) in [(15, 0, 6), (7, 8, 14)] {
            t.rect(x, y0, x, y1, rgb(0x646564));
        }
        t.rect(0, 0, 14, 0, rgb(0x929092)).rect(0, 8, 6, 8, rgb(0x929092)).rect(8, 8, 15, 8, rgb(0x929092));
    }));

    // ---- grass floor, dirt
    add("grass_top", Box::new(|t| {
        // the plains grass of the real-game pictures (measured on them: the game's tint and light)
        t.mottle(&[(0x49603a, 0.36), (0x536c43, 0.43), (0x617c4e, 0.21)], 2, 2, 0.35, 6);
    }));
    add("dirt", Box::new(|t| {
        t.mottle(&[(0x593d29, 0.13), (0x79553a, 0.42), (0x946c4b, 0.28), (0xb48560, 0.17)], 2, 2, 0.3, 7);
    }));
    add("grass_side", Box::new(|t| {
        t.mottle(&[(0x593d29, 0.13), (0x79553a, 0.42), (0x946c4b, 0.28), (0xb48560, 0.17)], 2, 2, 0.3, 7);
        let fringe = [3, 4, 3, 2, 3, 5, 4, 3, 2, 3, 4, 3, 2, 4, 3, 2];
        for x in 0..16 {
            for y in 0..fringe[x as usize] {
                let c = if (x + y) % 3 == 0 { 0x63813d } else if (x * 7 + y) % 5 == 0 { 0x485e2c } else { 0x567135 };
                t.set(x, y, rgb(c));
            }
        }
    }));

    // ---- observers: a face with two eyes on the front, a red lamp on the back, light stone bands
    let obs_bands = |t: &mut Tile, band: Rgba, body: Rgba, edge: Rgba| {
        t.fill(body);
        t.mottle_in(1, 4, 14, 12, &[(0x393939, 0.45), (0x3e3d3d, 0.25), (0x494646, 0.30)], 1, 4, 0.6, 8);
        t.rect(1, 1, 14, 2, band).rect(1, 13, 14, 14, band);
        t.map(|x, y, c| if (1..=2).contains(&y) || (13..=14).contains(&y) { scale(c, 0.92 + 0.16 * super::noise(x, y, 33)) } else { c });
        t.outline(0, 0, 15, 15, edge).rect(1, 3, 14, 3, rgb(0x2c2c2c)).rect(1, 12, 14, 12, rgb(0x2c2c2c));
    };
    add("observer_side", Box::new(move |t| {
        obs_bands(t, rgb(0x797979), rgb(0x3a3939), rgb(0x282727));
        // vertical grain of the dark body
        t.map(|x, y, c| if (4..=11).contains(&y) && x % 3 == 1 { scale(c, 1.18) } else { c });
    }));
    add("observer_back", Box::new(move |t| {
        obs_bands(t, rgb(0x777777), rgb(0x393939), rgb(0x2c2424));
        t.rect(7, 8, 8, 11, rgb(0x777777)).rect(7, 8, 7, 11, rgb(0x8a8a8a));
        t.outline(6, 4, 9, 7, rgb(0x1e1818)).rect(7, 5, 8, 6, rgb(0x4a0a0a)).rect(7, 5, 7, 5, rgb(0x6b1010));
    }));
    add("observer_back_on", Box::new(move |t| {
        obs_bands(t, rgb(0x8a7272), rgb(0x3e3434), rgb(0x2c2424));
        t.rect(7, 8, 8, 11, rgb(0x7c7272)).rect(7, 8, 7, 11, rgb(0x9a8a8a));
        t.outline(5, 3, 10, 8, rgb(0x5a2a2a)).outline(6, 4, 9, 7, rgb(0x8a3030)).rect(7, 5, 8, 6, rgb(0xff1c1c)).rect(7, 5, 7, 5, rgb(0xff8a7a));
    }));
    add("observer_front", Box::new(|t| {
        t.fill(rgb(0x3d3c3c));
        t.rect(1, 1, 14, 2, rgb(0x777777)).map(|x, y, c| if (1..=2).contains(&y) && (1..=14).contains(&x) { scale(c, 0.92 + 0.16 * super::noise(x, y, 35)) } else { c });
        t.rect(1, 3, 14, 4, rgb(0x2a2929));
        t.mottle_in(1, 5, 14, 14, &[(0x999999, 0.30), (0xadadad, 0.55), (0xbababa, 0.15)], 2, 2, 0.5, 9);
        t.grid(
            &[
                "KKKKKKKKKKKKKKKK",
                "K..............K",
                "K..............K",
                "K..............K",
                "K..............K",
                "D..............D",
                "D..............D",
                "D.mmm......mmm.D",
                "D.EEW......WEE.D",
                "D.EEE......EEE.D",
                "D..............D",
                "D......mm......D",
                "D..............D",
                "D..............D",
                "D..............D",
                "KKKKKKKKKKKKKKKK",
            ],
            &[('K', rgb(0x242424)), ('D', rgb(0x3d3c3c)), ('E', rgb(0x1a1a1a)), ('W', rgb(0x8f8f8f)), ('m', rgb(0x777777))],
        );
    }));
    add("observer_top", Box::new(|t| {
        t.mottle(&[(0x5d5d5d, 0.55), (0x6a6a6a, 0.45)], 2, 2, 0.5, 10).outline(0, 0, 15, 15, rgb(0x343434));
        // the arrow (it points to the back: the signal's way)
        for y in 2..=12 {
            let half = (y - 2) / 2;
            let (x0, x1) = (7 - half, 8 + half);
            for x in x0..=x1 {
                let c = if x == x0 { rgb(0x343434) } else if x == x1 { rgb(0x797979) } else if x <= 7 { rgb(0x9c9c9c) } else { rgb(0x8a8a8a) };
                t.set(x, y, c);
            }
        }
        t.rect(2, 13, 13, 13, rgb(0x343434));
    }));

    // ---- pistons: a planks band, cobble below; the face is planks with iron corners
    add("piston_side", Box::new(|t| {
        cobble(t, 1, 5, 14, 14, 0x9a9a9a, 0x7c7c7c, 0x666666, 0x59524b, 11);
        t.outline(0, 4, 15, 15, rgb(0x3c3c3c));
        for y in 0..4 {
            for x in 0..16 {
                let gray = x == 0 || x == 15 || x == 7 || x == 8;
                let c = if y == 3 {
                    rgb(0x59524b)
                } else if gray {
                    rgb(if y == 0 { 0x8f8f8f } else { 0x787878 })
                } else {
                    scale(rgb(0xb1986c), 0.9 + 0.15 * super::noise(x / 3, y, 12))
                };
                t.set(x, y, c);
            }
        }
    }));
    let planks_face = |t: &mut Tile| {
        // boards laid like bricks
        for y in 0..16 {
            for x in 0..16 {
                let row = y / 4;
                let off = if row % 2 == 1 { 4 } else { 0 };
                let seam = y % 4 == 3 || (x + off) % 8 == 7;
                let c = if seam { rgb(0x67502c) } else { scale(rgb(0xb19057), 0.9 + 0.14 * super::noise((x + off) / 8 + row * 3, y, 13)) };
                t.set(x, y, c);
            }
        }
    };
    let iron_corners = |t: &mut Tile| {
        let iron = rgb(0xcfcbcb);
        let dark = rgb(0x8f8a8a);
        for (x, y) in [(0, 0), (14, 0), (0, 14), (14, 14)] {
            t.rect(x, y, x + 1, y + 1, iron);
        }
        for (x0, y0, x1, y1) in [(7, 0, 8, 0), (7, 15, 8, 15), (0, 7, 0, 8), (15, 7, 15, 8)] {
            t.rect(x0, y0, x1, y1, iron);
        }
        t.set(1, 1, dark);
        t.set(14, 1, dark);
        t.set(1, 14, dark);
        t.set(14, 14, dark);
    };
    add("piston_top", Box::new(move |t| {
        planks_face(t);
        iron_corners(t);
    }));
    add("piston_top_sticky", Box::new(move |t| {
        planks_face(t);
        t.map(|_, _, c| scale(c, 0.8));
        // the slime: a lumpy green pad over most of the face
        let s = t.clone();
        for y in 1..15 {
            for x in 1..15 {
                let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt() + 2.0 * super::noise(x / 2, y / 2, 14);
                if d < 7.2 {
                    let c = if d > 6.2 { 0x4f7a36 } else if super::noise(x, y, 15) < 0.25 { 0xa8d888 } else if d < 3.5 { 0x87b664 } else { 0x6e964f };
                    t.set(x, y, rgb(c));
                } else {
                    t.set(x, y, s.get(x, y));
                }
            }
        }
        iron_corners(t);
    }));
    add("piston_bottom", Box::new(|t| {
        cobble(t, 0, 0, 15, 15, 0x8c8c8c, 0x707070, 0x5a5a5a, 0x535151, 16);
        t.outline(0, 0, 15, 15, rgb(0x3c3c3c));
    }));
    add("piston_inner", Box::new(|t| {
        cobble(t, 0, 0, 15, 15, 0x929292, 0x6f6f6f, 0x5a5a5a, 0x4d4c4c, 17);
        t.outline(0, 0, 15, 15, rgb(0x323232));
        t.outline(5, 5, 10, 10, rgb(0x929292)).rect(6, 6, 9, 9, rgb(0x323232)).rect(7, 7, 8, 8, rgb(0x1e1e1e));
    }));
    add("piston_arm", Box::new(|t| {
        t.map(|x, y, _| scale(rgb(0xb1986c), 0.88 + 0.16 * super::noise(x / 3, y, 18)));
    }));

    // ---- redstone
    add("redstone_block", Box::new(|t| {
        t.mottle(&[(0x730c00, 0.18), (0x9c1604, 0.36), (0xbd2008, 0.17), (0xe62008, 0.29)], 2, 2, 0.55, 19);
        t.outline(0, 0, 15, 15, rgb(0xbd2008));
    }));
    let diode_top = |t: &mut Tile, k: u32| {
        t.mottle(&[(0xb4b4b4, 0.7), (0xbcbcbc, 0.3)], 4, 4, 0.4, k);
        for y in [4, 8, 12] {
            t.rect(0, y, 15, y, rgb(0x868886));
            t.rect(0, y - 1, 15, y - 1, rgb(0xc7ccc7));
        }
        for (x, y0, y1) in [(4, 0, 3), (11, 5, 7), (3, 9, 11), (12, 13, 15)] {
            t.rect(x, y0, x, y1, rgb(0x868886));
        }
        t.outline(0, 0, 15, 15, rgb(0x868886));
    };
    add("repeater_top", Box::new(move |t| {
        diode_top(t, 20);
        t.outline(6, 2, 9, 13, rgb(0xc7ccc7)).rect(7, 3, 8, 12, rgb(0x430201)).rect(7, 3, 7, 12, rgb(0x5a0a08));
    }));
    add("repeater_top_on", Box::new(move |t| {
        diode_top(t, 20);
        t.outline(6, 2, 9, 13, rgb(0xc7ccc7)).rect(7, 3, 8, 12, rgb(0xd80102)).rect(7, 3, 7, 12, rgb(0xff4a3a));
    }));
    let comparator = move |t: &mut Tile, on: bool| {
        diode_top(t, 21);
        for y in 3..=10 {
            let half = (y - 3) / 2 + 1;
            t.rect(8 - half, y, 7 + half, y, rgb(if on { 0xd8c0bc } else { 0xcfcdc6 }));
            t.set(8 - half, y, rgb(0x7a716f));
            t.set(7 + half, y, rgb(0x7a716f));
        }
        t.rect(3, 11, 12, 12, rgb(if on { 0xe01010 } else { 0x4a0402 })).outline(3, 11, 12, 12, rgb(if on { 0xb00808 } else { 0x2a0201 }));
        if on {
            for (x, y) in [(7, 6), (6, 8), (9, 8), (8, 9)] {
                t.set(x, y, rgb(0xd88a8a));
            }
        }
    };
    add("comparator_top", Box::new(move |t| comparator(t, false)));
    add("comparator_top_on", Box::new(move |t| comparator(t, true)));
    let torch = |t: &mut Tile, head: u32, hi: u32, lo: u32| {
        t.fill(CLEAR).rect(7, 8, 8, 15, rgb(0x4f3f28)).rect(7, 8, 7, 15, rgb(0x9c7c4d));
        t.rect(7, 6, 8, 7, rgb(head)).set(7, 6, rgb(hi));
        t.set(8, 7, rgb(lo));
    };
    add("torch_on", Box::new(move |t| torch(t, 0xfd0000, 0xffb0a0, 0xaf0000)));
    add("torch_off", Box::new(move |t| torch(t, 0x6d1010, 0x8a3a30, 0x440000)));
    add("torch_top_on", Box::new(|t| {
        t.fill(CLEAR).rect(7, 7, 8, 8, rgb(0xfd0000)).set(7, 7, rgb(0xffb0a0));
    }));
    add("torch_top_off", Box::new(|t| {
        t.fill(CLEAR).rect(7, 7, 8, 8, rgb(0x6d1010));
    }));
    add("lever_handle", Box::new(|t| {
        t.fill(CLEAR).rect(7, 6, 8, 15, rgb(0x614e32)).rect(7, 6, 7, 15, rgb(0x927f63)).set(8, 6, rgb(0x877357));
    }));
    // the lamp: a center diamond and four corner quarters, lit or not, in a dark lattice
    let lamp = |t: &mut Tile, lattice: u32, fill: u32, hi: u32, lo: u32| {
        t.fill(rgb(lattice));
        for y in 1..15 {
            for x in 1..15 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let center = (fx - 8.0).abs() + (fy - 8.0).abs();
                let corner = [(0.0f32, 0.0f32), (16.0, 0.0), (0.0, 16.0), (16.0, 16.0)].iter().map(|(cx, cy)| (fx - cx).abs() + (fy - cy).abs()).fold(f32::MAX, f32::min);
                let (d, r) = if center < corner { (center, 5.6) } else { (corner, 6.1) };
                if d < r {
                    // light from the top left inside each diamond
                    let c = if d > r - 1.2 { if (fx - 8.0) + (fy - 8.0) < 0.0 && center < corner { hi } else { lo } } else { fill };
                    t.set(x, y, rgb(c));
                }
            }
        }
    };
    add("lamp_off", Box::new(move |t| lamp(t, 0x311a11, 0x8a512b, 0xa76438, 0x563017)));
    add("lamp_on", Box::new(move |t| lamp(t, 0x3b2015, 0xc78741, 0xf4cd97, 0x9a6a30)));
    // copper bulbs: 4 chevrons around a cross, by oxidation; lit = bright chevrons; powered = a red dot
    let bulbs: [(&str, [u32; 4], [u32; 4]); 4] = [
        ("bulb_0", [0x5f3315, 0x844929, 0xb96649, 0xd87c62], [0xa0543b, 0xd0765a, 0xfbb85f, 0xffedbb]),
        ("bulb_1", [0x544d3d, 0x766452, 0xa07c64, 0xc5887e], [0x8f745d, 0xc5887e, 0xe08d29, 0xfccd8c]),
        ("bulb_2", [0x4f5248, 0x456b5f, 0x728656, 0x6eab85], [0x5e7d56, 0x699e70, 0x79b694, 0xe4a653]),
        ("bulb_3", [0x315446, 0x3a6c58, 0x4e9880, 0x5eb796], [0x3d7d68, 0x5db593, 0xc27e3c, 0xdca649]),
    ];
    for (name, off, lit) in bulbs {
        let paint = move |t: &mut Tile, pal: [u32; 4], bright: bool, powered: bool| {
            // off: copper (light shades) with dark chevrons; lit: deeper copper with glowing ones
            let (base, arm, edge) = if bright { ([(pal[1], 0.7), (pal[0], 0.3)], pal[3], pal[2]) } else { ([(pal[2], 0.7), (pal[3], 0.3)], pal[1], pal[0]) };
            t.mottle(&base, 2, 2, 0.5, 22).outline(0, 0, 15, 15, rgb(if bright { pal[0] } else { pal[1] }));
            // chevrons: an L in each quarter with its corner toward the middle
            for (sx, sy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
                let at = |i: i32, j: i32| -> (i32, i32) {
                    let x = if sx > 0 { i } else { 15 - i };
                    let y = if sy > 0 { j } else { 15 - j };
                    (x, y)
                };
                for i in 2..=6 {
                    for (a, b) in [(i, 5), (i, 6), (5, i), (6, i)] {
                        let (x, y) = at(a, b);
                        t.set(x, y, rgb(arm));
                    }
                }
                let (x, y) = at(6, 6);
                t.set(x, y, rgb(edge));
                let (x, y) = at(2, 2);
                t.set(x, y, rgb(pal[3]));
            }
            t.rect(7, 7, 8, 8, rgb(if powered { 0xcc1010 } else if bright { pal[1] } else { pal[2] }));
        };
        let n = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        add(n(name.to_string()), Box::new(move |t| paint(t, off, false, false)));
        add(n(format!("{name}_lit")), Box::new(move |t| paint(t, lit, true, false)));
        add(n(format!("{name}_p")), Box::new(move |t| paint(t, off, false, true)));
        add(n(format!("{name}_lit_p")), Box::new(move |t| paint(t, lit, true, true)));
    }
    add("note_block", Box::new(|t| {
        t.fill(rgb(0x292820)).outline(1, 1, 14, 14, rgb(0x412818));
        for y in 2..14 {
            for x in 2..14 {
                t.set(x, y, rgb(if (x + y) % 2 == 0 { 0x88573d } else { 0x5a3723 }));
            }
        }
    }));
    add("target_side", Box::new(|t| {
        t.mottle(&[(0xf3e9dc, 0.8), (0xfefefd, 0.2)], 2, 2, 0.4, 23);
        t.outline(0, 0, 15, 15, rgb(0xebd7ba)).outline(1, 1, 14, 14, rgb(0xc73636)).outline(4, 4, 11, 11, rgb(0xc73636)).rect(7, 7, 8, 8, rgb(0xc73636));
    }));
    add("target_top", Box::new(|t| {
        t.mottle(&[(0xebd7ba, 0.5), (0xefe2d0, 0.5)], 2, 2, 0.4, 24);
        t.outline(1, 1, 14, 14, rgb(0xc73636)).outline(4, 4, 11, 11, rgb(0xc73636)).rect(7, 7, 8, 8, rgb(0xc73636));
    }));

    // ---- containers and machines
    add("hopper_outside", Box::new(|t| {
        // smooth iron-gray with faint horizontal streaks
        t.mottle(&[(0x434346, 0.3), (0x4b4b4b, 0.5), (0x555353, 0.2)], 16, 2, 0.85, 25);
        t.rect(0, 0, 15, 0, rgb(0x5d5a5a));
    }));
    add("hopper_inside", Box::new(|t| {
        t.mottle(&[(0x2d2d32, 0.4), (0x343438, 0.4), (0x3a393d, 0.2)], 4, 4, 0.6, 26);
    }));
    add("hopper_top", Box::new(|t| {
        t.fill(rgb(0x2d2d32));
        t.mottle_in(0, 0, 15, 1, &[(0x4f4f4f, 0.6), (0x5a5858, 0.4)], 4, 1, 0.6, 27);
        t.mottle_in(0, 14, 15, 15, &[(0x4f4f4f, 0.6), (0x5a5858, 0.4)], 4, 1, 0.6, 28);
        t.mottle_in(0, 2, 1, 13, &[(0x4f4f4f, 0.6), (0x5a5858, 0.4)], 1, 4, 0.6, 29);
        t.mottle_in(14, 2, 15, 13, &[(0x4f4f4f, 0.6), (0x5a5858, 0.4)], 1, 4, 0.6, 30);
        t.outline(2, 2, 13, 13, rgb(0x3e3d41));
    }));
    let stone_face = |t: &mut Tile, k: u32| {
        t.mottle(&[(0x6c6b6b, 0.25), (0x929292, 0.40), (0xb6b6b6, 0.35)], 4, 2, 0.5, k);
        t.outline(0, 0, 15, 15, rgb(0x6c6b6b));
        // the brows of the game's machine faces
        for (x0, x1) in [(2, 5), (10, 13)] {
            t.rect(x0, 2, x1, 2, rgb(0x424040));
        }
        t.rect(1, 3, 1, 4, rgb(0x424040)).rect(14, 3, 14, 4, rgb(0x424040));
    };
    add("dispenser_front", Box::new(move |t| {
        stone_face(t, 28);
        t.grid(
            &[
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "......dddd......",
                ".....dDDDDd.....",
                ".....dDKKDd.....",
                ".....dDKKDd.....",
                ".....dDDDDd.....",
                "......dddd......",
                "................",
                "................",
                "................",
            ],
            &[('d', rgb(0x6c6b6b)), ('D', rgb(0x424040)), ('K', rgb(0x1a1919))],
        );
    }));
    add("dropper_front", Box::new(move |t| {
        stone_face(t, 29);
        t.grid(
            &[
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "....dddddddd....",
                "...dDDDDDDDDd...",
                "...dDKKKKKKDd...",
                "....dDKKKKDd....",
                ".....dDDDDd.....",
                "................",
                "................",
                "................",
            ],
            &[('d', rgb(0x6c6b6b)), ('D', rgb(0x424040)), ('K', rgb(0x1a1919))],
        );
    }));
    add("furnace_side", Box::new(|t| {
        cobble(t, 0, 0, 15, 15, 0xacacac, 0x838383, 0x646464, 0x4a4848, 30);
        t.rect(0, 10, 15, 10, rgb(0x4a4848)).rect(0, 11, 15, 13, rgb(0xacacac)).rect(0, 14, 15, 14, rgb(0x646464)).rect(0, 15, 15, 15, rgb(0x4a4848));
        t.map(|x, y, c| if (11..=13).contains(&y) { scale(c, 0.94 + 0.1 * super::noise(x, y, 31)) } else { c });
    }));
    add("furnace_top", Box::new(|t| {
        cobble(t, 0, 0, 15, 15, 0x8c8c8c, 0x777777, 0x636262, 0x4a4848, 32);
    }));

    // ---- sculk sensor
    add("sculk_top", Box::new(|t| {
        t.mottle(&[(0x034150, 0.55), (0x074857, 0.45)], 2, 2, 0.4, 34);
        for (x, y) in [(2, 3), (11, 2), (6, 7), (13, 9), (3, 12), (9, 13), (12, 13)] {
            t.set(x, y, rgb(0x3c8287));
        }
        for (x, y) in [(5, 1), (14, 5), (1, 8), (8, 10)] {
            t.set(x, y, rgb(0x052a32));
        }
    }));
    add("sculk_side", Box::new(|t| {
        t.fill(CLEAR);
        t.mottle_in(0, 8, 15, 15, &[(0x0d1217, 0.45), (0x0e1e26, 0.35), (0x062e37, 0.20)], 2, 2, 0.4, 35);
        t.rect(0, 8, 15, 8, rgb(0x054f5d));
        for (x, y) in [(3, 11), (10, 13), (13, 10)] {
            t.set(x, y, rgb(0x29dfeb));
        }
    }));
    let tendril = |t: &mut Tile, body: u32, edge: u32, tip: u32| {
        t.fill(CLEAR);
        // a spike in x 4..12, y 0..8 (the part the model shows), widening downward
        for y in 1..8 {
            let half = (y + 1) / 3;
            for x in (7 - half)..=(8 + half) {
                let c = if y == 1 { tip } else if x == 7 - half || x == 8 + half { edge } else { body };
                t.set(x, y, rgb(c));
            }
        }
    };
    add("sculk_tendril", Box::new(move |t| tendril(t, 0x06262d, 0x164a57, 0x1d7589)));
    add("sculk_tendril_active", Box::new(move |t| tendril(t, 0x009295, 0x086e68, 0x29dfeb)));

    // ---- slime, honey, glass
    add("slime_block", Box::new(|t| {
        t.fill(rgba(0x73c262, 190)).outline(0, 0, 15, 15, rgba(0x60b348, 230)).outline(3, 3, 12, 12, rgba(0x60b348, 230)).rect(4, 4, 11, 11, rgba(0x7bcc65, 215)).rect(5, 5, 6, 6, rgba(0xa8e298, 230));
    }));
    add("honey_block", Box::new(|t| {
        t.fill(rgba(0xfdc134, 190)).outline(0, 0, 15, 15, rgba(0xf79909, 230)).outline(3, 3, 12, 12, rgba(0xfaab1c, 225)).rect(4, 4, 11, 11, rgba(0xfbd467, 210));
    }));
    add("glass", Box::new(|t| {
        t.fill(CLEAR).outline(0, 0, 15, 15, rgba(0xd0eae9, 255)).rect(1, 1, 1, 1, rgba(0x88bec9, 255)).rect(14, 14, 14, 14, rgba(0x88bec9, 255));
        for (x, y) in [(3, 2), (2, 3), (4, 2), (2, 4), (5, 3)] {
            t.set(x, y, rgba(0xffffff, 200));
        }
    }));

    // ---- rails: oak sleepers, iron (gold) rails, a red line when powered
    let rails = |t: &mut Tile, rail: u32, rail_dark: u32, sleeper: u32, line: Option<u32>| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(2, y, 13, y + 1, rgb(sleeper)).rect(2, y + 1, 13, y + 1, scale(rgb(sleeper), 0.7));
        }
        t.rect(2, 0, 2, 15, rgb(rail)).rect(3, 0, 3, 15, rgb(rail_dark)).rect(12, 0, 12, 15, rgb(rail)).rect(13, 0, 13, 15, rgb(rail_dark));
        if let Some(c) = line {
            t.rect(7, 0, 8, 15, rgb(c));
        }
    };
    add("rail", Box::new(move |t| rails(t, 0xa2a3a2, 0x6c6f6b, 0x8f6f3f, None)));
    add("powered_rail", Box::new(move |t| rails(t, 0xce9d33, 0x9a7426, 0x6d6655, Some(0x4e120b))));
    add("powered_rail_on", Box::new(move |t| rails(t, 0xce9d33, 0x9a7426, 0x69614f, Some(0xb10201))));
    add("activator_rail", Box::new(move |t| rails(t, 0xa69f93, 0x79715f, 0x58492d, Some(0x4d0202))));
    add("activator_rail_on", Box::new(move |t| rails(t, 0xa69f93, 0x79715f, 0x58492d, Some(0xd01010))));
    add("detector_rail", Box::new(move |t| {
        rails(t, 0xa2a3a2, 0x6c6f6b, 0x70644f, None);
        t.rect(5, 6, 10, 9, rgb(0x877e63)).rect(6, 7, 9, 8, rgb(0x561f13));
    }));

    // ---- wood, wool, metals
    add("oak_planks", Box::new(|t| {
        for y in 0..16 {
            for x in 0..16 {
                let row = y / 4;
                let off = [0, 6, 3, 9][row as usize];
                let seam = y % 4 == 3 || ((x + off) % 12 == 11 && row % 2 == 0) || ((x + off) % 16 == 15 && row % 2 == 1);
                let grain = super::noise(x / 4 + row * 5, y, 36);
                let c = if seam { 0x715831 } else if grain < 0.25 { 0x9d804a } else if grain > 0.75 { 0xbc9860 } else { 0xaf8f55 };
                t.set(x, y, rgb(c));
            }
        }
    }));
    add("oak_log", Box::new(|t| {
        t.mottle(&[(0x503f25, 0.37), (0x745a36, 0.42), (0x937344, 0.21)], 1, 8, 0.8, 37);
    }));
    add("oak_log_top", Box::new(|t| {
        for y in 0..16 {
            for x in 0..16 {
                let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs());
                let c = if d > 6.6 { 0x5b482a } else if (d as i32) % 2 == 0 { 0xab8c53 } else if d < 1.0 { 0x937240 } else { 0xb9955f };
                t.set(x, y, rgb(c));
            }
        }
    }));
    add("white_wool", Box::new(|t| {
        t.mottle(&[(0xd6dcdd, 0.22), (0xe3e6e7, 0.28), (0xf0f2f2, 0.23), (0xfbfbfb, 0.27)], 2, 2, 0.45, 38);
    }));
    add("orange_wool", Box::new(|t| {
        t.mottle(&[(0xe66605, 0.22), (0xee6f0d, 0.28), (0xf67b19, 0.32), (0xf98c26, 0.18)], 2, 2, 0.45, 39);
    }));
    add("iron_block", Box::new(|t| {
        t.mottle(&[(0xd8d8d8, 0.3), (0xe5e5e5, 0.5), (0xefefef, 0.2)], 4, 4, 0.4, 40);
        t.outline(0, 0, 15, 15, rgb(0xbcbcbc)).rect(0, 0, 15, 0, rgb(0xefefef)).rect(0, 0, 0, 15, rgb(0xefefef));
    }));
    add("gold_block", Box::new(|t| {
        t.mottle(&[(0xf6c726, 0.35), (0xffe046, 0.5), (0xfffe9c, 0.15)], 4, 4, 0.4, 41);
        t.outline(0, 0, 15, 15, rgb(0xd0922d)).rect(0, 0, 15, 0, rgb(0xfffe9c)).rect(0, 0, 0, 15, rgb(0xfffe9c));
    }));
    add("obsidian", Box::new(|t| {
        t.mottle(&[(0x000001, 0.25), (0x06030b, 0.29), (0x100c1c, 0.26), (0x2e2146, 0.20)], 2, 2, 0.5, 42);
    }));
    v
}
