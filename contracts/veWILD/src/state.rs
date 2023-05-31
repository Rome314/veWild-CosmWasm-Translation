use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Uint64};
use cw20::Denom;
use cw_storage_plus::{Item, Map};

pub const SECONDS_PER_DAY: Uint64 = 24 * 60 * 60;
pub const MIN_LOCK_PERIOD: Uint64 = 7 * SECONDS_PER_DAY;
pub const MAX_LOCK_PERIOD: Uint64 = 1460 * SECONDS_PER_DAY;
pub const WITHDRAW_DELAY: Uint64 = 1 * SECONDS_PER_DAY;

pub const TOKEN_NAME: String = String::from("veWILD");
pub const TOKEN_SYMBOL: String = String::from("veWILD");
// Does this need to be in CosmWasm?
// uint8  public constant decimals = 18;

pub const TOKEN_STATE: Item<TokenState> = Item::new("token_state");
pub const USER_STATE: Map<&Addr, UserState> = Map::new("user_state");

#[cw_serde]
pub struct TokenState {
    pub total_supply: Uint128,
    pub total_locked: Uint128,
    pub distribution_period: Uint64,

    // utility values
    pub locked_token: Addr,        // address of the token contract
    pub last_accrue_block: Uint64, //TODO: Check type
    pub last_income_block: Uint64, //TODO: Check type
    pub reward_per_token: Uint128,
    pub reward_rate_stored: Uint128, //TODO: make private
}

impl TokenState {
    pub fn pending_reward_per_token(&self, block_height: Uint64) -> Uint128 {
        if self.total_supply.is_zero() {
            return Uint128::zero();
        }

        let blocks_elapsed: u64 = block_height - self.last_accrue_block;
        return Uint128::from(blocks_elapsed) * self.reward_rate(block_height) / self.total_supply;
    }

    pub fn reward_rate(&self, block_height: u64) -> Uint128 {
        let blocks_elapsed: u64 = block_height - self.last_income_block;
        let resp = if blocks_elapsed < self.distribution_period {
            self.reward_rate_stored
        } else {
            0
        };
        return resp;
    }
}

#[cw_serde]
pub struct UserState {
    pub balance: Uint128,         // balance
    pub locked_balance: Uint128, // locked
    pub locked_until: Uint64,
    pub reward_snapshot: Uint128, //TODO: check type
    pub withdraw_at: Uint64,      //TODO: check type
}

impl UserState {
    pub fn pending_reward(
        self,
        current_reward_per_token: Uint128,
        pending_reward_rate: Uint128,
    ) -> Uint128 {
        let pending_reward_per_token = current_reward_per_token + pending_reward_rate;
        let reward_per_token_delta = pending_reward_per_token - self.reward_snapshot;
        return reward_per_token_delta * self.balance; //Decimals?
    }
}
