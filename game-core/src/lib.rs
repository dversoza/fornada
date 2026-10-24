use std::sync::LazyLock;

pub mod save;

pub const BUILDING_COUNT: usize = 8;

pub struct Building {
    pub id: &'static str,
    pub icon: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub base: f64,
    pub rate: f64,
    pub ups: [&'static str; 3],
}

pub struct Tier {
    pub need: u32,
    pub mult: f64,
}

pub const TIERS: [Tier; 3] = [
    Tier {
        need: 1,
        mult: 10.0,
    },
    Tier {
        need: 10,
        mult: 50.0,
    },
    Tier {
        need: 25,
        mult: 500.0,
    },
];

pub static BUILDINGS: [Building; BUILDING_COUNT] = [
    Building {
        id: "vovo",
        icon: "👵",
        name: "Vovó mineira",
        desc: "Enrola pães de queijo enquanto conta causo.",
        base: 15.0,
        rate: 0.1,
        ups: ["Avental florido", "Rádio na cozinha", "Neta ajudante"],
    },
    Building {
        id: "forno",
        icon: "🔥",
        name: "Forno a lenha",
        desc: "Uma fornada atrás da outra.",
        base: 100.0,
        rate: 1.0,
        ups: [
            "Lenha de eucalipto",
            "Tijolo refratário",
            "Forno de barro duplo",
        ],
    },
    Building {
        id: "padaria",
        icon: "🏪",
        name: "Padaria da esquina",
        desc: "Pão de queijo quentinho de hora em hora.",
        base: 1100.0,
        rate: 8.0,
        ups: ["Balcão de vidro", "Fila na porta", "Vitrine aquecida"],
    },
    Building {
        id: "queijaria",
        icon: "🧀",
        name: "Queijaria na Canastra",
        desc: "Queijo bom faz pão de queijo bom.",
        base: 12000.0,
        rate: 47.0,
        ups: ["Leite da manhã", "Cura de 30 dias", "Selo de origem"],
    },
    Building {
        id: "fabrica",
        icon: "🏭",
        name: "Fábrica de polvilho",
        desc: "Polvilho em escala industrial.",
        base: 130000.0,
        rate: 260.0,
        ups: ["Esteira automática", "Turno da noite", "Polvilho premium"],
    },
    Building {
        id: "trem",
        icon: "🚂",
        name: "Trem Minas–São Paulo",
        desc: "Leva pão de queijo pra todo canto.",
        base: 1.4e6,
        rate: 1400.0,
        ups: [
            "Vagão refrigerado",
            "Linha expressa",
            "Maria-fumaça turbinada",
        ],
    },
    Building {
        id: "exporta",
        icon: "🌎",
        name: "Exportação",
        desc: "O mundo descobriu o pão de queijo.",
        base: 2e7,
        rate: 7800.0,
        ups: [
            "Selo de qualidade",
            "Contêiner climatizado",
            "Embaixada do pão de queijo",
        ],
    },
    Building {
        id: "lua",
        icon: "🌙",
        name: "Padaria na Lua",
        desc: "Sem gravidade, a massa cresce mais.",
        base: 3.3e8,
        rate: 44000.0,
        ups: [
            "Forno em gravidade zero",
            "Queijo lunar",
            "Base no lado escuro",
        ],
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpgradeKind {
    Click,
    Pct,
    /// Doubles the production of the building at this index.
    Building(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unlock {
    Total(f64),
    Owned { building: usize, need: u32 },
}

pub struct Upgrade {
    pub id: String,
    pub icon: &'static str,
    pub name: &'static str,
    pub desc: String,
    pub cost: f64,
    pub kind: UpgradeKind,
    pub unlock: Unlock,
}

impl Upgrade {
    pub fn unlocked(&self, state: &GameState) -> bool {
        match self.unlock {
            Unlock::Total(t) => state.total >= t,
            Unlock::Owned { building, need } => state.owned[building] >= need,
        }
    }
}

fn fixed(
    id: &str,
    icon: &'static str,
    name: &'static str,
    desc: &str,
    cost: f64,
    kind: UpgradeKind,
    unlock: Unlock,
) -> Upgrade {
    Upgrade {
        id: id.to_string(),
        icon,
        name,
        desc: desc.to_string(),
        cost,
        kind,
        unlock,
    }
}

static UPGRADES: LazyLock<Vec<Upgrade>> = LazyLock::new(|| {
    use UpgradeKind::{Click, Pct};
    const CLICK: &str = "Cliques valem o dobro.";
    const PCT: &str = "Cada clique rende +1% da produção por segundo.";
    let mut all = vec![
        fixed(
            "c1",
            "🥄",
            "Polvilho azedo",
            CLICK,
            100.0,
            Click,
            Unlock::Total(50.0),
        ),
        fixed(
            "c2",
            "🧈",
            "Queijo meia-cura",
            CLICK,
            500.0,
            Click,
            Unlock::Total(300.0),
        ),
        fixed(
            "c3",
            "🥚",
            "Ovo caipira",
            CLICK,
            10000.0,
            Click,
            Unlock::Total(5000.0),
        ),
        fixed(
            "p1",
            "🤲",
            "Mão de vó",
            PCT,
            50000.0,
            Pct,
            Unlock::Total(20000.0),
        ),
        fixed(
            "p2",
            "🪵",
            "Rolo de massa de família",
            PCT,
            5e6,
            Pct,
            Unlock::Total(1e6),
        ),
        fixed(
            "p3",
            "📜",
            "Receita secreta da bisa",
            PCT,
            5e8,
            Pct,
            Unlock::Total(1e8),
        ),
    ];
    for (building, b) in BUILDINGS.iter().enumerate() {
        for (tier, t) in TIERS.iter().enumerate() {
            all.push(Upgrade {
                id: format!("{}-{}", b.id, tier),
                icon: b.icon,
                name: b.ups[tier],
                desc: format!("{}: produção ×2.", b.name),
                cost: b.base * t.mult,
                kind: UpgradeKind::Building(building),
                unlock: Unlock::Owned {
                    building,
                    need: t.need,
                },
            });
        }
    }
    all
});

/// Catalog upgrades in prototype order. The index is the bit position in `GameState::ups`.
pub fn upgrades() -> &'static [Upgrade] {
    &UPGRADES
}

/// All amounts are f64. Integers are exact up to 2^53 (about 9 quatrilhões); above
/// that, increments smaller than half an ulp are lost. Values overflow to infinity
/// near 1.8e308 (the UI shows the infinity sign).
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    pub bank: f64,
    pub total: f64,
    pub clicks: u64,
    pub owned: [u32; BUILDING_COUNT],
    /// Bitset over the `upgrades()` index.
    pub ups: u64,
    /// 1, 10 or 100.
    pub buy: u32,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            bank: 0.0,
            total: 0.0,
            clicks: 0,
            owned: [0; BUILDING_COUNT],
            ups: 0,
            buy: 1,
        }
    }
}

impl GameState {
    pub fn has_upgrade(&self, index: usize) -> bool {
        self.ups & (1 << index) != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuffKind {
    Prod,
    Click,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Buff {
    pub kind: BuffKind,
    pub mult: f64,
    pub until_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CafeOutcome {
    ClickFrenzy,
    ProdBoost,
    Lucky(f64),
}

/// Runtime state. Buffs are never saved.
#[derive(Clone, Debug, Default)]
pub struct Game {
    pub state: GameState,
    pub buffs: Vec<Buff>,
}

pub fn building_mult(state: &GameState, building: usize) -> f64 {
    let mut m = 1.0;
    for (i, u) in upgrades().iter().enumerate() {
        if u.kind == UpgradeKind::Building(building) && state.has_upgrade(i) {
            m *= 2.0;
        }
    }
    m
}

pub fn base_pps(state: &GameState) -> f64 {
    BUILDINGS.iter().enumerate().fold(0.0, |sum, (i, b)| {
        sum + state.owned[i] as f64 * b.rate * building_mult(state, i)
    })
}

/// Click value before buffs.
pub fn click_base(state: &GameState) -> f64 {
    let mut cm = 1.0;
    let mut pct = 0.0;
    for (i, u) in upgrades().iter().enumerate() {
        if !state.has_upgrade(i) {
            continue;
        }
        match u.kind {
            UpgradeKind::Click => cm *= 2.0,
            UpgradeKind::Pct => pct += 0.01,
            UpgradeKind::Building(_) => {}
        }
    }
    cm + base_pps(state) * pct
}

/// Same operation order as the prototype. libm::pow, not f64::powf: powf lowers to the
/// platform libm, so native and wasm32 would disagree. libm gives identical results everywhere.
pub fn cost_exact(state: &GameState, building: usize, n: u32) -> f64 {
    BUILDINGS[building].base
        * libm::pow(1.15, state.owned[building] as f64)
        * (libm::pow(1.15, n as f64) - 1.0)
        / 0.15
}

/// Client and server both run this crate, so costs agree with each other. Against the
/// prototype's V8 Math.pow they can differ by 1 for large costs (above about 4e14),
/// because musl pow and V8 pow round the last bit differently.
pub fn cost_for(state: &GameState, building: usize, n: u32) -> f64 {
    cost_exact(state, building, n).ceil()
}

pub fn revealed(state: &GameState, building: usize) -> bool {
    state.owned[building] > 0 || state.total >= BUILDINGS[building].base * 0.4
}

/// Indices into `upgrades()`, cheapest first. The sort is stable, so ties keep catalog order.
pub fn available_upgrades(state: &GameState) -> Vec<usize> {
    let all = upgrades();
    let mut available: Vec<usize> = (0..all.len())
        .filter(|&i| !state.has_upgrade(i) && all[i].unlocked(state))
        .collect();
    available.sort_by(|&a, &b| all[a].cost.total_cmp(&all[b].cost));
    available
}

const MAX_OFFLINE_SECS: f64 = 12.0 * 3600.0;

/// Pure; the caller applies the result to bank and total.
pub fn offline_gain(state: &GameState, away_secs: f64) -> f64 {
    if away_secs.is_nan() || away_secs <= 60.0 {
        return 0.0;
    }
    let earned = base_pps(state) * away_secs.min(MAX_OFFLINE_SECS) * 0.5;
    if earned >= 1.0 { earned } else { 0.0 }
}

impl Game {
    pub fn buff_mult(&self, kind: BuffKind) -> f64 {
        self.buffs
            .iter()
            .filter(|b| b.kind == kind)
            .fold(1.0, |m, b| m * b.mult)
    }

    pub fn pps(&self) -> f64 {
        base_pps(&self.state) * self.buff_mult(BuffKind::Prod)
    }

    pub fn click_value(&self) -> f64 {
        click_base(&self.state) * self.buff_mult(BuffKind::Prod) * self.buff_mult(BuffKind::Click)
    }

    fn gain(&mut self, amount: f64) {
        self.state.bank += amount;
        self.state.total += amount;
    }

    /// Production first, then expiry (prototype frame order). The frame covers
    /// `[now_ms - dt, now_ms]` and a production buff multiplies only the part before its
    /// `until_ms`, so a long hidden-tab frame cannot earn the boost past its expiry.
    /// The prototype applied the buff to the whole frame; this deviation is deliberate.
    pub fn tick(&mut self, dt_secs: f64, now_ms: f64) {
        let dt_secs = dt_secs.max(0.0);
        let start_ms = now_ms - dt_secs * 1000.0;
        let mut cuts: Vec<f64> = self
            .buffs
            .iter()
            .filter(|b| b.kind == BuffKind::Prod && b.until_ms > start_ms && b.until_ms < now_ms)
            .map(|b| b.until_ms)
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.push(now_ms);
        let base = base_pps(&self.state);
        let mut from_ms = start_ms;
        let mut gained = 0.0;
        for to_ms in cuts {
            let mult = self
                .buffs
                .iter()
                .filter(|b| b.kind == BuffKind::Prod && b.until_ms > from_ms)
                .fold(1.0, |m, b| m * b.mult);
            gained += base * mult * (to_ms - from_ms) / 1000.0;
            from_ms = to_ms;
        }
        self.gain(gained);
        self.buffs.retain(|b| b.until_ms > now_ms);
    }

    pub fn click(&mut self) -> f64 {
        let value = self.click_value();
        self.gain(value);
        self.state.clicks += 1;
        value
    }

    pub fn buy_building(&mut self, building: usize) -> bool {
        if building >= BUILDING_COUNT {
            return false;
        }
        let n = self.state.buy;
        let cost = cost_for(&self.state, building, n);
        if self.state.bank.is_nan() || self.state.bank < cost {
            return false;
        }
        self.state.bank -= cost;
        self.state.owned[building] = self.state.owned[building].saturating_add(n);
        true
    }

    pub fn buy_upgrade(&mut self, index: usize) -> bool {
        let Some(upgrade) = upgrades().get(index) else {
            return false;
        };
        if self.state.has_upgrade(index)
            || !upgrade.unlocked(&self.state)
            || self.state.bank.is_nan()
            || self.state.bank < upgrade.cost
        {
            return false;
        }
        self.state.bank -= upgrade.cost;
        self.state.ups |= 1 << index;
        true
    }

    pub fn set_buy(&mut self, n: u32) {
        if matches!(n, 1 | 10 | 100) {
            self.state.buy = n;
        }
    }

    /// `roll` is in [0, 1).
    pub fn cafe(&mut self, roll: f64, now_ms: f64) -> CafeOutcome {
        if roll < 0.1 {
            self.buffs.push(Buff {
                kind: BuffKind::Click,
                mult: 77.0,
                until_ms: now_ms + 13000.0,
            });
            CafeOutcome::ClickFrenzy
        } else if roll < 0.55 {
            self.buffs.push(Buff {
                kind: BuffKind::Prod,
                mult: 7.0,
                until_ms: now_ms + 30000.0,
            });
            CafeOutcome::ProdBoost
        } else {
            let amount = (self.state.bank * 0.15).min(base_pps(&self.state) * 900.0) + 13.0;
            self.gain(amount);
            CafeOutcome::Lucky(amount)
        }
    }

    pub fn reset(&mut self) {
        self.state = GameState::default();
        self.buffs.clear();
    }
}
