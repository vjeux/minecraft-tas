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
    /// Also every candidate with ONE more block in any free cell touching the build:
    /// sticky,piston,observer,stone (every facing), sculk / csculk (a second sensor: dividers,
    /// frequency filters through a calibrated sensor's back), amethyst, wool. Found the
    /// moving-observer hybrids (P 72-216).
    #[arg(long, default_value = "")]
    extra: String,
    #[command(flatten)]
    out: Out,
}

/// The extra blocks of `--extra`, every facing.
fn extra_states(spec: &str) -> Vec<String> {
    let mut v = Vec::new();
    for w in spec.split(',') {
        for d in DIRS {
            match w {
                "sticky" => v.push(format!("minecraft:sticky_piston[extended=false,facing={}]", dir_name(d))),
                "piston" => v.push(format!("minecraft:piston[extended=false,facing={}]", dir_name(d))),
                "observer" => v.push(format!("minecraft:observer[facing={},powered=false]", dir_name(d))),
                _ => {}
            }
        }
        if w == "stone" {
            v.push(STONE.to_string());
        }
        if w == "sculk" {
            v.push(Sensor::Plain.state([0, 0, 1]));
        }
        if w == "csculk" {
            for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                v.push(Sensor::Cal.state(d));
            }
        }
        if w == "amethyst" {
            v.push("minecraft:amethyst_block".to_string());
        }
        if w == "wool" {
            v.push("minecraft:white_wool".to_string());
        }
    }
    v
}

