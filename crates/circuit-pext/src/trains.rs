//! `mctas circuit pext trains`: pulse-toggle trains for a stack of sticky pistons (an abstract
//! 1D model, no timing): pistons P1 (fixed at 0), P2..Pn at 1..n-1 and the payload at n. A 2 gt
//! pulse on a piston at p: if p+1 is occupied, push the contiguous line from p+1 by one (spit);
//! else if p+2 is occupied, pull it to p+1 (reach-through); else nothing. A train is a sequence of
//! pulsed pistons; it WORKS when it takes the rest state to a state with the payload at 2n (N = n)
//! and the same train takes that state back to rest. For each step the tool prints where the
//! pulsed piston stands on each edge and which static cell sets can deliver it: one cell (same
//! position), both cells at once (simultaneous: harmless when every other piston hit either sits
//! still with nothing to move, or would move a piston extending in the same step), or staggered
//! (first one cell, the other 2 gt later: harmless when the second hits nothing it can move then).

use anyhow::Result;

/// Positions of P2..Pn then the payload (P1 is at 0).
type State = Vec<i32>;

fn occupied(s: &State, c: i32) -> Option<usize> {
    if c == 0 {
        return Some(usize::MAX); // P1
    }
    s.iter().position(|&x| x == c)
}

/// Apply a pulse to piston k (0 = P1, else index k-1 in the state). Returns None when nothing moves.
pub fn pulse(s: &State, k: usize) -> Option<State> {
    let p = if k == 0 { 0 } else { s[k - 1] };
    let mut t = s.clone();
    if occupied(s, p + 1).is_some() {
        // push the contiguous line
        let mut c = p + 1;
        while let Some(i) = occupied(s, c) {
            if i == usize::MAX {
                return None;
            }
            t[i] += 1;
            c += 1;
        }
        Some(t)
    } else if let Some(i) = occupied(s, p + 2) {
        if i == usize::MAX {
            return None;
        }
        t[i] -= 1;
        Some(t)
    } else {
        None
    }
}

fn run(s: &State, train: &[usize]) -> Option<(State, Vec<State>)> {
    let mut cur = s.clone();
    let mut before = Vec::new();
    for &k in train {
        before.push(cur.clone());
        cur = pulse(&cur, k)?;
    }
    Some((cur, before))
}

/// Every train of length <= max over n pistons that works both ways.
pub fn trains(n: usize, max: usize) -> Vec<(Vec<usize>, State)> {
    let rest: State = (1..=n as i32).collect();
    let mut out = Vec::new();
    let mut stack: Vec<Vec<usize>> = vec![vec![]];
    while let Some(t) = stack.pop() {
        if !t.is_empty()
            && let Some((e, _)) = run(&rest, &t)
            && *e.last().unwrap() == 2 * n as i32
            && run(&e, &t).map(|(r, _)| r == rest).unwrap_or(false)
        {
            out.push((t.clone(), e));
        }
        if t.len() < max {
            for k in 0..n {
                if t.last() == Some(&k) {
                    continue;
                }
                let mut u = t.clone();
                u.push(k);
                // prune: must still be applicable from rest
                if run(&rest, &u).is_some() {
                    stack.push(u);
                }
            }
        }
    }
    out.sort_by_key(|(t, _)| (t.len(), t.clone()));
    out
}

fn name(k: usize) -> char {
    (b'A' + k as u8) as char
}

/// Who stands at world cell c (piston index k: 0 = P1, or the payload = n), with the frame shift.
fn at(s: &State, c: i32, shift: i32, n: usize) -> Option<usize> {
    if c - shift == 0 {
        return Some(0);
    }
    s.iter().position(|&x| x + shift == c).map(|i| if i + 1 == n { n } else { i + 1 })
}

/// Is a simultaneous pulse on piston q (state s) harmless while piston k acts in the same tick?
/// Harmless: q does nothing, or q would pull k (k is extending then: the pull fails).
fn harmless_with(s: &State, q: usize, k: usize) -> bool {
    let n = s.len();
    if q == n {
        return true; // the payload
    }
    let qp = if q == 0 { 0 } else { s[q - 1] };
    let kp = if k == 0 { 0 } else { s[k - 1] };
    let free = |c: i32| occupied(s, c).is_none();
    if !free(qp + 1) {
        return false; // a push: order-dependent or harmful
    }
    match occupied(s, qp + 2) {
        None => true,
        Some(_) => qp + 2 == kp,
    }
}

