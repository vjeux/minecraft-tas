//! The torch forest: a wall torch hung on a tower block starts the neighbouring column one level
//! up (its block, strongly powered by the wall torch, is a new tower's base: the source's value
//! inverted). No dust: towers may touch. [`grow`] finds the lowest chain of such branches from any
//! tower to a target column, relay columns in between.

use super::*;
use std::collections::{BTreeMap, HashSet};

type S = (i32, i32, i32, u16);

/// Grow the tower at `target` showing `f` at levels 0 mod 4; `avoid`: columns kept for later
/// towers; bases at most `y_max`, columns within `bound`.
pub(super) fn grow(w: &mut World, f: u16, target: (i32, i32), label: &str, avoid: &HashSet<(i32, i32)>, y_max: i32, bound: i32, res: i32, top: i32) -> Result<usize> {
    // a free cell, kept for no later tower, beside no dust (a tower block or torch would power it)
    let free = |w: &World, x: i32, z: i32, y: i32| -> bool {
        !w.cells.contains_key(&[x, y, z])
            && x.abs() <= bound
            && z.abs() <= bound
            && (!avoid.contains(&(x, z)) || (x, z) == target)
            && H.iter().all(|h| !matches!(w.get([x + h[0], y, z + h[1]]), Some(K::Dust | K::Corner | K::Lamp)))
    };
    // parent: state -> (previous state, true when it is a relay column's own block)
    let mut parent: HashMap<S, (Option<S>, bool)> = HashMap::new();
    let mut buckets: BTreeMap<i32, Vec<S>> = BTreeMap::new();
    for t in &w.towers {
        for y in t.b.max(0)..=t.top.min(y_max) {
            if (y - t.b) % 2 == 0 {
                let s = (t.x, t.z, y, t.value(y));
                parent.insert(s, (None, false));
                buckets.entry(y).or_default().push(s);
            }
        }
    }
    let mut goal: Option<S> = None;
    'search: while let Some((&y, _)) = buckets.iter().next() {
        let states = buckets.remove(&y).unwrap();
        for s in states {
            let (x, z, _, v) = s;
            for h in H {
                let (nx, nz) = (x + h[0], z + h[1]);
                if !free(w, nx, nz, y) || !free(w, nx, nz, y + 1) {
                    continue;
                }
                let n = (nx, nz, y + 1, !v);
                if (nx, nz) == target {
                    let t = Tower { x: nx, z: nz, b: y + 1, top: y + 1, s: !v, label: String::new(), uses: Vec::new() };
                    // the target column itself (designed to stand beside the rings) only needs room
                    if t.value(res) == f && (y..=top).all(|l| !w.cells.contains_key(&[nx, l, nz])) {
                        parent.insert(n, (Some(s), true));
                        goal = Some(n);
                        break 'search;
                    }
                    continue;
                }
                if y + 1 <= y_max && !parent.contains_key(&n) {
                    parent.insert(n, (Some(s), true));
                    buckets.entry(y + 1).or_default().push(n);
                }
            }
            // a relay column climbs on
            if parent.get(&s).is_some_and(|p| p.1) && y + 2 <= y_max && free(w, x, z, y + 1) && free(w, x, z, y + 2) {
                let n = (x, z, y + 2, !v);
                if !parent.contains_key(&n) {
                    parent.insert(n, (Some(s), true));
                    buckets.entry(y + 2).or_default().push(n);
                }
            }
        }
    }
    let goal = goal.ok_or_else(|| anyhow!("no branch chain reaches {label}"))?;
    let mut path = vec![goal];
    while let Some((Some(p), _)) = parent.get(path.last().unwrap()) {
        path.push(*p);
    }
    path.reverse();
    // the source tower's branch block
    let src = path[0];
    if let Some(i) = w.towers.iter().position(|t| t.x == src.0 && t.z == src.1 && t.b <= src.2 && t.top >= src.2) {
        w.towers[i].uses.push(src.2);
    }
    // one new column per run of states in the same column
    let mut i = 1;
    let mut id = 0;
    while i < path.len() {
        let start = path[i];
        let prev = path[i - 1];
        let mut j = i;
        while j + 1 < path.len() && (path[j + 1].0, path[j + 1].1) == (start.0, start.1) {
            j += 1;
        }
        let last = path[j];
        let is_target = (start.0, start.1) == target;
        let top = if is_target { top } else { last.2 };
        let name = if is_target { label.to_string() } else { format!("{label}~") };
        id = if is_target { w.put_tower_raw(start.0, start.1, start.2, top, start.3, &name) } else { w.put_tower(start.0, start.1, start.2, top, start.3, &name) }
            .ok_or_else(|| anyhow!("{label}: branch column {:?} is taken", (start.0, start.1)))?;
        if !is_target {
            w.towers[id].uses.push(last.2);
        }
        let dir = dir_name((start.0 - prev.0, start.1 - prev.1));
        w.cells.insert([start.0, start.2 - 1, start.1], K::WTorch(dir));
        i = j + 1;
    }
    Ok(id)
}
