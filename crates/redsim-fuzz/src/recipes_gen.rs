//! `redsim-fuzz recipes-gen`: the game's crafting recipes that redsim's items can fill, written as
//! `crates/redsim/src/redstone/recipes_gen.rs` (generated: never edit it by hand). Reads the
//! decompiled data (`data/minecraft/recipe/*.json`, `data/minecraft/tags/item/*.json`), keeps every
//! `crafting_shaped` / `crafting_shapeless` recipe whose every ingredient accepts one of redsim's
//! items (`container::all_items`; an ingredient's other items can never be in a crafter's grid, so
//! they are dropped), sorted by id. Fails when another kind of crafting recipe (transmute, dye,
//! special...) names one of redsim's items: those need code.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

fn strip(id: &str) -> String {
    id.trim_start_matches("minecraft:").to_string()
}

/// Item tags, nested tags expanded.
struct Tags {
    raw: BTreeMap<String, Vec<String>>,
}

impl Tags {
    fn load(dir: &Path) -> Result<Tags> {
        let mut raw = BTreeMap::new();
        let mut stack = vec![(dir.to_path_buf(), String::new())];
        while let Some((d, prefix)) = stack.pop() {
            for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))? {
                let e = e?;
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if p.is_dir() {
                    stack.push((p, format!("{prefix}{name}/")));
                } else if let Some(stem) = name.strip_suffix(".json") {
                    let v: Value = serde_json::from_str(&std::fs::read_to_string(&p)?)?;
                    let values = v["values"].as_array().cloned().unwrap_or_default();
                    let list = values
                        .iter()
                        .filter_map(|x| match x {
                            Value::String(s) => Some(s.clone()),
                            Value::Object(o) => o.get("id").and_then(|i| i.as_str()).map(String::from),
                            _ => None,
                        })
                        .collect();
                    raw.insert(format!("{prefix}{stem}"), list);
                }
            }
        }
        Ok(Tags { raw })
    }

    fn items(&self, tag: &str, out: &mut BTreeSet<String>, depth: usize) -> Result<()> {
        if depth > 32 {
            bail!("tag #{tag}: nesting too deep");
        }
        let list = self.raw.get(tag).with_context(|| format!("unknown item tag #{tag}"))?;
        for v in list {
            match v.strip_prefix('#') {
                Some(t) => self.items(&strip(t), out, depth + 1)?,
                None => {
                    out.insert(strip(v));
                }
            }
        }
        Ok(())
    }
}

/// An ingredient's items (an id, a #tag, or a list of them).
fn ingredient_items(v: &Value, tags: &Tags) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let add = |s: &str, out: &mut BTreeSet<String>| -> Result<()> {
        match s.strip_prefix('#') {
            Some(t) => tags.items(&strip(t), out, 0),
            None => {
                out.insert(strip(s));
                Ok(())
            }
        }
    };
    match v {
        Value::String(s) => add(s, &mut out)?,
        Value::Array(a) => {
            for x in a {
                let s = x.as_str().with_context(|| format!("ingredient list entry {x}"))?;
                add(s, &mut out)?;
            }
        }
        other => bail!("ingredient {other} not understood"),
    }
    Ok(out)
}

fn known_subset(items: &BTreeSet<String>, known: &BTreeSet<String>) -> Vec<String> {
    items.iter().filter(|i| known.contains(*i)).cloned().collect()
}

fn lit(list: &[String]) -> String {
    format!("&[{}]", list.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", "))
}

/// Every string a JSON value holds (for the other crafting types' ingredient-like fields).
fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