/// How each step can be delivered by static cells: "single", "sim" (both cells at once) or
/// "stagger a>b" (cell a first, b 2 gt later), or "NONE". `shift` = the frame's displacement on
/// the on-edge (rider mode: 1; the off-edge has 0).
pub fn delivery(n: usize, train: &[usize], shift: i32) -> Vec<String> {
    let rest: State = (1..=n as i32).collect();
    let (e, on) = run(&rest, train).unwrap();
    let (_, off) = run(&e, train).unwrap();
    let mut out = Vec::new();
    for (i, &k) in train.iter().enumerate() {
        let pos = |s: &State| if k == 0 { 0 } else { s[k - 1] };
        let (po, pf) = (pos(&on[i]) + shift, pos(&off[i]));
        if po == pf {
            out.push(format!("single x{po}"));
            continue;
        }
        // simultaneous: on-edge cell pf, off-edge cell po
        let q_on = at(&on[i], pf, shift, n);
        let q_off = at(&off[i], po, 0, n);
        let ok_on = q_on.is_none_or(|q| harmless_with(&on[i], q, k));
        let ok_off = q_off.is_none_or(|q| harmless_with(&off[i], q, k));
        if ok_on && ok_off {
            out.push(format!("sim x{po}+x{pf}"));
            continue;
        }
        // staggered: own cell first on one edge means the other edge's cell first on the other;
        // the late pulse is harmless when it hits nothing that acts (the acting piston's moved
        // blocks are busy then)
        let busy_ok = |s: &State, c: i32, sh: i32| -> bool {
            match at(s, c, sh, n) {
                None => true,
                Some(q) if q == n => true,
                Some(q) => {
                    // q acts on the state before k's move landed: harmless if it does nothing or
                    // only touches k or the block k moves
                    let qp = if q == 0 { 0 } else { s[q - 1] };
                    let kp = if k == 0 { 0 } else { s[k - 1] };
                    let free = |c: i32| occupied(s, c).is_none();
                    if !free(qp + 1) {
                        qp + 1 == kp || qp + 1 == kp + 1
                    } else {
                        occupied(s, qp + 2).is_none() || qp + 2 == kp || qp + 2 == kp + 1 || qp + 2 == kp + 2
                    }
                }
            }
        };
        // order "po then pf": on-edge k first (late pulse at pf), off-edge pf... the off-edge's
        // first pulse hits po (someone else) BEFORE k acts: must be a no-op
        let early_ok = |s: &State, c: i32, sh: i32| at(s, c, sh, n).is_none_or(|q| q == n || pulse(s, q).is_none());
        if busy_ok(&on[i], pf, shift) && early_ok(&off[i], po, 0) {
            out.push(format!("stagger x{po}>x{pf}"));
            continue;
        }
        if early_ok(&on[i], pf, shift) && busy_ok(&off[i], po, 0) {
            out.push(format!("stagger x{pf}>x{po}"));
            continue;
        }
        out.push(format!("NONE x{po}/x{pf}"));
    }
    out
}

/// The shortest train (breadth-first over the pair of states it acts on: the on-edge state from
/// rest and the off-edge state from the all-spat extended state) whose every step is deliverable
/// by static cells (`delivery`'s rules), up to `max` pulses. A pulse that moves nothing on one
/// edge is allowed (it hits that edge's piston harmlessly).
pub fn bfs(n: usize, shift: i32, max: usize) -> Option<Vec<usize>> {
    use std::collections::{HashMap, VecDeque};
    let rest: State = (1..=n as i32).collect();
    let ext: State = (1..=n as i32).map(|i| 2 * i).collect();
    let start = (rest.clone(), ext.clone());
    let goal = (ext.clone(), rest.clone());
    let mut prev: HashMap<(State, State), ((State, State), usize)> = HashMap::new();
    let mut depth: HashMap<(State, State), usize> = HashMap::new();
    depth.insert(start.clone(), 0);
    let mut q = VecDeque::new();
    q.push_back(start.clone());
    let bound = 2 * n as i32;
    while let Some(cur) = q.pop_front() {
        let d = depth[&cur];
        if cur == goal {
            let mut t = Vec::new();
            let mut c = cur;
            while let Some((p, k)) = prev.get(&c) {
                t.push(*k);
                c = p.clone();
            }
            t.reverse();
            return Some(t);
        }
        if d >= max {
            continue;
        }
        for k in 0..n {
            let on2 = pulse(&cur.0, k);
            let off2 = pulse(&cur.1, k);
            if on2.is_none() && off2.is_none() {
                continue;
            }
            let on2 = on2.unwrap_or_else(|| cur.0.clone());
            let off2 = off2.unwrap_or_else(|| cur.1.clone());
            if on2.iter().chain(off2.iter()).any(|&x| x > bound) {
                continue;
            }
            if step_delivery(&cur.0, &cur.1, k, shift).starts_with("NONE") {
                continue;
            }
            let next = (on2, off2);
            if !depth.contains_key(&next) {
                depth.insert(next.clone(), d + 1);
                prev.insert(next.clone(), (cur.clone(), k));
                q.push_back(next);
            }
        }
    }
    None
}

