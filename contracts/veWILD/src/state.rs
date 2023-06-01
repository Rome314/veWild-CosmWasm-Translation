use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ Addr, Deps, DepsMut, StdResult, Uint128, Uint64, Response };
use cw20::Denom;
use cw20_base::contract::query_token_info;
use cw_storage_plus::{ Item, Map };

use crate::cw20_client::CW20Client;
use crate::error::ContractError;
use crate::{ cw20_client, events::* };

// Does this need to be in CosmWasm?
// uint8  public constant decimals = 18;

pub const TOKEN_STATE: Item<TokenState> = Item::new("token_state");
pub const USER_STATE: Map<&Addr, UserState> = Map::new("user_state");

#[cw_serde]
pub struct TokenState {
    pub total_locked: Uint128,
    pub distribution_period: Uint64,

    // utility values
    pub locked_token: Addr, // address of the token contract
    pub last_accrue_block: Uint64, //TODO: Check type
    pub last_income_block: Uint64, //TODO: Check type
    pub reward_per_token: Uint128,
    pub reward_rate_stored: Uint128, //TODO: make private
}

pub struct UpdateRewardRateInput {
    add_amount: Uint128,
    new_distribution_period: Uint64,
    current_block: Uint64,
}

impl TokenState {
    pub fn default() -> Self {
        Self {
            total_locked: Uint128::zero(),
            distribution_period: Uint64::zero(),
            locked_token: Addr::unchecked(""),
            last_accrue_block: Uint64::zero(),
            last_income_block: Uint64::zero(),
            reward_per_token: Uint128::zero(),
            reward_rate_stored: Uint128::zero(),
        }
    }

    pub fn locked_token_client(&self, deps: DepsMut) -> CW20Client {
        cw20_client::CW20Client::new(deps, self.locked_token)
    }

    pub fn set_distribution_period(
        &self,
        current_block: Uint64,
        blocks: Uint64
    ) -> Result<Response, ContractError> {
        if blocks.is_zero() {
            return Result::Err(ContractError::ZeroDistributionPeriod {});
        }
        self.accrue(current_block);
        self.update_reward_rate(UpdateRewardRateInput {
            add_amount: Uint128::zero(),
            new_distribution_period: blocks,
            current_block: current_block,
        })?;

        let event = ContractEvent::NewDistributionPeriod { value: blocks };
        let resp = Response::new().add_attributes(event.to_attributes());

        Ok(resp)
    }

    pub fn update_reward_rate(
        &self,
        input: UpdateRewardRateInput
    ) -> Result<Uint128, ContractError> {
        /*
        Avoid inflation of blocksElapsed inside of _pendingRewardPerToken()
        Ensures _pendingRewardPerToken() is 0 and all rewards are accounted for
        */
        if !input.current_block.eq(&self.last_accrue_block) {
            return Result::Err(ContractError::AccrueFirst {});
        }
        let blocks_elapsed: Uint64 = self.distribution_period.min(
            input.current_block - self.last_income_block
        );

        let unvested_income = self.reward_rate_stored.mul(
            Uint128::from(self.distribution_period - blocks_elapsed)
        );

        self.reward_rate_stored =
            (unvested_income + input.add_amount) / Uint128::from(input.new_distribution_period);
        self.distribution_period = input.new_distribution_period;
        self.last_income_block = input.current_block;

        return Ok(unvested_income);
    }

    pub fn accrue(&self, current_block: Uint64) {
        self.reward_per_token += self.pending_reward_per_token(current_block);
        self.last_accrue_block = current_block;
    }

    pub fn reward_rate(&self, block_height: Uint64) -> Uint128 {
        let blocks_elapsed: Uint64 = block_height - self.last_income_block;
        let resp = if blocks_elapsed < self.distribution_period {
            self.reward_rate_stored
        } else {
            Uint128::zero()
        };
        return resp;
    }
}

#[cw_serde]
pub struct UserState {
    pub locked_balance: Uint128, // locked
    pub locked_until: Uint64,
    pub reward_snapshot: Uint128, //TODO: check type
    pub withdraw_at: Uint64, //TODO: check type
}

