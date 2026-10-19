//! The generators: observer chains, repeater chains, amethyst rings.

use crate::build::{add, dir_name, dist2, neg, sub, travel, Build, Emitter, Sensor, DIRS};
use crate::{report, simulate_all, Out};
use anyhow::Result;
use clock::Pos;

const STONE: &str = "minecraft:stone";

#[derive(clap::Args)]
pub struct ObsArgs {
    /// cal | plain | both
    #[arg(long, default_value = "both")]
    sensor: String,
    /// Longest observer chain.
    #[arg(long, default_value_t = 3)]
    max_k: usize,
    /// note,trapdoor,irontrapdoor,gate,piston,sticky,pistonext,dispenser | all
    #[arg(long, default_value = "note,trapdoor,irontrapdoor,gate,piston,sticky,pistonext")]
    emitters: String,
    /// Also: a stone block at the last observer's output, the emitter beside it.
    #[arg(long)]
    conductor: bool,
    #[command(flatten)]
    out: Out,
}

/// Self-avoiding observer paths from the sensor at the origin: `path[0]` = sensor, `path[j]` = the
/// j-th observer (watching `path[j-1]`). The first step is east, up or down (the other horizontal
/// directions are rotations of east).
fn obs_paths(max_k: usize) -> Vec<Vec<Pos>> {
    fn rec(path: &mut Vec<Pos>, max_k: usize, out: &mut Vec<Vec<Pos>>) {
        out.push(path.clone());
        if path.len() > max_k {
            return;
        }
        let last = *path.last().unwrap();
        for d in DIRS {
            let q = add(last, d);
            if path.contains(&q) {
                continue;
            }
            path.push(q);
            rec(path, max_k, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    for d in [[1, 0, 0], [0, 1, 0], [0, -1, 0]] {
        let mut p = vec![[0, 0, 0], d];
        rec(&mut p, max_k, &mut out);
    }
    out
}

/// A sensor at `p` (its back, for a calibrated one, pointing away from `away_from`).
fn put_sensor(b: &mut Build, s: Sensor, p: Pos, away_from: Option<Pos>) {
    let back = match away_from {
        Some(q) if q[1] == p[1] => sub(p, q),
        _ => [0, 0, -1],
    };
    let back = if back.iter().map(|v| v.abs()).sum::<i32>() == 1 { back } else { [0, 0, -1] };
    b.put(p, s.state(back));
    b.output = Some(p);
}

fn observer(to_watch: Pos, at: Pos) -> String {
    format!("minecraft:observer[facing={},powered=false]", dir_name(sub(to_watch, at)))
}

pub fn obs(a: &ObsArgs) -> Result<()> {
    let ems = Emitter::parse_list(&a.emitters);
    let mut cands: Vec<(clock::Clock, String)> = Vec::new();
    let mut tried = 0usize;
    for sensor in Sensor::parse_list(&a.sensor) {
        for path in obs_paths(a.max_k) {
            let k = path.len() - 1;
            if k == 0 {
                continue;
            }
            let mut base = Build::default();
            put_sensor(&mut base, sensor, path[0], Some(path[1]));
            for j in 1..=k {
                base.put(path[j], observer(path[j - 1], path[j]));
            }
            let last = path[k];
            let dir = sub(last, path[k - 1]);
            let out = add(last, dir);
            if !base.free(out) {
                continue;
            }
            let how_path: Vec<String> = path[1..].iter().map(|p| format!("{},{},{}", p[0], p[1], p[2])).collect();
            for &e in &ems {
                let mut b = base.clone();
                if b.emitter(out, e) {
                    tried += 1;
                    let how = format!("obs {} k={k} path {} emitter {} at the output (travel {})", sensor.tag(), how_path.join(" "), e.tag(), travel(path[0], out));
                    if let Some(c) = b.clock("cand") {
                        cands.push((c, how.clone()));
                    }
                }
                if a.conductor {
                    for u in DIRS {
                        if u == neg(dir) {
                            continue;
                        }
                        let mut b = base.clone();
                        if !b.put(out, STONE) {
                            continue;
                        }
                        let ep = add(out, u);
                        if b.emitter(ep, e) {
                            tried += 1;
                            let how = format!("obs {} k={k} path {} stone at the output, emitter {} beside it (travel {})", sensor.tag(), how_path.join(" "), e.tag(), travel(path[0], ep));
                            if let Some(c) = b.clock("cand") {
                                cands.push((c, how.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
    let found = simulate_all(cands, &a.out);
    println!("obs: {tried} candidates, {} clocks", found.len());
    report(found, &a.out)
}

#[derive(clap::Args)]
pub struct ChainArgs {
    /// cal | plain | both
    #[arg(long, default_value = "both")]
    sensor: String,
    /// Most repeaters in the line.
    #[arg(long, default_value_t = 3)]
    max_reps: usize,
    /// Fewest repeaters.
    #[arg(long, default_value_t = 0)]
    min_reps: usize,
    /// line (the repeaters read the sensor) and/or below (they read the stone under it).
    #[arg(long, default_value = "line,below")]
    layouts: String,
    /// obs (observer in line), obsx (observer, stone, emitter beside), obsside, obstop, direct
    /// (the emitter at the last repeater's output), torch (a wall torch on a stone the last
    /// repeater powers, the emitter beside the torch).
    #[arg(long, default_value = "obs,obsx,obsside,obstop,direct,torch")]
    terms: String,
    #[arg(long, default_value = "note,trapdoor,gate,piston,sticky")]
    emitters: String,
    #[command(flatten)]
    out: Out,
}

/// Non-increasing delay lists of length m (1..=4 each).
fn delay_lists(m: usize) -> Vec<Vec<u8>> {
    fn rec(m: usize, max: u8, cur: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
        if cur.len() == m {
            out.push(cur.clone());
            return;
        }
        for d in (1..=max).rev() {
            cur.push(d);
            rec(m, d, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    rec(m, 4, &mut Vec::new(), &mut out);
    out
}

/// Emitter states for a cell that is POWERED at rest (by a lit torch): a note block plays, a
/// trapdoor / gate opens, a piston extends, a dispenser fires on its placement update, so the
/// clock starts by itself.
fn powered_emitter(b: &mut Build, p: Pos, e: Emitter) -> bool {
    match e {
        Emitter::Note => b.put(p, "minecraft:note_block[instrument=harp,note=0,powered=false]") && b.keep_air(add(p, [0, 1, 0])),
        Emitter::Trapdoor => b.put(p, "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]"),
        Emitter::IronTrapdoor => b.put(p, "minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]"),
        Emitter::Gate => b.put(p, "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]"),
        Emitter::Piston(d) => b.put(p, format!("minecraft:piston[extended=false,facing={}]", dir_name(d))) && b.keep_air(add(p, d)),
        Emitter::Sticky(d) => b.put(p, format!("minecraft:sticky_piston[extended=false,facing={}]", dir_name(d))) && b.keep_air(add(p, d)),
        Emitter::PistonExt(_) => false,
        Emitter::Dispenser => b.put(p, "minecraft:dispenser[facing=up,triggered=false]"),
    }
}

pub fn chain(a: &ChainArgs) -> Result<()> {
    let ems = Emitter::parse_list(&a.emitters);
    let terms: Vec<&str> = a.terms.split(',').collect();
    let mut cands: Vec<(clock::Clock, String)> = Vec::new();
    let mut tried = 0usize;
    for sensor in Sensor::parse_list(&a.sensor) {
        for layout in a.layouts.split(',') {
            // The repeater row y; the sensor's cell.
            let (y, sp) = match layout {
                "line" => (1, [0, 1, 0]),
                "below" => (1, [0, 2, 0]),
                other => anyhow::bail!("unknown layout {other}"),
            };
            for m in a.min_reps..=a.max_reps {
                if layout == "below" && m == 0 {
                    continue;
                }
                for delays in delay_lists(m) {
                    for powered in [false, true] {
                        if m == 0 && powered {
                            continue;
                        }
                        let mut base = Build::default();
                        put_sensor(&mut base, sensor, sp, Some([1, sp[1], 0]));
                        if layout == "below" {
                            base.put([0, 1, 0], STONE);
                        }
                        for (i, d) in delays.iter().enumerate() {
                            let x = i as i32 + 1;
                            base.put([x, y, 0], format!("minecraft:repeater[delay={d},facing=west,locked=false,powered={powered}]"));
                            base.put([x, y - 1, 0], STONE);
                        }
                        // The head: the last repeater (or the sensor / its stone), output toward +x.
                        let hx = m as i32;
                        let head = [hx, y, 0];
                        let watched = if m == 0 { sp } else { head };
                        let dl: Vec<String> = delays.iter().map(|d| d.to_string()).collect();
                        let tag = format!("chain {} {layout} reps [{}]{}", sensor.tag(), dl.join(","), if powered { " placed powered" } else { "" });
                        let mut local: Vec<(Build, String)> = Vec::new();
                        for t in &terms {
                            for &e in &ems {
                                match *t {
                                    "obs" | "obsx" => {
                                        let op = [hx + 1, y, 0];
                                        if m == 0 && layout == "line" && sp[1] != y {
                                            continue;
                                        }
                                        let mut b = base.clone();
                                        if !b.put(op, observer(if m == 0 { sp } else { head }, op)) {
                                            continue;
                                        }
                                        let out = [hx + 2, y, 0];
                                        if *t == "obs" {
                                            let mut b2 = b.clone();
                                            if b2.emitter(out, e) {
                                                local.push((b2, format!("{tag}, observer in line, {} (travel {})", e.tag(), travel(sp, out))));
                                            }
                                        } else {
                                            for u in [[1, 0, 0], [0, 0, 1], [0, 1, 0], [0, -1, 0]] {
                                                let mut b2 = b.clone();
                                                let ep = add(out, u);
                                                if b2.put(out, STONE) && b2.emitter(ep, e) {
                                                    local.push((b2, format!("{tag}, observer, stone, {} beside (travel {})", e.tag(), travel(sp, ep))));
                                                }
                                            }
                                        }
                                    }
                                    "obsside" | "obstop" => {
                                        if m == 0 {
                                            continue;
                                        }
                                        let (op, out) = if *t == "obsside" { ([hx, y, 1], [hx, y, 2]) } else { ([hx, y + 1, 0], [hx, y + 2, 0]) };
                                        let mut b = base.clone();
                                        if b.put(op, observer(watched, op)) && b.emitter(out, e) {
                                            local.push((b, format!("{tag}, observer on the {} of the last repeater, {} (travel {})", if *t == "obsside" { "side" } else { "top" }, e.tag(), travel(sp, out))));
                                        }
                                    }
                                    "direct" => {
                                        if m == 0 {
                                            continue;
                                        }
                                        let ep = [hx + 1, y, 0];
                                        let mut b = base.clone();
                                        let ok = if powered { powered_emitter(&mut b, ep, e) } else { b.emitter(ep, e) };
                                        if ok {
                                            local.push((b, format!("{tag}, {} at the last repeater's output (travel {})", e.tag(), travel(sp, ep))));
                                        }
                                    }
                                    "torch" => {
                                        if m == 0 {
                                            continue;
                                        }
                                        let xp = [hx + 1, y, 0];
                                        let tp = [hx + 1, y, 1];
                                        let mut b = base.clone();
                                        if !(b.put(xp, STONE) && b.put(tp, "minecraft:redstone_wall_torch[facing=south,lit=true]")) {
                                            continue;
                                        }
                                        for u in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 1, 0]] {
                                            let ep = add(tp, u);
                                            let mut b2 = b.clone();
                                            if powered_emitter(&mut b2, ep, e) {
                                                local.push((b2, format!("{tag}, wall torch on a stone, {} beside the torch (travel {})", e.tag(), travel(sp, ep))));
                                            }
                                        }
                                    }
                                    other => anyhow::bail!("unknown terminal {other}"),
                                }
                            }
                        }
                        for (b, how) in local {
                            tried += 1;
                            if let Some(c) = b.clock("cand") {
                                cands.push((c, how.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
    let found = simulate_all(cands, &a.out);
    println!("chain: {tried} candidates, {} clocks", found.len());
    report(found, &a.out)
}

#[derive(clap::Args)]
pub struct RingArgs {
    /// Hop lengths (floor(distance) from each amethyst to the next sensor), e.g. 15,15,15.
    #[arg(long)]
    hops: String,
    /// Random embeddings to try.
    #[arg(long, default_value_t = 20_000)]
    tries: u32,
    /// Coordinates stay within +-R of the first sensor (one horizontal plane).
    #[arg(long, default_value_t = 20)]
    radius: i32,
    /// Seed.
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Stop after this many valid embeddings.
    #[arg(long, default_value_t = 200)]
    max_embeddings: usize,
    #[command(flatten)]
    out: Out,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i32
    }
}

pub fn ring(a: &RingArgs) -> Result<()> {
    let hops: Vec<i32> = a.hops.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
    let n = hops.len();
    let mut rng = Rng(a.seed.max(1) * 0x9E37_79B9_7F4A_7C15);
    let horiz = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
    let mut cands: Vec<(clock::Clock, String)> = Vec::new();
    let mut tried = 0usize;
    'outer: for _ in 0..a.tries {
        // Sensors s[i] with amethyst am[i] beside them; hop i: am[i] -> s[i+1].
        let mut s: Vec<Pos> = vec![[0, 0, 0]];
        let mut am: Vec<Pos> = Vec::new();
        for i in 0..n {
            let a_i = add(s[i], horiz[rng.range(0, 3) as usize]);
            am.push(a_i);
            if i + 1 == n {
                break;
            }
            // The next sensor: a random cell at the hop's travel time from a_i.
            let mut placed = false;
            for _ in 0..200 {
                let q = [rng.range(-a.radius, a.radius), 0, rng.range(-a.radius, a.radius)];
                if dist2(a_i, q) <= 256 && travel(a_i, q) == hops[i] && s.iter().chain(am.iter()).all(|p| dist2(*p, q) >= 4) {
                    s.push(q);
                    placed = true;
                    break;
                }
            }
            if !placed {
                continue 'outer;
            }
        }
        // Closing hop: am[n-1] -> s[0].
        if dist2(am[n - 1], s[0]) > 256 || travel(am[n - 1], s[0]) != hops[n - 1] {
            continue;
        }
        // Each amethyst touches only its own sensor; no two blocks overlap.
        let mut b = Build::default();
        let mut ok = true;
        for i in 0..n {
            ok &= b.put(am[i], "minecraft:amethyst_block");
        }
        for i in 0..n {
            for (j, aj) in am.iter().enumerate() {
                if j != i && dist2(*aj, s[i]) == 1 {
                    ok = false;
                }
            }
        }
        if !ok {
            continue;
        }
        // The kick: a note block K that only s[0] hears, placed behind s[0] (its back), touching
        // nothing else.
        let back = sub(s[0], am[0]);
        let k = add(s[0], back);
        if !(1..n).all(|j| dist2(k, s[j]) > 256) || am.iter().any(|p| dist2(*p, k) <= 1) {
            continue;
        }
        // s[0] faces away from K (no output into its back); the others face anywhere.
        b.put(s[0], format!("minecraft:calibrated_sculk_sensor[facing={},power=0,sculk_sensor_phase=inactive,waterlogged=false]", dir_name(neg(back))));
        for p in &s[1..] {
            ok &= b.put(*p, "minecraft:calibrated_sculk_sensor[facing=north,power=0,sculk_sensor_phase=inactive,waterlogged=false]");
        }
        ok &= b.put(k, "minecraft:note_block[instrument=harp,note=0,powered=false]") && b.keep_air(add(k, [0, 1, 0]));
        if !ok {
            continue;
        }
        b.output = Some(s[0]);
        b.start = Some((k, "use".into()));
        tried += 1;
        let hs: Vec<String> = hops.iter().map(|h| h.to_string()).collect();
        let how = format!("ring of {n} calibrated sensors + amethyst, hops [{}], kick: the note block behind the first", hs.join(","));
        if let Some(c) = b.clock("cand") {
            cands.push((c, how.clone()));
        }
        if cands.len() >= a.max_embeddings {
            break;
        }
    }
    let found = simulate_all(cands, &a.out);
    println!("ring: {tried} embeddings simulated, {} clocks", found.len());
    report(found, &a.out)
}
