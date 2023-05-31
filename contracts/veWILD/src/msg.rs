use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    pub locked_token: Addr,
    pub distribution_period: u64,
}

#[cw_serde]
pub struct LockMsg {
    pub amount: u128,
    pub new_locked_until: u64,
}

#[cw_serde]
pub struct RequestWithdrawMsg {}

#[cw_serde]
pub struct WithdrawMsg {}

#[cw_serde]
pub struct ClaimMsg {}

#[cw_serde]
pub struct AddIncomeMsg {
    pub add_amount: u128,
}

#[cw_serde]
pub struct SetDistributionPeriodMsg {
    pub blocks: u64,
}

// This is for differentiating the messages in execute()
#[cw_serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Lock(LockMsg),
    RequestWithdraw(RequestWithdrawMsg),
    Withdraw(WithdrawMsg),
    Claim(ClaimMsg),
    AddIncome(AddIncomeMsg),
    SetDistributionPeriod(SetDistributionPeriodMsg),
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
