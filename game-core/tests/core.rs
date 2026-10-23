use game_core::save::{
    CAFE_CREDIT_START, SAVE_SCHEMA_VERSION, SaveError, Violation, load_save, to_json, validate,
};
use game_core::*;

// Dumped by node from the prototype HTML: B, TIERS and UPGRADES, with every unlock
// function probed for its exact threshold (bisection on total, scan on owned).
type ProtoBuilding = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    f64,
    f64,
    [&'static str; 3],
);
type ProtoUpgrade = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    f64,
    UpgradeKind,
    Unlock,
);

const PROTOTYPE_BUILDINGS: [ProtoBuilding; 8] = [
    (
        "vovo",
        "👵",
        "Vovó mineira",
        "Enrola pãezinhos enquanto conta causo.",
        15.0,
        0.1,
        ["Avental florido", "Rádio na cozinha", "Neta ajudante"],
    ),
    (
        "forno",
        "🔥",
        "Forno a lenha",
        "Uma fornada atrás da outra.",
        100.0,
        1.0,
        [
            "Lenha de eucalipto",
            "Tijolo refratário",
            "Forno de barro duplo",
        ],
    ),
    (
        "padaria",
        "🏪",
        "Padaria da esquina",
        "Pão quentinho de hora em hora.",
        1100.0,
        8.0,
        ["Balcão de vidro", "Fila na porta", "Vitrine aquecida"],
    ),
    (
        "queijaria",
        "🧀",
        "Queijaria na Canastra",
        "Queijo bom faz pão bom.",
        12000.0,
        47.0,
        ["Leite da manhã", "Cura de 30 dias", "Selo de origem"],
    ),
    (
        "fabrica",
        "🏭",
        "Fábrica de polvilho",
        "Polvilho em escala industrial.",
        130000.0,
        260.0,
        ["Esteira automática", "Turno da noite", "Polvilho premium"],
    ),
    (
        "trem",
        "🚂",
        "Trem Minas–São Paulo",
        "Leva pão de queijo pra todo canto.",
        1400000.0,
        1400.0,
        [
            "Vagão refrigerado",
            "Linha expressa",
            "Maria-fumaça turbinada",
        ],
    ),
    (
        "exporta",
        "🌎",
        "Exportação",
        "O mundo descobriu o pão de queijo.",
        20000000.0,
        7800.0,
        [
            "Selo de qualidade",
            "Contêiner climatizado",
            "Embaixada do pão de queijo",
        ],
    ),
    (
        "lua",
        "🌙",
        "Padaria na Lua",
        "Sem gravidade, a massa cresce mais.",
        330000000.0,
        44000.0,
        [
            "Forno em gravidade zero",
            "Queijo lunar",
            "Base no lado escuro",
        ],
    ),
];

