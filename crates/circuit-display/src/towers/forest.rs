//! The torch forest: a wall torch hung on a tower block starts the neighbouring column one level
//! up (its block, strongly powered by the wall torch, is a new tower's base: the source's value
//! inverted). No dust: towers may touch. A straight line of repeaters from a tower block into a
//! free block starts a column at the same level with the same value (horizontal transport for
//! free). [`grow`] finds the lowest chain of such moves from any tower to a target column, relay
//! columns in between.

use super::*;
use std::collections::{BTreeMap, HashSet, VecDeque};

type S = (i32, i32, i32, u16);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Move {
    Source,
    Branch,
    Climb,
    /// A repeater line of n cells ended in this block.
    Line(i32),
}

/// Grow the tower at `target` showing `f` at levels `res` mod 4; `avoid`: columns kept for later
/// towers; bases at most `y_max`, columns within `bounds` (x min, x max, z min, z max); `lines`: the longest repeater line (0:
/// none).
#[allow(clippy::too_many_arguments)]
pub(super) fn grow(w: &mut World, f: u16, target: (i32, i32), label: &str, avoid: &HashSet<(i32, i32)>, y_max: i32, bounds: [i32; 4], res: i32, top: i32, lines: i32) -> Result<usize> {
    // a free cell, kept for no later tower, beside no dust (a tower block or torch would power it)
    let free = |w: &World, x: i32, z: i32, y: i32| -> bool {
        !w.cells.contains_key(&[x, y, z])
            && x >= bounds[0]
            && x <= bounds[1]
            && z >= bounds[2]
            && z <= bounds[3]
            && (!avoid.contains(&(x, z)) || (x, z) == target)
            && H.iter().all(|h| !matches!(w.get([x + h[0], y, z + h[1]]), Some(K::Dust | K::Corner | K::Lamp)))
    };
    // a repeater's cell: free, and its support free (not over a torch) or a plain stone
    let rep_ok = |w: &World, x: i32, z: i32, y: i32| -> bool {
        free(w, x, z, y)
            && !avoid.contains(&(x, z))
            && match w.get([x, y - 1, z]) {
                None => y - 1 >= -1 && !matches!(w.get([x, y - 2, z]), Some(K::TTorch(_) | K::Torch | K::WTorch(_))),
                Some(K::Stone) => true,
                _ => false,
            }
    };
    let target_ok = |w: &World, x: i32, z: i32, b: i32, v: u16| -> bool {
        let t = Tower { x, z, b, top: b, s: v, label: String::new(), uses: Vec::new() };
        t.value(res) == f && (b..=top).all(|l| !w.cells.contains_key(&[x, l, z]))
    };
    let mut parent: HashMap<S, (Option<S>, Move)> = HashMap::new();
    // the cells a path to `s` builds (its own blocks, torches, repeaters and their supports)
    fn path_cells(parent: &HashMap<S, (Option<S>, Move)>, s: S) -> HashSet<P> {
        let mut out = HashSet::new();
        let mut cur = s;
        while let Some(&(Some(p), m)) = parent.get(&cur) {
            out.insert([cur.0, cur.2, cur.1]);
            match m {
                Move::Branch => {
                    out.insert([cur.0, cur.2 - 1, cur.1]);
                }
                Move::Climb => {
                    out.insert([cur.0, cur.2 - 1, cur.1]);
                }
                Move::Line(len) => {
                    let h = [(cur.0 - p.0).signum(), (cur.1 - p.1).signum()];
                    for k in 1..=len {
                        out.insert([p.0 + k * h[0], cur.2, p.1 + k * h[1]]);
                        out.insert([p.0 + k * h[0], cur.2 - 1, p.1 + k * h[1]]);
                    }
                }
                Move::Source => {}
            }
            cur = p;
        }
        out
    }
    let mut buckets: BTreeMap<i32, VecDeque<S>> = BTreeMap::new();
    for t in &w.towers {
        for y in t.b.max(0)..=t.top.min(y_max) {
            if (y - t.b) % 2 == 0 {
                let s = (t.x, t.z, y, t.value(y));
                parent.insert(s, (None, Move::Source));
                buckets.entry(y).or_default().push_back(s);
            }
        }
    }
    let mut goal: Option<S> = None;
    'search: while let Some((&y, _)) = buckets.iter().next() {
        let mut queue = buckets.remove(&y).unwrap();
        while let Some(s) = queue.pop_front() {
            let (x, z, _, v) = s;
            let mine = path_cells(&parent, s);
            let near = |c: P| mine.contains(&c);
            for h in H {
                // a branch: a wall torch beside, the column one level up
                let (nx, nz) = (x + h[0], z + h[1]);
                if free(w, nx, nz, y) && free(w, nx, nz, y + 1) && !near([nx, y, nz]) && !near([nx, y + 1, nz]) {
                    let n = (nx, nz, y + 1, !v);
                    if (nx, nz) == target {
                        if target_ok(w, nx, nz, y + 1, !v) {
                            parent.insert(n, (Some(s), Move::Branch));
                            goal = Some(n);
                            break 'search;
                        }
                    } else if y + 1 <= y_max && !parent.contains_key(&n) {
                        parent.insert(n, (Some(s), Move::Branch));
                        buckets.entry(y + 1).or_default().push_back(n);
                    }
                }
                // a repeater line along h into a block at the same level
                for len in 1..=lines {
                    let (cx, cz) = (x + len * h[0], z + len * h[1]);
                    if !rep_ok(w, cx, cz, y) || near([cx, y, cz]) || near([cx, y - 1, cz]) {
                        break;
                    }
                    let (ex, ez) = (x + (len + 1) * h[0], z + (len + 1) * h[1]);
                    if near([ex, y, ez]) || near([ex, y + 1, ez]) {
                        continue;
                    }
                    let n = (ex, ez, y, v);
                    if (ex, ez) == target {
                        if target_ok(w, ex, ez, y, v) {
                            parent.insert(n, (Some(s), Move::Line(len)));
                            goal = Some(n);
                            break 'search;
                        }
                        continue;
                    }
                    if free(w, ex, ez, y) && !parent.contains_key(&n) {
                        parent.insert(n, (Some(s), Move::Line(len)));
                        queue.push_back(n);
                    }
                }
            }
            // a relay column climbs on
            if parent.get(&s).is_some_and(|p| p.1 != Move::Source) && y + 2 <= y_max && free(w, x, z, y + 1) && free(w, x, z, y + 2) && !near([x, y + 1, z]) && !near([x, y + 2, z]) {
                let n = (x, z, y + 2, !v);
                if !parent.contains_key(&n) {
                    parent.insert(n, (Some(s), Move::Climb));
                    buckets.entry(y + 2).or_default().push_back(n);
                }
            }
        }
    }
    if goal.is_none() && std::env::var("DISPLAY_DEBUG").as_deref() == Ok("2") {
        eprintln!("{label} -> {target:?} (want {f:04x} at {res} mod 4, bases <= {y_max}):");
        for l in -1..=y_max.min(4) {
            eprint!("{}", w.ascii(l));
        }
    }
    let goal = goal.ok_or_else(|| anyhow!("no branch chain reaches {label}"))?;
    let mut path = vec![goal];
    while let Some((Some(p), _)) = parent.get(path.last().unwrap()) {
        path.push(*p);
    }
    path.reverse();
    let mv = |s: &S| parent[s].1;
    // the source tower's branch block
    let src = path[0];
    if let Some(i) = w.towers.iter().position(|t| t.x == src.0 && t.z == src.1 && t.b <= src.2 && t.top >= src.2) {
        w.towers[i].uses.push(src.2);
    }
    // one new column per run of climbs
    let mut i = 1;
    let mut id = 0;
    while i < path.len() {
        let start = path[i];
        let prev = path[i - 1];
        let mut j = i;
        while j + 1 < path.len() && mv(&path[j + 1]) == Move::Climb {
            j += 1;
        }
        let last = path[j];
        let is_target = (start.0, start.1) == target;
        let top = if is_target { top } else { last.2 };
        let name = if is_target { label.to_string() } else { format!("{label}~") };
        // the move into this column
        match mv(&start) {
            Move::Line(len) => {
                let h = [(start.0 - prev.0).signum(), (start.1 - prev.1).signum()];
                for k in 1..=len {
                    let c = [prev.0 + k * h[0], start.2, prev.1 + k * h[1]];
                    w.cells.insert(c, K::Rep(dir_name((-h[0], -h[1]))));
                    w.cells.entry([c[0], c[1] - 1, c[2]]).or_insert(K::Stone);
                }
            }
            _ => {
                let dir = dir_name((start.0 - prev.0, start.1 - prev.1));
                w.cells.insert([start.0, start.2 - 1, start.1], K::WTorch(dir));
            }
        }
        id = w.put_tower_raw(start.0, start.1, start.2, top, start.3, &name)
            .ok_or_else(|| anyhow!("{label}: branch column {:?} is taken", (start.0, start.1)))?;
        if !is_target {
            w.towers[id].uses.push(last.2);
        }
        i = j + 1;
    }
    Ok(id)
}
