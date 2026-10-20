//! Hand-drawn tiles in the look of the game's own textures (drawn here, not copied: no game asset
//! is shipped). Colors are measured from the game (`mctas web palette` on a local reference, never
//! committed); shapes follow its layout (an observer's face and bands, a piston's planks band...).
//! Each replaces the procedural tile of the same name.

use super::Tile;

type Painter = Box<dyn Fn(&mut Tile)>;

pub fn tiles() -> Vec<(&'static str, Painter)> {
    Vec::new()
}
