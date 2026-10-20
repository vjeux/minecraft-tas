//! Real-game recordings played back (clocks redsim does not model: boats, minecarts, fluids):
//! `designs/clocks/traces/<stem>.trace` from track clk-misc (`mctas clock misc trace-export`),
//! JSON Lines: a header, then per game tick the changed cells (every cell at t = 0) and every
//! entity. Played once through the warmup, then looped over one period.

use crate::design::Pos;
use crate::json::Json;
use crate::mesh::EntityDraw;
use crate::session::RBlock;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TraceEntity {
    pub id: u32,
    pub kind: String,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Debug)]
pub struct Replay {
    pub name: String,
    pub period: u32,
    pub warmup: u32,
    /// Last recorded tick.
    pub ticks: u32,
    /// Played once to `loop_range.1`, then from `loop_range.0` again.
    pub loop_range: (u32, u32),
    pub output: Option<Pos>,
    changes: Vec<Vec<(Pos, String)>>,
    entities: Vec<Vec<TraceEntity>>,
    pub tick: u32,
    blocks: HashMap<Pos, String>,
    at_loop: HashMap<Pos, String>,
}

fn is_air(s: &str) -> bool {
    matches!(s.strip_prefix("minecraft:").unwrap_or(s).split('[').next().unwrap_or(""), "air" | "cave_air" | "void_air")
}

