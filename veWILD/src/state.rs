use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw20::Denom;
use cw_storage_plus::{Item, Map};

pub const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
pub const MIN_LOCK_PERIOD: u64 = 1460 * SECONDS_PER_DAY;
pub const WITHDRAW_DELAY: u64 = 1 * SECONDS_PER_DAY;

pub const TOKEN_NAME: String = String::from("veWILD");
pub const TOKEN_SYMBOL: String = String::from("veWILD");
// Does this need to be in CosmWasm?
// uint8  public constant decimals = 18;

pub const TOKEN_STATE:Item<TokenState> = Item::new("token_state");
pub const USER_STATE: Map<&Addr,UserState> = Map::new("user_state");


#[cw_serde]
pub struct TokenState {
    pub total_supply: Uint128,
    pub total_locked: Uint128,
    pub distribution_period: u64,

    // utility values
    pub locked_token: Addr,     // address of the token contract
    pub last_accrue_block: u64, //TODO: Check type
    pub last_income_block: u64, //TODO: Check type
    pub reward_per_token: Uint128,
    pub reward_rate_stored: Uint128, //TODO: make private
}

#[cw_serde]
pub struct UserState {
    pub balance: Uint128,         // balance
    pub locaked_balance: Uint128, // locked
    pub locked_until: u64,
    pub reward_snapshot: u64, //TODO: check type
    pub withdraw_at: u64,     //TODO: check type
}
