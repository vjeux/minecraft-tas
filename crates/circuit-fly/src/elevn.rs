//! `circuit fly elevator --floors N`: the N-floor elevator (track fly; judged by circuit::elevator
//! as `elevator-floors N S`). The 3-floor scheme of elev3.rs stacked: the target is a thermometer
//! code G1..G(N-1) (G_k = target >= k) in a master latch of sliders; every middle floor k carries
//! its own stops (planes of the gates G_k, !G(k+1), D), its own down unit (armed by G_k's level, a
//! 35-tick line) and up unit (armed by !G(k+1): fast disarm, late arm), its own presence and
//! obsidian kickers. Buttons: one per destination at every floor; each floor's press reaches the
//! destination's merge block by an observer chain, whose reader writes every bit.

use crate::elev3::{B, BULB, DIRS, DUST, OBS, Pos, STONE, add, bottom_flag, cmp, keepout, machine, middle, obs, piston, rep, route_chain, route_level, sticky, top_flag};
use mctas::design::Design;
use std::collections::{HashMap, HashSet};

/// What a middle floor's units need from the routers.
struct Mid {
    /// down-stop plane pistons: G in, G out, !G' in, !G' out, !D in, !D out
    pd: Vec<Pos>,
    /// up-stop plane pistons: G in, G out, !G' in, !G' out, D in, D out
    pu: Vec<Pos>,
    /// the down unit's arming block (G_k's slow level line ends here)
    x_d: Pos,
    /// the up unit's !G(k+1) block (one fast line ends here)
    ng_y: Pos,
    /// the not-present source and the block feeding D16''s subtracting side
    np: Pos,
    np_in: Pos,
    /// the gated pistons and their kickers
    fu: Pos,
    fd: Pos,
    ku: Pos,
    kd: Pos,
}