/// `b` plus one extra block in every free cell touching it.
fn with_extras(b: &Build, states: &[String]) -> Vec<(Build, String)> {
    let mut cells: Vec<Pos> = Vec::new();
    for p in b.blocks.keys() {
        for d in DIRS {
            let q = add(*p, d);
            if b.free(q) && !cells.contains(&q) && b.start.as_ref().is_none_or(|(k, _)| *k != q) {
                cells.push(q);
            }
        }
    }
    let mut out = Vec::new();
    for c in cells {
        for s in states {
            let mut b2 = b.clone();
            if b2.put(c, s.clone()) {
                out.push((b2, format!(" + {} at {},{},{}", clock::short(s), c[0], c[1], c[2])));
            }
        }
    }
    out
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

/// Queue `b` (and, with extras, every one-more-block variant of it); returns how many.
fn push_with_extras(cands: &mut Vec<(clock::Clock, String)>, b: &Build, how: String, extras: &[String]) -> usize {
    let mut n = 0;
    if let Some(c) = b.clock("cand") {
        cands.push((c, how.clone()));
        n += 1;
    }
    if !extras.is_empty() {
        for (b2, tail) in with_extras(b, extras) {
            if let Some(c) = b2.clock("cand") {
                cands.push((c, format!("{how}{tail}")));
                n += 1;
            }
        }
    }
    n
}

pub fn obs(a: &ObsArgs) -> Result<()> {
    let ems = Emitter::parse_list(&a.emitters);
    let extras = extra_states(&a.extra);
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
                    tried += push_with_extras(&mut cands, &b, how, &extras);
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
                            tried += push_with_extras(&mut cands, &b, how, &extras);
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
pub struct RingBigArgs {
    /// cal | plain (plain: radius 8, so hops <= 8, re-arm 40 gt: at least 6 sensors).
    #[arg(long, default_value = "cal")]
    sensor: String,
    /// Sensors in the ring (each with its resonator: amethyst, the first one a note block that
    /// is also the kick).
    #[arg(long)]
    n: usize,
    /// Target hop (floor(distance) from a resonator to the next sensor), <= 16.
    #[arg(long, default_value_t = 15)]
    hop: i32,
    /// Most wool blocks shielding the kick.
    #[arg(long, default_value_t = 2)]
    wool: usize,
    #[command(flatten)]
    out: Out,
}

/// A ring of n calibrated sensors on a circle (side ~ hop): only consecutive sensors are within
/// 16 of each other's resonators, so a big ring keeps its direction by distance; the first
/// sensor's resonator is a note block, right-clicked as the kick. P = the sum of the hops.
pub fn ring_big(a: &RingBigArgs) -> Result<()> {
    let n = a.n;
    let plain = a.sensor == "plain";
    // The listener radius squared (plain 8, calibrated 16).
    let r2 = if plain { 64 } else { 256 };
    let mut cands: Vec<(clock::Clock, String)> = Vec::new();
    // Try a few radii around the ideal one (side = hop + 1: resonator to next sensor = hop).
    let side = a.hop as f64 + 1.0;
    let r0 = side / (2.0 * (std::f64::consts::PI / n as f64).sin());
    for dr in -6..=6 {
        for phase in 0..8 {
            let r = r0 + dr as f64 * 0.25;
            let ph = phase as f64 * 0.1;
            let s: Vec<Pos> = (0..n)
                .map(|i| {
                    let t = ph + 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                    [(r * t.cos()).round() as i32, 0, (r * t.sin()).round() as i32]
                })
                .collect();
            // Resonators: the horizontal neighbor of s[i] nearest s[i+1].
            let mut am: Vec<Pos> = Vec::new();
            for i in 0..n {
                let nx = s[(i + 1) % n];
                let best = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().map(|d| add(s[i], *d)).min_by_key(|q| dist2(*q, nx)).unwrap();
                am.push(best);
            }
            let hops: Vec<i32> = (0..n).map(|i| travel(am[i], s[(i + 1) % n])).collect();
            if (0..n).any(|i| dist2(am[i], s[(i + 1) % n]) > r2) {
                continue;
            }
            let mut b = Build::default();
            let mut ok = true;
            for i in 0..n {
                let res = if i == 0 { "minecraft:note_block[instrument=harp,note=0,powered=false]" } else { "minecraft:amethyst_block" };
                ok &= b.put(am[i], res);
            }
            ok &= b.keep_air(add(am[0], [0, 1, 0]));
            for i in 0..n {
                let facing = if i == 0 { [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().copied().find(|d| add(s[0], *d) != am[0]).unwrap() } else { [0, 0, -1] };
                ok &= b.put(s[i], if plain { Sensor::Plain.state([0, 0, 1]) } else { format!("minecraft:calibrated_sculk_sensor[facing={},power=0,sculk_sensor_phase=inactive,waterlogged=false]", dir_name(neg(facing))) });
            }
            // A resonator must touch only its own sensor.
            for i in 0..n {
                for j in 0..n {
                    if i != j && dist2(am[i], s[j]) == 1 {
                        ok = false;
                    }
                }
            }
            if !ok {
                continue;
            }
            // Shield the kick from every sensor but s[1] (and s[0]).
            let mut lines: Vec<Vec<Pos>> = Vec::new();
            for j in 2..n {
                if dist2(am[0], s[j]) <= r2 {
                    lines.push(segment_cells(am[0], s[j]).into_iter().filter(|c| b.free(*c)).take(3).collect());
                }
            }
            if lines.len() > a.wool || lines.iter().any(|l| l.is_empty()) {
                continue;
            }
            let mut combos: Vec<Vec<Pos>> = vec![Vec::new()];
            for l in &lines {
                combos = combos.iter().flat_map(|c| l.iter().map(move |cell| { let mut c2 = c.clone(); if !c2.contains(cell) { c2.push(*cell); } c2 })).collect();
            }
            b.output = Some(s[0]);
            b.start = Some((am[0], "use".into()));
            let hs: Vec<String> = hops.iter().map(|h| h.to_string()).collect();
            for wools in combos {
                let mut b2 = b.clone();
                for w in &wools {
                    b2.put(*w, "minecraft:white_wool");
                }
                let how = format!("ring of {n} {} sensors on a circle (r {r:.2}), hops [{}], first resonator a note block (the kick), the others amethyst{}", if plain { "plain" } else { "calibrated" }, hs.join(","), if wools.is_empty() { String::new() } else { format!(", {} wool", wools.len()) });
                if let Some(c) = b2.clock("cand") {
                    cands.push((c, how));
                }
            }
        }
    }
    let found = simulate_all(cands, &a.out);
    println!("ring-big: {} clocks", found.len());
    report(found, &a.out)
}

#[derive(clap::Args)]
pub struct RingArgs {
    /// Hop lengths (floor(distance) from each resonator to the next sensor), e.g. 15,15,15;
    /// several tuples separated by ';'.
    #[arg(long, default_value = "")]
    hops: String,
    /// Every hop tuple n:lo:hi (3-rings: consecutive pairs summing to >= 20).
    #[arg(long)]
    tuples: Option<String>,
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
    /// behind (a separate note block behind the first sensor) | note (the first sensor's
    /// resonator is a note block, right-clicked as the kick).
    #[arg(long, default_value = "behind")]
    kick: String,
    /// Most wool blocks shielding the kick from the sensors that must not hear it.
    #[arg(long, default_value_t = 0)]
    wool: usize,
    /// Candidate wool cells tried per shielded line (nearest the kick first).
    #[arg(long, default_value_t = 3)]
    wool_choices: usize,
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
    // Hop tuples: --hops "15,15,15;11,10,14" or --tuples n:lo:hi (every tuple of n hops in lo..=hi
    // whose consecutive pairs sum to >= 20, the steady-state condition of a 3-ring).
    let mut tuples: Vec<Vec<i32>> = Vec::new();
    if let Some(t) = &a.tuples {
        let v: Vec<i32> = t.split(':').map(|s| s.parse()).collect::<Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--tuples n:lo:hi");
        let (n, lo, hi) = (v[0] as usize, v[1], v[2]);
        let mut cur = vec![lo; n];
        loop {
            if (0..n).all(|i| n != 3 || cur[i] + cur[(i + 1) % n] >= 20) {
                tuples.push(cur.clone());
            }
            let mut i = 0;
            while i < n && cur[i] == hi {
                cur[i] = lo;
                i += 1;
            }
            if i == n {
                break;
            }
            cur[i] += 1;
        }
    }
    for h in a.hops.split(';').filter(|s| !s.is_empty()) {
        tuples.push(h.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?);
    }
    let mut rng = Rng(a.seed.max(1) * 0x9E37_79B9_7F4A_7C15);
    let mut cands: Vec<(clock::Clock, String)> = Vec::new();
    let mut tried = 0usize;
    for hops in &tuples {
        tried += ring_cands(a, hops, &mut rng, &mut cands);
    }
    let found = simulate_all(cands, &a.out);
    println!("ring: {} hop tuples, {tried} embeddings simulated, {} clocks", tuples.len(), found.len());
    report(found, &a.out)
}

fn ring_cands(a: &RingArgs, hops: &[i32], rng: &mut Rng, all: &mut Vec<(clock::Clock, String)>) -> usize {
    let n = hops.len();
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
        // The kick. `behind`: a separate note block K behind s[0] (its back), that no other sensor
        // may hear. `note`: s[0]'s own resonator is a note block (s[0] powers it, so it plays when
        // s[0] fires, like an amethyst block resonating) and the kick right-clicks it: s[1] hears
        // it as the first hop, s[0] fires at once (harmless), and every sensor from s[2] on must
        // not hear it. A sensor that would hear the kick gets a wool block on the line
        // (`sculk::is_occluded`), up to --wool of them.
        let (kp, first_quiet) = if a.kick == "note" { (am[0], 2) } else { (add(s[0], sub(s[0], am[0])), 1) };
        if a.kick == "note" {
            b.blocks.insert(am[0], "minecraft:note_block[instrument=harp,note=0,powered=false]".into());
            if !b.keep_air(add(am[0], [0, 1, 0])) {
                continue;
            }
        } else if am.iter().any(|p| dist2(*p, kp) <= 1) || !b.free(kp) {
            continue;
        }
        // s[0] gives no power into its back: a separate kick note block sits there; with the note
        // resonator, the back is any other horizontal side.
        let facing_back = if a.kick == "note" { horiz.iter().copied().find(|d| add(s[0], *d) != am[0]).unwrap() } else { sub(s[0], am[0]) };
        b.put(s[0], format!("minecraft:calibrated_sculk_sensor[facing={},power=0,sculk_sensor_phase=inactive,waterlogged=false]", dir_name(neg(facing_back))));
        for p in &s[1..] {
            ok &= b.put(*p, "minecraft:calibrated_sculk_sensor[facing=north,power=0,sculk_sensor_phase=inactive,waterlogged=false]");
        }
        if a.kick != "note" {
            ok &= b.put(kp, "minecraft:note_block[instrument=harp,note=0,powered=false]") && b.keep_air(add(kp, [0, 1, 0]));
        }
        if !ok {
            continue;
        }
        // Lines to shield, each with its candidate wool cells (nearest the kick first).
        let mut lines: Vec<Vec<Pos>> = Vec::new();
        for j in first_quiet..n {
            if dist2(kp, s[j]) <= 256 {
                let cells: Vec<Pos> = segment_cells(kp, s[j]).into_iter().filter(|c| b.free(*c)).take(a.wool_choices).collect();
                lines.push(cells);
            }
        }
        if lines.len() > a.wool || lines.iter().any(|l| l.is_empty()) {
            continue;
        }
        b.output = Some(s[0]);
        b.start = Some((kp, "use".into()));
        let hs: Vec<String> = hops.iter().map(|h| h.to_string()).collect();
        // Every combination of one wool cell per line.
        let mut combos: Vec<Vec<Pos>> = vec![Vec::new()];
        for l in &lines {
            let mut next = Vec::new();
            for c in &combos {
                for cell in l {
                    let mut c2 = c.clone();
                    if !c2.contains(cell) {
                        c2.push(*cell);
                    }
                    next.push(c2);
                }
            }
            combos = next;
        }
        for wools in combos {
            let mut b2 = b.clone();
            for w in &wools {
                b2.put(*w, "minecraft:white_wool");
            }
            tried += 1;
            let how = format!(
                "ring of {n} calibrated sensors, hops [{}], {}{}",
                hs.join(","),
                if a.kick == "note" { "the first sensor's resonator is a note block (the kick: right-click it)" } else { "kick: a note block behind the first sensor" },
                if wools.is_empty() { String::new() } else { format!(", {} wool shielding the kick", wools.len()) }
            );
            if let Some(c) = b2.clock("cand") {
                cands.push((c, how.clone()));
            }
        }
        if cands.len() >= a.max_embeddings {
            break;
        }
    }
    all.extend(cands);
    tried
}

/// The cells a center-to-center segment crosses, from `a` toward `b` (endpoints excluded).
fn segment_cells(a: Pos, b: Pos) -> Vec<Pos> {
    let mut out: Vec<Pos> = Vec::new();
    let steps = 400;
    for i in 1..steps {
        let t = i as f64 / steps as f64;
        let c = [0, 1, 2].map(|k| (a[k] as f64 + 0.5 + t * (b[k] - a[k]) as f64).floor() as i32);
        if c != a && c != b && !out.contains(&c) {
            out.push(c);
        }
    }
    out
}
