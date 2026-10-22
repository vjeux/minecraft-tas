//! `circuit fly elevator --floors 3`: the multi-floor elevator generator (track fly, coordinator
//! 2026-10-21; judged by circuit::elevator as `elevator-floors N S`). Destination buttons: every
//! floor has a button per floor; the car rests at the floor of the last press.
//!
//! Layout (north-centric; the machine column at x = 0..2, z = 0): the v8 engine with M_s (a stone
//! the slime drags) at (0,1,-1) and X (a stone the upper honey drags) at (1,3,-1). Push steps: up =
//! A pushes the top part, down = s pushes the bottom part; a stop must block a push. Middle floor
//! (offset m): up-stop = a piston head above X ((1,m+4,-2) facing south), guarded by obsidian at
//! (1,m+4,1) (a head extended while X passes shoves X into H2, and H2 into the obsidian: the push
//! fails); down-stop = a head below M_s ((0,m,-2)), guarded by obsidian at (0,m,1); up-flag in at
//! (2,m+1,0) (its back into A's dock cell), down-flag in at (-1,m+2,0) (into s's dock cell).

use mctas::design::Design;

type Pos = [i32; 3];

const OBS: &str = "minecraft:obsidian";
const STONE: &str = "minecraft:stone";
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const BULB: &str = "minecraft:waxed_copper_bulb[lit=false,powered=false]";

pub fn rep(facing: &str, delay: u8) -> String {
    format!("minecraft:repeater[delay={delay},facing={facing},locked=false,powered=false]")
}
pub fn cmp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
}
pub fn obs(facing: &str) -> String {
    format!("minecraft:observer[facing={facing},powered=false]")
}
pub fn sticky(facing: &str) -> String {
    format!("minecraft:sticky_piston[extended=false,facing={facing}]")
}
pub fn piston(facing: &str) -> String {
    format!("minecraft:piston[extended=false,facing={facing}]")
}

pub struct B {
    pub d: Design,
    pub stations: Vec<Pos>,
}

impl B {
    pub fn new(name: String) -> B {
        let mut b = B { d: Design::default(), stations: Vec::new() };
        b.d.name = name;
        b
    }
    pub fn put(&mut self, p: Pos, s: &str) {
        if let Some(old) = self.d.blocks.insert(p, s.to_string()) {
            panic!("elevator: cell {p:?} taken twice ({old} and {s})");
        }
    }
    pub fn st(&mut self, p: Pos, s: &str) {
        self.put(p, s);
        self.stations.push(p);
    }
    /// A station cell that is empty at rest (a part moves there).
    pub fn free(&mut self, p: Pos) {
        self.stations.push(p);
    }
    pub fn on(&mut self, p: Pos, s: &str) {
        self.st(p, s);
        self.st([p[0], p[1] - 1, p[2]], STONE);
    }
    pub fn dust(&mut self, p: Pos) {
        self.on(p, DUST);
    }
    pub fn lever(&mut self, p: Pos, name: &str) {
        self.on(p, "minecraft:lever[face=floor,facing=north,powered=false]");
        self.d.ports.insert(format!("input.{name}"), p);
        self.d.meta.insert(format!("port input.{name}"), "lever".into());
    }
    pub fn finish(mut self) -> Design {
        self.stations.sort();
        self.stations.dedup();
        for (k, p) in self.stations.iter().enumerate() {
            self.d.ports.insert(format!("station.{k}"), *p);
        }
        self.d
    }
}

/// The machine at the bottom floor (north-centric: M_s and X on the north side).
pub fn machine(b: &mut B) {
    for (p, st) in [
        ([0, 0, 0], obs("down")),
        ([0, 1, 0], "minecraft:slime_block".into()),
        ([1, 1, 0], sticky("up")),
        ([0, 2, 0], sticky("down")),
        ([1, 2, 0], "minecraft:honey_block".into()),
        ([0, 3, 0], obs("up")),
        ([1, 3, 0], "minecraft:honey_block".into()),
        ([2, 3, 0], STONE.into()),
        ([0, 1, -1], STONE.into()),
        ([1, 3, -1], STONE.into()),
    ] {
        b.put(p, &st);
    }
}