/// One middle floor at offset `m` (the step C units of elev3.rs, parametrized).
fn mid_unit(b: &mut B, m: i32) -> Mid {
    let [_us, ds, _, fd] = middle(b, m);
    // up unit (south side): presence and not-present observers on P's dock, a subtracting
    // comparator C' (rear: presence through two 4-tick repeaters; sides: not-present and !G')
    let y = m + 3;
    b.st([2, y, 1], &obs("north"));
    b.st([2, y, 2], BULB);
    b.on([2, y, 3], &cmp("north", "compare"));
    b.on([2, y, 4], &rep("north", 4));
    b.on([2, y, 5], &rep("north", 4));
    b.on([2, y, 6], &cmp("north", "subtract"));
    let fu = [2, y, 7];
    b.st(fu, &piston("south"));
    b.free([2, y, 8]);
    b.st([3, y, 0], &obs("west"));
    b.st([4, y, 0], "minecraft:waxed_copper_bulb[lit=true,powered=false]");
    b.on([4, y, 1], &cmp("north", "compare"));
    for z in 2..=5 {
        b.on([4, y, z], &rep("north", 1));
    }
    b.st([4, y, 6], STONE);
    b.on([3, y, 6], &rep("east", 1));
    b.on([1, y, 6], &rep("west", 1));
    b.on([0, y, 6], DUST);
    let y4 = y + 1;
    b.st([0, y4, 6], STONE);
    let ng_y = [-2, y4, 6];
    b.st(ng_y, STONE);
    b.on([-1, y4, 6], &rep("west", 1));
    for x in [-3, -4, -5] {
        b.on([x, y4, 6], &rep("east", 4));
    }
    b.st([-6, y4, 6], STONE);
    b.on([-6, y4, 7], &rep("north", 4));
    b.st([-6, y4, 8], STONE);
    for x in [-5, -4, -3, -2, -1] {
        b.on([x, y4, 8], &rep("west", 4));
    }
    b.st([0, y4, 8], STONE);
    b.on([0, y4, 7], &rep("south", 4));
    // down unit: c_d (the down piston's back) holds a stone at rest; a sticky piston on G_k's
    // delayed level pushes it out
    let c_d = [fd[0], fd[1], fd[2] - 1];
    let g_d = [c_d[0] - 1, c_d[1], c_d[2]];
    b.st(c_d, STONE);
    b.free([c_d[0] + 1, c_d[1], c_d[2]]);
    b.st(g_d, &sticky("east"));
    let x_d = [g_d[0] - 1, g_d[1], g_d[2]];
    b.st(x_d, STONE);
    // presence and D16' (presence delayed minus not-present)
    b.st([0, m + 1, -2], &obs("south"));
    b.st([0, m + 1, -3], BULB);
    b.on([0, m + 1, -4], &cmp("south", "compare"));
    b.on([0, m + 1, -5], &rep("south", 4));
    b.on([0, m + 1, -6], &rep("south", 4));
    b.on([0, m + 1, -7], &cmp("south", "subtract"));
    b.on([-1, m + 1, -7], &rep("west", 1));
    let np_in = [-2, m + 1, -7];
    b.st(np_in, STONE);
    b.st([-1, m + 1, -1], &obs("east"));
    b.st([-2, m + 1, -1], "minecraft:waxed_copper_bulb[lit=true,powered=false]");
    b.on([-3, m + 1, -1], &cmp("east", "compare"));
    let np = [-4, m + 1, -1];
    b.st(np, STONE);
    b.st([0, m + 1, -8], STONE);
    b.st([0, m + 2, -8], DUST);
    b.on([-1, m + 2, -8], &rep("east", 1));
    b.st([-2, m + 2, -8], STONE);
    b.st([-2, m + 2, -7], &rep("north", 1));
    b.on([-2, m + 2, -6], &rep("north", 1));
    b.on([-2, m + 2, -5], DUST);
    b.on([-1, m + 2, -5], DUST);
    b.on([-1, m + 2, -4], &rep("north", 1));
    // the stop planes
    let gate = |b: &mut B, c: Pos, park: Pos, pin: (Pos, &str), pout: (Pos, &str), on: bool| {
        b.st(pin.0, &piston(pin.1));
        b.st(pout.0, &piston(pout.1));
        if on {
            b.st(c, STONE);
            b.free(park);
        } else {
            b.free(c);
            b.st(park, STONE);
        }
        [pin.0, pout.0]
    };
    let y = m;
    b.st([-9, y, -4], "minecraft:redstone_block");
    let mut pd = Vec::new();
    for (x, on) in [(-7, false), (-5, true), (-3, false)] {
        b.on([x - 1, y, -4], &rep("west", 1));
        pd.extend(gate(b, [x, y, -4], [x, y, -5], ([x, y, -6], "south"), ([x, y, -3], "north"), on));
    }
    b.on([-2, y, -4], &rep("west", 1));
    b.st([-1, y, -4], STONE);
    b.on([-1, y, -3], &rep("north", 1));
    b.st([ds[0] - 1, ds[1], ds[2]], STONE);
    let y = m + 4;
    b.st([-8, y, -2], "minecraft:redstone_block");
    let mut pu = Vec::new();
    for (x, on) in [(-6, false), (-4, true), (-2, true)] {
        b.on([x - 1, y, -2], &rep("west", 1));
        pu.extend(gate(b, [x, y, -2], [x, y, -3], ([x, y, -4], "south"), ([x, y, -1], "north"), on));
    }
    for x in [-1, 0] {
        b.on([x, y, -2], &rep("west", 1));
    }
    Mid { pd, pu, x_d, ng_y, np, np_in, fu, fd, ku: [1, m + 1, 1], kd: [-1, m + 2, 0] }
}

/// A destination's button column: (x, z) of its buttons (each within reach of the rider).
fn col(d: usize) -> (i32, i32) {
    match d {
        0..=2 => (d as i32 + 1, -4),
        3 => (4, -3),
        _ => (0, -3),
    }
}