const PROTOTYPE_UPGRADES: [ProtoUpgrade; 30] = [
    (
        "c1",
        "🥄",
        "Polvilho azedo",
        "Cliques valem o dobro.",
        100.0,
        UpgradeKind::Click,
        Unlock::Total(50.0),
    ),
    (
        "c2",
        "🧈",
        "Queijo meia-cura",
        "Cliques valem o dobro.",
        500.0,
        UpgradeKind::Click,
        Unlock::Total(300.0),
    ),
    (
        "c3",
        "🥚",
        "Ovo caipira",
        "Cliques valem o dobro.",
        10000.0,
        UpgradeKind::Click,
        Unlock::Total(5000.0),
    ),
    (
        "p1",
        "🤲",
        "Mão de vó",
        "Cada clique rende +1% da produção por segundo.",
        50000.0,
        UpgradeKind::Pct,
        Unlock::Total(20000.0),
    ),
    (
        "p2",
        "🪵",
        "Rolo de massa de família",
        "Cada clique rende +1% da produção por segundo.",
        5000000.0,
        UpgradeKind::Pct,
        Unlock::Total(1000000.0),
    ),
    (
        "p3",
        "📜",
        "Receita secreta da bisa",
        "Cada clique rende +1% da produção por segundo.",
        500000000.0,
        UpgradeKind::Pct,
        Unlock::Total(100000000.0),
    ),
    (
        "vovo-0",
        "👵",
        "Avental florido",
        "Vovó mineira: produção ×2.",
        150.0,
        UpgradeKind::Building(0),
        Unlock::Owned {
            building: 0,
            need: 1,
        },
    ),
    (
        "vovo-1",
        "👵",
        "Rádio na cozinha",
        "Vovó mineira: produção ×2.",
        750.0,
        UpgradeKind::Building(0),
        Unlock::Owned {
            building: 0,
            need: 10,
        },
    ),
    (
        "vovo-2",
        "👵",
        "Neta ajudante",
        "Vovó mineira: produção ×2.",
        7500.0,
        UpgradeKind::Building(0),
        Unlock::Owned {
            building: 0,
            need: 25,
        },
    ),
    (
        "forno-0",
        "🔥",
        "Lenha de eucalipto",
        "Forno a lenha: produção ×2.",
        1000.0,
        UpgradeKind::Building(1),
        Unlock::Owned {
            building: 1,
            need: 1,
        },
    ),
    (
        "forno-1",
        "🔥",
        "Tijolo refratário",
        "Forno a lenha: produção ×2.",
        5000.0,
        UpgradeKind::Building(1),
        Unlock::Owned {
            building: 1,
            need: 10,
        },
    ),
    (
        "forno-2",
        "🔥",
        "Forno de barro duplo",
        "Forno a lenha: produção ×2.",
        50000.0,
        UpgradeKind::Building(1),
        Unlock::Owned {
            building: 1,
            need: 25,
        },
    ),
    (
        "padaria-0",
        "🏪",
        "Balcão de vidro",
        "Padaria da esquina: produção ×2.",
        11000.0,
        UpgradeKind::Building(2),
        Unlock::Owned {
            building: 2,
            need: 1,
        },
    ),
    (
        "padaria-1",
        "🏪",
        "Fila na porta",
        "Padaria da esquina: produção ×2.",
        55000.0,
        UpgradeKind::Building(2),
        Unlock::Owned {
            building: 2,
            need: 10,
        },
    ),
    (
        "padaria-2",
        "🏪",
        "Vitrine aquecida",
        "Padaria da esquina: produção ×2.",
        550000.0,
        UpgradeKind::Building(2),
        Unlock::Owned {
            building: 2,
            need: 25,
        },
    ),
    (
        "queijaria-0",
        "🧀",
        "Leite da manhã",
        "Queijaria na Canastra: produção ×2.",
        120000.0,
        UpgradeKind::Building(3),
        Unlock::Owned {
            building: 3,
            need: 1,
        },
    ),
    (
        "queijaria-1",
        "🧀",
        "Cura de 30 dias",
        "Queijaria na Canastra: produção ×2.",
        600000.0,
        UpgradeKind::Building(3),
        Unlock::Owned {
            building: 3,
            need: 10,
        },
    ),
    (
        "queijaria-2",
        "🧀",
        "Selo de origem",
        "Queijaria na Canastra: produção ×2.",
        6000000.0,
        UpgradeKind::Building(3),
        Unlock::Owned {
            building: 3,
            need: 25,
        },
    ),
    (
        "fabrica-0",
        "🏭",
        "Esteira automática",
        "Fábrica de polvilho: produção ×2.",
        1300000.0,
        UpgradeKind::Building(4),
        Unlock::Owned {
            building: 4,
            need: 1,
        },
    ),
    (
        "fabrica-1",
        "🏭",
        "Turno da noite",
        "Fábrica de polvilho: produção ×2.",
        6500000.0,
        UpgradeKind::Building(4),
        Unlock::Owned {
            building: 4,
            need: 10,
        },
    ),
    (
        "fabrica-2",
        "🏭",
        "Polvilho premium",
        "Fábrica de polvilho: produção ×2.",
        65000000.0,
        UpgradeKind::Building(4),
        Unlock::Owned {
            building: 4,
            need: 25,
        },
    ),
    (
        "trem-0",
        "🚂",
        "Vagão refrigerado",
        "Trem Minas–São Paulo: produção ×2.",
        14000000.0,
        UpgradeKind::Building(5),
        Unlock::Owned {
            building: 5,
            need: 1,
        },
    ),
    (
        "trem-1",
        "🚂",
        "Linha expressa",
        "Trem Minas–São Paulo: produção ×2.",
        70000000.0,
        UpgradeKind::Building(5),
        Unlock::Owned {
            building: 5,
            need: 10,
        },
    ),
    (
        "trem-2",
        "🚂",
        "Maria-fumaça turbinada",
        "Trem Minas–São Paulo: produção ×2.",
        700000000.0,
        UpgradeKind::Building(5),
        Unlock::Owned {
            building: 5,
            need: 25,
        },
    ),
    (
        "exporta-0",
        "🌎",
        "Selo de qualidade",
        "Exportação: produção ×2.",
        200000000.0,
        UpgradeKind::Building(6),
        Unlock::Owned {
            building: 6,
            need: 1,
        },
    ),
    (
        "exporta-1",
        "🌎",
        "Contêiner climatizado",
        "Exportação: produção ×2.",
        1000000000.0,
        UpgradeKind::Building(6),
        Unlock::Owned {
            building: 6,
            need: 10,
        },
    ),
    (
        "exporta-2",
        "🌎",
        "Embaixada do pão de queijo",
        "Exportação: produção ×2.",
        10000000000.0,
        UpgradeKind::Building(6),
        Unlock::Owned {
            building: 6,
            need: 25,
        },
    ),
    (
        "lua-0",
        "🌙",
        "Forno em gravidade zero",
        "Padaria na Lua: produção ×2.",
        3300000000.0,
        UpgradeKind::Building(7),
        Unlock::Owned {
            building: 7,
            need: 1,
        },
    ),
    (
        "lua-1",
        "🌙",
        "Queijo lunar",
        "Padaria na Lua: produção ×2.",
        16500000000.0,
        UpgradeKind::Building(7),
        Unlock::Owned {
            building: 7,
            need: 10,
        },
    ),
    (
        "lua-2",
        "🌙",
        "Base no lado escuro",
        "Padaria na Lua: produção ×2.",
        165000000000.0,
        UpgradeKind::Building(7),
        Unlock::Owned {
            building: 7,
            need: 25,
        },
    ),
];

fn upgrade_index(id: &str) -> usize {
    upgrades().iter().position(|u| u.id == id).unwrap()
}

fn ups_bits(ids: &[&str]) -> u64 {
    ids.iter().fold(0, |bits, id| bits | 1 << upgrade_index(id))
}

fn owned_from(entries: &[(&str, u32)]) -> [u32; BUILDING_COUNT] {
    let mut owned = [0; BUILDING_COUNT];
    for (id, n) in entries {
        owned[BUILDINGS.iter().position(|b| b.id == *id).unwrap()] = *n;
    }
    owned
}

fn state(entries: &[(&str, u32)], ups: &[&str]) -> GameState {
    GameState {
        owned: owned_from(entries),
        ups: ups_bits(ups),
        ..GameState::default()
    }
}