/// `delivery` for one step: piston k pulsed with the on-edge state `on` and off-edge state `off`.
pub fn step_delivery(on: &State, off: &State, k: usize, shift: i32) -> String {
    let n = on.len();
    let pos = |s: &State| if k == 0 { 0 } else { s[k - 1] };
    let (po, pf) = (pos(on) + shift, pos(off));
    if po == pf {
        return format!("single x{po}");
    }
    let q_on = at(on, pf, shift, n);
    let q_off = at(off, po, 0, n);
    if q_on.is_none_or(|q| harmless_with(on, q, k)) && q_off.is_none_or(|q| harmless_with(off, q, k)) {
        return format!("sim x{po}+x{pf}");
    }
    let busy_ok = |s: &State, c: i32, sh: i32| -> bool {
        match at(s, c, sh, n) {
            None => true,
            Some(q) if q == n => true,
            Some(q) => {
                let qp = if q == 0 { 0 } else { s[q - 1] };
                let kp = if k == 0 { 0 } else { s[k - 1] };
                let free = |c: i32| occupied(s, c).is_none();
                if !free(qp + 1) {
                    qp + 1 == kp || qp + 1 == kp + 1
                } else {
                    occupied(s, qp + 2).is_none() || qp + 2 == kp || qp + 2 == kp + 1 || qp + 2 == kp + 2
                }
            }
        }
    };
    let early_ok = |s: &State, c: i32, sh: i32| at(s, c, sh, n).is_none_or(|q| q == n || pulse(s, q).is_none());
    if busy_ok(on, pf, shift) && early_ok(off, po, 0) {
        return format!("stagger x{po}>x{pf}");
    }
    if early_ok(on, pf, shift) && busy_ok(off, po, 0) {
        return format!("stagger x{pf}>x{po}");
    }
    format!("NONE x{po}/x{pf}")
}

pub fn report_bfs(n: usize, shift: i32, max: usize) -> Result<()> {
    match bfs(n, shift, max) {
        None => println!("no fully deliverable train for n = {n}, shift {shift}, up to {max} pulses"),
        Some(t) => {
            let word: String = t.iter().map(|&k| name(k)).collect();
            println!("shortest fully deliverable train for n = {n}, shift {shift}: {word} ({} pulses)", t.len());
            let mut on: State = (1..=n as i32).collect();
            let mut off: State = (1..=n as i32).map(|i| 2 * i).collect();
            for (i, &k) in t.iter().enumerate() {
                let d = step_delivery(&on, &off, k, shift);
                println!("  step {}: {}: {}   on {:?} off {:?}", i + 1, name(k), d, on, off);
                on = pulse(&on, k).unwrap_or(on);
                off = pulse(&off, k).unwrap_or(off);
            }
        }
    }
    Ok(())
}

pub fn report(n: usize, max: usize, show: usize, shift: i32) -> Result<()> {
    let rest: State = (1..=n as i32).collect();
    let all = trains(n, max);
    let mut ranked: Vec<(usize, &(Vec<usize>, State), Vec<String>)> = all
        .iter()
        .map(|t| {
            let d = delivery(n, &t.0, shift);
            let bad = d.iter().filter(|s| s.starts_with("NONE")).count();
            (bad, t, d)
        })
        .collect();
    ranked.sort_by_key(|(bad, t, _)| (*bad, t.0.len()));
    println!("{} working trains for n = {n} pistons (N = {n}), length <= {max}, frame shift {shift}", all.len());
    for (bad, (t, e), d) in ranked.iter().take(show) {
        let word: String = t.iter().map(|&k| name(k)).collect();
        println!("train {word}: {} undeliverable steps, extended state {:?}", bad, e);
        let (_, on) = run(&rest, t).unwrap();
        let (_, off) = run(e, t).unwrap();
        for (i, &k) in t.iter().enumerate() {
            let pos = |s: &State| if k == 0 { 0 } else { s[k - 1] };
            println!("  step {}: {} rel {} (on) / {} (off): {}   on {:?} off {:?}", i + 1, name(k), pos(&on[i]), pos(&off[i]), d[i], on[i], off[i]);
        }
    }
    Ok(())
}