/// The N-floor elevator, floors `s` apart (N in 3..=5, s >= 6: a free layer between floors).
pub fn floors_n(n: usize, s: i32) -> Design {
    assert!((3..=5).contains(&n) && s >= 6, "elevator: 3..=5 floors, at least 6 apart");
    let ni = n as i32;
    let h = (ni - 1) * s;
    let mut b = B::new(format!("fly-elevator{n}-{s}"));
    machine(&mut b);
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    let fb = bottom_flag(&mut b);
    let ft = top_flag(&mut b, h);
    // the master latch: bit k (1..n-1) a slider at y_k (reset: free, stone, block; set: stone,
    // block, free between PR_k at x = 1 and PS_k at x = 5, z = -10); taps watch each piston's back
    let lb = (h - 2 * (ni - 2)) / 2;
    let yk = |k: usize| lb + 2 * (k as i32 - 1);
    let mut ps = vec![[0; 3]; n];
    let mut pr = vec![[0; 3]; n];
    let mut ts = vec![[0; 3]; n];
    let mut tr = vec![[0; 3]; n];
    let mut lv_g = vec![[0; 3]; n];
    let mut lv_ng = vec![[0; 3]; n];
    for k in 1..n {
        let y = yk(k);
        pr[k] = [1, y, -13];
        ps[k] = [5, y, -13];
        b.st(pr[k], &piston("east"));
        b.free([2, y, -13]);
        b.st([3, y, -13], STONE);
        b.st([4, y, -13], "minecraft:redstone_block");
        b.st(ps[k], &piston("west"));
        ts[k] = [6, y, -13];
        tr[k] = [0, y, -13];
        b.st(ts[k], &obs("west"));
        b.st(tr[k], &obs("east"));
        // G_k's level beside the set cell, !G_k's beside the block's rest cell
        if k <= n - 2 {
            b.on([3, y, -14], &rep("south", 1));
            lv_g[k] = [3, y, -15];
            b.on(lv_g[k], DUST);
        }
        if k >= 2 {
            b.on([4, y, -12], &rep("north", 1));
            lv_ng[k] = [4, y, -11];
            b.on(lv_ng[k], DUST);
        }
    }
    // the ends: bottom unit (a G1 copy beside the bottom flag's piston) and top unit (a G(n-1)
    // copy feeding the top flag's piston)
    let c_b = [fb[0], fb[1], fb[2] - 1];
    let (o_b, i_b) = ([c_b[0] - 1, c_b[1], c_b[2]], [c_b[0] + 2, c_b[1], c_b[2]]);
    b.free(c_b);
    b.st(o_b, &piston("east"));
    b.st([c_b[0] + 1, c_b[1], c_b[2]], STONE);
    b.st(i_b, &piston("west"));
    let opb = [0, 1, -2];
    b.st(opb, &obs("south"));
    b.st([0, 1, -3], "minecraft:waxed_copper_bulb[lit=true,powered=false]");
    b.on([0, 1, -4], &cmp("south", "compare"));
    for p in [[0, 1, -5], [1, 1, -5], [2, 1, -5]] {
        b.dust(p);
    }
    b.on([2, 1, -4], &rep("north", 3));
    let (pr_t, ps_t) = ([-2, h + 2, -3], [2, h + 2, -3]);
    b.st(pr_t, &piston("east"));
    b.free([-1, h + 2, -3]);
    b.st([0, h + 2, -3], STONE);
    b.st([1, h + 2, -3], "minecraft:redstone_block");
    b.st(ps_t, &piston("west"));
    b.on([ft[0], ft[1], ft[2] - 1], &rep("north", 1));
    let relay = [-1, h + 1, -1];
    // the middle floors
    let mids: Vec<Mid> = (1..n - 1).map(|k| mid_unit(&mut b, k as i32 * s)).collect();
    // buttons: per floor and destination a button on a bulb, a comparator, an edge observer
    let mut edge = vec![vec![[0; 3]; n]; n];
    for f in 0..n {
        let y = f as i32 * s;
        for (d, e) in edge[f].iter_mut().enumerate() {
            let (x, z) = col(d);
            b.st([x, y + 4, z], "minecraft:stone_button[face=floor,facing=north,powered=false]");
            b.d.ports.insert(format!("input.{f}-{d}"), [x, y + 4, z]);
            b.d.meta.insert(format!("port input.{f}-{d}"), "button".into());
            b.st([x, y + 3, z], BULB);
            b.on([x, y + 3, z - 1], &cmp("south", "compare"));
            b.st([x, y + 3, z - 2], &obs("south"));
            // a delay stage (a block, three repeaters, set after routing to even out the floors'
            // latencies); its last repeater is the chain's source
            b.st([x, y + 3, z - 3], STONE);
            for dz in 4..=6 {
                b.on([x, y + 3, z - dz], &rep("south", 1));
            }
            *e = [x, y + 3, z - 6];
        }
    }
    // the destinations' merge blocks west of the latch: each floor's chain ends in M_d, a repeater
    // reads it, an observer watching that repeater is the destination's source
    let mut src = vec![[0; 3]; n];
    let mut merge = vec![[0; 3]; n];
    for (d, (sd, md)) in src.iter_mut().zip(merge.iter_mut()).enumerate() {
        let x = -4 - 3 * d as i32;
        *md = [x, lb + 1, -15];
        b.st(*md, STONE);
        for dz in 16..=18 {
            b.on([x, lb + 1, -dz], &rep("south", 1));
        }
        *sd = [x, lb + 1, -19];
        b.st(*sd, &obs("south"));
    }
    // routing: level lines first, then the observer chains
    let mut avoid = keepout(h);
    for p in &b.stations {
        avoid.insert(*p);
    }
    let mut reserved: HashSet<Pos> = HashSet::new();
    let (lo, hi) = ([-16, 0, -24], [18, h + 8, 12]);
    let front = |p: Pos, d: Pos| Some(add(p, d));
    let mut lines: Vec<(Pos, Pos, Option<Pos>, u8)> = Vec::new();
    for (i, md) in mids.iter().enumerate() {
        let k = i + 1;
        lines.push((lv_g[k], md.x_d, None, 35));
        lines.push((lv_ng[k + 1], md.ng_y, None, 1));
        lines.push((md.np, md.np_in, None, 1));
    }
    let kick_room: Vec<Pos> = mids.iter().flat_map(|md| [md.ku, md.kd]).flat_map(|k| DIRS.iter().flat_map(move |d| [add(k, *d), add(add(k, *d), *d), add(add(add(k, *d), *d), *d)])).collect();
    reserved.extend(kick_room.iter().copied());
    route_lines(&mut b, &avoid, &mut reserved, &mut lines, lo, hi);
    for c in &kick_room {
        reserved.remove(c);
    }
    let mut jobs: Vec<(Pos, Pos, Option<Pos>)> = Vec::new();
    for md in &mids {
        jobs.push((md.fu, md.ku, None));
        jobs.push((md.fd, md.kd, None));
    }
    // the buttons into their merge blocks
    for (f, row) in edge.iter().enumerate() {
        for (d, e) in row.iter().enumerate() {
            let _ = f;
            jobs.push((*e, merge[d], None));
        }
    }
    // each destination writes every bit
    for (d, sd) in src.iter().enumerate() {
        for k in 1..n {
            if k <= d {
                jobs.push((*sd, ps[k], front(ps[k], [-1, 0, 0])));
            } else {
                jobs.push((*sd, pr[k], front(pr[k], [1, 0, 0])));
            }
        }
    }
    // the ends' copies
    jobs.push((ts[1], i_b, front(i_b, [-1, 0, 0])));
    jobs.push((tr[1], o_b, front(o_b, [1, 0, 0])));
    jobs.push((ts[n - 1], ps_t, front(ps_t, [-1, 0, 0])));
    jobs.push((tr[n - 1], pr_t, front(pr_t, [1, 0, 0])));
    // the stop planes of every middle floor
    for (i, md) in mids.iter().enumerate() {
        let k = i + 1;
        let (g_in, g_out, ng_in, ng_out) = (ts[k], tr[k], tr[k + 1], ts[k + 1]);
        for (p, srcs) in [(&md.pu, [g_in, g_out, ng_in, ng_out, opb, relay]), (&md.pd, [g_in, g_out, ng_in, ng_out, relay, opb])] {
            for (j, t) in p.iter().enumerate() {
                let f = if j % 2 == 0 { [0, 0, 1] } else { [0, 0, -1] };
                jobs.push((srcs[j], *t, front(*t, f)));
            }
        }
    }
    route_chains(&mut b, &avoid, &mut reserved, &mut jobs, lo, hi);
    // even out the latencies (in redstone ticks: an observer hop or a repeater tick each): every
    // press reaches its merge block at the same time, and every destination's writes reach a bit
    // within a tick or two of each other
    let hops = |b: &B, from: Pos, to: Pos| -> Option<u32> { chain_depth(b, from, to) };
    for d in 0..n {
        let lat: Vec<u32> = (0..n).map(|f| hops(&b, edge[f][d], merge[d]).unwrap_or(0)).collect();
        let top = *lat.iter().max().unwrap();
        for f in 0..n {
            // three repeaters: 3..=12 ticks
            let mut extra = (top - lat[f]).min(9);
            let (x, z) = col(d);
            let y = f as i32 * s + 3;
            for dz in 4..=6 {
                let t = 1 + extra.min(3);
                extra -= t - 1;
                b.d.blocks.insert([x, y, z - dz], rep("south", t as u8));
            }
        }
        if std::env::var("ELEV_DEBUG").is_ok() {
            eprintln!("dest {d}: chain hops per floor {lat:?}");
        }
    }
    let mut wl = vec![vec![0u32; n]; n];
    for (d, sd) in src.iter().enumerate() {
        for k in 1..n {
            let t = if k <= d { ps[k] } else { pr[k] };
            wl[d][k] = hops(&b, *sd, t).unwrap_or(0);
        }
    }
    // per destination, a pad (0..=9 extra ticks in its reader's three repeaters) chosen so that
    // each bit's writes from every destination land as close together as possible
    let arrive: Vec<u32> = (0..n).map(|d| (0..n).map(|f| hops(&b, edge[f][d], merge[d]).unwrap_or(0)).max().unwrap()).collect();
    let mut best = (u32::MAX, vec![0u32; n]);
    let combos = 10u32.pow(n as u32);
    for code in 0..combos {
        let pad: Vec<u32> = (0..n).map(|d| code / 10u32.pow(d as u32) % 10).collect();
        let worst = (1..n)
            .map(|k| {
                let l: Vec<u32> = (0..n).map(|d| arrive[d] + pad[d] + wl[d][k]).collect();
                l.iter().max().unwrap() - l.iter().min().unwrap()
            })
            .max()
            .unwrap();
        if worst < best.0 {
            best = (worst, pad);
        }
    }
    for d in 0..n {
        let x = -4 - 3 * d as i32;
        let mut extra = best.1[d];
        for dz in 16..=18 {
            let t = 1 + extra.min(3);
            extra -= t - 1;
            b.d.blocks.insert([x, lb + 1, -dz], rep("south", t as u8));
        }
    }
    if std::env::var("ELEV_DEBUG").is_ok() {
        eprintln!("write hops per destination and bit {wl:?}; arrivals {arrive:?}; pads {:?}, worst bit spread {} ticks", best.1, best.0);
    }
    b.d.meta.insert("circuit".into(), format!("elevator-floors {n} {s}"));
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.d.ports.insert("rider".into(), [2, 4, 0]);
    b.finish()
}

