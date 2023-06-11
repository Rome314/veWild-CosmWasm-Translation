use std::{ vec, collections::HashMap };
use cosmwasm_std::{
    Addr,
    attr,
    CosmosMsg,
    Empty,
    Env,
    from_binary,
    MessageInfo,
    QuerierResult,
    Response,
    StdResult,
    SystemError,
    SystemResult,
    to_binary,
    Uint128,
    Uint64,
    WasmQuery,
    WasmMsg,
    testing::{ mock_dependencies, mock_env, mock_info },
};
use cw20::Cw20ExecuteMsg;
use cw20_base::{ state::{ BALANCES, TOKEN_INFO, TokenInfo, MinterData }, contract::execute_mint };
use crate::{ state::*, consts::*, msg::*, events::*, error::*, test_helpers::*, * };

#[cfg(test)]
mod execute_tests {
    use crate::internal::internal_funcs;

    use super::{ * };

    #[test]
    fn proper_instantiation() {
        let mut deps_binding = mock_dependencies();
        let env = mock_env();

        let resp = instantiate(
            deps_binding.as_mut(),
            env.to_owned(),
            mock_info("creator", &[]),
            InstantiateMsg {
                locked_token: Addr::unchecked(MOCK_LOCKED_TOKEN),
                distribution_period: Uint64::from(1000 as u16),
            }
        ).unwrap();

        // Test token state
        let token_state = TOKEN_STATE.load(deps_binding.as_ref().storage).unwrap();

        let mut expected_token_state = TokenState::default();
        expected_token_state.locked_token = Addr::unchecked(MOCK_LOCKED_TOKEN);
        expected_token_state.distribution_period = Uint64::from(1000 as u16);
        expected_token_state.last_accrue_block = Uint64::from(env.block.height);
        expected_token_state.last_income_block = Uint64::from(env.block.height);

        assert_eq!(expected_token_state, token_state);

        let expected_token_info = TokenInfo {
            name: "veWILD".to_string(),
            symbol: "veWILD".to_string(),
            decimals: TOKEN_DECIMALS as u8,
            total_supply: Uint128::zero(),
            mint: Some(MinterData {
                minter: env.contract.address.clone(),
                cap: None,
            }),
        };

        let token_info = TOKEN_INFO.load(deps_binding.as_ref().storage).unwrap();
        assert_eq!(expected_token_info, token_info);

        let expected_response: Response<Empty> = Response::new().add_event(
            ContractEvent::make_new_distribution_period(Uint64::from(1000 as u16)).to_cosmos_event()
        );

        assert_eq!(expected_response, resp);
    }

