//! Deep Signal — the deterministic world and simulation (no rendering, no clock, no I/O).
//!
//! Everything that decides game state is integer or fixed-point arithmetic, and the one
//! trigonometric table is built from IEEE basic operations only (a fixed polynomial), so a
//! recorded `(tic, key)` trace replays to the same state on any machine and at any
//! presentation cadence. Rendering reads this state; it never writes it.

/// Fixed-point unit: one map tile = `ONE` units.
pub const ONE: i32 = 1024;
/// Angle unit: a full turn is `TURN` steps.
pub const TURN: i32 = 4096;
/// Simulation rate.
pub const TIC_HZ: u32 = 35;

const MOVE_PER_TIC: i32 = 92; // ~3.1 tiles/s
const STRAFE_PER_TIC: i32 = 70;
const TURN_PER_TIC: i32 = 44; // ~135°/s
const RADIUS: i32 = 220; // collision radius, ~0.21 tile
const HAZARD_DAMAGE: u32 = 9; // per tic standing on a live hazard
const MAX_INTEGRITY: u32 = 1000;
/// Tics a level transition lasts (the descent effect reads its progress).
pub const TRANSITION_TICS: u64 = 52;
/// Tics the "signal lost" sequence lasts before the level restarts.
pub const LOST_TICS: u64 = 70;

/// `sin` of angle step `a` in Q16 (65536 = 1.0), from a table built with basic IEEE ops.
pub fn sin_q16(a: i32) -> i32 {
    SIN.with(|t| t[a.rem_euclid(TURN) as usize])
}

/// `cos` of angle step `a` in Q16.
pub fn cos_q16(a: i32) -> i32 {
    sin_q16(a + TURN / 4)
}

thread_local! {
    static SIN: Vec<i32> = build_sin_table();
}

/// sin over one quadrant by a fixed odd Taylor polynomial (degree 15) evaluated with +, -, *
/// only — IEEE-exact operations, so the table is identical everywhere — then mirrored.
fn build_sin_table() -> Vec<i32> {
    let quarter = (TURN / 4) as usize;
    let mut q = Vec::with_capacity(quarter + 1);
    let pi_2 = std::f64::consts::FRAC_PI_2;
    for i in 0..=quarter {
        let x = pi_2 * (i as f64) / (quarter as f64);
        let x2 = x * x;
        // x - x^3/3! + x^5/5! - ... - x^15/15!, Horner form with exact reciprocal constants.
        let mut s = -1.0 / 1_307_674_368_000.0;
        s = s * x2 + 1.0 / 6_227_020_800.0;
        s = s * x2 - 1.0 / 39_916_800.0;
        s = s * x2 + 1.0 / 362_880.0;
        s = s * x2 - 1.0 / 5_040.0;
        s = s * x2 + 1.0 / 120.0;
        s = s * x2 - 1.0 / 6.0;
        s = s * x2 + 1.0;
        let v = s * x * 65536.0;
        q.push((v + 0.5).floor() as i32);
    }
    (0..TURN as usize)
        .map(|i| {
            let k = i % (2 * quarter);
            let m = if k <= quarter {
                q[k]
            } else {
                q[2 * quarter - k]
            };
            if i < 2 * quarter {
                m
            } else {
                -m
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Floor,
    /// Wall with a material index (selects texture and palette).
    Wall(u8),
    /// A live hazard floor tile: costs integrity while stood on.
    Hazard,
    /// The uplink; passable only once every shard is collected.
    Exit,
}

impl Tile {
    pub fn blocks(self, exit_open: bool) -> bool {
        match self {
            Tile::Wall(_) => true,
            Tile::Exit => !exit_open,
            _ => false,
        }
    }
}

/// Tiny deterministic PRNG (xorshift64*), seeded per level.
#[derive(Clone, Debug)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<Tile>,
}

impl Map {
    pub fn at(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            Tile::Wall(0)
        } else {
            self.tiles[(y * self.w + x) as usize]
        }
    }

    /// A perfect maze on a `cw × ch` cell grid (tiles `2cw+1 × 2ch+1`) by an iterative
    /// recursive backtracker, then a few loops knocked through so it is not a pure tree.
    pub fn generate(cw: i32, ch: i32, rng: &mut Rng) -> Self {
        let (w, h) = (2 * cw + 1, 2 * ch + 1);
        let mut tiles = vec![Tile::Wall(0); (w * h) as usize];
        let mut seen = vec![false; (cw * ch) as usize];
        let mut stack = vec![(0i32, 0i32)];
        seen[0] = true;
        tiles[(w + 1) as usize] = Tile::Floor;
        while let Some(&(cx, cy)) = stack.last() {
            let mut options = [(0i32, 0i32); 4];
            let mut n = 0;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx >= 0 && ny >= 0 && nx < cw && ny < ch && !seen[(ny * cw + nx) as usize] {
                    options[n] = (dx, dy);
                    n += 1;
                }
            }
            if n == 0 {
                stack.pop();
                continue;
            }
            let (dx, dy) = options[rng.below(n as u64) as usize];
            let (nx, ny) = (cx + dx, cy + dy);
            seen[(ny * cw + nx) as usize] = true;
            tiles[((2 * cy + 1 + dy) * w + 2 * cx + 1 + dx) as usize] = Tile::Floor;
            tiles[((2 * ny + 1) * w + 2 * nx + 1) as usize] = Tile::Floor;
            stack.push((nx, ny));
        }
        // Knock through some interior walls to create loops.
        for _ in 0..(cw * ch / 4) {
            let x = 1 + rng.below((w - 2) as u64) as i32;
            let y = 1 + rng.below((h - 2) as u64) as i32;
            let i = (y * w + x) as usize;
            if tiles[i] != Tile::Floor && (x + y) % 2 == 1 {
                tiles[i] = Tile::Floor;
            }
        }
        // Wall materials by region, so corridors read differently.
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if let Tile::Wall(_) = tiles[i] {
                    let m = if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                        0
                    } else {
                        1 + ((x / 6 + y / 6) % 3) as u8
                    };
                    tiles[i] = Tile::Wall(m);
                }
            }
        }
        Map { w, h, tiles }
    }
}