pub fn run(data: &Path, out: &Path) -> Result<()> {
    let known: BTreeSet<String> = redsim::redstone::container::all_items().map(String::from).collect();
    let tags = Tags::load(&data.join("tags/item"))?;
    let mut files: Vec<_> = std::fs::read_dir(data.join("recipe"))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    let mut body = String::new();
    let (mut kept, mut seen, mut unknown_results) = (0, 0, BTreeSet::new());
    let mut needs_code = Vec::new();
    for f in files {
        let id = f.file_stem().unwrap().to_string_lossy().to_string();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f)?).with_context(|| format!("{}", f.display()))?;
        let kind = v["type"].as_str().unwrap_or("").trim_start_matches("minecraft:").to_string();
        if !kind.starts_with("crafting") {
            continue;
        }
        seen += 1;
        let result = v["result"]["id"].as_str().map(strip).unwrap_or_default();
        let count = v["result"]["count"].as_u64().unwrap_or(1);
        match kind.as_str() {
            "crafting_shaped" => {
                let pattern: Vec<String> = v["pattern"].as_array().context("pattern")?.iter().map(|l| l.as_str().unwrap_or("").to_string()).collect();
                let mut key = BTreeMap::new();
                let mut ok = true;
                for (k, ing) in v["key"].as_object().context("key")? {
                    let items = known_subset(&ingredient_items(ing, &tags)?, &known);
                    if items.is_empty() {
                        ok = false;
                    }
                    key.insert(k.chars().next().unwrap(), items);
                }
                if !ok {
                    continue;
                }
                let rows = shrink(&pattern);
                let height = rows.len();
                let width = rows.first().map(|r| r.chars().count()).unwrap_or(0);
                let mut cells = Vec::new();
                for r in &rows {
                    for c in r.chars() {
                        cells.push(if c == ' ' { "None".to_string() } else { format!("Some({})", lit(&key[&c])) });
                    }
                }
                writeln!(body, "    Recipe {{ id: {id:?}, shape: Shape::Shaped {{ width: {width}, height: {height}, cells: &[{}] }}, result: {result:?}, count: {count} }},", cells.join(", "))?;
            }
            "crafting_shapeless" => {
                let mut ings = Vec::new();
                let mut ok = true;
                for ing in v["ingredients"].as_array().context("ingredients")? {
                    let items = known_subset(&ingredient_items(ing, &tags)?, &known);
                    if items.is_empty() {
                        ok = false;
                    }
                    ings.push(lit(&items));
                }
                if !ok {
                    continue;
                }
                writeln!(body, "    Recipe {{ id: {id:?}, shape: Shape::Shapeless {{ ingredients: &[{}] }}, result: {result:?}, count: {count} }},", ings.join(", "))?;
            }
            _ => {
                // other crafting kinds: fine while none of their items is redsim's
                let mut all = Vec::new();
                strings(&v, &mut all);
                let mut named = BTreeSet::new();
                for s in all {
                    if let Some(t) = s.strip_prefix('#') {
                        if tags.items(&strip(t), &mut named, 0).is_err() {
                            continue;
                        }
                    } else {
                        named.insert(strip(&s));
                    }
                }
                let hit = known_subset(&named, &known);
                if !hit.is_empty() && kind != "crafting_special_repairitem" {
                    needs_code.push(format!("{id} ({kind}): {}", hit.join(", ")));
                }
                continue;
            }
        }
        kept += 1;
        if !known.contains(&result) {
            unknown_results.insert(result);
        }
    }
    if !needs_code.is_empty() {
        bail!("crafting recipes of a code-defined kind name redsim's items (model them first):\n  {}", needs_code.join("\n  "));
    }
    let mut src = String::new();
    writeln!(src, "//! GENERATED by `redsim-fuzz recipes-gen` from the game's recipe data: every `crafting_shaped` /")?;
    writeln!(src, "//! `crafting_shapeless` recipe whose every ingredient accepts one of redsim's items")?;
    writeln!(src, "//! (`container::all_items`), sorted by id, each ingredient keeping only those items. Do not edit:")?;
    writeln!(src, "//! add items to `container::MORE_ITEMS` and run the generator again.")?;
    writeln!(src)?;
    writeln!(src, "use super::crafting::{{Recipe, Shape}};")?;
    writeln!(src)?;
    writeln!(src, "pub static RECIPES: &[Recipe] = &[")?;
    src.push_str(&body);
    writeln!(src, "];")?;
    std::fs::write(out, src)?;
    println!("{kept} of {seen} crafting recipes kept -> {}", out.display());
    if !unknown_results.is_empty() {
        println!("results redsim does not know (crafting one is UNMODELED): {}", unknown_results.into_iter().collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

/// `ShapedRecipePattern.shrink`.
fn shrink(pattern: &[String]) -> Vec<String> {
    let mut left = usize::MAX;
    let mut right: i64 = 0;
    let mut top = 0usize;
    let mut bottom = 0usize;
    for (i, line) in pattern.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let first = chars.iter().position(|&c| c != ' ').unwrap_or(chars.len());
        left = left.min(first);
        let last = chars.iter().rposition(|&c| c != ' ').map(|x| x as i64).unwrap_or(-1);
        right = right.max(last);
        if last < 0 {
            if top == i {
                top += 1;
            }
            bottom += 1;
        } else {
            bottom = 0;
        }
    }
    if pattern.len() == bottom {
        return Vec::new();
    }
    (0..pattern.len() - bottom - top).map(|l| pattern[l + top].chars().skip(left).take(right as usize + 1 - left).collect()).collect()
}
