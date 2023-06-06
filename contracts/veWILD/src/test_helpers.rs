use crate::consts::TOKEN_DECIMALS;
use crate::contract::*;
use crate::events::ContractEvent;
use crate::msg::*;
use cosmwasm_std::ContractResult;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::MessageInfo;
use cosmwasm_std::QuerierResult;
use cosmwasm_std::Response;
use cosmwasm_std::SystemError;
use cosmwasm_std::SystemResult;
use cosmwasm_std::Uint128;
use cosmwasm_std::WasmQuery;
use cosmwasm_std::from_binary;
use cosmwasm_std::to_binary;
use cosmwasm_std::{ Addr, Uint64 };
use cw20::BalanceResponse;
use cw20::Cw20QueryMsg;
// use cw_multi_test::{ App, ContractWrapper, Executor };
use cosmwasm_std::testing::{ mock_dependencies, mock_env, mock_info };

const MOCK_LOCKED_TOKEN: &str = "cw20";

pub fn mock_instantiate(deps: DepsMut, env: Env, info: MessageInfo) {
    instantiate(deps, env, info, InstantiateMsg {
        locked_token: Addr::unchecked(MOCK_LOCKED_TOKEN),
        distribution_period: Uint64::from(1000 as u16),
    }).unwrap();
}

pub fn cw20_mock_querier(contract_balance: Uint128) -> Box<dyn Fn(&WasmQuery) -> QuerierResult> {
    let expected_address = String::from(MOCK_LOCKED_TOKEN);
    Box::new(move |request| -> QuerierResult {
        match request {
            WasmQuery::Smart { contract_addr, msg } => {
                match contract_addr {
                    _ if contract_addr.eq(&expected_address) => {
                        let balance_msg_res = from_binary(&msg);
                        match balance_msg_res {
                            Ok(Cw20QueryMsg::Balance { address }) => {
                                SystemResult::Ok(
                                    ContractResult::Ok(
                                        to_binary(
                                            &(BalanceResponse {
                                                balance: contract_balance.to_owned(),
                                            })
                                        ).unwrap()
                                    )
                                )
                            }
                            Err(_) => {
                                SystemResult::Err(SystemError::InvalidRequest {
                                    error: "Invalid query message".into(),
                                    request: msg.to_owned(),
                                })
                            }
                            _ =>
                                SystemResult::Err(SystemError::InvalidRequest {
                                    error: "Invalid query message".into(),
                                    request: msg.to_owned(),
                                }),
                        }
                    }
                    _ =>
                        SystemResult::Err(SystemError::InvalidRequest {
                            error: "Invalid query message".into(),
                            request: msg.to_owned(),
                        }),
                }
            }
            WasmQuery::Raw { contract_addr, key } =>
                SystemResult::Err(SystemError::InvalidRequest {
                    error: "Invalid query message".into(),
                    request: key.to_owned(),
                }),
            WasmQuery::ContractInfo { contract_addr } =>
                SystemResult::Err(SystemError::InvalidRequest {
                    error: "Invalid query message".into(),
                    request: to_binary(contract_addr).unwrap(),
                }),
            _ =>
                SystemResult::Err(SystemError::InvalidRequest {
                    error: "Invalid query message".into(),
                    request: Default::default(),
                }),
        }
    })
}

pub fn apply_decimals(amount: Uint128) -> Uint128 {
    amount * Uint128::new(10).pow(TOKEN_DECIMALS)
}

pub fn assert_has_events(result: &Response, expected_events: Vec<ContractEvent>) -> bool {
    let actual_events: Vec<ContractEvent> = result.attributes
        .iter()
        .filter_map(|attr| {
            match attr.key.as_str() {
                "Lock" =>
                    Some(ContractEvent::Lock {
                        account: attr.value.to_string(),
                        locked_balance: Uint128::from(attr.value.parse::<u128>().unwrap()),
                        ve_balance: Uint128::from(attr.value.parse::<u128>().unwrap()),
                        locked_until: Uint64::from(attr.value.parse::<u64>().unwrap()),
                    }),
                "WithdrawRequest" =>
                    Some(ContractEvent::WithdrawRequest {
                        account: attr.value.to_string(),
                        amount: Uint128::from(attr.value.parse::<u128>().unwrap()),
                        withdraw_at: Uint64::from(attr.value.parse::<u64>().unwrap()),
                    }),
                "Withdraw" =>
                    Some(ContractEvent::Withdraw {
                        account: attr.value.to_string(),
                        amount: Uint128::from(attr.value.parse::<u128>().unwrap()),
                    }),
                "Claim" =>
                    Some(ContractEvent::Claim {
                        account: attr.value.to_string(),
                        claim_amount: Uint128::from(attr.value.parse::<u128>().unwrap()),
                        ve_balance: Uint128::from(attr.value.parse::<u128>().unwrap()),
                    }),
                "NewIncome" =>
                    Some(ContractEvent::NewIncome {
                        add_amount: Uint128::from(attr.value.parse::<u128>().unwrap()),
                        remaining_amount: Uint128::from(attr.value.parse::<u128>().unwrap()),
                        reward_rate: Uint128::from(attr.value.parse::<u128>().unwrap()),
                    }),
                "NewDistributionPeriod" =>
                    Some(ContractEvent::NewDistributionPeriod {
                        value: Uint64::from(attr.value.parse::<u64>().unwrap()),
                    }),
                _ => None,
            }
        })
        .collect();

    expected_events.iter().all(|event| actual_events.contains(event))
}
