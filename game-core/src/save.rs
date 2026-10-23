use crate::{BUILDING_COUNT, BUILDINGS, GameState, base_pps, click_base, cost_exact, upgrades};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SAVE_SCHEMA_VERSION: u32 = 1;

/// Holding Enter on the focused button auto-repeats at about 30 clicks/s and the
/// prototype supports keyboard clicks, so the cap sits above that.
pub const CLICKS_PER_SEC: f64 = 40.0;
/// A page reload resets the first cafe to 30-60 s, so 30 s is the shortest honest gap.
pub const CAFE_MIN_GAP_SECS: f64 = 30.0;
/// Latency skew between consecutive PUTs on mobile networks.
pub const DT_SLACK_SECS: f64 = 10.0;

#[derive(Debug)]
pub enum SaveError {
    UnsupportedVersion(u32),
    Malformed(serde_json::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Violation {
    UnknownBuilding(String),
    UnknownUpgrade(String),
    InvalidBuy(u32),
    InvalidAmount {
        field: &'static str,
    },
    NotMonotone {
        field: &'static str,
    },
    TooManyClicks {
        claimed: u64,
        allowed: u64,
    },
    TotalTooHigh {
        claimed: f64,
        allowed: f64,
    },
    LockedUpgrade(String),
    /// Claimed purchases cost more than the total ever earned; owned and ups were reverted.
    Unaffordable {
        expected: f64,
    },
    Underpaid {
        expected: f64,
        actual: f64,
    },
    BankOutOfRange,
}

/// Prototype `pdq-clicker-v1` localStorage value. Unknown fields (the prototype's `t`) are ignored.
#[derive(Serialize, Deserialize)]
struct SaveV1 {
    #[serde(default, deserialize_with = "nan_on_null")]
    bank: f64,
    #[serde(default, deserialize_with = "nan_on_null")]
    total: f64,
    #[serde(default)]
    clicks: u64,
    #[serde(default)]
    owned: BTreeMap<String, u64>,
    #[serde(default)]
    ups: Vec<String>,
    #[serde(default = "default_buy")]
    buy: u32,
}

/// JSON.stringify writes null for Infinity and NaN. validate replaces the NaN.
fn nan_on_null<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(f64::NAN))
}

fn default_buy() -> u32 {
    1
}

pub fn to_json(state: &GameState) -> String {
    let owned = BUILDINGS
        .iter()
        .zip(state.owned)
        .filter(|&(_, n)| n > 0)
        .map(|(b, n)| (b.id.to_string(), u64::from(n)))
        .collect();
    let ups = upgrades()
        .iter()
        .enumerate()
        .filter(|&(i, _)| state.has_upgrade(i))
        .map(|(_, u)| u.id.clone())
        .collect();
    let save = SaveV1 {
        bank: state.bank,
        total: state.total,
        clicks: state.clicks,
        owned,
        ups,
        buy: state.buy,
    };
    serde_json::to_string(&save).expect("save fields always serialize")
}

pub fn load_save(
    schema_version: u32,
    json: &str,
) -> Result<(GameState, Vec<Violation>), SaveError> {
    match schema_version {
        1 => load_v1(json),
        other => Err(SaveError::UnsupportedVersion(other)),
    }
}

fn load_v1(json: &str) -> Result<(GameState, Vec<Violation>), SaveError> {
    let save: SaveV1 = serde_json::from_str(json).map_err(SaveError::Malformed)?;
    let mut violations = Vec::new();
    let mut state = GameState {
        bank: save.bank,
        total: save.total,
        clicks: save.clicks,
        buy: save.buy,
        ..GameState::default()
    };
    for (id, n) in save.owned {
        match BUILDINGS.iter().position(|b| b.id == id) {
            Some(i) => state.owned[i] = u32::try_from(n).unwrap_or(u32::MAX),
            None => violations.push(Violation::UnknownBuilding(id)),
        }
    }
    for id in save.ups {
        match upgrades().iter().position(|u| u.id == id) {
            Some(i) => state.ups |= 1 << i,
            None => violations.push(Violation::UnknownUpgrade(id)),
        }
    }
    fix_buy(&mut state, &mut violations);
    Ok((state, violations))
}

fn fix_buy(state: &mut GameState, violations: &mut Vec<Violation>) {
    if !matches!(state.buy, 1 | 10 | 100) {
        violations.push(Violation::InvalidBuy(state.buy));
        state.buy = 1;
    }
}

/// Tolerance scales with total too: bank and total round differently once total is large.
/// Written so that NaN and infinity count as underpaid.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn underpaid(state: &GameState, expected: f64) -> bool {
    let actual = state.total - state.bank;
    !(actual >= expected - 1e-9 * expected.max(state.total).max(1.0))
}

fn is_valid_amount(x: f64) -> bool {
    x.is_finite() && x >= 0.0
}