/// Tiles reachable from `from` without passing a wall or the (shut) exit; index `y * w + x`.
pub fn reachable_while_shut(map: &Map, from: (i32, i32)) -> Vec<bool> {
    let mut seen = vec![false; (map.w * map.h) as usize];
    let mut queue = std::collections::VecDeque::from([from]);
    seen[(from.1 * map.w + from.0) as usize] = true;
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if map.at(nx, ny).blocks(false) {
                continue;
            }
            let i = (ny * map.w + nx) as usize;
            if !seen[i] {
                seen[i] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    seen
}

/// Player intent for one tic, derived from held actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intent {
    pub forward: i32, // -1, 0, 1
    pub strafe: i32,  // -1, 0, 1
    pub turn: i32,    // -1, 0, 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    /// Descending to the next level; started at this tic.
    Descending {
        since: u64,
    },
    /// Integrity reached zero; restarting the level.
    SignalLost {
        since: u64,
    },
    /// All levels cleared.
    Clear,
}

/// Something that happened this tic, for the presentation layer (effects, sound, HUD log).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happening {
    ShardTaken { left: u32 },
    ExitOpened,
    Damaged { amount: u32 },
    LevelEntered { level: u32 },
    SignalLost,
    Cleared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub seed: u64,
    pub level: u32,
    pub levels: u32,
    pub map: Map,
    pub x: i32,
    pub y: i32,
    pub angle: i32,
    pub shards: Vec<(i32, i32)>,
    pub shards_total: u32,
    pub integrity: u32,
    pub phase: Phase,
    pub tic: u64,
    /// Tic of the most recent damage (drives the tear effect).
    pub last_hit: Option<u64>,
    pub hits: u32,
}

impl World {
    pub fn new(seed: u64, levels: u32) -> Self {
        let mut w = World {
            seed,
            level: 1,
            levels: levels.max(1),
            map: Map {
                w: 1,
                h: 1,
                tiles: vec![Tile::Wall(0)],
            },
            x: 0,
            y: 0,
            angle: 0,
            shards: Vec::new(),
            shards_total: 0,
            integrity: MAX_INTEGRITY,
            phase: Phase::Playing,
            tic: 0,
            last_hit: None,
            hits: 0,
        };
        w.build_level();
        w
    }

    pub fn max_integrity() -> u32 {
        MAX_INTEGRITY
    }

    pub fn exit_open(&self) -> bool {
        self.shards.is_empty()
    }

    fn build_level(&mut self) {
        let mut rng =
            Rng::new(self.seed ^ u64::from(self.level).wrapping_mul(0xA24B_AED4_963E_E407));
        let size = 6 + self.level as i32; // levels grow
        let mut map = Map::generate(size, size, &mut rng);
        let floor: Vec<(i32, i32)> = (0..map.h)
            .flat_map(|y| (0..map.w).map(move |x| (x, y)))
            .filter(|&(x, y)| map.at(x, y) == Tile::Floor && (x, y) != (1, 1))
            .collect();
        // Exit in the far corner cell; shards and hazards on distinct floor tiles.
        let (ex, ey) = (map.w - 2, map.h - 2);
        map.tiles[(ey * map.w + ex) as usize] = Tile::Exit;
        // Shards may only go where the player can walk while the uplink is still SHUT: in a
        // maze the exit cell can be the only link to part of the map, and a shard behind it
        // would make the sector unwinnable (the exit opens only once every shard is taken).
        let open = reachable_while_shut(&map, (1, 1));
        let mut pool: Vec<(i32, i32)> = floor
            .into_iter()
            .filter(|&(x, y)| (x, y) != (ex, ey) && open[(y * map.w + x) as usize])
            .collect();
        let take = |rng: &mut Rng, pool: &mut Vec<(i32, i32)>| {
            let i = rng.below(pool.len() as u64) as usize;
            pool.swap_remove(i)
        };
        let shard_count = 3 + self.level.min(4);
        self.shards = (0..shard_count)
            .map(|_| take(&mut rng, &mut pool))
            .collect();
        for _ in 0..(2 + self.level * 2) {
            let (hx, hy) = take(&mut rng, &mut pool);
            if (hx - 1).abs() + (hy - 1).abs() > 3 {
                map.tiles[(hy * map.w + hx) as usize] = Tile::Hazard;
            }
        }
        self.shards_total = shard_count;
        self.map = map;
        self.x = ONE + ONE / 2;
        self.y = ONE + ONE / 2;
        self.angle = 0;
        self.integrity = MAX_INTEGRITY;
    }