/// A middle floor at offset m: the guarded stops and the two flags (their pistons' cells
/// returned: up-stop, down-stop, up-flag, down-flag).
pub fn middle(b: &mut B, m: i32) -> [Pos; 4] {
    b.st([1, m + 4, -2], &piston("south"));
    b.st([1, m + 4, 1], OBS);
    b.free([1, m + 4, -1]);
    b.st([0, m, -2], &piston("south"));
    b.st([0, m, 1], OBS);
    b.free([0, m, -1]);
    b.st([2, m + 1, -1], &obs("east"));
    b.st([2, m + 1, -2], &sticky("south"));
    b.free([2, m + 1, 0]);
    b.st([-1, m + 2, -1], &obs("west"));
    b.st([-1, m + 2, -2], &sticky("south"));
    b.free([-1, m + 2, 0]);
    [[1, m + 4, -2], [0, m, -2], [2, m + 1, -2], [-1, m + 2, -2]]
}

/// The bottom flag (in at (2,1,0), its back into A) and its piston at (2,1,-2) (returned).
pub fn bottom_flag(b: &mut B) -> Pos {
    b.st([2, 1, -1], &obs("east"));
    b.st([2, 1, -2], &sticky("south"));
    b.free([2, 1, 0]);
    [2, 1, -2]
}

/// The top flag at (0,h+3,-1) facing north (its back into o's dock cell: s below it fires by
/// quasi-connectivity), pushed out to (0,h+4,-1) by its piston at (0,h+2,-1) (returned) while
/// the target is the top; its arrival relay: an observer chain from M_s's top dock cell.
pub fn top_flag(b: &mut B, h: i32) -> Pos {
    for (p, st) in [
        ([-1, h + 1, -1], obs("east")),
        ([-1, h + 2, -1], obs("down")),
        ([-1, h + 3, -1], obs("down")),
        ([-1, h + 3, -2], obs("south")),
        ([0, h + 3, -2], obs("west")),
        ([0, h + 3, -1], obs("north")),
        ([0, h + 2, -1], sticky("up")),
    ] {
        b.st(p, &st);
    }
    b.free([0, h + 4, -1]);
    [0, h + 2, -1]
}

/// The 3-floor mechanism with levers (ideal inputs): `b` bottom flag, `t` holds the top flag
/// out, `us`/`ds` the middle stops, `uf`/`df` the middle flags.
pub fn levers3(s: i32) -> Design {
    let (m, h) = (s, 2 * s);
    let mut b = B::new(format!("fly-elevator3-levers-{s}"));
    machine(&mut b);
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    let fb = bottom_flag(&mut b);
    b.lever([fb[0] + 1, fb[1], fb[2]], "b");
    let ft = top_flag(&mut b, h);
    b.lever([ft[0], ft[1], ft[2] - 1], "t");
    let [us, ds, uf, df] = middle(&mut b, m);
    b.lever([us[0], us[1], us[2] - 1], "us");
    b.lever([ds[0], ds[1], ds[2] - 1], "ds");
    b.lever([uf[0], uf[1], uf[2] - 1], "uf");
    b.lever([df[0], df[1], df[2] - 1], "df");
    b.d.meta.insert("circuit".into(), "flying-machine".into());
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.finish()
}