/// Route level lines with protected ends and move-to-front retries (as in elev3::step_c).
fn route_lines(b: &mut B, avoid: &HashSet<Pos>, reserved: &mut HashSet<Pos>, lines: &mut Vec<(Pos, Pos, Option<Pos>, u8)>, lo: Pos, hi: Pos) {
    let ends = |p: Pos| -> Vec<Pos> {
        let mut v = Vec::new();
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let c = [p[0] + d[0], p[1], p[2] + d[2]];
            v.push(c);
            v.push([c[0], c[1] - 1, c[2]]);
        }
        v
    };
    let base = (b.d.clone(), b.stations.clone(), reserved.clone(), b.empty.clone());
    for attempt in 0.. {
        let mut prot: HashSet<Pos> = HashSet::new();
        for (s, t, _, _) in lines.iter() {
            prot.extend(ends(*s));
            prot.extend(ends(*t));
        }
        let mut failed = None;
        for (k, (src, dst, f, delay)) in lines.iter().enumerate() {
            let mut res = reserved.clone();
            res.extend(prot.iter().copied().filter(|c| !ends(*src).contains(c) && !ends(*dst).contains(c)));
            match route_level(b, avoid, &mut res, *src, *dst, *f, lo, hi, *delay) {
                Some(cells) => {
                    reserved.extend(res.difference(&prot).copied().collect::<Vec<_>>());
                    for c in cells {
                        prot.remove(&c);
                    }
                }
                None => {
                    failed = Some(k);
                    break;
                }
            }
        }
        match failed {
            None => return,
            Some(k) => {
                if attempt >= 30 {
                    panic!("elevator: no repeater route from {:?} to {:?}", lines[k].0, lines[k].1);
                }
                let job = lines.remove(k);
                lines.insert(0, job);
                b.d = base.0.clone();
                b.stations = base.1.clone();
                b.empty = base.3.clone();
                *reserved = base.2.clone();
            }
        }
    }
}