fn funded(bank: f64, total: f64, entries: &[(&str, u32)], ups: &[&str]) -> GameState {
    GameState {
        bank,
        total,
        ..state(entries, ups)
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

fn buff(kind: BuffKind, mult: f64) -> Buff {
    Buff {
        kind,
        mult,
        until_ms: 1.0,
    }
}

#[test]
fn catalog_matches_prototype() {
    for (b, expected) in BUILDINGS.iter().zip(PROTOTYPE_BUILDINGS) {
        assert_eq!(
            (b.id, b.icon, b.name, b.desc, b.base, b.rate, b.ups),
            expected
        );
    }
    assert_eq!(upgrades().len(), PROTOTYPE_UPGRADES.len());
    for (u, expected) in upgrades().iter().zip(PROTOTYPE_UPGRADES) {
        let actual = (
            u.id.as_str(),
            u.icon,
            u.name,
            u.desc.as_str(),
            u.cost,
            u.kind,
            u.unlock,
        );
        assert_eq!(actual, expected);
        let (below, at) = match u.unlock {
            Unlock::Total(t) => (
                funded(0.0, t.next_down(), &[], &[]),
                funded(0.0, t, &[], &[]),
            ),
            Unlock::Owned { building, need } => {
                let mut below = GameState::default();
                below.owned[building] = need - 1;
                let mut at = GameState::default();
                at.owned[building] = need;
                (below, at)
            }
        };
        assert!(!u.unlocked(&below) && u.unlocked(&at), "{}", u.id);
    }
}

#[test]
fn cost_for_matches_prototype() {
    // Expected values come from running the prototype's costFor in node.
    let at_zero: [[f64; 3]; BUILDING_COUNT] = [
        [15.0, 305.0, 117431246.0],
        [100.0, 2031.0, 782874968.0],
        [1100.0, 22335.0, 8611624639.0],
        [12000.0, 243645.0, 93944996057.0],
        [130000.0, 2639484.0, 1017737457274.0],
        [1400000.0, 28425206.0, 10960249539870.0],
        [20000000.0, 406074365.0, 156574993426704.0],
        [330000000.0, 6700227019.0, 2583487391540615.0],
    ];
    for (b, row) in at_zero.iter().enumerate() {
        for (n, expected) in [1, 10, 100].into_iter().zip(row) {
            assert_eq!(
                cost_for(&GameState::default(), b, n),
                *expected,
                "b={b} n={n}"
            );
        }
    }

    // (building, owned, n, expected): large counts, then values whose unrounded cost sits
    // within 1e-15 of an integer, where one ulp of pow error flips the ceil.
    let cases: [(usize, u32, u32, f64); 10] = [
        (0, 50, 10, 330035.0),
        (1, 100, 100, 919340604121432.0),
        (2, 75, 1, 39240155.0),
        (3, 30, 100, 6220264655343.0),
        (7, 25, 100, 85045699035949340.0),
        (0, 106, 100, 318973602245890.0),
        (3, 64, 100, 720336522870470.0),
        (7, 100, 1, 387523438731092.0),
        (6, 101, 10, 548387376802000.0),
        (6, 4, 100, 273850642097014.0),
    ];
    for (b, owned, n, expected) in cases {
        let mut s = GameState::default();
        s.owned[b] = owned;
        assert_eq!(cost_for(&s, b, n), expected, "b={b} owned={owned} n={n}");
    }

    // libm (musl) pow and V8 pow round the last bit differently for some large costs.
    // (building, owned, n, prototype value, our value): ours must be within 1 or 1 ulp.
    let differing: [(usize, u32, u32, f64, f64); 3] = [
        (0, 118, 100, 1706588549059401.0, 1706588549059402.0),
        (5, 200, 10, 39198701820954860000.0, 39198701820954870000.0),
        (0, 400, 100, 2.2331598289837927e+32, 2.233159828983793e+32),
    ];
    for (b, owned, n, prototype, ours) in differing {
        let mut s = GameState::default();
        s.owned[b] = owned;
        let actual = cost_for(&s, b, n);
        assert_eq!(actual, ours, "b={b} owned={owned} n={n}");
        assert!(
            (actual - prototype).abs() <= (prototype * f64::EPSILON).max(1.0),
            "b={b}"
        );
    }
}

#[test]
fn multipliers_match_prototype() {
    // Expected values come from the prototype's bMult, basePps and clickValue in node.
    let scenarios = [
        (
            state(
                &[("vovo", 3), ("forno", 2), ("padaria", 12)],
                &["c1", "c2", "p1", "vovo-0", "vovo-1", "forno-0"],
            ),
            [4.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            101.2,
            // none, prod x7, click x77, prod x7 and click x77
            [
                5.0120000000000005,
                35.084,
                385.92400000000004,
                2701.4680000000003,
            ],
            [101.2, 708.4, 101.2, 708.4],
        ),
        (
            state(
                &[
                    ("vovo", 40),
                    ("forno", 30),
                    ("padaria", 26),
                    ("queijaria", 11),
                    ("fabrica", 1),
                    ("trem", 5),
                    ("exporta", 2),
                    ("lua", 1),
                ],
                &[
                    "c1",
                    "c2",
                    "c3",
                    "p1",
                    "p2",
                    "vovo-0",
                    "vovo-1",
                    "vovo-2",
                    "padaria-0",
                    "padaria-1",
                    "lua-0",
                    "exporta-2",
                ],
            ),
            [8.0, 1.0, 4.0, 1.0, 1.0, 1.0, 2.0, 2.0],
            127871.0,
            [2565.42, 17957.940000000002, 197537.34, 1382761.3800000001],
            [127871.0, 895097.0, 127871.0, 895097.0],
        ),
        (
            GameState::default(),
            [1.0; BUILDING_COUNT],
            0.0,
            [1.0, 7.0, 77.0, 539.0],
            [0.0; 4],
        ),
    ];
    let buff_sets: [Vec<Buff>; 4] = [
        vec![],
        vec![buff(BuffKind::Prod, 7.0)],
        vec![buff(BuffKind::Click, 77.0)],
        vec![buff(BuffKind::Prod, 7.0), buff(BuffKind::Click, 77.0)],
    ];
    for (s, mults, base, clicks, pps) in scenarios {
        for (b, expected) in mults.iter().enumerate() {
            assert_eq!(building_mult(&s, b), *expected, "building_mult b={b}");
        }
        assert_eq!(base_pps(&s), base);
        for (k, buffs) in buff_sets.iter().enumerate() {
            let game = Game {
                state: s.clone(),
                buffs: buffs.clone(),
            };
            assert_eq!(game.click_value(), clicks[k], "click_value buffs={k}");
            assert_eq!(game.pps(), pps[k], "pps buffs={k}");
        }
    }
}

#[test]
fn purchases_deduct_exactly_and_refuse_when_not_allowed() {
    let mut game = Game {
        state: funded(400.0, 400.0, &[], &[]),
        ..Game::default()
    };
    game.set_buy(7);
    assert_eq!(game.state.buy, 1);
    game.set_buy(10);
    assert!(game.buy_building(0));
    assert_eq!((game.state.bank, game.state.owned[0]), (95.0, 10));
    assert!(!game.buy_building(1), "100 needed 2031");
    assert!(!game.buy_building(BUILDING_COUNT));
    assert_eq!((game.state.bank, game.state.owned[1]), (95.0, 0));

    game.state.bank = 304.0;
    game.state.owned[0] = 0;
    assert!(!game.buy_building(0), "x10 costs 305");
    assert_eq!((game.state.bank, game.state.owned[0]), (304.0, 0));

    game.set_buy(100);
    game.state.bank = 117431246.0;
    assert!(game.buy_building(0));
    assert_eq!((game.state.bank, game.state.owned[0]), (0.0, 100));

    let c1 = upgrade_index("c1");
    let mut game = Game {
        state: funded(1000.0, 49.0, &[], &[]),
        ..Game::default()
    };
    assert!(!game.buy_upgrade(c1), "locked below total 50");
    game.state.total = 50.0;
    game.state.bank = 99.0;
    assert!(!game.buy_upgrade(c1), "unlocked but poor");
    game.state.bank = 150.0;
    assert!(game.buy_upgrade(c1));
    assert_eq!(game.state.bank, 50.0);
    assert!(game.state.has_upgrade(c1));
    game.state.bank = 1000.0;
    assert!(!game.buy_upgrade(c1), "already owned");
    assert!(!game.buy_upgrade(upgrades().len()));
    assert!(!game.buy_upgrade(upgrade_index("vovo-0")), "needs 1 vovo");
    game.state.owned[0] = 1;
    assert!(game.buy_upgrade(upgrade_index("vovo-0")));
    assert_eq!(game.state.bank, 850.0);
    assert_eq!(building_mult(&game.state, 0), 2.0);

    // A NaN bank (a null in the save) buys nothing.
    game.state.bank = f64::NAN;
    game.state.owned[1] = 1;
    assert!(!game.buy_building(0));
    assert!(!game.buy_upgrade(upgrade_index("forno-0")));
    assert_eq!(game.state.owned[0], 1);
    assert!(!game.state.has_upgrade(upgrade_index("forno-0")));
}

#[test]
fn available_upgrades_sort_by_cost_with_stable_ties() {
    let s = funded(0.0, 1e6, &[("forno", 25)], &["c1"]);
    let ids: Vec<&str> = available_upgrades(&s)
        .into_iter()
        .map(|i| upgrades()[i].id.as_str())
        .collect();
    // p1 and forno-2 both cost 50000; p1 is earlier in the catalog.
    assert_eq!(
        ids,
        ["c2", "forno-0", "forno-1", "c3", "p1", "forno-2", "p2"]
    );

    assert!(!revealed(&funded(0.0, 439.9, &[], &[]), 2));
    assert!(revealed(&funded(0.0, 440.0, &[], &[]), 2));
    assert!(revealed(&state(&[("lua", 1)], &[]), 7));
}

#[test]
fn cafe_rolls_buffs_and_lucky_gain() {
    let owned_100 = |bank: f64| Game {
        state: funded(bank, bank, &[("vovo", 100)], &[]),
        ..Game::default()
    };

    let mut game = owned_100(1000.0);
    assert_eq!(
        game.cafe(0.1_f64.next_down(), 1000.0),
        CafeOutcome::ClickFrenzy
    );
    assert_eq!(
        game.buffs,
        [Buff {
            kind: BuffKind::Click,
            mult: 77.0,
            until_ms: 14000.0
        }]
    );
    assert_eq!(game.click(), 77.0);

    let mut game = owned_100(1000.0);
    assert_eq!(game.cafe(0.1, 1000.0), CafeOutcome::ProdBoost);
    assert_eq!(
        game.cafe(0.55_f64.next_down(), 1000.0),
        CafeOutcome::ProdBoost
    );
    assert_eq!(game.buffs.len(), 2);
    assert_eq!(game.buffs[1].until_ms, 31000.0);
    assert_eq!(game.pps(), 10.0 * 49.0);
    assert_eq!(game.state.bank, 1000.0);

    // Values from the prototype formula min(bank*.15, basePps*900)+13.
    let mut game = owned_100(1000.0);
    assert_eq!(game.cafe(0.55, 0.0), CafeOutcome::Lucky(163.0));
    assert_eq!((game.state.bank, game.state.total), (1163.0, 1163.0));
    let mut game = owned_100(1e6);
    assert_eq!(game.cafe(0.99, 0.0), CafeOutcome::Lucky(9013.0));
    assert_eq!(game.state.total, 1e6 + 9013.0);
    assert!(game.buffs.is_empty());

    // Prod buff applies during the tick that expires it, not after.
    let mut game = owned_100(0.0);
    game.cafe(0.3, 1000.0);
    game.tick(1.0, 30999.0);
    assert_eq!((game.state.bank, game.buffs.len()), (70.0, 1));
    game.tick(1.0, 31000.0);
    assert_eq!((game.state.bank, game.buffs.len()), (140.0, 0));
    game.tick(1.0, 32000.0);
    assert_eq!(game.state.bank, 150.0);
    game.tick(-5.0, 33000.0);
    assert_eq!(game.state.bank, 150.0);

    game.reset();
    assert_eq!(game.state, GameState::default());
    assert!(game.buffs.is_empty());
}

#[test]
fn prod_buff_pays_only_until_it_expires_within_a_long_frame() {
    let state = funded(0.0, 0.0, &[("forno", 10)], &[]);
    let pps = base_pps(&state);
    let mut game = Game {
        state,
        ..Game::default()
    };
    game.cafe(0.3, 0.0);
    // Frame [0, 300 s] but the x7 boost ends at 30 s: 7*pps*30 + pps*270.
    game.tick(300.0, 300_000.0);
    assert!(close(game.state.total, 7.0 * pps * 30.0 + pps * 270.0));
    assert!(game.buffs.is_empty());

    // 10 s left at the start of a 300 s frame.
    let mut game = Game {
        state: funded(0.0, 0.0, &[("forno", 10)], &[]),
        ..Game::default()
    };
    game.buffs.push(Buff {
        kind: BuffKind::Prod,
        mult: 7.0,
        until_ms: 10_000.0,
    });
    game.tick(300.0, 300_000.0);
    assert!(close(game.state.total, 7.0 * pps * 10.0 + pps * 290.0));

    // Two overlapping boosts multiply while both are active.
    let mut game = Game {
        state: funded(0.0, 0.0, &[("forno", 10)], &[]),
        ..Game::default()
    };
    for until_ms in [10_000.0, 20_000.0] {
        game.buffs.push(Buff {
            kind: BuffKind::Prod,
            mult: 7.0,
            until_ms,
        });
    }
    game.tick(30.0, 30_000.0);
    assert!(close(
        game.state.total,
        49.0 * pps * 10.0 + 7.0 * pps * 10.0 + pps * 10.0
    ));
}

#[test]
fn offline_gain_caps_and_thresholds() {
    // Values from the prototype's offline math with 100 vovo (10/s).
    let s = state(&[("vovo", 100)], &[]);
    assert_eq!(offline_gain(&s, 60.0), 0.0);
    assert_eq!(offline_gain(&s, 60.5), 302.5);
    assert_eq!(offline_gain(&s, 61.0), 305.0);
    assert_eq!(offline_gain(&s, 3600.0), 18000.0);
    assert_eq!(offline_gain(&s, 43200.0), 216000.0);
    assert_eq!(offline_gain(&s, 200000.0), 216000.0);
    assert_eq!(offline_gain(&GameState::default(), 3600.0), 0.0);
    assert_eq!(offline_gain(&s, f64::NAN), 0.0);
    assert_eq!(offline_gain(&s, f64::INFINITY), 216000.0);
    // Smallest producer (1 vovo, 0.1/s) at 61 s earns 3.05, above the 1 pao floor.
    assert!(close(offline_gain(&state(&[("vovo", 1)], &[]), 61.0), 3.05));
}

#[test]
fn prototype_save_loads_and_round_trips() {
    let prototype = r#"{"bank":1234.5,"total":5000,"clicks":77,"owned":{"vovo":3,"forno":1,"ghost":9},"ups":["c1","vovo-0","nope"],"buy":10,"t":1700000000000}"#;
    let (loaded, violations) = load_save(SAVE_SCHEMA_VERSION, prototype).unwrap();
    assert_eq!(
        loaded,
        GameState {
            bank: 1234.5,
            total: 5000.0,
            clicks: 77,
            owned: owned_from(&[("vovo", 3), ("forno", 1)]),
            ups: ups_bits(&["c1", "vovo-0"]),
            buy: 10,
        }
    );
    assert_eq!(
        violations,
        [
            Violation::UnknownBuilding("ghost".into()),
            Violation::UnknownUpgrade("nope".into())
        ]
    );

    let json = to_json(&loaded);
    assert_eq!(
        json,
        r#"{"bank":1234.5,"total":5000.0,"clicks":77,"owned":{"forno":1,"vovo":3},"ups":["c1","vovo-0"],"buy":10}"#
    );
    let (again, violations) = load_save(SAVE_SCHEMA_VERSION, &json).unwrap();
    assert_eq!((again, violations), (loaded, vec![]));

    let awkward = funded(
        0.1 + 0.2,
        1e300 / 3.0,
        &[("lua", u32::MAX)],
        &["p3", "lua-2"],
    );
    assert_eq!(load_save(1, &to_json(&awkward)).unwrap(), (awkward, vec![]));

    let (defaults, violations) = load_save(1, "{}").unwrap();
    assert_eq!((defaults, violations), (GameState::default(), vec![]));
    let (fixed, violations) = load_save(1, r#"{"buy":7}"#).unwrap();
    assert_eq!(fixed.buy, 1);
    assert_eq!(violations, [Violation::InvalidBuy(7)]);
}

/// `validate` with an explicit credit, as a tuple.
fn vc(
    prev: &GameState,
    next: GameState,
    elapsed: Option<f64>,
    credit: f64,
) -> (GameState, Vec<Violation>) {
    let v = validate(prev, next, elapsed, credit);
    (v.state, v.violations)
}

/// `validate` with the credit of a new row.
fn vd(prev: &GameState, next: GameState, elapsed: Option<f64>) -> (GameState, Vec<Violation>) {
    vc(prev, next, elapsed, CAFE_CREDIT_START)
}

#[test]
fn load_save_rejects_bad_version_and_shapes() {
    assert!(matches!(
        load_save(2, "{}"),
        Err(SaveError::UnsupportedVersion(2))
    ));
    assert!(matches!(
        load_save(0, "{}"),
        Err(SaveError::UnsupportedVersion(0))
    ));
    for bad in [
        "not json",
        r#"{"bank":"lots"}"#,
        r#"{"owned":{"vovo":-1}}"#,
        r#"{"ups":"c1"}"#,
    ] {
        assert!(
            matches!(load_save(1, bad), Err(SaveError::Malformed(_))),
            "{bad}"
        );
    }

    // JSON.stringify writes null for Infinity and NaN: loads as NaN, validate replaces it.
    let (loaded, _) = load_save(1, r#"{"bank":null,"total":50}"#).unwrap();
    assert!(loaded.bank.is_nan());
    let (out, v) = vd(&GameState::default(), loaded, None);
    assert_eq!((out.bank, out.total), (0.0, 50.0));
    assert_eq!(v, [Violation::InvalidAmount { field: "bank" }]);
}

#[test]
fn save_round_trips_exactly_and_maxed_save_fits_2kb() {
    // Without serde_json's float_roundtrip about 9% of random f64 drift by one ulp.
    let mut bits = 0x9E3779B97F4A7C15u64;
    for _ in 0..20000 {
        bits = bits
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = f64::from_bits(bits >> 2 | 0x3000_0000_0000_0000);
        assert!(x.is_finite());
        let s = funded(x, x * 1.7, &[], &[]);
        assert_eq!(load_save(1, &to_json(&s)).unwrap(), (s, vec![]), "{x:e}");
    }

    let maxed = GameState {
        bank: 1.7976931348623157e308,
        total: 1.7976931348623157e308,
        clicks: u64::MAX,
        owned: [u32::MAX; BUILDING_COUNT],
        ups: (1u64 << upgrades().len()) - 1,
        buy: 100,
    };
    let json = to_json(&maxed);
    assert!(json.len() < 2048, "{} bytes", json.len());
    assert_eq!(load_save(1, &json).unwrap(), (maxed, vec![]));
}

const STEP: f64 = 0.025;

struct Client {
    game: Game,
    accepted: GameState,
    cafe_credit: f64,
    violations: Vec<Violation>,
    now_ms: f64,
    since_save: f64,
    steps: u64,
    shopping: bool,
    building_upgrades_only: bool,
}

impl Client {
    fn new(state: GameState) -> Self {
        let mut client = Client {
            game: Game {
                state,
                ..Game::default()
            },
            accepted: GameState::default(),
            cafe_credit: CAFE_CREDIT_START,
            violations: Vec::new(),
            now_ms: 0.0,
            since_save: 0.0,
            steps: 0,
            shopping: true,
            building_upgrades_only: false,
        };
        client.submit(None);
        client
    }

    fn submit(&mut self, elapsed: Option<f64>) {
        let json = to_json(&self.game.state);
        let (loaded, mut violations) = load_save(SAVE_SCHEMA_VERSION, &json).unwrap();
        let checked = validate(&self.accepted, loaded, elapsed, self.cafe_credit);
        violations.extend(checked.violations);
        assert_eq!(
            checked.state, self.game.state,
            "validation changed an honest save"
        );
        self.accepted = checked.state;
        self.cafe_credit = checked.cafe_credit;
        self.violations.extend(violations);
    }

    /// `skew` is how much shorter the server's elapsed time is than the client's.
    fn save(&mut self, skew: f64) {
        let elapsed = (self.since_save - skew).max(0.0);
        self.since_save = 0.0;
        self.submit(Some(elapsed));
    }

    fn cafe(&mut self, roll: f64) -> CafeOutcome {
        self.game.cafe(roll, self.now_ms)
    }

    /// Frame loop at 40 fps. `clicks_per_sec` is 0, 20 or 40. Shops once per second.
    fn play(&mut self, secs: f64, clicks_per_sec: u64) {
        for _ in 0..(secs / STEP).round() as u64 {
            self.steps += 1;
            self.now_ms += STEP * 1000.0;
            self.since_save += STEP;
            self.game.tick(STEP, self.now_ms);
            if clicks_per_sec > 0 && self.steps.is_multiple_of(40 / clicks_per_sec) {
                self.game.click();
            }
            if self.shopping && self.steps.is_multiple_of(40) {
                self.shop();
            }
        }
    }

    /// One frame after a long hidden-tab pause.
    fn hidden_frame(&mut self, secs: f64) {
        self.now_ms += secs * 1000.0;
        self.since_save += secs;
        self.game.tick(secs, self.now_ms);
    }

    /// Reload after `secs` away: offline gain applied, buffs lost.
    fn away(&mut self, secs: f64) -> f64 {
        let earned = offline_gain(&self.game.state, secs);
        self.game.state.bank += earned;
        self.game.state.total += earned;
        self.game.buffs.clear();
        self.now_ms += secs * 1000.0;
        self.since_save += secs;
        earned
    }

    fn shop(&mut self) {
        for i in available_upgrades(&self.game.state) {
            if self.building_upgrades_only
                && !matches!(upgrades()[i].kind, UpgradeKind::Building(_))
            {
                continue;
            }
            self.game.buy_upgrade(i);
        }
        for b in (0..BUILDING_COUNT).rev() {
            self.game.buy_building(b);
        }
    }
}

#[test]
fn honest_session_produces_no_violations() {
    let mut c = Client::new(GameState::default());

    // Early game, where production is tiny next to click value.
    c.play(25.0, 20);
    c.save(0.0);

    // Two click frenzies at 40 clicks/s, a reload between them (buffs lost, first cafe
    // 30 s after reload), one save covering both. The player saves up instead of
    // shopping, so the bound is near-tight: spending mid-window would raise the final
    // pps and click value and loosen it.
    c.shopping = false;
    assert_eq!(c.cafe(0.05), CafeOutcome::ClickFrenzy);
    c.play(13.0, 40);
    c.game.buffs.clear();
    c.play(30.0, 20);
    assert_eq!(c.cafe(0.05), CafeOutcome::ClickFrenzy);
    c.play(13.0, 40);
    c.save(0.0);
    c.shopping = true;

    // Prod buff active across a 60 s hidden-tab frame.
    assert_eq!(c.cafe(0.3), CafeOutcome::ProdBoost);
    c.play(5.0, 20);
    c.hidden_frame(60.0);
    c.play(5.0, 20);
    c.save(0.0);

    // A boost with 30 s left, then a 600 s hidden frame: it pays only until it expires.
    assert_eq!(c.cafe(0.3), CafeOutcome::ProdBoost);
    c.hidden_frame(600.0);
    c.save(0.0);

    // Latency skew: the server sees 8 s less, then 8 s more.
    c.play(20.0, 40);
    c.save(8.0);
    c.play(20.0, 40);
    c.save(-8.0);

    // Offline gap; the server's elapsed covers it.
    assert!(c.away(7200.0) > 0.0);
    c.save(0.0);
    assert!(c.game.state.owned.iter().any(|&n| n >= 10));

    // Rich player: x100 purchases, then a lucky cafe at its cap saved 1 s later. No click
    // or pct upgrades, so the lucky term is the largest and the 1 s window is tight.
    let mut rich = Client::new(funded(1e14, 1e14, &[], &[]));
    rich.building_upgrades_only = true;
    rich.game.set_buy(100);
    rich.play(30.0, 20);
    rich.save(0.0);
    let pps = base_pps(&rich.game.state);
    assert!(
        rich.game.state.bank * 0.15 > pps * 900.0,
        "cafe not at its cap"
    );
    assert_eq!(rich.cafe(0.9), CafeOutcome::Lucky(pps * 900.0 + 13.0));
    rich.play(1.0, 40);
    rich.save(0.0);
    assert!(rich.game.state.owned.iter().filter(|&&n| n >= 100).count() >= 3);

    assert_eq!(c.violations, []);
    assert_eq!(rich.violations, []);
}

#[test]
fn validate_bound_is_exact_and_clamps_inflated_total_and_clicks() {
    // Slack 10 s and 40 clicks/s; elapsed 0 means a 10 s window and no credit refill.
    // No production: one cafe frenzy, 76 * 13 * 40 * 1 = 39520, is the whole allowance.
    let claim = funded(1e9, 1e9, &[], &[]);
    let v = validate(&GameState::default(), claim.clone(), Some(0.0), 1.0);
    assert_eq!((v.state.total, v.state.bank), (39520.0, 39520.0));
    assert_eq!(
        v.violations,
        [Violation::TotalTooHigh {
            claimed: 1e9,
            allowed: 39520.0
        }]
    );
    assert_eq!(v.cafe_credit, 0.0);

    // The allowance is the credit: refilled by raw elapsed / 30 s and capped at 2.
    let allowed = |elapsed, credit| {
        let v = validate(&GameState::default(), claim.clone(), Some(elapsed), credit);
        (v.state.total, v.cafe_credit)
    };
    assert_eq!(allowed(0.0, 0.0), (0.0, 0.0));
    assert_eq!(allowed(15.0, 0.0), (0.5 * 39520.0, 0.0));
    assert_eq!(allowed(30.0, 0.0), (39520.0, 0.0));
    assert_eq!(allowed(60.0, 0.0), (2.0 * 39520.0, 0.0));
    assert_eq!(allowed(6000.0, 0.0), (2.0 * 39520.0, 0.0));
    assert_eq!(allowed(0.0, 1.5), (1.5 * 39520.0, 0.0));
    assert_eq!(allowed(0.0, 5.0), (2.0 * 39520.0, 0.0));
    // Slack widens the production and click window, never the credit.
    assert!(allowed(29.9, 0.0).0 < 39520.0);

    // 10 vovo/s, 10 s window: 400 clicks, base = 10*10 + 400*1 = 500, bound = 500 + 39520.
    let prev = funded(1e6, 1e6, &[("vovo", 100)], &[]);
    let mut cheat = funded(1e6 + 1e7, 1e6 + 1e7, &[("vovo", 100)], &[]);
    cheat.clicks = 100000;
    let (out, v) = vc(&prev, cheat, Some(0.0), 1.0);
    assert_eq!(
        (out.clicks, out.total, out.bank),
        (400, 1040020.0, 1040020.0)
    );
    assert_eq!(
        v,
        [
            Violation::TooManyClicks {
                claimed: 100000,
                allowed: 400
            },
            Violation::TotalTooHigh {
                claimed: 1e7,
                allowed: 40020.0
            }
        ]
    );

    // Without clicks the bound is 10*10 + 39520. A total exactly at it is untouched and
    // spends the whole credit.
    let at_bound = funded(1e6 + 39620.0, 1e6 + 39620.0, &[("vovo", 100)], &[]);
    let v = validate(&prev, at_bound.clone(), Some(0.0), 1.0);
    assert_eq!(
        (v.state, v.violations, v.cafe_credit),
        (at_bound, vec![], 0.0)
    );

    // Gain within base spends no credit.
    let within = funded(1e6 + 50.0, 1e6 + 50.0, &[("vovo", 100)], &[]);
    let v = validate(&prev, within, Some(0.0), 1.0);
    assert_eq!((v.violations, v.cafe_credit), (vec![], 1.0));

    // Fake items must not raise the bound. Every building and upgrade claimed at once is
    // reverted as unaffordable, and the total is clamped again with the real (empty) items.
    let mut fake = funded(2e18, 2e18, &[], &[]);
    fake.owned = [u32::MAX; BUILDING_COUNT];
    fake.ups = (1u64 << upgrades().len()) - 1;
    let v = validate(&GameState::default(), fake, Some(0.0), 1.0);
    let (out, violations) = (v.state, v.violations);
    assert_eq!(
        (out.total, out.owned, out.ups),
        (39520.0, [0; BUILDING_COUNT], 0)
    );
    assert!(
        out.bank <= out.total && out.bank >= out.total - 1024.0,
        "{}",
        out.bank
    );
    assert!(violations.contains(&Violation::Unaffordable {
        expected: f64::INFINITY
    }));
    assert!(matches!(
        violations.last(),
        Some(Violation::TotalTooHigh {
            allowed: 39520.0,
            ..
        })
    ));
    // The credit is charged for the final (reverted) state, not the fake one.
    assert_eq!(v.cafe_credit, 0.0);

    // Locked upgrades raise pps and click value only until they are dropped.
    // With 5 forno (5/s): forno-1, forno-2 and p3 give pps 20 and cv 1.2, bound
    // 20*10 + 39520*1.2 = 47624. Once dropped: 5*10 + 39520 = 39570.
    let prev = funded(0.0, 0.0, &[("forno", 5)], &[]);
    let cheat = funded(1e12, 1e12, &[("forno", 5)], &["forno-1", "forno-2", "p3"]);
    let v = validate(&prev, cheat, Some(0.0), 1.0);
    assert_eq!(
        (v.state.total, v.state.bank, v.state.ups),
        (39570.0, 39570.0, 0)
    );
    assert_eq!(
        v.violations,
        [
            Violation::TotalTooHigh {
                claimed: 1e12,
                allowed: 47624.0
            },
            Violation::LockedUpgrade("p3".into()),
            Violation::LockedUpgrade("forno-1".into()),
            Violation::LockedUpgrade("forno-2".into()),
            Violation::TotalTooHigh {
                claimed: 47624.0,
                allowed: 39570.0
            },
        ]
    );
    assert_eq!(v.cafe_credit, 0.0);

    // High production: the lucky cafe (900 * 1000 + 13) beats the frenzy,
    // bound = 1000*10 + 900013.
    let prev = funded(0.0, 0.0, &[("vovo", 10000)], &[]);
    let (out, _) = vc(
        &prev,
        funded(1e12, 1e12, &[("vovo", 10000)], &[]),
        Some(0.0),
        1.0,
    );
    assert_eq!((out.total, out.bank), (910013.0, 910013.0));
}

#[test]
fn frequent_puts_cannot_farm_cafe_allowances() {
    // 100 vovo (10/s). The worker refuses PUTs closer than 5 s, so the fastest farmer saves
    // every 5 s and claims 1e9 more than the last accepted total each time.
    // Per-PUT ceiling: pps * (elapsed + 10 s slack) + elapsed/30 of a cafe. The slack is
    // granted on every PUT, so it is still farmable: at most 10 s of production per 5 s
    // (3x production), bounded by the 5 s floor, not by the credit.
    let items = [("vovo", 100)];
    let mut accepted = funded(1e6, 1e6, &items, &[]);
    let mut credit = CAFE_CREDIT_START;
    let gap = 5.0;
    let base = 10.0 * (gap + 10.0);
    let extra = 76.0 * 13.0 * 40.0;
    let mut gains = Vec::new();
    for _ in 0..200 {
        let claim = funded(accepted.total + 1e9, accepted.total + 1e9, &items, &[]);
        let v = validate(&accepted, claim, Some(gap), credit);
        assert!(
            v.violations
                .iter()
                .any(|x| matches!(x, Violation::TotalTooHigh { .. }))
        );
        gains.push(v.state.total - accepted.total);
        accepted = v.state;
        credit = v.cafe_credit;
    }
    // The first PUT spends the starting allowance plus its own refill.
    let first = base + (CAFE_CREDIT_START + gap / 30.0) * extra;
    assert!(close(gains[0], first), "{} vs {first}", gains[0]);
    // Afterwards each PUT gets the ceiling and nothing more: no full cafe per PUT.
    let steady = base + gap / 30.0 * extra;
    for g in &gains[1..] {
        assert!(close(*g, steady), "{g} vs {steady}");
    }
    let sum: f64 = gains.iter().sum();
    let ceiling = 200.0 * base + (CAFE_CREDIT_START + 200.0 * gap / 30.0) * extra;
    assert!(sum <= ceiling * (1.0 + 1e-9), "{sum} > {ceiling}");
}

#[test]
fn validate_enforces_monotone_unlock_and_spending() {
    let c1 = ups_bits(&["c1"]);

    let mut prev = funded(500.0, 1000.0, &[("vovo", 5)], &["c1"]);
    prev.clicks = 100;
    let mut regressed = funded(500.0, 900.0, &[("vovo", 2)], &[]);
    regressed.clicks = 50;
    let (out, v) = vd(&prev, regressed, Some(100.0));
    assert_eq!(out, prev);
    assert_eq!(
        v,
        [
            Violation::NotMonotone { field: "owned" },
            Violation::NotMonotone { field: "ups" },
            Violation::NotMonotone { field: "clicks" },
            Violation::NotMonotone { field: "total" },
        ]
    );

    // c1 was paid for (100) and stays; c3 needs total 5000 and is dropped unpaid.
    let prev = funded(200.0, 200.0, &[], &[]);
    let next = funded(100.0, 200.0, &[], &["c1", "c3"]);
    let (out, v) = vd(&prev, next, Some(10.0));
    assert_eq!((out.ups, out.bank), (c1, 100.0));
    assert_eq!(v, [Violation::LockedUpgrade("c3".into())]);

    // Honest x10 purchase: ceil(304.55) = 305 paid, no violation.
    let prev = funded(400.0, 400.0, &[], &[]);
    let paid = funded(95.0, 400.0, &[("vovo", 10)], &[]);
    let (out, v) = vd(&prev, paid.clone(), Some(60.0));
    assert_eq!((out, v), (paid, vec![]));

    // Same purchase without paying and affordable: items stay, bank is rebuilt from the
    // invariant. Exact cost from the prototype JS.
    let prev = funded(1000.0, 1000.0, &[], &[]);
    let (out, v) = vd(
        &prev,
        funded(1000.0, 1000.0, &[("vovo", 10)], &[]),
        Some(60.0),
    );
    assert_eq!(out.owned, owned_from(&[("vovo", 10)]));
    assert!(close(out.bank, 1000.0 - 304.5557735707907), "{}", out.bank);
    match v.as_slice() {
        [Violation::Underpaid { expected, actual }] => {
            assert!(close(*expected, 304.5557735707907));
            assert_eq!(*actual, 0.0);
        }
        other => panic!("{other:?}"),
    }

    // Local import (no elapsed): no time bound, but spending must still add up.
    // 50 vovo exact cost from the prototype JS, plus vovo-0 at 150.
    let import = funded(1e6, 1e6, &[("vovo", 50)], &["vovo-0"]);
    let (out, v) = vd(&GameState::default(), import, None);
    assert!(
        close(out.bank, 1e6 - (108265.74415839535 + 150.0)),
        "{}",
        out.bank
    );
    assert_eq!((out.total, out.owned[0]), (1e6, 50));
    assert!(matches!(v.as_slice(), [Violation::Underpaid { .. }]));

    // Unaffordable claims are reverted. 1.15^10000 overflows, so the cost is infinite
    // and every NaN-prone comparison must still flag it.
    let prev = funded(1e6, 1e6, &[], &[]);
    let (out, v) = vd(&prev, funded(1e6, 1e6, &[("lua", 10000)], &[]), Some(10.0));
    assert_eq!(out, prev);
    assert_eq!(
        v,
        [Violation::Unaffordable {
            expected: f64::INFINITY
        }]
    );

    // Finite but beyond the total ever earned (cost 2583487391540615).
    let (out, v) = vd(&prev, funded(1e6, 1e6, &[("lua", 100)], &[]), Some(10.0));
    assert_eq!(out, prev);
    match v.as_slice() {
        [Violation::Unaffordable { expected }] => assert!(close(*expected, 2583487391540615.0)),
        other => panic!("{other:?}"),
    }

    // Unaffordable upgrade alone (150 > total 100).
    let prev = funded(100.0, 100.0, &[("vovo", 1)], &[]);
    let (out, v) = vd(
        &prev,
        funded(100.0, 100.0, &[("vovo", 1)], &["vovo-0"]),
        Some(10.0),
    );
    assert_eq!(out, prev);
    assert_eq!(v, [Violation::Unaffordable { expected: 150.0 }]);

    // After the revert the bank is checked against the previous spend: it cannot rise
    // from 5e5 to 9e5 without income.
    let prev = funded(5e5, 1e6, &[], &[]);
    let (out, v) = vd(&prev, funded(9e5, 1e6, &[("lua", 100)], &[]), Some(10.0));
    assert_eq!((out.owned, out.bank), (prev.owned, 5e5));
    assert!(matches!(
        v.as_slice(),
        [
            Violation::Unaffordable { .. },
            Violation::Underpaid {
                expected: 5e5,
                actual: 1e5
            }
        ]
    ));

    let prev = funded(10.0, 20.0, &[], &[]);
    let junk = GameState {
        bank: f64::NAN,
        total: -5.0,
        buy: 7,
        ..GameState::default()
    };
    let (out, v) = vd(&prev, junk, None);
    assert_eq!((out.bank, out.total, out.buy), (10.0, 20.0, 1));
    assert_eq!(
        v,
        [
            Violation::InvalidAmount { field: "bank" },
            Violation::InvalidAmount { field: "total" },
            Violation::InvalidBuy(7)
        ]
    );

    // Final clamp, reachable only when the previous state is itself inconsistent.
    let corrupt = funded(500.0, 100.0, &[], &[]);
    let (out, v) = vd(&corrupt, corrupt.clone(), None);
    assert_eq!(out.bank, 100.0);
    assert_eq!(v, [Violation::BankOutOfRange]);
}
