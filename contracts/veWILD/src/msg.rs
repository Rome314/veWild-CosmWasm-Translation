use cosmwasm_schema::{ cw_serde };
use cosmwasm_std::{ Addr, Uint64, Uint128 };

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
pub enum QueryMsg {
    // BalanceOf { address: HumanAddr },
    // LockedBalanceOf { address: HumanAddr },
    // LockedUntil { address: HumanAddr },
    // More queries based on the contract ...
}