/// Route observer chains (a source's chains may branch from its earlier chains), keeping every
/// target's approach for its own chain, with move-to-front retries.
fn route_chains(b: &mut B, avoid: &HashSet<Pos>, reserved: &mut HashSet<Pos>, jobs: &mut Vec<(Pos, Pos, Option<Pos>)>, lo: Pos, hi: Pos) {
    let base = (b.d.clone(), b.stations.clone(), reserved.clone(), b.empty.clone());
    let approach = |t: Pos| -> Vec<Pos> { DIRS.iter().flat_map(|d| [add(t, *d), add(add(t, *d), *d)]).collect() };
    for attempt in 0.. {
        let mut nets: HashMap<Pos, Vec<Pos>> = HashMap::new();
        let mut prot: HashSet<Pos> = jobs.iter().flat_map(|(_, t, _)| approach(*t)).collect();
        let mut failed = None;
        for (k, (src, dst, f)) in jobs.iter().enumerate() {
            let mut sources = vec![*src];
            sources.extend(nets.get(src).cloned().unwrap_or_default());
            let mine = approach(*dst);
            let mut res = reserved.clone();
            res.extend(prot.iter().copied().filter(|c| !mine.contains(c) && !sources.contains(c)));
            match route_chain(b, avoid, &mut res, &sources, *dst, *f, lo, hi) {
                Some(cells) => {
                    reserved.extend(res.difference(&prot).copied().collect::<Vec<_>>());
                    for c in &cells {
                        prot.remove(c);
                    }
                    nets.entry(*src).or_default().extend(cells);
                }
                None => {
                    failed = Some(k);
                    break;
                }
            }
        }
        match failed {
            None => return,
            Some(k) => {
                if attempt >= 40 {
                    panic!("elevator: no observer route from {:?} to {:?}", jobs[k].0, jobs[k].1);
                }
                let job = jobs.remove(k);
                jobs.insert(0, job);
                b.d = base.0.clone();
                b.stations = base.1.clone();
                b.empty = base.3.clone();
                *reserved = base.2.clone();
            }
        }
    }
}

fn facing(s: &str) -> Option<Pos> {
    let f = s.split("facing=").nth(1)?.split([',', ']']).next()?;
    Some(match f {
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "south" => [0, 0, 1],
        _ => [0, 0, -1],
    })
}

/// The observer hops from `from` (a chain's source) to the observer whose back powers `to`,
/// walking the observers that watch each other (a breadth-first walk: chains may branch).
fn chain_depth(b: &B, from: Pos, to: Pos) -> Option<u32> {
    let is_obs = |p: Pos| b.d.blocks.get(&p).is_some_and(|s| s.contains("observer"));
    let mut q = std::collections::VecDeque::from([(from, 0u32)]);
    let mut seen = HashSet::from([from]);
    while let Some((c, d)) = q.pop_front() {
        for dir in DIRS {
            let o = add(c, dir);
            if seen.contains(&o) || !is_obs(o) {
                continue;
            }
            let f = facing(&b.d.blocks[&o])?;
            // o watches c
            if add(o, f) != c {
                continue;
            }
            if add(o, [-f[0], -f[1], -f[2]]) == to {
                return Some(d + 1);
            }
            seen.insert(o);
            q.push_back((o, d + 1));
        }
    }
    None
}
