use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub token_contract: Option<Addr>,
}

pub const CONFIG: Item<Config> = Item::new("config");
//NOTE: replaced below with set for optimization
//pub const WHITELIST: Map<&Addr, bool> = Map::new("whitelist");
pub const WHITELIST: Map<&Addr, ()> = Map::new("whitelist");
