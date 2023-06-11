use std::collections::HashMap;
use std::iter::Map;

use crate::consts::TOKEN_DECIMALS;
use crate::contract::*;
use crate::events::ContractEvent;
use crate::msg::*;
use cosmwasm_std::Attribute;
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

pub fn assert_has_events(result: &Response, expected_events: Vec<ContractEvent>) {
    let result_events = result.events.clone();
    assert_eq!(
        result_events.len(),
        expected_events.len(),
        "events length not equal, actual events: {:?}, expected events: {:?}",
        result_events,
        expected_events
    );
    for event in expected_events.iter() {
        let cosmos_event = event.to_cosmos_event();

        let found_event = result_events.iter().find(|e| e.ty.eq(&cosmos_event.ty));
        match found_event {
            Some(found_event) => {
                assert_eq!(found_event, cosmos_event, "{:?} event not equal", cosmos_event.ty);
            }
            None => panic!("Event not found: {:?}", cosmos_event.ty),
        }
    }
}

pub fn assert_has_attributes(result: &Response, attrs: HashMap<&str, String>) {
    let result_attrs = result.attributes.clone();
    assert_eq!(
        result_attrs.len(),
        attrs.len(),
        "attributes length not equal, actual attributes: {:?}, expected attributes: {:?}",
        result_attrs,
        attrs
    );

    result_attrs.iter().for_each(|attr| {
        let expected_value = attrs.get(attr.key.as_str());
        match expected_value {
            Some(expected_value) => {
                assert_eq!(
                    attr.value,
                    expected_value.to_string(),
                    "{:?} attribute value not equal, actual: {:?}, expected: {:?}",
                    attr.key,
                    attr.value,
                    expected_value
                );
            }
            None => panic!("unexpected attribute: {:?}", attr.key),
        }
    });
}