impl Replay {
    pub fn parse(text: &str) -> Result<Replay, String> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let head = Json::parse(lines.next().ok_or("empty trace")?).map_err(|e| format!("trace header: {e}"))?;
        if head.get("trace").and_then(Json::as_i64) != Some(1) {
            return Err("not a version-1 trace".into());
        }
        let num = |k: &str| head.get(k).and_then(Json::as_i64).map(|v| v as u32);
        let period = num("period").ok_or("trace: no period")?;
        let warmup = num("warmup").unwrap_or(0);
        let mut changes = Vec::new();
        let mut entities = Vec::new();
        for (k, l) in lines.enumerate() {
            let v = Json::parse(l).map_err(|e| format!("trace line {}: {e}", k + 2))?;
            let mut cells = Vec::new();
            for b in v.get("blocks").and_then(Json::as_arr).unwrap_or(&[]) {
                let (Some(p), Some(s)) = (b.as_pos(), b.as_arr().and_then(|a| a.get(3)).and_then(Json::as_str)) else { continue };
                cells.push((p, s.to_string()));
            }
            let mut ents = Vec::new();
            for e in v.get("entities").and_then(Json::as_arr).unwrap_or(&[]) {
                let pos = e.get("pos").and_then(Json::as_arr).filter(|a| a.len() >= 3).map(|a| [a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32]);
                ents.push(TraceEntity {
                    id: e.get("id").and_then(Json::as_i64).unwrap_or(0) as u32,
                    kind: e.get("type").and_then(Json::as_str).unwrap_or("minecraft:marker").to_string(),
                    pos: pos.unwrap_or([0.0; 3]),
                    yaw: e.get("yaw").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                    pitch: e.get("pitch").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                });
            }
            changes.push(cells);
            entities.push(ents);
        }
        if changes.is_empty() {
            return Err("trace has no ticks".into());
        }
        let ticks = changes.len() as u32 - 1;
        let lr = head.get("loop").and_then(Json::as_arr).filter(|a| a.len() == 2).map(|a| (a[0].as_i64().unwrap_or(0) as u32, a[1].as_i64().unwrap_or(0) as u32)).unwrap_or((warmup, warmup + period));
        let loop_range = (lr.0.min(ticks), lr.1.min(ticks).max(lr.0.min(ticks)));
        let mut r = Replay {
            name: head.get("name").and_then(Json::as_str).unwrap_or("").to_string(),
            period,
            warmup,
            ticks,
            loop_range,
            output: head.get("output").and_then(Json::as_pos),
            changes,
            entities,
            tick: 0,
            blocks: HashMap::new(),
            at_loop: HashMap::new(),
        };
        r.reset();
        // the state at the loop's start, to jump back to
        let mut s = r.clone();
        while s.tick < s.loop_range.0 {
            s.advance();
        }
        r.at_loop = s.blocks;
        Ok(r)
    }

    fn apply(&mut self, t: u32) {
        if let Some(cells) = self.changes.get(t as usize) {
            for (p, s) in cells {
                if is_air(s) {
                    self.blocks.remove(p);
                } else {
                    self.blocks.insert(*p, s.clone());
                }
            }
        }
    }

    fn advance(&mut self) {
        if self.tick < self.ticks {
            self.tick += 1;
            self.apply(self.tick);
        }
    }

    pub fn reset(&mut self) {
        self.tick = 0;
        self.blocks.clear();
        self.apply(0);
    }

    /// One game tick; at the end of the loop, back to its start (the same state, one period on).
    pub fn step(&mut self) {
        if self.tick + 1 > self.loop_range.1 && self.loop_range.1 > self.loop_range.0 {
            self.tick = self.loop_range.0;
            self.blocks = self.at_loop.clone();
            return;
        }
        self.advance();
    }

    pub fn render_blocks(&self) -> Vec<RBlock> {
        let mut v: Vec<RBlock> = self.blocks.iter().map(|(p, s)| RBlock { cell: *p, offset: [0.0; 3], state: s.clone() }).collect();
        v.sort_by_key(|b| b.cell);
        v
    }

    /// The recorded state of a cell now (None: air).
    pub fn state_at(&self, p: Pos) -> Option<&str> {
        self.blocks.get(&p).map(|s| s.as_str())
    }

    /// The entities between this tick and the next (`partial` in 0..1), interpolated like the
    /// client does.
    pub fn entities(&self, partial: f32) -> Vec<EntityDraw> {
        let now = self.entities.get(self.tick as usize).cloned().unwrap_or_default();
        let next_t = if self.tick + 1 > self.loop_range.1 && self.loop_range.1 > self.loop_range.0 { self.loop_range.0 + 1 } else { self.tick + 1 };
        let next = self.entities.get(next_t as usize);
        now.iter()
            .map(|e| {
                let n = next.and_then(|v| v.iter().find(|x| x.id == e.id));
                let (pos, yaw) = match n {
                    Some(n) if partial > 0.0 => {
                        let mut dy = n.yaw - e.yaw;
                        while dy > 180.0 {
                            dy -= 360.0;
                        }
                        while dy < -180.0 {
                            dy += 360.0;
                        }
                        ([e.pos[0] + (n.pos[0] - e.pos[0]) * partial, e.pos[1] + (n.pos[1] - e.pos[1]) * partial, e.pos[2] + (n.pos[2] - e.pos[2]) * partial], e.yaw + dy * partial)
                    }
                    _ => (e.pos, e.yaw),
                };
                EntityDraw { kind: e.kind.clone(), pos, yaw }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_and_loops() {
        let t = "{\"trace\":1,\"period\":2,\"warmup\":1,\"ticks\":5,\"loop\":[1,3]}\n\
{\"t\":0,\"blocks\":[[0,0,0,\"minecraft:stone\"]],\"entities\":[{\"id\":0,\"type\":\"minecraft:oak_boat\",\"pos\":[0.5,1,0.5],\"yaw\":0}]}\n\
{\"t\":1,\"blocks\":[[1,0,0,\"minecraft:water\"]],\"entities\":[{\"id\":0,\"type\":\"minecraft:oak_boat\",\"pos\":[1.5,1,0.5],\"yaw\":0}]}\n\
{\"t\":2,\"blocks\":[[1,0,0,\"minecraft:air\"]],\"entities\":[{\"id\":0,\"type\":\"minecraft:oak_boat\",\"pos\":[2.5,1,0.5],\"yaw\":0}]}\n\
{\"t\":3,\"blocks\":[[1,0,0,\"minecraft:water\"]],\"entities\":[{\"id\":0,\"type\":\"minecraft:oak_boat\",\"pos\":[1.5,1,0.5],\"yaw\":0}]}\n";
        let mut r = Replay::parse(t).unwrap();
        assert_eq!(r.render_blocks().len(), 1);
        r.step();
        assert_eq!(r.tick, 1);
        assert_eq!(r.render_blocks().len(), 2);
        assert_eq!(r.entities(0.5)[0].pos[0], 2.0);
        r.step();
        r.step();
        assert_eq!(r.tick, 3);
        r.step();
        assert_eq!(r.tick, 1);
        assert_eq!(r.render_blocks().len(), 2);
    }
}
