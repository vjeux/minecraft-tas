//! Crafting for crafters (clk-misc), read in the 26.3 source: `CraftingInput.ofPositioned` (the
//! grid trimmed to its filled rows and columns), `ShapedRecipePattern.matches` (same size, same
//! ingredient count, each cell's ingredient, the mirrored pattern too), `ShapelessRecipe.matches`
//! (same count, then a matching of items to ingredients: `StackedItemContents.canCraft`), and
//! `RecipeManager.getRecipeFor` (the first matching recipe; [`find`] refuses when several match,
//! since the registry's order is not modeled). The recipes are the game's own, generated into
//! `recipes_gen.rs` (`redsim-fuzz recipes-gen`) for every recipe redsim's items can fill.

use super::container::{Stack, is_damageable, item_id, item_name};

/// A recipe's shape: a shaped pattern (cells row by row, `None` = must be empty) or shapeless
/// ingredients. Each ingredient lists the redsim items it accepts.
pub enum Shape {
    Shaped { width: usize, height: usize, cells: &'static [Option<&'static [&'static str]>] },
    Shapeless { ingredients: &'static [&'static [&'static str]] },
}

/// A crafting recipe (`crafting_shaped` / `crafting_shapeless`): its id, shape and result.
pub struct Recipe {
    pub id: &'static str,
    pub shape: Shape,
    pub result: &'static str,
    pub count: u8,
}

/// `CraftingInput`: the grid trimmed to its filled rows and columns (row-major item ids).
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub width: usize,
    pub height: usize,
    pub items: Vec<Option<u8>>,
    pub ingredient_count: usize,
}

/// `CraftingInput.ofPositioned(3, 3, items)` of a crafter's nine slots.
pub fn input_of(slots: &[Option<Stack>]) -> Input {
    let (width, height) = (3usize, 3usize);
    let mut left = width - 1;
    let mut right = 0usize;
    let mut top = height - 1;
    let mut bottom = 0usize;
    for y in 0..height {
        let mut row_empty = true;
        for x in 0..width {
            if slots[x + y * width].is_some() {
                left = left.min(x);
                right = right.max(x);
                row_empty = false;
            }
        }
        if !row_empty {
            top = top.min(y);
            bottom = bottom.max(y);
        }
    }
    let new_width = right as i32 - left as i32 + 1;
    let new_height = bottom as i32 - top as i32 + 1;
    if new_width <= 0 || new_height <= 0 {
        return Input { width: 0, height: 0, items: Vec::new(), ingredient_count: 0 };
    }
    let (nw, nh) = (new_width as usize, new_height as usize);
    let mut items = Vec::with_capacity(nw * nh);
    for y in 0..nh {
        for x in 0..nw {
            items.push(slots[x + left + (y + top) * width].map(|s| s.item));
        }
    }
    let ingredient_count = items.iter().filter(|i| i.is_some()).count();
    Input { width: nw, height: nh, items, ingredient_count }
}

fn accepts(ingredient: &[&str], item: u8) -> bool {
    ingredient.contains(&item_name(item))
}

/// `Ingredient.testOptionalIngredient`.
fn test_optional(expected: Option<&[&str]>, actual: Option<u8>) -> bool {
    match (expected, actual) {
        (None, a) => a.is_none(),
        (Some(_), None) => false,
        (Some(e), Some(a)) => accepts(e, a),
    }
}

fn shaped_matches(width: usize, height: usize, cells: &[Option<&'static [&'static str]>], input: &Input, x_flip: bool) -> bool {
    for y in 0..height {
        for x in 0..width {
            let expected = if x_flip { cells[width - x - 1 + y * width] } else { cells[x + y * width] };
            if !test_optional(expected, input.items[x + y * input.width]) {
                return false;
            }
        }
    }
    true
}

/// Kuhn's matching: every ingredient gets a distinct input item it accepts.
fn perfect_matching(ingredients: &[&'static [&'static str]], items: &[u8]) -> bool {
    fn try_assign(i: usize, ingredients: &[&[&str]], items: &[u8], seen: &mut [bool], owner: &mut [Option<usize>]) -> bool {
        for j in 0..items.len() {
            if seen[j] || !accepts(ingredients[i], items[j]) {
                continue;
            }
            seen[j] = true;
            if owner[j].is_none() || try_assign(owner[j].unwrap(), ingredients, items, seen, owner) {
                owner[j] = Some(i);
                return true;
            }
        }
        false
    }
    if ingredients.len() != items.len() {
        return false;
    }
    let mut owner = vec![None; items.len()];
    for i in 0..ingredients.len() {
        let mut seen = vec![false; items.len()];
        if !try_assign(i, ingredients, items, &mut seen, &mut owner) {
            return false;
        }
    }
    true
}

impl Recipe {
    /// `Recipe.matches(input, level)`.
    pub fn matches(&self, input: &Input) -> bool {
        match self.shape {
            Shape::Shaped { width, height, cells } => {
                let count = cells.iter().filter(|c| c.is_some()).count();
                if input.ingredient_count != count {
                    return false;
                }
                // ShapedRecipePattern.matches: the mirrored pattern (skipped when symmetrical: the
                // same answer) or the pattern itself
                input.width == width && input.height == height && (shaped_matches(width, height, cells, input, true) || shaped_matches(width, height, cells, input, false))
            }
            Shape::Shapeless { ingredients } => {
                if input.ingredient_count != ingredients.len() {
                    return false;
                }
                let items: Vec<u8> = input.items.iter().flatten().copied().collect();
                if input.items.len() == 1 && ingredients.len() == 1 {
                    return items.first().is_some_and(|&i| accepts(ingredients[0], i));
                }
                perfect_matching(ingredients, &items)
            }
        }
    }
}

/// What a crafter's grid crafts: `Ok(None)` = no recipe, `Ok(Some((item, count)))`, or an error
/// for what is not modeled (an unknown result, several matching recipes, the code-defined repair
/// recipe's damageable items).
pub fn find(slots: &[Option<Stack>]) -> Result<Option<(u8, u8)>, String> {
    let input = input_of(slots);
    if input.ingredient_count == 0 {
        return Ok(None);
    }
    if input.items.iter().flatten().any(|&i| is_damageable(i)) {
        return Err("a crafter grid holding a damageable item (the repair recipe is code-defined: not modeled)".into());
    }
    let found: Vec<&Recipe> = super::recipes_gen::RECIPES.iter().filter(|r| r.matches(&input)).collect();
    match found.as_slice() {
        [] => Ok(None),
        [r] => match item_id(r.result) {
            Some(id) => Ok(Some((id, r.count))),
            None => Err(format!("a crafter crafts {} ({}): the item is not modeled", r.result, r.id)),
        },
        many => Err(format!("{} recipes match a crafter's grid ({}): the game's recipe order is not modeled", many.len(), many.iter().map(|r| r.id).collect::<Vec<_>>().join(", "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(items: &[(usize, &str)]) -> Vec<Option<Stack>> {
        let mut g = vec![None; 9];
        for (slot, name) in items {
            g[*slot] = Some(Stack { item: item_id(name).unwrap(), count: 1 });
        }
        g
    }

    #[test]
    fn iron_and_slime_round_trips() {
        let nine = |n: &str| grid(&(0..9).map(|i| (i, n)).collect::<Vec<_>>());
        assert_eq!(find(&nine("iron_ingot")).unwrap(), Some((item_id("iron_block").unwrap(), 1)));
        assert_eq!(find(&grid(&[(4, "iron_block")])).unwrap(), Some((item_id("iron_ingot").unwrap(), 9)));
        assert_eq!(find(&nine("slime_ball")).unwrap(), Some((item_id("slime_block").unwrap(), 1)));
        assert_eq!(find(&grid(&[(8, "slime_block")])).unwrap(), Some((item_id("slime_ball").unwrap(), 9)));
        // partial fills of a crafter (slots fill in order)
        assert_eq!(find(&grid(&[(0, "iron_ingot")])).unwrap(), Some((item_id("iron_nugget").unwrap(), 9)));
        assert_eq!(find(&grid(&[(0, "iron_ingot"), (1, "iron_ingot")])).unwrap(), Some((item_id("heavy_weighted_pressure_plate").unwrap(), 1)));
        assert_eq!(find(&grid(&[(0, "slime_ball"), (1, "slime_ball")])).unwrap(), None);
        assert_eq!(find(&grid(&[])).unwrap(), None);
    }

    #[test]
    fn trimming_keeps_the_filled_box() {
        let g = grid(&[(4, "stone"), (8, "stone")]);
        let i = input_of(&g);
        assert_eq!((i.width, i.height, i.ingredient_count), (2, 2, 2));
        assert_eq!(i.items, vec![Some(0), None, None, Some(0)]);
    }
}
