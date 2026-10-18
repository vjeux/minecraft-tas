//! Block positions and directions, with the game's iteration orders and `BlockPos.hashCode`.

/// `net.minecraft.core.BlockPos`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos { x, y, z }
    }
    pub fn rel(self, d: Dir) -> BlockPos {
        let [dx, dy, dz] = d.step();
        BlockPos::new(self.x + dx, self.y + dy, self.z + dz)
    }
    pub fn above(self) -> BlockPos {
        self.rel(Dir::Up)
    }
    pub fn below(self) -> BlockPos {
        self.rel(Dir::Down)
    }
    pub fn offset(self, dx: i32, dy: i32, dz: i32) -> BlockPos {
        BlockPos::new(self.x + dx, self.y + dy, self.z + dz)
    }
    /// `Vec3i.hashCode`: `(y + z * 31) * 31 + x` (wrapping, like Java ints).
    pub fn java_hash(self) -> i32 {
        self.y.wrapping_add(self.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(self.x)
    }
    /// `ChunkPos` of this position.
    pub fn chunk(self) -> (i32, i32) {
        (self.x >> 4, self.z >> 4)
    }
    pub fn arr(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }
    pub fn from_arr(a: [i32; 3]) -> BlockPos {
        BlockPos::new(a[0], a[1], a[2])
    }
}

impl std::fmt::Display for BlockPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}

/// `net.minecraft.core.Direction`, in ordinal order (`Direction.values()`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum Dir {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Dir {
    /// `Direction.values()`: D, U, N, S, W, E.
    pub const ALL: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
    /// `NeighborUpdater.UPDATE_ORDER`: W, E, D, U, N, S.
    pub const UPDATE_ORDER: [Dir; 6] = [Dir::West, Dir::East, Dir::Down, Dir::Up, Dir::North, Dir::South];
    /// `BlockBehaviour.UPDATE_SHAPE_ORDER`: W, E, N, S, D, U.
    pub const SHAPE_ORDER: [Dir; 6] = [Dir::West, Dir::East, Dir::North, Dir::South, Dir::Down, Dir::Up];
    /// `Direction.Plane.HORIZONTAL`: N, E, S, W.
    pub const HORIZONTAL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];
    /// `Direction.Plane.VERTICAL`: U, D.
    pub const VERTICAL: [Dir; 2] = [Dir::Up, Dir::Down];

    pub fn step(self) -> [i32; 3] {
        match self {
            Dir::Down => [0, -1, 0],
            Dir::Up => [0, 1, 0],
            Dir::North => [0, 0, -1],
            Dir::South => [0, 0, 1],
            Dir::West => [-1, 0, 0],
            Dir::East => [1, 0, 0],
        }
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
        }
    }
    /// `Direction.getClockWise()` (horizontal only: N -> E -> S -> W).
    pub fn clockwise(self) -> Dir {
        match self {
            Dir::North => Dir::East,
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            d => d,
        }
    }
    pub fn counter_clockwise(self) -> Dir {
        match self {
            Dir::North => Dir::West,
            Dir::West => Dir::South,
            Dir::South => Dir::East,
            Dir::East => Dir::North,
            d => d,
        }
    }
    pub fn axis(self) -> Axis {
        match self {
            Dir::Down | Dir::Up => Axis::Y,
            Dir::North | Dir::South => Axis::Z,
            Dir::West | Dir::East => Axis::X,
        }
    }
    pub fn is_horizontal(self) -> bool {
        self.axis() != Axis::Y
    }
    pub fn name(self) -> &'static str {
        match self {
            Dir::Down => "down",
            Dir::Up => "up",
            Dir::North => "north",
            Dir::South => "south",
            Dir::West => "west",
            Dir::East => "east",
        }
    }
    pub fn from_name(s: &str) -> Option<Dir> {
        Some(match s {
            "down" => Dir::Down,
            "up" => Dir::Up,
            "north" => Dir::North,
            "south" => Dir::South,
            "west" => Dir::West,
            "east" => Dir::East,
            _ => return None,
        })
    }
    /// The direction from `a` to the adjacent `b`.
    pub fn between(a: BlockPos, b: BlockPos) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| a.rel(*d) == b)
    }
}

/// The iteration order of a `java.util.HashSet<BlockPos>` holding these (distinct) positions,
/// added in this order, when the set never resized past 16 buckets (at most 12 entries): buckets
/// by `(h ^ h >>> 16) & 15`, insertion order inside a bucket.
pub fn java_hashset_order(positions: &[BlockPos]) -> Vec<BlockPos> {
    debug_assert!(positions.len() <= 12);
    let mut v: Vec<(u32, usize, BlockPos)> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let h = p.java_hash() as u32;
            ((h ^ (h >> 16)) & 15, i, *p)
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, _, p)| p).collect()
}
