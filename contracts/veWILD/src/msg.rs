use cosmwasm_schema::{ cw_serde, QueryResponses };
use cosmwasm_std::{ Addr, Uint64, Uint128 };

#[cw_serde]
pub struct InstantiateMsg {
    pub locked_token: Addr,
    pub distribution_period: Uint64,
}

#[cw_serde]
pub struct RequestWithdrawMsg {}

#[cw_serde]
pub struct WithdrawMsg {}

#[cw_serde]
pub struct ClaimMsg {}

// This is for differentiating the messages in execute()
#[cw_serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    LockMsg {
        amount: Uint128,
        new_locked_until: Uint64,
    },
    RequestWithdrawMsg,
    WithdrawMsg,
    ClaimMsg,
    AddIncomeMsg {
        add_amount: Uint128,
    },
    SetDistributionPeriodMsg {
        blocks: Uint64,
    },
}

// More queries based on the contract ...

// This is for differentiating the messages in query()
#[cw_serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // BalanceOf { address: HumanAddr },
    // LockedBalanceOf { address: HumanAddr },
    // LockedUntil { address: HumanAddr },
    // More queries based on the contract ...
}