/// Floor f's destination buttons (x = 1, 2, 3 for floors 0, 1, 2) at (x, f+4, -4), each on a
/// copper bulb with a comparator and an observer: one pulse per press however long it is held.
/// The bottom floor's pulses climb observer chains (x, y, -7), the top floor's come down them; all
/// three floors meet in the merge block M_x at (x, m+3, -7).
pub fn buttons(b: &mut B, s: i32) {
    let (m, h) = (s, 2 * s);
    for (k, f) in [0, m, h].into_iter().enumerate() {
        for x in 1..=3 {
            let d = x - 1;
            b.st([x, f + 4, -4], "minecraft:stone_button[face=floor,facing=north,powered=false]");
            b.d.ports.insert(format!("input.{k}-{d}"), [x, f + 4, -4]);
            b.d.meta.insert(format!("port input.{k}-{d}"), "button".into());
            b.st([x, f + 3, -4], BULB);
            b.on([x, f + 3, -5], &cmp("south", "compare"));
            b.st([x, f + 3, -6], &obs("south"));
        }
    }
    for x in 1..=3 {
        // bottom: a side observer, then the chain up to the merge block
        b.st([x, 3, -7], &obs("south"));
        for y in 4..=m + 2 {
            b.st([x, y, -7], &obs("down"));
        }
        b.st([x, m + 3, -7], STONE);
        // top: a side observer, then the chain down to the merge block
        b.st([x, h + 3, -7], &obs("south"));
        for y in m + 4..=h + 2 {
            b.st([x, y, -7], &obs("up"));
        }
    }
}

/// The target latches: G1 = (target >= 1) at y = m+2, G2 = (target >= 2) at y = m+4, z = -10,
/// each a stone and a redstone block between a reset piston (x = 1) and a set piston (x = 5):
/// reset = free (2), stone (3), block (4); set = stone (2), block (3), free (4). Writes: floor 0
/// resets both, floor 1 sets G1 and resets G2, floor 2 sets both. K0 (1,m+3,-10) between the
/// reset pistons takes floor 0's pulses, K2 (5,m+3,-10) between the set pistons floor 2's; floor
/// 1's hops up an observer chain, crosses over at m+7 and comes down to PS1's and PR2's north
/// faces.
pub fn latches(b: &mut B, s: i32) {
    let m = s;
    for (y, _) in [(m + 2, "G1"), (m + 4, "G2")] {
        b.st([1, y, -10], &piston("east"));
        b.free([2, y, -10]);
        b.st([3, y, -10], STONE);
        b.st([4, y, -10], "minecraft:redstone_block");
        b.st([5, y, -10], &piston("west"));
    }
    // floor 0 -> K0
    b.on([1, m + 3, -8], &rep("south", 1));
    b.on([1, m + 3, -9], &rep("south", 1));
    b.st([1, m + 3, -10], STONE);
    // floor 2 -> K2
    b.on([4, m + 3, -7], &rep("west", 1));
    b.dust([5, m + 3, -7]);
    b.dust([5, m + 3, -8]);
    b.on([5, m + 3, -9], &rep("south", 1));
    b.st([5, m + 3, -10], STONE);
    // floor 1: up, over, down
    b.on([2, m + 3, -8], &rep("south", 1));
    b.st([2, m + 3, -9], &obs("south"));
    for y in m + 4..=m + 6 {
        b.st([2, y, -9], &obs("down"));
    }
    b.st([2, m + 7, -9], STONE);
    b.on([2, m + 7, -10], &rep("south", 1));
    for p in [[2, m + 7, -11], [2, m + 7, -12], [3, m + 7, -12], [4, m + 7, -12]] {
        b.dust(p);
    }
    // (the crossing's end repeaters stand on the observers that watch them)
    b.st([1, m + 7, -12], &rep("east", 1));
    b.st([5, m + 7, -12], &rep("west", 1));
    for y in m + 4..=m + 6 {
        b.st([1, y, -12], &obs("up"));
    }
    b.st([1, m + 4, -11], &obs("north"));
    for y in m + 2..=m + 6 {
        b.st([5, y, -12], &obs("up"));
    }
    b.st([5, m + 2, -11], &obs("north"));
}

