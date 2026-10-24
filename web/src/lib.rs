use game_core::{
    BUILDINGS, BuffKind, CafeOutcome, Game, GameState, available_upgrades, base_pps, building_mult,
    cost_for, revealed,
    save::{CAFE_CREDIT_START, SAVE_SCHEMA_VERSION, load_save, to_json, validate},
    upgrades,
};
use serde_json::json;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Client {
    game: Game,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Client {
        Client {
            game: Game::default(),
        }
    }

    pub fn load(&mut self, json: &str) -> Result<(), JsError> {
        let (state, _) = load_save(1, json).map_err(|e| JsError::new(&e.to_string()))?;
        self.game.state = validate(&GameState::default(), state, None, CAFE_CREDIT_START).state;
        Ok(())
    }

    pub fn to_json(&self) -> String {
        to_json(&self.game.state)
    }

    pub fn tick(&mut self, dt_secs: f64, now_ms: f64) {
        self.game.tick(dt_secs, now_ms);
    }

    pub fn click(&mut self) -> f64 {
        self.game.click()
    }

    pub fn buy_building(&mut self, building: u32) -> bool {
        self.game.buy_building(building as usize)
    }

    pub fn buy_upgrade(&mut self, index: u32) -> bool {
        self.game.buy_upgrade(index as usize)
    }

    pub fn set_buy(&mut self, n: u32) {
        self.game.set_buy(n);
    }

    pub fn buy(&self) -> u32 {
        self.game.state.buy
    }

    /// Returns [kind, lucky_amount]. Kind: 0 click frenzy, 1 production boost, 2 lucky (amount set).
    pub fn cafe(&mut self, roll: f64, now_ms: f64) -> Vec<f64> {
        match self.game.cafe(roll, now_ms) {
            CafeOutcome::ClickFrenzy => vec![0.0, 0.0],
            CafeOutcome::ProdBoost => vec![1.0, 0.0],
            CafeOutcome::Lucky(amount) => vec![2.0, amount],
        }
    }

    pub fn reset(&mut self) {
        self.game.reset();
    }

    pub fn bank(&self) -> f64 {
        self.game.state.bank
    }

    pub fn total(&self) -> f64 {
        self.game.state.total
    }

    pub fn clicks(&self) -> f64 {
        self.game.state.clicks as f64
    }

    pub fn owned(&self, building: u32) -> u32 {
        self.game.state.owned[building as usize]
    }

    pub fn pps(&self) -> f64 {
        self.game.pps()
    }

    /// Value of one click including active buffs.
    pub fn click_value(&self) -> f64 {
        self.game.click_value()
    }

    /// Production per second without buffs.
    pub fn base_pps(&self) -> f64 {
        base_pps(&self.game.state)
    }

    pub fn building_rate(&self, building: u32) -> f64 {
        let i = building as usize;
        BUILDINGS[i].rate * building_mult(&self.game.state, i)
    }

    pub fn cost(&self, building: u32) -> f64 {
        cost_for(&self.game.state, building as usize, self.game.state.buy)
    }

    pub fn revealed(&self, building: u32) -> bool {
        revealed(&self.game.state, building as usize)
    }

    pub fn available_upgrades(&self) -> Vec<u32> {
        available_upgrades(&self.game.state)
            .into_iter()
            .map(|i| i as u32)
            .collect()
    }

    /// Flat (kind, mult, until_ms) triples. Kind: 0 production, 1 click.
    pub fn buffs(&self) -> Vec<f64> {
        self.game
            .buffs
            .iter()
            .flat_map(|b| {
                let kind = match b.kind {
                    BuffKind::Prod => 0.0,
                    BuffKind::Click => 1.0,
                };
                [kind, b.mult, b.until_ms]
            })
            .collect()
    }
}

#[wasm_bindgen]
pub fn schema_version() -> u32 {
    SAVE_SCHEMA_VERSION
}

#[wasm_bindgen]
pub fn catalog_json() -> String {
    let buildings: Vec<_> = BUILDINGS
        .iter()
        .map(|b| json!({"icon": b.icon, "name": b.name, "desc": b.desc, "rate": b.rate}))
        .collect();
    let upgrades: Vec<_> = upgrades()
        .iter()
        .map(|u| json!({"icon": u.icon, "name": u.name, "desc": u.desc, "cost": u.cost}))
        .collect();
    json!({"buildings": buildings, "upgrades": upgrades}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_boundary_contract() {
        let catalog: serde_json::Value = serde_json::from_str(&catalog_json()).unwrap();
        for (i, b) in BUILDINGS.iter().enumerate() {
            let j = &catalog["buildings"][i];
            assert_eq!(
                (
                    j["icon"].as_str(),
                    j["name"].as_str(),
                    j["desc"].as_str(),
                    j["rate"].as_f64()
                ),
                (Some(b.icon), Some(b.name), Some(b.desc), Some(b.rate))
            );
        }
        for (i, u) in upgrades().iter().enumerate() {
            let j = &catalog["upgrades"][i];
            assert_eq!(
                (
                    j["icon"].as_str(),
                    j["name"].as_str(),
                    j["desc"].as_str(),
                    j["cost"].as_f64()
                ),
                (
                    Some(u.icon),
                    Some(u.name),
                    Some(u.desc.as_str()),
                    Some(u.cost)
                )
            );
        }
        assert_eq!(
            catalog["buildings"].as_array().unwrap().len(),
            BUILDINGS.len()
        );
        assert_eq!(
            catalog["upgrades"].as_array().unwrap().len(),
            upgrades().len()
        );

        let mut client = Client::new();
        client
            .load(r#"{"bank":1000,"total":100000,"owned":{"forno":10},"ups":["forno-0"]}"#)
            .unwrap();
        assert_eq!(client.building_rate(1), 2.0);
        assert_eq!(client.building_rate(0), 0.1);

        assert_eq!(client.cafe(0.05, 100.0), vec![0.0, 0.0]);
        assert_eq!(client.cafe(0.3, 100.0), vec![1.0, 0.0]);
        assert_eq!(client.buffs(), vec![1.0, 77.0, 13100.0, 0.0, 7.0, 30100.0]);
        assert_eq!(client.pps(), client.base_pps() * 7.0);
        // click_base 1 * prod 7 * click frenzy 77
        assert_eq!(client.click_value(), 539.0);
        // min(1000 * 0.15, base_pps 20 * 900) + 13
        assert_eq!(client.cafe(0.9, 100.0), vec![2.0, 163.0]);
    }
}
