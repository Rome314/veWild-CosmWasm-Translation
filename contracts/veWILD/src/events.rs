use cosmwasm_std::{ Uint128, Uint64, Attribute, Event };

#[derive(Clone, Debug, PartialEq)]
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
    Burn {
        amount: Uint128,
        from: String,
    },
    Mint {
        amount: Uint128,
        to: String,
    },
}

impl ContractEvent {
    pub fn make_lock(
        account: String,
        locked_balance: Uint128,
        ve_balance: Uint128,
        locked_until: Uint64
    ) -> Self {
        ContractEvent::Lock {
            account,
            locked_balance,
            ve_balance,
            locked_until,
        }
    }

    pub fn make_withdraw_request(account: String, amount: Uint128, withdraw_at: Uint64) -> Self {
        ContractEvent::WithdrawRequest {
            account,
            amount,
            withdraw_at,
        }
    }

    pub fn make_withdraw(account: String, amount: Uint128) -> Self {
        ContractEvent::Withdraw {
            account,
            amount,
        }
    }

    pub fn make_claim(account: String, claim_amount: Uint128, ve_balance: Uint128) -> Self {
        ContractEvent::Claim {
            account,
            claim_amount,
            ve_balance,
        }
    }

    pub fn make_new_income(
        add_amount: Uint128,
        remaining_amount: Uint128,
        reward_rate: Uint128
    ) -> Self {
        ContractEvent::NewIncome {
            add_amount,
            remaining_amount,
            reward_rate,
        }
    }

    pub fn make_new_distribution_period(value: Uint64) -> Self {
        ContractEvent::NewDistributionPeriod { value }
    }

    pub fn make_burn(amount: Uint128, from: String) -> Self {
        ContractEvent::Burn { amount, from }
    }

    pub fn make_mint(amount: Uint128, to: String) -> Self {
        ContractEvent::Mint { amount, to }
    }
    pub fn to_cosmos_event(&self) -> Event {
        match self {
            ContractEvent::Lock { account, locked_balance, ve_balance, locked_until } =>
                Event::new("lock").add_attributes(
                    vec![
                        attr("account", account.as_str()),
                        attr("locked_balance", &locked_balance.to_string()),
                        attr("ve_balance", &ve_balance.to_string()),
                        attr("locked_until", &locked_until.to_string())
                    ]
                ),
            ContractEvent::WithdrawRequest { account, amount, withdraw_at } =>
                Event::new("withdraw_request").add_attributes(
                    vec![
                        attr("account", account.as_str()),
                        attr("amount", &amount.to_string()),
                        attr("withdraw_at", &withdraw_at.to_string())
                    ]
                ),
            ContractEvent::Withdraw { account, amount } =>
                Event::new("withdraw").add_attributes(
                    vec![attr("account", account.as_str()), attr("amount", &amount.to_string())]
                ),
            ContractEvent::Claim { account, claim_amount, ve_balance } =>
                Event::new("claim").add_attributes(
                    vec![
                        attr("account", account.as_str()),
                        attr("claim_amount", &claim_amount.to_string()),
                        attr("ve_balance", &ve_balance.to_string())
                    ]
                ),
            ContractEvent::NewIncome { add_amount, remaining_amount, reward_rate } =>
                Event::new("new_income").add_attributes(
                    vec![
                        attr("add_amount", &add_amount.to_string()),
                        attr("remaining_amount", &remaining_amount.to_string()),
                        attr("reward_rate", &reward_rate.to_string())
                    ]
                ),
            ContractEvent::NewDistributionPeriod { value } =>
                Event::new("new_distribution_period").add_attributes(
                    vec![attr("value", &value.to_string())]
                ),
            ContractEvent::Burn { amount, from } =>
                Event::new("burn").add_attributes(
                    vec![attr("amount", &amount.to_string()), attr("from", from.as_str())]
                ),
            ContractEvent::Mint { amount, to } =>
                Event::new("mint").add_attributes(
                    vec![attr("amount", &amount.to_string()), attr("to", to.as_str())]
                ),
        }
    }
}

// Helper function for creating attributes
fn attr(key: &str, value: &str) -> Attribute {
    Attribute::from((key.to_string(), value.to_string()))
}