/// Step B: the mechanism with the buttons and the latches (the stops and flags still levers).
pub fn step_b(s: i32) -> Design {
    let (m, h) = (s, 2 * s);
    let mut b = B::new(format!("fly-elevator3-b-{s}"));
    machine(&mut b);
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    bottom_flag(&mut b);
    top_flag(&mut b, h);
    middle(&mut b, m);
    buttons(&mut b, s);
    latches(&mut b, s);
    b.d.meta.insert("circuit".into(), format!("elevator-floors 3 {s}"));
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.d.ports.insert("rider".into(), [2, 4, 0]);
    b.finish()
}

const DIRS: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn neg(a: Pos) -> Pos {
    [-a[0], -a[1], -a[2]]
}
fn dir_name(d: Pos) -> &'static str {
    match d {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}

/// The cells no logic block may take: the machine's path (every cell any machine block crosses
/// between the bottom floor and offset `h`) and every cell beside a slime or honey block's path
/// (a movable block there is dragged along), plus the cells above the platform (the rider).
pub fn keepout(h: i32) -> std::collections::HashSet<Pos> {
    let mut k = std::collections::HashSet::new();
    let machine: [(Pos, bool); 10] = [([0, 0, 0], false), ([0, 1, 0], true), ([1, 1, 0], false), ([0, 2, 0], false), ([1, 2, 0], true), ([0, 3, 0], false), ([1, 3, 0], true), ([2, 3, 0], false), ([0, 1, -1], false), ([1, 3, -1], false)];
    for (p, sticky) in machine {
        for y in 0..=h {
            let c = [p[0], p[1] + y, p[2]];
            k.insert(c);
            if sticky {
                for d in DIRS {
                    k.insert(add(c, d));
                }
            }
        }
    }
    for y in 4..=h + 6 {
        k.insert([2, y, 0]);
    }
    k
}

/// Route an observer chain from `source` (any block whose state changes: the first observer
/// watches it) to `target` (a piston or block the last observer's back points into). Each observer
/// faces the previous cell; at a turn its back powers the cell straight ahead, which must be empty
/// and is reserved. Avoids `avoid`, taken cells, and cells some existing observer watches.
/// Returns the cells placed; panics when no route exists within the box `lo..=hi`.
pub fn route_chain(b: &mut B, avoid: &std::collections::HashSet<Pos>, reserved: &mut std::collections::HashSet<Pos>, source: Pos, target: Pos, front: Option<Pos>, lo: Pos, hi: Pos) -> Option<Vec<Pos>> {
    use std::collections::{HashMap, HashSet, VecDeque};
    // an observer's back powers one cell: it must not be a piston other than the target, nor the
    // cell above one (quasi-connectivity)
    let quiet = |o: Pos| {
        let piston = |p: Pos| b.d.blocks.get(&p).is_some_and(|s| s.contains("piston"));
        o == target || (!piston(o) && !piston(add(o, [0, -1, 0])))
    };
    let inside = |c: Pos| (0..3).all(|i| c[i] >= lo[i] && c[i] <= hi[i]);
    let watched: HashSet<Pos> = b
        .d
        .blocks
        .iter()
        .filter(|(_, s)| s.contains("observer"))
        .map(|(p, s)| {
            let f = s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("north");
            let d = match f {
                "east" => [1, 0, 0],
                "west" => [-1, 0, 0],
                "up" => [0, 1, 0],
                "down" => [0, -1, 0],
                "south" => [0, 0, 1],
                _ => [0, 0, -1],
            };
            add(*p, d)
        })
        .collect();
    let free = |c: Pos, b: &B, reserved: &HashSet<Pos>| inside(c) && !b.d.blocks.contains_key(&c) && !avoid.contains(&c) && !reserved.contains(&c) && !watched.contains(&c) && c != target && c != source;
    // state: (cell, incoming direction index)
    let mut prev: HashMap<(Pos, usize), (Pos, usize)> = HashMap::new();
    let mut q: VecDeque<(Pos, usize)> = VecDeque::new();
    for (i, d) in DIRS.iter().enumerate() {
        let c = add(source, *d);
        if free(c, b, reserved) && quiet(add(c, *d)) {
            q.push_back((c, i));
            prev.insert((c, i), (source, 99));
        }
    }
    let mut goal = None;
    while let Some((c, i)) = q.pop_front() {
        if add(c, DIRS[i]) == target && Some(c) != front {
            goal = Some((c, i));
            break;
        }
        for (j, d) in DIRS.iter().enumerate() {
            if *d == neg(DIRS[i]) {
                continue;
            }
            let n = add(c, *d);
            if !free(n, b, reserved) || prev.contains_key(&(n, j)) || !quiet(add(n, *d)) {
                continue;
            }
            if j != i {
                // a turn: this observer's back powers the straight-ahead cell
                let ahead = add(c, DIRS[i]);
                if !free(ahead, b, reserved) && ahead != n {
                    continue;
                }
            }
            prev.insert((n, j), (c, i));
            q.push_back((n, j));
        }
    }
    let Some(mut s) = goal else { return None };
    let mut path = Vec::new();
    loop {
        path.push(s);
        let p = prev[&s];
        if p.1 == 99 {
            break;
        }
        s = p;
    }
    path.reverse();
    let mut cells = Vec::new();
    for (k, (c, i)) in path.iter().enumerate() {
        b.st(*c, &obs(dir_name(neg(DIRS[*i]))));
        cells.push(*c);
        // a turn after this cell: reserve the straight-ahead cell
        if let Some((_, j)) = path.get(k + 1)
            && j != i
        {
            reserved.insert(add(*c, DIRS[*i]));
        }
    }
    Some(cells)
}