/// Clamps the total gained since `prev` to the largest plausible amount for `next`'s
/// items and clicks; the bank loses the same excess. No-op without elapsed time.
fn clamp_total(
    prev: &GameState,
    next: &mut GameState,
    dt: Option<f64>,
    violations: &mut Vec<Violation>,
) {
    let Some(dt) = dt else { return };
    let pps = base_pps(next);
    let cv = click_base(next);
    // ponytail: every PUT gets one cafe allowance, so frequent PUTs can farm it. Add a server-side cafe credit or a PUT rate limit.
    let n_cafe = (dt / CAFE_MIN_GAP_SECS).floor() + 1.0;
    let click_frenzy = 76.0 * 13.0 * CLICKS_PER_SEC * cv;
    let lucky = 900.0 * pps + 13.0;
    // A prod boost adds 6*30*(pps + 40*cv), always below max(click_frenzy, lucky), so it is omitted.
    let cafe_extra = click_frenzy.max(lucky);
    let d_clicks = (next.clicks - prev.clicks) as f64;
    let bound = pps * dt + d_clicks * cv + n_cafe * cafe_extra;

    let claimed = next.total - prev.total;
    if claimed > bound {
        violations.push(Violation::TotalTooHigh {
            claimed,
            allowed: bound,
        });
        let clamped_total = prev.total + bound;
        next.bank -= next.total - clamped_total;
        next.total = clamped_total;
    }
}

/// Light anti-cheat: never rejects, clamps to the largest plausible value and reports
/// what it changed. `elapsed_secs` is server time since the last accepted save; `None`
/// (first save or local import) skips the time-based bound.
pub fn validate(
    prev: &GameState,
    mut next: GameState,
    elapsed_secs: Option<f64>,
) -> (GameState, Vec<Violation>) {
    let mut violations = Vec::new();

    if !is_valid_amount(next.bank) {
        next.bank = prev.bank;
        violations.push(Violation::InvalidAmount { field: "bank" });
    }
    if !is_valid_amount(next.total) {
        next.total = prev.total;
        violations.push(Violation::InvalidAmount { field: "total" });
    }
    fix_buy(&mut next, &mut violations);

    let mut owned_regressed = false;
    for b in 0..BUILDING_COUNT {
        if next.owned[b] < prev.owned[b] {
            next.owned[b] = prev.owned[b];
            owned_regressed = true;
        }
    }
    if owned_regressed {
        violations.push(Violation::NotMonotone { field: "owned" });
    }
    if prev.ups & !next.ups != 0 {
        next.ups |= prev.ups;
        violations.push(Violation::NotMonotone { field: "ups" });
    }
    if next.clicks < prev.clicks {
        next.clicks = prev.clicks;
        violations.push(Violation::NotMonotone { field: "clicks" });
    }
    if next.total < prev.total {
        next.total = prev.total;
        violations.push(Violation::NotMonotone { field: "total" });
    }

    let dt = elapsed_secs.map(|elapsed| elapsed.max(0.0) + DT_SLACK_SECS);
    if let Some(dt) = dt {
        let allowed_clicks = (CLICKS_PER_SEC * dt).ceil() as u64;
        let claimed = next.clicks - prev.clicks;
        if claimed > allowed_clicks {
            violations.push(Violation::TooManyClicks {
                claimed,
                allowed: allowed_clicks,
            });
            next.clicks = prev.clicks + allowed_clicks;
        }
    }
    clamp_total(prev, &mut next, dt, &mut violations);

    let all = upgrades();
    let mut dropped_locked = false;
    for (i, u) in all.iter().enumerate() {
        if next.has_upgrade(i) && !prev.has_upgrade(i) && !u.unlocked(&next) {
            next.ups &= !(1 << i);
            violations.push(Violation::LockedUpgrade(u.id.clone()));
            dropped_locked = true;
        }
    }
    // The bound depends on the items, so it is recomputed whenever they change.
    if dropped_locked {
        clamp_total(prev, &mut next, dt, &mut violations);
    }

    // total - bank is the cumulative spend. ceil makes honest spending >= the exact cost.
    let mut expected = prev.total - prev.bank;
    for b in 0..BUILDING_COUNT {
        let n = next.owned[b] - prev.owned[b];
        if n > 0 {
            expected += cost_exact(prev, b, n);
        }
    }
    for (i, u) in all.iter().enumerate() {
        if next.has_upgrade(i) && !prev.has_upgrade(i) {
            expected += u.cost;
        }
    }
    if underpaid(&next, expected) {
        if next.total - expected >= 0.0 {
            violations.push(Violation::Underpaid {
                expected,
                actual: next.total - next.bank,
            });
            next.bank = next.total - expected;
        } else {
            violations.push(Violation::Unaffordable { expected });
            next.owned = prev.owned;
            next.ups = prev.ups;
            clamp_total(prev, &mut next, dt, &mut violations);
            let spent = prev.total - prev.bank;
            if underpaid(&next, spent) {
                violations.push(Violation::Underpaid {
                    expected: spent,
                    actual: next.total - next.bank,
                });
                next.bank = next.total - spent;
            }
        }
    }

    if !(0.0..=next.total).contains(&next.bank) {
        next.bank = next.bank.clamp(0.0, next.total);
        violations.push(Violation::BankOutOfRange);
    }

    (next, violations)
}
