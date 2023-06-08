use cosmwasm_std::{ Uint128, Uint64, Attribute, Event, StdError };

use crate::error::ContractError;

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
    

    // pub fn from_cosmos_event(ev: &Event) -> Option<ContractEvent> {
    //     match ev.ty.as_str() {
    //         "lock" =>
    //             Some(ContractEvent::Lock {
    //                 account: ev.value.to_string(),
    //                 locked_balance: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //                 ve_balance: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //                 locked_until: Uint64::from(ev.value.parse::<u64>().unwrap()),
    //             }),
    //         "withdraw_request" =>
    //             Some(ContractEvent::WithdrawRequest {
    //                 account: ev.value.to_string(),
    //                 amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //                 withdraw_at: Uint64::from(ev.value.parse::<u64>().unwrap()),
    //             }),
    //         "withdraw" =>
    //             Some(ContractEvent::Withdraw {
    //                 account: ev.value.to_string(),
    //                 amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //             }),
    //         "claim" =>
    //             Some(ContractEvent::Claim {
    //                 account: ev.value.to_string(),
    //                 claim_amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //                 ve_balance: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //             }),
    //         "new_income" =>
    //             Some(ContractEvent::NewIncome {
    //                 add_amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //                 remaining_amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //                 reward_rate: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //             }),
    //         "new_distribution_period" =>
    //             Some(ContractEvent::NewDistributionPeriod {
    //                 value: Uint64::from(ev.value.parse::<u64>().unwrap()),
    //             }),
    //         "mint" =>
    //             Some(ContractEvent::Mint {
    //                 to: ev.value.to_string(),
    //                 amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //             }),
    //         "burn" =>
    //             Some(ContractEvent::Burn {
    //                 from: ev.value.to_string(),
    //                 amount: Uint128::from(ev.value.parse::<u128>().unwrap()),
    //             }),
    //         _ => None,
    //     }
    // }
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