/// Step C: local copies of the target for each flag, written by observer chains from the master
/// latch's pistons (their motion is the write pulse: S1/R1/S2/R2); the middle stops still levers.
/// Top: a G2 slider whose redstone block feeds a repeater into the top flag's piston. Bottom: a
/// G1 slider moves a stone next to the bottom flag's piston; the bottom presence (an observer on
/// M_s's dock toggling a bulb, a comparator) powers that stone through a 6 gt repeater. Middle:
/// the up-flag piston beside a stone a G2 slider moves in, the down-flag piston beside a stone a
/// G1 slider moves out (in = target 0); both stones fed by the middle presence delayed 16 gt.
pub fn step_c(s: i32) -> Design {
    let (m, h) = (s, 2 * s);
    let mut b = B::new(format!("fly-elevator3-c-{s}"));
    machine(&mut b);
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    let fb = bottom_flag(&mut b);
    let ft = top_flag(&mut b, h);
    let [us, ds, fu, fd] = middle(&mut b, m);
    buttons(&mut b, s);
    latches(&mut b, s);
    // taps on the master latch's pistons
    let (t_s1, t_r1, t_s2, t_r2) = ([5, m + 1, -10], [1, m + 1, -10], [5, m + 5, -10], [1, m + 5, -10]);
    b.st(t_s1, &obs("up"));
    b.st(t_r1, &obs("up"));
    b.st(t_s2, &obs("down"));
    b.st(t_r2, &obs("down"));
    // top copy (G2): free (-1), stone (0), block (1) between PR_t (-2) and PS_t (2)
    let (pr_t, ps_t) = ([-2, h + 2, -3], [2, h + 2, -3]);
    b.st(pr_t, &piston("east"));
    b.free([-1, h + 2, -3]);
    b.st([0, h + 2, -3], STONE);
    b.st([1, h + 2, -3], "minecraft:redstone_block");
    b.st(ps_t, &piston("west"));
    b.on([ft[0], ft[1], ft[2] - 1], &rep("north", 1));
    // middle up-flag unit: c_u beside its piston, a stone a vertical G2 slider moves in
    let c_u = [fu[0], fu[1], fu[2] - 1];
    let (r_u, s_u) = ([c_u[0], c_u[1] - 1, c_u[2]], [c_u[0], c_u[1] + 2, c_u[2]]);
    b.free(c_u);
    b.st(r_u, &piston("up"));
    b.st([c_u[0], c_u[1] + 1, c_u[2]], STONE);
    b.st(s_u, &piston("down"));
    // middle down-flag unit: c_d beside its piston holds the stone at target 0
    let c_d = [fd[0], fd[1], fd[2] - 1];
    let (o_d, i_d) = ([c_d[0], c_d[1] - 1, c_d[2]], [c_d[0], c_d[1] + 2, c_d[2]]);
    b.st(c_d, STONE);
    b.st(o_d, &piston("up"));
    b.free([c_d[0], c_d[1] + 1, c_d[2]]);
    b.st(i_d, &piston("down"));
    // bottom unit: c_b beside the bottom flag's piston; stone in (from (3,1,-3)) at G1
    let c_b = [fb[0], fb[1], fb[2] - 1];
    let (o_b, i_b) = ([c_b[0] - 1, c_b[1], c_b[2]], [c_b[0] + 2, c_b[1], c_b[2]]);
    b.free(c_b);
    b.st(o_b, &piston("east"));
    b.st([c_b[0] + 1, c_b[1], c_b[2]], STONE);
    b.st(i_b, &piston("west"));
    // bottom presence
    b.st([0, 1, -2], &obs("south"));
    b.st([0, 1, -3], "minecraft:waxed_copper_bulb[lit=true,powered=false]");
    b.on([0, 1, -4], &cmp("south", "compare"));
    for p in [[0, 1, -5], [1, 1, -5], [2, 1, -5]] {
        b.dust(p);
    }
    b.on([2, 1, -4], &rep("north", 3));
    // middle presence and the 16 gt delays into c_u and c_d
    b.st([0, m + 1, -2], &obs("south"));
    b.st([0, m + 1, -3], BULB);
    b.on([0, m + 1, -4], &cmp("south", "compare"));
    b.dust([0, m + 1, -5]);
    b.on([1, m + 1, -5], &rep("west", 4));
    b.dust([2, m + 1, -5]);
    b.on([2, m + 1, -4], &rep("north", 4));
    b.on([-1, m + 1, -5], &rep("east", 4));
    b.dust([-2, m + 2, -5]);
    b.dust([-3, m + 2, -5]);
    b.dust([-3, m + 2, -4]);
    b.dust([-3, m + 2, -3]);
    b.on([-2, m + 2, -3], &rep("west", 4));
    // the middle stops on levers for now
    b.lever([us[0] - 1, us[1], us[2]], "us");
    b.lever([ds[0] - 1, ds[1], ds[2]], "ds");
    // the chains
    let mut avoid = keepout(h);
    for p in &b.stations {
        avoid.insert(*p);
    }
    let mut reserved = std::collections::HashSet::new();
    let (lo, hi) = ([-9, 0, -16], [10, h + 6, -1]);
    let front = |p: Pos, d: Pos| Some(add(p, d));
    let jobs = [(t_s1, o_d, [0, 1, 0]), (t_r1, i_d, [0, -1, 0]), (t_s2, s_u, [0, -1, 0]), (t_r2, r_u, [0, 1, 0]), (t_s1, i_b, [-1, 0, 0]), (t_r1, o_b, [1, 0, 0]), (t_s2, ps_t, [-1, 0, 0]), (t_r2, pr_t, [1, 0, 0])];
    for (src, dst, f) in jobs {
        if route_chain(&mut b, &avoid, &mut reserved, src, dst, front(dst, f), lo, hi).is_none() {
            panic!("elevator: no observer route from {src:?} to {dst:?}");
        }
    }
    // (a test design: its lever inputs keep it off the elevator judge)
    b.d.meta.insert("circuit".into(), "flying-machine".into());
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.d.ports.insert("rider".into(), [2, 4, 0]);
    b.finish()
}
