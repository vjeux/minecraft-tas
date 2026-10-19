//! A Boolean algebra the door model is written over once: `Conc` evaluates it on plain bools
//! (the fast reference used by the differential tests against redsim), `Sat` builds a Tseitin
//! circuit in CaDiCaL (the bounded model checker). Same code, two meanings.

use rustsat::solvers::Solve;
use rustsat::types::{Clause, Lit, TernaryVal};
use std::collections::HashMap;

pub trait Alg {
    type B: Copy + PartialEq + std::fmt::Debug;
    fn k(&self, v: bool) -> Self::B;
    fn not(&self, a: Self::B) -> Self::B;
    fn and(&mut self, xs: &[Self::B]) -> Self::B;
    fn or(&mut self, xs: &[Self::B]) -> Self::B {
        let n: Vec<Self::B> = xs.iter().map(|&x| self.not(x)).collect();
        let r = self.and(&n);
        self.not(r)
    }
    fn xor(&mut self, a: Self::B, b: Self::B) -> Self::B;
    fn ite(&mut self, c: Self::B, x: Self::B, y: Self::B) -> Self::B {
        if x == y {
            return x;
        }
        let nc = self.not(c);
        let p = self.and(&[c, x]);
        let q = self.and(&[nc, y]);
        self.or(&[p, q])
    }
    /// A constraint every real door satisfies (its violation means the run is not a door).
    fn require(&mut self, a: Self::B, why: &'static str);
    /// A situation the model does not decide exactly (an order race, an unmodeled case).
    fn flag(&mut self, a: Self::B, why: &'static str);
    fn val(&self, a: Self::B) -> Option<bool>;
}

#[derive(Default)]
pub struct Conc {
    pub violations: Vec<&'static str>,
    pub flags: Vec<&'static str>,
}

impl Alg for Conc {
    type B = bool;
    fn k(&self, v: bool) -> bool {
        v
    }
    fn not(&self, a: bool) -> bool {
        !a
    }
    fn and(&mut self, xs: &[bool]) -> bool {
        xs.iter().all(|&x| x)
    }
    fn or(&mut self, xs: &[bool]) -> bool {
        xs.iter().any(|&x| x)
    }
    fn xor(&mut self, a: bool, b: bool) -> bool {
        a != b
    }
    fn ite(&mut self, c: bool, x: bool, y: bool) -> bool {
        if c { x } else { y }
    }
    fn require(&mut self, a: bool, why: &'static str) {
        if !a && !self.violations.contains(&why) {
            self.violations.push(why);
        }
    }
    fn flag(&mut self, a: bool, why: &'static str) {
        if a && !self.flags.contains(&why) {
            self.flags.push(why);
        }
    }
    fn val(&self, a: bool) -> Option<bool> {
        Some(a)
    }
}

/// What a flag does in the SAT model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagMode {
    /// forbid every flagged situation (the claim then covers race-free doors only)
    Forbid,
    /// record only
    Ignore,
}

pub struct Sat {
    pub s: rustsat_cadical::CaDiCaL<'static, 'static>,
    n: u32,
    tru: Lit,
    cache: HashMap<[u32; 4], Lit>,
    bigcache: HashMap<Box<[u32]>, Lit>,
    xcache: HashMap<(Lit, Lit), Lit>,
    pub clauses: usize,
    pub mode: FlagMode,
    pub flags: Vec<(Lit, &'static str)>,
    /// requirements as literals (kept for reporting, also added as units)
    pub reqs: usize,
}

impl Default for Sat {
    fn default() -> Self {
        Self::new(FlagMode::Forbid)
    }
}

impl Sat {
    pub fn new(mode: FlagMode) -> Sat {
        let mut s = Sat { s: rustsat_cadical::CaDiCaL::default(), n: 1, tru: Lit::positive(0), cache: HashMap::new(), bigcache: HashMap::new(), xcache: HashMap::new(), clauses: 0, mode, flags: Vec::new(), reqs: 0 };
        let t = s.tru;
        s.s.add_unit(t).unwrap();
        s
    }
    pub fn var(&mut self) -> Lit {
        let l = Lit::positive(self.n);
        self.n += 1;
        l
    }
    pub fn vars(&self) -> u32 {
        self.n
    }
    pub fn add(&mut self, c: &[Lit]) {
        if c.iter().any(|&l| l == self.tru) {
            return;
        }
        let v: Clause = c.iter().copied().filter(|&l| l != !self.tru).collect();
        self.s.add_clause(v).unwrap();
        self.clauses += 1;
    }
    pub fn value(&self, l: Lit) -> bool {
        if l == self.tru {
            return true;
        }
        if l == !self.tru {
            return false;
        }
        matches!(self.s.lit_val(l), Ok(TernaryVal::True))
    }
    pub fn tru(&self) -> Lit {
        self.tru
    }
    /// Exactly one of `xs`.
    pub fn exactly_one(&mut self, xs: &[Lit]) {
        self.add(xs);
        self.at_most_one(xs);
    }
    pub fn at_most_one(&mut self, xs: &[Lit]) {
        if xs.len() <= 6 {
            for i in 0..xs.len() {
                for j in i + 1..xs.len() {
                    self.add(&[!xs[i], !xs[j]]);
                }
            }
            return;
        }
        // sequential counter
        let mut prev: Option<Lit> = None;
        for (i, &x) in xs.iter().enumerate() {
            let s = self.var();
            self.add(&[!x, s]);
            if let Some(p) = prev {
                self.add(&[!p, s]);
                self.add(&[!p, !x]);
            }
            if i + 1 == xs.len() {
                break;
            }
            prev = Some(s);
        }
    }
    /// Totalizer outputs: o[j] true when at least j+1 inputs are true (upper-bound direction).
    pub fn totalizer(&mut self, xs: &[Lit]) -> Vec<Lit> {
        if xs.len() <= 1 {
            return xs.to_vec();
        }
        let (a, b) = xs.split_at(xs.len() / 2);
        let a = self.totalizer(a);
        let b = self.totalizer(b);
        let r: Vec<Lit> = (0..a.len() + b.len()).map(|_| self.var()).collect();
        for i in 0..=a.len() {
            for j in 0..=b.len() {
                if i + j == 0 {
                    continue;
                }
                let mut c = vec![r[i + j - 1]];
                if i > 0 {
                    c.push(!a[i - 1]);
                }
                if j > 0 {
                    c.push(!b[j - 1]);
                }
                self.add(&c);
            }
        }
        r
    }
}

impl Alg for Sat {
    type B = Lit;
    fn k(&self, v: bool) -> Lit {
        if v { self.tru } else { !self.tru }
    }
    fn not(&self, a: Lit) -> Lit {
        !a
    }
    fn and(&mut self, xs: &[Lit]) -> Lit {
        let mut v: Vec<Lit> = Vec::with_capacity(xs.len());
        for &x in xs {
            if x == !self.tru {
                return !self.tru;
            }
            if x != self.tru {
                v.push(x);
            }
        }
        v.sort_unstable();
        v.dedup();
        for w in v.windows(2) {
            if w[0] == !w[1] {
                return !self.tru;
            }
        }
        match v.len() {
            0 => self.tru,
            1 => v[0],
            _ => {
                let key = if v.len() <= 4 {
                    let mut k = [u32::MAX; 4];
                    for (j, l) in v.iter().enumerate() {
                        k[j] = l.lidx() as u32;
                    }
                    Some(k)
                } else {
                    None
                };
                if let Some(k) = &key {
                    if let Some(&o) = self.cache.get(k) {
                        return o;
                    }
                } else {
                    let kb: Box<[u32]> = v.iter().map(|l| l.lidx() as u32).collect();
                    if let Some(&o) = self.bigcache.get(&kb) {
                        return o;
                    }
                }
                let o = self.var();
                for &x in &v {
                    self.add(&[!o, x]);
                }
                let mut c: Vec<Lit> = v.iter().map(|&x| !x).collect();
                c.push(o);
                self.add(&c);
                if let Some(k) = key {
                    self.cache.insert(k, o);
                } else {
                    let kb: Box<[u32]> = v.iter().map(|l| l.lidx() as u32).collect();
                    self.bigcache.insert(kb, o);
                }
                o
            }
        }
    }
    fn xor(&mut self, a: Lit, b: Lit) -> Lit {
        let t = self.tru;
        if a == t {
            return !b;
        }
        if a == !t {
            return b;
        }
        if b == t {
            return !a;
        }
        if b == !t {
            return a;
        }
        if a == b {
            return !t;
        }
        if a == !b {
            return t;
        }
        // normalize: positive literals in key, track parity
        let (mut x, mut y) = (a, b);
        let mut neg = false;
        if x.is_neg() {
            x = !x;
            neg = !neg;
        }
        if y.is_neg() {
            y = !y;
            neg = !neg;
        }
        if y < x {
            std::mem::swap(&mut x, &mut y);
        }
        let o = if let Some(&o) = self.xcache.get(&(x, y)) {
            o
        } else {
            let o = self.var();
            self.add(&[!o, x, y]);
            self.add(&[!o, !x, !y]);
            self.add(&[o, !x, y]);
            self.add(&[o, x, !y]);
            self.xcache.insert((x, y), o);
            o
        };
        if neg { !o } else { o }
    }
    fn require(&mut self, a: Lit, _why: &'static str) {
        self.reqs += 1;
        self.add(&[a]);
    }
    fn flag(&mut self, a: Lit, why: &'static str) {
        if a == !self.tru {
            return;
        }
        self.flags.push((a, why));
        if self.mode == FlagMode::Forbid {
            self.add(&[!a]);
        }
    }
    fn val(&self, a: Lit) -> Option<bool> {
        if a == self.tru {
            Some(true)
        } else if a == !self.tru {
            Some(false)
        } else {
            None
        }
    }
}
