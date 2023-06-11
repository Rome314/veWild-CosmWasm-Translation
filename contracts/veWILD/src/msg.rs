use cosmwasm_schema::{ cw_serde, QueryResponses };
use cosmwasm_std::{ Addr, Uint64, Uint128 };
use cw20::{ BalanceResponse, TokenInfoResponse };

use crate::state::{ UserState, TokenState };

#[cw_serde]
pub struct InstantiateMsg {
    pub locked_token: Addr,
    pub distribution_period: Uint64,
}

// This is for differentiating the messages in execute()
#[cw_serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Lock {
        amount: Uint128,
        new_locked_until: Uint64,
    },
    RequestWithdraw,
    Withdraw,
    Claim,
    AddIncome {
        add_amount: Uint128,
    },
    SetDistributionPeriod {
        blocks: Uint64,
    },
}

// More queries based on the contract ...

// This is for differentiating the messages in query()
#[cw_serde(rename_all = "snake_case")]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    #[returns(BalanceResponse)]
    Balance {
        address: String,
    },
    /// Implements CW20. Returns metadata on the contract - name, decimals, supply, etc.
    #[returns(TokenInfoResponse)]
    TokenInfo {},

    #[returns(RewardRateResponse)] RewardRate {},
    #[returns(PendingAccountRewardResponse)] PendingAccountReward {
        address: Addr,
    },

    #[returns(UserInfoResponse)] UserInfo {
        address: Addr,
    },

    #[returns(VeTokenInfoResponse)] VeTokenInfo {},
}

#[cw_serde(rename_all = "snake_case")]
pub struct RewardRateResponse {
    pub reward_rate: Uint128,
}

#[cw_serde(rename_all = "snake_case")]
pub struct PendingAccountRewardResponse {
    pub pending_account_reward: Uint128,
}

#[cw_serde(rename_all = "snake_case")]
pub struct VeTokenInfoResponse {
    pub total_locked: Uint128,
    pub distribution_period: Uint64,
    pub locked_token: Addr, // address of the token contract
    pub last_accrue_block: Uint64,
    pub last_income_block: Uint64,
    pub reward_per_token: Uint128,
}

impl VeTokenInfoResponse {
    pub fn from_token_state(token_state: TokenState) -> Self {
        VeTokenInfoResponse {
            total_locked: token_state.total_locked,
            distribution_period: token_state.distribution_period,
            locked_token: token_state.locked_token,
            last_accrue_block: token_state.last_accrue_block,
            last_income_block: token_state.last_income_block,
            reward_per_token: token_state.reward_per_token,
        }
    }
}

#[cw_serde(rename_all = "snake_case")]
pub struct UserInfoResponse {
    pub locked_balance: Uint128,
    pub locked_until: Uint64,
    pub reward_snapshot: Uint128,
    pub withdraw_at: Uint64,
}

impl UserInfoResponse {
    pub fn from_user_state(user_state: UserState) -> Self {
        UserInfoResponse {
            locked_balance: user_state.locked_balance,
            locked_until: user_state.locked_until,
            reward_snapshot: user_state.reward_snapshot,
            withdraw_at: user_state.withdraw_at,
        }
    }
}