    #[test]
    fn test_execute_lock_unsufficient_reserves() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());
        let user_addr = Addr::unchecked("user");

        let initial_locked = apply_decimals(Uint128::from(1u8));

        let initial_user_state = UserState {
            locked_balance: initial_locked.clone(),
            reward_snapshot: Uint128::zero(),
            locked_until: Uint64::from(env.block.time.seconds() + MIN_LOCK_PERIOD),
            balance: Uint128::zero(),
            withdraw_at: Uint64::zero(),
        };

        USER_STATE.save(
            deps.as_mut().storage,
            &Addr::unchecked(user_addr.clone()),
            &initial_user_state
        ).unwrap();

        internal_funcs
            ::set_balance(deps.as_mut(), &env, &info, &user_addr, initial_locked.clone())
            .unwrap();

        TOKEN_STATE.update(
            deps.as_mut().storage,
            |mut state| -> StdResult<TokenState> {
                state.total_locked = initial_locked.clone();
                state.reward_per_token = apply_decimals(Uint128::from(1u8)) / Uint128::from(100u8);
                Ok(state)
            }
        ).unwrap();

        let amount = apply_decimals(Uint128::from(1u8));
        let lock_period = Uint64::from(MIN_LOCK_PERIOD * 2);
        let new_locked_until = Uint64::from(env.block.time.seconds() + lock_period.u64());

        let msg = ExecuteMsg::Lock {
            amount: amount.clone(),
            new_locked_until: new_locked_until,
        };

        deps.querier.update_wasm(cw20_mock_querier(amount.clone()));

        let info = mock_info(user_addr.as_str(), &[]);
        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(err, ContractError::InsufficientReserves {});
    }

    #[test]
    fn test_execute_lock_add_to_existing() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());
        let user_addr = Addr::unchecked("user");

        let initial_locked = apply_decimals(Uint128::from(1u8));

        let initial_user_state = UserState {
            locked_balance: initial_locked.clone(),
            reward_snapshot: Uint128::zero(),
            locked_until: Uint64::from(env.block.time.seconds() + MIN_LOCK_PERIOD),
            balance: Uint128::zero(),
            withdraw_at: Uint64::zero(),
        };

        USER_STATE.save(
            deps.as_mut().storage,
            &Addr::unchecked(user_addr.clone()),
            &initial_user_state
        ).unwrap();

        internal_funcs
            ::set_balance(deps.as_mut(), &env, &info, &user_addr, initial_locked.clone())
            .unwrap();

        TOKEN_STATE.update(
            deps.as_mut().storage,
            |mut state| -> StdResult<TokenState> {
                state.total_locked = initial_locked.clone();
                state.reward_per_token = apply_decimals(Uint128::from(1u8)) / Uint128::from(100u8);
                Ok(state)
            }
        ).unwrap();

        let token_state = TOKEN_STATE.load(deps.as_ref().storage).unwrap();

        let amount = apply_decimals(Uint128::from(1u8));
        let lock_period = Uint64::from(MIN_LOCK_PERIOD * 2);
        let new_locked_until = Uint64::from(env.block.time.seconds() + lock_period.u64());

        let msg = ExecuteMsg::Lock {
            amount: amount.clone(),
            new_locked_until: new_locked_until,
        };

        let expected_unvested_income =
            token_state.reward_per_token * Uint128::from(token_state.distribution_period);
        // Make sure that enough reserves;
        deps.querier.update_wasm(
            cw20_mock_querier(token_state.total_locked + amount.clone() + expected_unvested_income)
        );

        let info = mock_info(user_addr.as_str(), &[]);
        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // TODO: test states

        assert_eq!(resp.messages.len(), 2);

        let expected_claim_amount =
            (token_state.reward_per_token * initial_locked.clone()) /
            apply_decimals(Uint128::from(1u8));

        let expected_transfer_message = CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: String::from(MOCK_LOCKED_TOKEN),
            msg: to_binary(
                &(Cw20ExecuteMsg::Transfer {
                    recipient: user_addr.to_string(),
                    amount: expected_claim_amount.clone(),
                })
            ).unwrap(),
            funds: vec![],
        });

        assert_eq!(resp.messages[0].msg, expected_transfer_message);

        let expected_transfer_from_message = CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: String::from(MOCK_LOCKED_TOKEN),
            msg: to_binary(
                &(Cw20ExecuteMsg::TransferFrom {
                    recipient: env.contract.address.to_string(),
                    amount: amount.clone(),
                    owner: info.sender.to_string(),
                })
            ).unwrap(),
            funds: vec![],
        });

        let expected_balance_on_claim =
            (initial_user_state.locked_balance * Uint128::from(MIN_LOCK_PERIOD)) /
            Uint128::from(MAX_LOCK_PERIOD);

        let expected_balance_at_the_end =
            ((amount + initial_user_state.locked_balance) * Uint128::from(lock_period.clone())) /
            Uint128::from(MAX_LOCK_PERIOD);

        let initial_balance = initial_locked.clone();
        let expected_burn_amount = initial_balance.clone() - expected_balance_on_claim.clone();
        let expected_mint_amount = expected_balance_at_the_end - expected_balance_on_claim;

        let expected_response: Response<Empty> = Response::new()
            .add_messages(vec![expected_transfer_message, expected_transfer_from_message])
            .add_events(
                vec![
                    ContractEvent::make_claim(
                        info.sender.to_string(),
                        expected_claim_amount.clone(),
                        expected_balance_on_claim.clone()
                    ).to_cosmos_event(),
                    ContractEvent::make_lock(
                        info.sender.to_string(),
                        amount.clone() + initial_user_state.locked_balance,
                        expected_balance_at_the_end.clone(),
                        new_locked_until.clone()
                    ).to_cosmos_event()
                ]
            )
            .add_attributes(
                vec![
                    attr("action", String::from("burn")),
                    attr("from", info.sender.to_string()),
                    attr("amount", expected_burn_amount.to_string()),
                    attr("action", String::from("mint")),
                    attr("to", info.sender.to_string()),
                    attr("amount", expected_mint_amount.to_string())
                ]
            );

        assert_eq!(resp, expected_response);
    }

    #[test]
    fn test_execute_lock_created_new() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());

        let amount = apply_decimals(Uint128::from(1u8));
        let lock_period = Uint64::from(MIN_LOCK_PERIOD * 2);
        let new_locked_until = Uint64::from(env.block.time.seconds() + lock_period.u64());

        let msg = ExecuteMsg::Lock {
            amount: amount.clone(),
            new_locked_until: new_locked_until,
        };

        deps.querier.update_wasm(cw20_mock_querier(amount.clone()));

        let info = mock_info("user", &[]);
        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let expected_balance =
            (amount * Uint128::from(lock_period.clone())) / Uint128::from(MAX_LOCK_PERIOD);

        assert_eq!(resp.messages.len(), 1);

        let expected_message = CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: String::from(MOCK_LOCKED_TOKEN),
            msg: to_binary(
                &(Cw20ExecuteMsg::TransferFrom {
                    recipient: env.contract.address.to_string(),
                    amount: amount.clone(),
                    owner: info.sender.to_string(),
                })
            ).unwrap(),
            funds: vec![],
        });

        let expected_response: Response<Empty> = Response::new()
            .add_messages(vec![expected_message])
            .add_events(
                vec![
                    ContractEvent::make_lock(
                        info.sender.to_string(),
                        amount.clone(),
                        expected_balance.clone(),
                        new_locked_until.clone()
                    ).to_cosmos_event()
                ]
            )
            .add_attributes(
                vec![
                    attr("action", String::from("mint")),
                    attr("to", info.sender.to_string()),
                    attr("amount", expected_balance.to_string())
                ]
            );

        assert_eq!(resp, expected_response);
    }

    #[test]
    fn test_execute_lock_errors() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());

        let amount = apply_decimals(Uint128::from(1u8));
        // Too short period
        let new_locked_until = Uint64::from(env.block.height + 1000);
        let msg = ExecuteMsg::Lock {
            amount: amount.clone(),
            new_locked_until: new_locked_until,
        };

        let error = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(error, ContractError::LockPeriodTooShort {});

        // Too long period
        let new_locked_until = Uint64::from(env.block.time.seconds() + MAX_LOCK_PERIOD + 1);
        let msg = ExecuteMsg::Lock {
            amount: amount.clone(),
            new_locked_until: new_locked_until,
        };

        let error = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(error, ContractError::LockPeriodTooLong {});

        // Can not reduce lock time
        let mut user_state = UserState::default();
        user_state.locked_until = Uint64::from(env.block.time.seconds() + MIN_LOCK_PERIOD + 1000);
        USER_STATE.save(deps.as_mut().storage, &info.sender, &user_state).unwrap();

        let new_locked_until = Uint64::from(env.block.time.seconds() + MIN_LOCK_PERIOD + 500);
        let msg = ExecuteMsg::Lock {
            amount: amount.clone(),
            new_locked_until: new_locked_until,
        };

        let error = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(error, ContractError::CannotReduceLockedTime {});
    }

    #[test]
    fn test_execute_request_withdraw_with_claim() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.clone(), info.to_owned());

        let user_addr = Addr::unchecked("user");
        let info = mock_info(user_addr.as_str(), &[]);

        let user_ve_balance = apply_decimals(Uint128::from(1u8));
        internal_funcs
            ::set_balance(deps.as_mut(), &env, &info, &user_addr, user_ve_balance.clone())
            .unwrap();

        let mut initial_user_state = USER_STATE.load(deps.as_mut().storage, &user_addr).unwrap();
        initial_user_state.locked_balance = apply_decimals(Uint128::from(1u8));
        initial_user_state.locked_until = Uint64::from(env.block.time.seconds());
        USER_STATE.save(deps.as_mut().storage, &info.sender, &initial_user_state).unwrap();

        let reward_per_token = apply_decimals(Uint128::from(1u8)) / Uint128::from(10u8);
        TOKEN_STATE.update(
            deps.as_mut().storage,
            |mut state| -> StdResult<TokenState> {
                state.reward_per_token = reward_per_token.clone();
                Ok(state)
            }
        ).unwrap();

        let msg = ExecuteMsg::RequestWithdraw {};

        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let mut expected_user_state = initial_user_state.clone();
        expected_user_state.reward_snapshot = reward_per_token.clone();
        expected_user_state.withdraw_at = Uint64::from(env.block.time.seconds() + WITHDRAW_DELAY);
        expected_user_state.balance = Uint128::zero();

        assert_eq!(
            expected_user_state,
            USER_STATE.load(deps.as_mut().storage, &info.sender).unwrap()
        );

        let expected_claim_amount =
            (initial_user_state.balance * reward_per_token) / apply_decimals(Uint128::from(1u8));

        let expected_response: Response<Empty> = Response::new()
            .add_events(
                vec![
                    (ContractEvent::Claim {
                        account: info.sender.to_string(),
                        claim_amount: expected_claim_amount,
                        ve_balance: Uint128::zero(), //Because user locked_balance == 0
                    }).to_cosmos_event(),
                    (ContractEvent::WithdrawRequest {
                        account: info.sender.to_string(),
                        withdraw_at: expected_user_state.withdraw_at,
                        amount: expected_user_state.locked_balance,
                    }).to_cosmos_event()
                ]
            )
            .add_messages(
                vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: MOCK_LOCKED_TOKEN.to_string(),
                        msg: to_binary(
                            &(Cw20ExecuteMsg::Transfer {
                                recipient: info.sender.to_string(),
                                amount: expected_claim_amount,
                            })
                        ).unwrap(),
                        funds: vec![],
                    })
                ]
            )
            .add_attributes(
                vec![
                    attr("action", "burn".to_string()),
                    attr("from", user_addr.to_string()),
                    attr("amount", user_ve_balance.to_string())
                ]
            );

        assert_eq!(expected_response, resp);
    }

    #[test]
    fn test_execute_request_withdraw_without_claim() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.clone(), info.to_owned());

        let mut user_state = UserState::default();
        user_state.locked_balance = apply_decimals(Uint128::from(1u8));
        user_state.locked_until = Uint64::from(env.block.time.seconds());
        USER_STATE.save(deps.as_mut().storage, &info.sender, &user_state).unwrap();

        let msg = ExecuteMsg::RequestWithdraw {};

        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let mut expected_user_state = user_state;
        expected_user_state.withdraw_at = Uint64::from(env.block.time.seconds() + WITHDRAW_DELAY);

        assert_eq!(
            expected_user_state,
            USER_STATE.load(deps.as_mut().storage, &info.sender).unwrap()
        );

        let expected_response: Response<Empty> = Response::new().add_event(
            ContractEvent::make_withdraw_request(
                info.sender.to_string(),
                expected_user_state.locked_balance,
                expected_user_state.withdraw_at
            ).to_cosmos_event()
        );

        assert_eq!(expected_response, resp);
    }

    #[test]
    fn test_execute_request_withdraw_errors() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.clone(), info.to_owned());

        // 1. Nothing to withdraw
        let mut user_state = UserState::default();
        USER_STATE.save(deps.as_mut().storage, &info.sender, &user_state).unwrap();

        let msg = ExecuteMsg::RequestWithdraw {};

        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(err, ContractError::NothingToWithdraw {});

        // 2. Not enough time passed
        user_state.locked_balance = apply_decimals(Uint128::from(1u8));
        user_state.locked_until = Uint64::from(env.block.time.seconds() + MIN_LOCK_PERIOD + 1000);
        USER_STATE.save(deps.as_mut().storage, &info.sender, &user_state).unwrap();

        let msg = ExecuteMsg::RequestWithdraw {};
        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(err, ContractError::WithdrawBeforeUnlock {});
    }

    #[test]
    fn test_execute_withdraw_success() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.clone(), info.to_owned());

        let lock_balance = apply_decimals(Uint128::from(1u8));
        let mut initial_user_state = UserState::default();
        initial_user_state.locked_balance = lock_balance.clone();
        initial_user_state.withdraw_at = Uint64::from(env.block.time.seconds());
        USER_STATE.save(deps.as_mut().storage, &info.sender, &initial_user_state).unwrap();

        let mut initial_token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        initial_token_state.total_locked = lock_balance.clone();
        TOKEN_STATE.save(deps.as_mut().storage, &initial_token_state).unwrap();

        deps.querier.update_wasm(cw20_mock_querier(lock_balance.clone()));

        let msg = ExecuteMsg::Withdraw {};
        let resp = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let expected_message = CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: MOCK_LOCKED_TOKEN.to_string(),
            msg: to_binary(
                &(Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: lock_balance.clone(),
                })
            ).unwrap(),
            funds: vec![],
        });
        let expected_resp: Response<Empty> = Response::new()
            .add_event(
                ContractEvent::make_withdraw(
                    info.sender.to_string(),
                    lock_balance.clone()
                ).to_cosmos_event()
            )
            .add_message(expected_message.clone());

        assert_eq!(resp, expected_resp);

        let mut expected_user_state = initial_user_state.clone();
        expected_user_state.locked_balance = Uint128::zero();
        expected_user_state.withdraw_at = Uint64::zero();

        let mut expected_token_state = initial_token_state.clone();
        expected_token_state.total_locked = Uint128::zero();

        assert_eq!(
            expected_user_state,
            USER_STATE.load(deps.as_mut().storage, &info.sender).unwrap()
        );

        assert_eq!(expected_token_state, TOKEN_STATE.load(deps.as_mut().storage).unwrap());
    }

    #[test]
    fn test_execute_withdraw_errors() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.clone(), info.to_owned());

        let mut user_state = UserState::default();
        USER_STATE.save(deps.as_mut().storage, &info.sender, &user_state).unwrap();

        let msg = ExecuteMsg::Withdraw {};

        // 1. Nothing to withdraw
        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap_err();

        assert_eq!(err, ContractError::WithdrawDelayNotOver {});

        // 2. Not enough time passed
        user_state.withdraw_at = Uint64::from(env.block.time.seconds() + 1);
        USER_STATE.save(deps.as_mut().storage, &info.sender, &user_state).unwrap();

        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap_err();
        assert_eq!(err, ContractError::WithdrawDelayNotOver {});
    }

    #[test]
    fn test_execute_withdraw_not_enough_reserves() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.clone(), info.to_owned());

        let lock_balance = apply_decimals(Uint128::from(1u8));
        let mut initial_user_state = UserState::default();
        initial_user_state.locked_balance = lock_balance.clone();
        initial_user_state.withdraw_at = Uint64::from(env.block.time.seconds());
        USER_STATE.save(deps.as_mut().storage, &info.sender, &initial_user_state).unwrap();

        let mut initial_token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        initial_token_state.total_locked = lock_balance.clone() * Uint128::from(2u8); //to sure that there is left locked balance after withdraw
        TOKEN_STATE.save(deps.as_mut().storage, &initial_token_state).unwrap();

        deps.querier.update_wasm(cw20_mock_querier(lock_balance.clone() / Uint128::from(2u8)));

        let msg = ExecuteMsg::Withdraw {};
        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();

        assert_eq!(err, ContractError::InsufficientReserves {});
    }

    #[test]
    fn test_execute_claim() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());
        let user_addr = Addr::unchecked("user");

        let initial_user_balance = apply_decimals(Uint128::from(1000u16));
        let user_locked_until_delta = Uint64::from(1000u64);
        let user_locked_balance = apply_decimals(Uint128::from(1000u16));
        let reward_per_token = Uint128::from(300000u64);

        internal_funcs
            ::set_balance(deps.as_mut(), &env, &info, &user_addr, initial_user_balance.clone())
            .unwrap();

        TOKEN_STATE.update(
            deps.as_mut().storage,
            |mut state| -> StdResult<_> {
                state.reward_per_token = reward_per_token.clone();
                Ok(state)
            }
        ).unwrap();

        USER_STATE.update(
            deps.as_mut().storage,
            &user_addr,
            |state| -> StdResult<_> {
                let mut user = state.unwrap_or_default();
                user.locked_balance = user_locked_balance.clone();
                user.locked_until = Uint64::from(
                    env.block.time.plus_seconds(user_locked_until_delta.clone().u64()).seconds()
                );
                Ok(user)
            }
        ).unwrap();

        let initial_token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        let initial_user_state = USER_STATE.load(deps.as_mut().storage, &user_addr).unwrap();

        deps.querier.update_wasm(cw20_mock_querier(user_locked_balance.clone()));

        let resp = execute(
            deps.as_mut(),
            env.clone(),
            mock_info(user_addr.as_str(), &[]),
            ExecuteMsg::Claim {}
        ).unwrap();

        let expected_pending_reward = initial_user_state.pending_reward(
            initial_token_state.reward_per_token
        );
        let expected_balance =
            (initial_user_state.locked_balance * Uint128::from(1000u128)) /
            Uint128::from(MAX_LOCK_PERIOD);

        let expected_message = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: initial_token_state.locked_token.to_string(),
            msg: to_binary(
                &(Cw20ExecuteMsg::Transfer {
                    recipient: user_addr.clone().into(),
                    amount: expected_pending_reward.clone(),
                })
            ).unwrap(),
            funds: vec![],
        });

        let expected_resp: Response<Empty> = Response::new()
            .add_message(expected_message)
            .add_event(
                ContractEvent::make_claim(
                    user_addr.to_string(),
                    expected_pending_reward.clone(),
                    expected_balance.clone()
                ).to_cosmos_event()
            );

        assert_eq!(expected_resp, resp);
    }
    #[test]
    fn test_execute_claim_insufficient_reserves() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());
        let user_addr = Addr::unchecked("user");

        let initial_user_balance = apply_decimals(Uint128::from(1000u16));
        let user_locked_until_delta = Uint64::from(1000u64);
        let user_locked_balance = apply_decimals(Uint128::from(1000u16));
        let reward_per_token = Uint128::from(300000u64);

        internal_funcs
            ::set_balance(deps.as_mut(), &env, &info, &user_addr, initial_user_balance.clone())
            .unwrap();

        TOKEN_STATE.update(
            deps.as_mut().storage,
            |mut state| -> StdResult<_> {
                state.reward_per_token = reward_per_token.clone();
                state.total_locked = user_locked_balance.clone();
                Ok(state)
            }
        ).unwrap();

        USER_STATE.update(
            deps.as_mut().storage,
            &user_addr,
            |state| -> StdResult<_> {
                let mut user = state.unwrap_or_default();
                user.locked_balance = user_locked_balance.clone();
                user.locked_until = Uint64::from(
                    env.block.time.plus_seconds(user_locked_until_delta.clone().u64()).seconds()
                );
                Ok(user)
            }
        ).unwrap();

        deps.querier.update_wasm(
            cw20_mock_querier(user_locked_balance.clone() / Uint128::from(2u128))
        );

        let err = execute(
            deps.as_mut(),
            env.clone(),
            mock_info(user_addr.as_str(), &[]),
            ExecuteMsg::Claim {}
        ).unwrap_err();

        assert_eq!(err, ContractError::InsufficientReserves {});
    }

    #[test]
    fn test_execute_add_income() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        let current_block = Uint64::from(env.block.height);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());
        let initial_locked = apply_decimals(Uint128::from(1u8));
        let initial_distribution_period = Uint64::from(1000u64);
        let initial_reward_rate_stored =
            apply_decimals(Uint128::from(1u128)) / Uint128::from(100u8);

        let mut initial_token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        initial_token_state.total_locked = initial_locked.clone();
        initial_token_state.distribution_period = initial_distribution_period.clone();
        initial_token_state.last_income_block = current_block.clone(); // assume for maximal unvested_amount
        initial_token_state.reward_rate_stored = initial_reward_rate_stored.clone();

        TOKEN_STATE.save(deps.as_mut().storage, &initial_token_state).unwrap();

        let add_amount = initial_locked.clone(); // twice total locked amount

        let expected_unvested_income =
            initial_reward_rate_stored.clone() * Uint128::from(initial_distribution_period.clone());
        let expected_new_reward_per_token =
            (expected_unvested_income.clone() + add_amount.clone()) /
            Uint128::from(initial_distribution_period);

        deps.querier.update_wasm(cw20_mock_querier(initial_locked.clone())); //any random value we don't check insufficient reserves error here

        let resp = execute(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[]),
            ExecuteMsg::AddIncome {
                add_amount,
            }
        ).unwrap();

        let mut expected_token_state = initial_token_state.clone();
        expected_token_state.last_income_block = current_block.clone();
        expected_token_state.reward_rate_stored = expected_new_reward_per_token.clone();

        assert_eq!(expected_token_state, TOKEN_STATE.load(deps.as_mut().storage).unwrap());

        let expected_message = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: initial_token_state.locked_token.to_string(),
            msg: to_binary(
                &(Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.into_string(),
                    recipient: env.contract.address.to_string(),
                    amount: add_amount.clone(),
                })
            ).unwrap(),
            funds: vec![],
        });

        let expected_response: Response<Empty> = Response::new()
            .add_event(
                ContractEvent::make_new_income(
                    add_amount.clone(),
                    expected_unvested_income.clone(),
                    expected_new_reward_per_token.clone()
                ).to_cosmos_event()
            )
            .add_message(expected_message);

        assert_eq!(expected_response, resp);
    }

    #[test]
    fn test_execute_add_income_insufficient_reserves() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        let current_block = Uint64::from(env.block.height);

        mock_instantiate(deps.as_mut().branch(), env.clone(), info.to_owned());
        let initial_locked = apply_decimals(Uint128::from(1u8));
        let initial_distribution_period = Uint64::from(1000u64);
        let initial_reward_rate_stored =
            apply_decimals(Uint128::from(1u128)) / Uint128::from(100u8);

        let mut initial_token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        initial_token_state.total_locked = initial_locked.clone();
        initial_token_state.total_supply = initial_locked.clone();
        initial_token_state.distribution_period = initial_distribution_period.clone();
        initial_token_state.last_income_block = current_block.clone(); // assume for maximal unvested_amount
        initial_token_state.reward_rate_stored = initial_reward_rate_stored.clone();
        initial_token_state.last_accrue_block =
            current_block.clone() - initial_distribution_period.clone();

        TOKEN_STATE.save(deps.as_mut().storage, &initial_token_state).unwrap();

        let add_amount = initial_locked.clone(); // twice total locked amount

        let expected_reward_per_token = initial_token_state.pending_reward_per_token(
            current_block.clone()
        );
        let expected_unvested_income =
            expected_reward_per_token * Uint128::from(initial_distribution_period.clone());

        let expected_minimal_reserves = expected_unvested_income.clone() + initial_locked.clone();

        deps.querier.update_wasm(
            cw20_mock_querier(expected_minimal_reserves.clone() - Uint128::from(1u8))
        );

        let err = execute(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[]),
            ExecuteMsg::AddIncome {
                add_amount,
            }
        ).unwrap_err();

        assert_eq!(err, ContractError::InsufficientReserves {});

        deps.querier.update_wasm(cw20_mock_querier(expected_minimal_reserves.clone()));

        let _resp = execute(
            deps.as_mut(),
            env.clone(),
            mock_info("creator", &[]),
            ExecuteMsg::AddIncome {
                add_amount,
            }
        ).unwrap();
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

        let mut deps_2 = mock_dependencies();
        let mut expected_state = TOKEN_STATE.load(deps.storage).unwrap();
        TOKEN_STATE.save(deps_2.as_mut().storage, &expected_state).unwrap();
        expected_state
            .set_distribution_period(
                deps_2.as_mut().storage,
                current_block.clone(),
                new_distribution_period.clone()
            )
            .unwrap();

        env.block.height = current_block.into();
        let msg = ExecuteMsg::SetDistributionPeriod { blocks: new_distribution_period };

        let resp = execute(deps.branch(), env.clone(), info, msg).unwrap();

        assert_eq!(expected_state, TOKEN_STATE.load(deps.storage).unwrap());

        let expected_response: Response<Empty> = Response::new().add_event(
            ContractEvent::make_new_distribution_period(
                new_distribution_period.clone()
            ).to_cosmos_event()
        );
        assert_eq!(expected_response, resp);
    }
}
