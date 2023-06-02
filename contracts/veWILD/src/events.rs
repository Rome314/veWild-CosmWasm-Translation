use cosmwasm_std::{  Uint128, Uint64, Attribute };

pub enum ContractEvent {
    Lock {
        account: String,
        locked_balance: Uint128,
        ve_balance: Uint128,
        locked_until: Uint64,
    },
    WithdrawRequest {
        account: String,
        amount: Uint128,
        withdraw_at: Uint64,
    },
    Withdraw {
        account: String,
        amount: Uint128,
    },
    Claim {
        account: String,
        claim_amount: Uint128,
        ve_balance: Uint128,
    },
    NewIncome {
        add_amount: Uint128,
        remaining_amount: Uint128,
        reward_rate: Uint128,
    },
    NewDistributionPeriod {
        value: Uint64,
    },
}

impl ContractEvent {
    pub fn to_attributes(&self) -> Vec<Attribute> {
        match self {
            ContractEvent::Lock { account, locked_balance, ve_balance, locked_until } =>
                vec![
                    attr("action", "lock"),
                    attr("account", account.as_str()),
                    attr("locked_balance", &locked_balance.to_string()),
                    attr("ve_balance", &ve_balance.to_string()),
                    attr("locked_until", &locked_until.to_string())
                ],
            ContractEvent::WithdrawRequest { account, amount, withdraw_at } =>
                vec![
                    attr("action", "withdraw_request"),
                    attr("account", account.as_str()),
                    attr("amount", &amount.to_string()),
                    attr("withdraw_at", &withdraw_at.to_string())
                ],
            ContractEvent::Withdraw { account, amount } =>
                vec![
                    attr("action", "withdraw"),
                    attr("account", account.as_str()),
                    attr("amount", &amount.to_string())
                ],
            ContractEvent::Claim { account, claim_amount, ve_balance } =>
                vec![
                    attr("action", "claim"),
                    attr("account", account.as_str()),
                    attr("claim_amount", &claim_amount.to_string()),
                    attr("ve_balance", &ve_balance.to_string())
                ],
            ContractEvent::NewIncome { add_amount, remaining_amount, reward_rate } =>
                vec![
                    attr("action", "new_income"),
                    attr("add_amount", &add_amount.to_string()),
                    attr("remaining_amount", &remaining_amount.to_string()),
                    attr("reward_rate", &reward_rate.to_string())
                ],
            ContractEvent::NewDistributionPeriod { value } =>
                vec![attr("action", "new_distribution_period"), attr("value", &value.to_string())],
        }
    }
}

// Helper function for creating attributes
fn attr(key: &str, value: &str) -> Attribute {
    Attribute::from((key.to_string(), value.to_string()))
}