    /// Hazards pulse: live on even 2-second windows of the simulation clock (`tic`).
    pub fn hazard_live(&self) -> bool {
        (self.tic / (2 * u64::from(TIC_HZ))) % 2 == 0
    }

    fn blocked(&self, x: i32, y: i32) -> bool {
        let open = self.exit_open();
        for (dx, dy) in [
            (-RADIUS, -RADIUS),
            (RADIUS, -RADIUS),
            (-RADIUS, RADIUS),
            (RADIUS, RADIUS),
        ] {
            let (tx, ty) = ((x + dx).div_euclid(ONE), (y + dy).div_euclid(ONE));
            if self.map.at(tx, ty).blocks(open) {
                return true;
            }
        }
        false
    }

    /// Advances exactly one tic. Returns what happened, for presentation.
    pub fn step(&mut self, intent: Intent) -> Vec<Happening> {
        let mut out = Vec::new();
        self.tic += 1;
        match self.phase {
            Phase::Clear => return out,
            Phase::Descending { since } => {
                if self.tic - since >= TRANSITION_TICS {
                    self.level += 1;
                    self.build_level();
                    self.phase = Phase::Playing;
                    out.push(Happening::LevelEntered { level: self.level });
                }
                return out;
            }
            Phase::SignalLost { since } => {
                if self.tic - since >= LOST_TICS {
                    self.build_level();
                    self.phase = Phase::Playing;
                    out.push(Happening::LevelEntered { level: self.level });
                }
                return out;
            }
            Phase::Playing => {}
        }
        self.angle = (self.angle + intent.turn * TURN_PER_TIC).rem_euclid(TURN);
        let (c, s) = (cos_q16(self.angle), sin_q16(self.angle));
        let f = intent.forward * MOVE_PER_TIC;
        let st = intent.strafe * STRAFE_PER_TIC;
        // Forward along (c, s); strafe along (-s, c). Q16 -> tile units.
        let dx = ((i64::from(c) * i64::from(f) - i64::from(s) * i64::from(st)) >> 16) as i32;
        let dy = ((i64::from(s) * i64::from(f) + i64::from(c) * i64::from(st)) >> 16) as i32;
        if dx != 0 && !self.blocked(self.x + dx, self.y) {
            self.x += dx;
        }
        if dy != 0 && !self.blocked(self.x, self.y + dy) {
            self.y += dy;
        }
        let (tx, ty) = (self.x.div_euclid(ONE), self.y.div_euclid(ONE));
        if let Some(i) = self.shards.iter().position(|&p| p == (tx, ty)) {
            self.shards.swap_remove(i);
            let left = self.shards.len() as u32;
            out.push(Happening::ShardTaken { left });
            if left == 0 {
                out.push(Happening::ExitOpened);
            }
        }
        if self.map.at(tx, ty) == Tile::Hazard && self.hazard_live() {
            self.integrity = self.integrity.saturating_sub(HAZARD_DAMAGE);
            self.last_hit = Some(self.tic);
            self.hits += 1;
            out.push(Happening::Damaged {
                amount: HAZARD_DAMAGE,
            });
            if self.integrity == 0 {
                self.phase = Phase::SignalLost { since: self.tic };
                out.push(Happening::SignalLost);
            }
        }
        if self.map.at(tx, ty) == Tile::Exit && self.exit_open() {
            if self.level >= self.levels {
                self.phase = Phase::Clear;
                out.push(Happening::Cleared);
            } else {
                self.phase = Phase::Descending { since: self.tic };
            }
        }
        out
    }

    /// A compact digest of the authoritative state (replay receipts).
    pub fn digest(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: u64| {
            for b in v.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        mix(self.tic);
        mix(u64::from(self.level));
        mix(self.x as u64);
        mix(self.y as u64);
        mix(self.angle as u64);
        mix(u64::from(self.integrity));
        mix(u64::from(self.hits));
        mix(self.last_hit.unwrap_or(u64::MAX));
        mix(self.shards.len() as u64);
        for &(x, y) in &self.shards {
            mix(((x as u64) << 32) | y as u64);
        }
        mix(match self.phase {
            Phase::Playing => 1,
            Phase::Descending { since } => 2 + (since << 4),
            Phase::SignalLost { since } => 3 + (since << 4),
            Phase::Clear => 4,
        });
        h
    }
}
