use std::f32::consts::E;

use crate::consts::TOKEN_DECIMALS;
use crate::contract::*;
use crate::msg::*;
use crate::state::TOKEN_STATE;
use crate::state::TokenState;
use cosmwasm_std::ContractResult;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::MessageInfo;
use cosmwasm_std::QuerierResult;
use cosmwasm_std::StdError;
use cosmwasm_std::SystemError;
use cosmwasm_std::SystemResult;
use cosmwasm_std::Uint128;
use cosmwasm_std::WasmQuery;
use cosmwasm_std::from_binary;
use cosmwasm_std::testing::MockQuerier;
use cosmwasm_std::to_binary;
use cosmwasm_std::{ Addr, Uint64 };
use cw20::BalanceResponse;
use cw20::Cw20QueryMsg;
use cw20_base::state::MinterData;
use cw20_base::state::TOKEN_INFO;
use cw20_base::state::TokenInfo;
// use cw_multi_test::{ App, ContractWrapper, Executor };
use cosmwasm_std::testing::{ mock_dependencies, mock_env, mock_info };

const MOCK_LOCKED_TOKEN: &str = "cw20";

fn mock_instantiate(deps: DepsMut, env: Env, info: MessageInfo) {
    instantiate(deps, env, info, InstantiateMsg {
        locked_token: Addr::unchecked(MOCK_LOCKED_TOKEN),
        distribution_period: Uint64::from(1000 as u16),
    }).unwrap();
}

fn cw20_mock_querier(contract_balance: Uint128) -> Box<dyn Fn(&WasmQuery) -> QuerierResult> {
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

#[cfg(test)]
mod utils_tests {
    use std::{ rc::Rc, cell::RefCell };

    use cosmwasm_std::StdResult;
    use crate::{ contract::utils::{ * }, error::ContractError };

    use super::*;
    #[test]
    fn test_check_reserves() {
        let mut deps_binding = mock_dependencies();
        let mut env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps_binding.as_mut(), env.to_owned(), info.to_owned());
        // 1. enough balance

        deps_binding.querier.update_wasm(cw20_mock_querier(Uint128::from(1000 as u16)));

        let deps = deps_binding.as_mut();
        TOKEN_STATE.update(
            deps.storage,
            |mut state| -> StdResult<_> {
                state.total_locked = Uint128::from(1000 as u16);
                Ok(state)
            }
        ).unwrap();

        env.block.height += 500;
        let _res = check_reserves(deps.as_ref(), &env.to_owned()).unwrap();

        // 2. not enough balance
        let mut deps_binding = mock_dependencies();
        let mut env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps_binding.as_mut(), env.to_owned(), info.to_owned());
        deps_binding.querier.update_wasm(cw20_mock_querier(Uint128::from(100 as u16)));

        let deps = deps_binding.as_mut();
        TOKEN_STATE.update(
            deps.storage,
            |mut state| -> StdResult<_> {
                state.total_locked = Uint128::from(1000 as u16);
                Ok(state)
            }
        ).unwrap();

        let err = check_reserves(deps.as_ref(), &env.to_owned()).unwrap_err();
        assert_eq!(err, ContractError::InsufficientReserves {});
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn proper_instantiation() {
        let mut deps_binding = mock_dependencies();
        let env = mock_env();

        let _resp = instantiate(
            deps_binding.as_mut(),
            env.to_owned(),
            mock_info("creator", &[]),
            InstantiateMsg {
                locked_token: Addr::unchecked("cw20"),
                distribution_period: Uint64::from(1000 as u16),
            }
        ).unwrap();

        // Test token state
        let token_state = TOKEN_STATE.load(deps_binding.as_ref().storage).unwrap();

        let mut expected_token_state = TokenState::default();
        expected_token_state.locked_token = Addr::unchecked("cw20");
        expected_token_state.distribution_period = Uint64::from(1000 as u16);
        expected_token_state.last_accrue_block = Uint64::from(env.block.height);
        expected_token_state.last_income_block = Uint64::from(env.block.height);

        assert_eq!(expected_token_state, token_state);

        let expected_token_info = TokenInfo {
            name: "veWILD".to_string(),
            symbol: "veWILD".to_string(),
            decimals: TOKEN_DECIMALS,
            total_supply: Uint128::zero(),
            mint: Some(MinterData {
                minter: env.contract.address.clone(),
                cap: None,
            }),
        };

        let token_info = TOKEN_INFO.load(deps_binding.as_ref().storage).unwrap();
        assert_eq!(expected_token_info, token_info);

        // TODO: test events
    }

    #[test]
    fn test_execute_set_distribution_period() {
        let mut binding = mock_dependencies();
        let mut env = mock_env();
        let mut deps = binding.as_mut();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.branch(), env.clone(), info.to_owned());

        let current_block = Uint64::from(env.block.height + 1000);
        let new_distribution_period = Uint64::from(2000u64);

        let mut expected_state = TOKEN_STATE.load(deps.storage).unwrap();

        expected_state
            .set_distribution_period(
                mock_dependencies().as_mut().storage,
                current_block.clone(),
                new_distribution_period.clone()
            )
            .unwrap();

        env.block.height = current_block.into();
        let msg = ExecuteMsg::SetDistributionPeriodMsg { blocks: new_distribution_period };

        let resp = execute(deps.branch(), env.clone(), info, msg).unwrap();

        assert_eq!(expected_state, TOKEN_STATE.load(deps.storage).unwrap());
        assert_eq!(
            resp.attributes
                .iter()
                .find(|attr| attr.key == "action")
                .unwrap().value,
            "new_distribution_period"
        );
        assert_eq!(
            resp.attributes
                .iter()
                .find(|attr| attr.key == "value")
                .unwrap().value,
            new_distribution_period.to_string()
        );
    }
}
