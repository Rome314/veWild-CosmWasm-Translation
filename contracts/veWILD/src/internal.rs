use cosmwasm_std::{ WasmMsg, Deps, Uint64, Uint128, DepsMut, MessageInfo, Env, Response, Addr };
use cw20_base::contract::{ query_balance, execute_mint, execute_burn, query_token_info };

use crate::{
    error::ContractError,
    state::{ TOKEN_STATE, USER_STATE, UserState },
    events::ContractEvent,
    consts::MAX_LOCK_PERIOD,
    cw20_client::CW20Client,
};

//  Internal functions
pub mod internal_funcs {
    use super::*;

    /// unvested_income = reward_per_token * (distribution_period - blocks_elapsed)
    /// reserve_balance MUST BE  >= total_locked + unvested_income
    pub fn check_reserves(deps: Deps, env: &Env) -> Result<(), ContractError> {
        let token_state = TOKEN_STATE.load(deps.storage)?;

        let reserve_balance = CW20Client::new(
            &deps.querier,
            token_state.locked_token.clone()
        ).balance(env.contract.address.clone())?;

        let current_block = Uint64::from(env.block.height);
        let blocks_elapsed = token_state.distribution_period.min(
            current_block - token_state.last_income_block
        );

        let unvested_income =
            token_state.reward_per_token *
            Uint128::from(token_state.distribution_period - blocks_elapsed);

        if reserve_balance < token_state.total_locked + unvested_income {
            return Err(ContractError::InsufficientReserves {});
        }
        Ok(())
    }

    pub fn claim(
        mut deps: DepsMut,
        env: &Env,
        info: &MessageInfo
    ) -> Result<Response, ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.accrue(deps.storage, current_block)?;

        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let pending_reward = user_state.pending_reward(token_state.reward_per_token.clone());

        let mut messages: Vec<WasmMsg> = vec![];
        if !pending_reward.is_zero() {
            let msg = CW20Client::new(&deps.querier, token_state.locked_token.clone()).make_transfer_msg(
                info.sender.to_owned(),
                pending_reward
            )?;
            messages.push(msg.into());
        }

        user_state.reward_snapshot = token_state.reward_per_token;

        let user_address = info.sender.clone().to_string();

        USER_STATE.save(deps.storage, &info.sender.to_owned(), &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let mut response: Response = update_lock(
            deps.branch(),
            env,
            info,
            &info.sender,
            user_state.locked_until
        )?;

        response = response.add_messages(messages);

        if !pending_reward.is_zero() {
            let user_balance = query_balance(deps.as_ref(), user_address.clone())?.balance;

            let event = ContractEvent::Claim {
                account: user_address,
                claim_amount: pending_reward,
                ve_balance: user_balance,
            };
            response = response.add_event(event.to_cosmos_event());
        }

        Ok(response)
    }

    pub fn update_lock(
        deps: DepsMut,
        env: &Env,
        info: &MessageInfo,
        account: &Addr,
        new_locked_until: Uint64
    ) -> Result<Response, ContractError> {
        let current_ts = Uint64::from(env.block.time.seconds());

        let lock_seconds = if new_locked_until > current_ts {
            Uint128::from(new_locked_until - current_ts)
        } else {
            Uint128::zero()
        };

        let mut user_state = USER_STATE.load(deps.storage, &account).unwrap_or_default();

        let new_balance =
            (user_state.locked_balance * lock_seconds) / Uint128::from(MAX_LOCK_PERIOD);

        user_state.locked_until = new_locked_until;

        USER_STATE.save(deps.storage, &account, &user_state)?;

        return set_balance(deps, &env, &info, &account, new_balance);
    }

    pub fn set_balance(
        mut deps: DepsMut,
        env: &Env,
        info: &MessageInfo,
        account: &Addr,
        amount: Uint128
    ) -> Result<Response, ContractError> {
        let mut user_state: UserState = USER_STATE.load(deps.storage, account).unwrap_or_default();
        let token_state = TOKEN_STATE.load(deps.storage)?;

        if !user_state.reward_snapshot.eq(&token_state.reward_per_token) {
            return Result::Err(ContractError::ClaimFirst {});
        }

        // TODO: check if this is correct way for internal transactions
        let mut cw_info = info.clone();
        let mut cw20_result: Result<Response, cw20_base::ContractError> = Ok(Response::default());
        if amount > user_state.balance {
            cw_info.sender = env.contract.address.clone();
            cw20_result = execute_mint(
                deps.branch(),
                env.to_owned(),
                cw_info, //contract info, because user can't mint by itself
                account.to_string(),
                amount - user_state.balance
            );
        } else if amount < user_state.balance {
            cw_info.sender = account.clone();
            cw20_result = execute_burn(
                deps.branch(),
                env.to_owned(),
                cw_info, // original info, because we need burn from that user
                user_state.balance - amount
            );
        }

        let total_supply = query_token_info(deps.as_ref()).unwrap().total_supply;
        TOKEN_STATE.update(
            deps.storage,
            |mut state| -> Result<_, ContractError> {
                state.total_supply = total_supply;
                Ok(state)
            }
        )?;

        let user_balance = query_balance(deps.as_ref(), account.to_string())?.balance;

        user_state.balance = user_balance;
        USER_STATE.save(deps.storage, account, &user_state)?;

        match cw20_result {
            Ok(resp) => {
                return Ok(resp);
            }
            Err(err) => {
                return Err(ContractError::CW20BaseError(err.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod internal_tests {
    use cosmwasm_std::{
        testing::{ mock_dependencies, mock_env, mock_info },
        StdResult,
        CosmosMsg,
        attr,
        Empty,
        to_binary,
    };
    use cw20::Cw20ExecuteMsg;
    use cw20_base::state::{ BALANCES, TOKEN_INFO };

    use crate::test_helpers::{ mock_instantiate, apply_decimals, cw20_mock_querier };

    use super::*;
    use internal_funcs::*;

    #[test]
    fn test_claim_with_reward() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.to_owned(), info.to_owned());

        let user_addr = Addr::unchecked("user");

        let initial_user_balance = apply_decimals(Uint128::from(1000u16));
        let user_locked_until_delta = Uint64::from(1000u64);
        let user_locked_balance = apply_decimals(Uint128::from(1000u16));
        let reward_per_token = Uint128::from(300000u64);

        set_balance(deps.as_mut(), &env, &info, &user_addr, initial_user_balance.clone()).unwrap();

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

        let token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        let user_state = USER_STATE.load(deps.as_mut().storage, &user_addr).unwrap();

        let current_block = Uint64::from(env.block.height.clone());

        let mut expected_token_state = token_state.clone();
        expected_token_state.reward_per_token = reward_per_token.clone();
        expected_token_state.last_accrue_block = current_block.clone();

        let expected_pending_reward = user_state.pending_reward(token_state.reward_per_token);
        let expected_balance =
            (user_state.locked_balance * Uint128::from(1000u128)) / Uint128::from(MAX_LOCK_PERIOD);

        let mut expected_user_state = user_state.clone();
        expected_user_state.balance = expected_balance.clone();
        expected_user_state.reward_snapshot = reward_per_token.clone();

        let mut user_info = info.clone();
        user_info.sender = user_addr.clone();
        let resp = claim(deps.as_mut(), &env, &user_info).unwrap();

        expected_token_state.total_supply = expected_balance.clone();

        assert_eq!(TOKEN_STATE.load(deps.as_mut().storage).unwrap(), expected_token_state);
        assert_eq!(
            USER_STATE.load(deps.as_mut().storage, &user_addr).unwrap(),
            expected_user_state
        );
        assert_eq!(
            BALANCES.load(deps.as_ref().storage, &user_addr).unwrap(),
            expected_balance.clone()
        );

        let expected_message = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_state.locked_token.to_string(),
            msg: to_binary(
                &(Cw20ExecuteMsg::Transfer {
                    recipient: user_addr.clone().into(),
                    amount: expected_pending_reward.clone(),
                })
            ).unwrap(),
            funds: vec![],
        });

        let expected_response: Response<Empty> = Response::new()
            .add_message(expected_message)
            .add_event(
                ContractEvent::make_claim(
                    user_addr.to_string(),
                    expected_pending_reward,
                    expected_balance.clone()
                ).to_cosmos_event()
            )
            .add_attributes(
                vec![
                    attr("action", "burn"),
                    attr("from", user_addr.to_string()),
                    attr("amount", user_locked_balance - expected_balance)
                ]
            );
        assert_eq!(resp, expected_response);
    }

    #[test]
    fn test_update_lock() {
        let mut deps_binding = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps_binding.as_mut(), env.to_owned(), info.to_owned());

        let user_addr = Addr::unchecked("user");
        let mut user_state = UserState::default();
        user_state.locked_balance = Uint128::from(1000u16);
        USER_STATE.save(deps_binding.as_mut().storage, &user_addr, &user_state).unwrap();

        // 1. Set non-zero balance

        let new_locked_until = Uint64::from(env.block.time.plus_seconds(300000).seconds());

        let resp = update_lock(
            deps_binding.as_mut(),
            &env,
            &info,
            &user_addr,
            new_locked_until
        ).unwrap();

        assert_eq!(
            BALANCES.load(deps_binding.as_mut().storage, &user_addr).unwrap(),
            Uint128::from(2u8)
        ); //(1000 * 300000)/126144000 = 2

        let expected_response: Response<Empty> = Response::new().add_attributes(
            vec![attr("action", "mint"), attr("to", user_addr.to_string()), attr("amount", "2")]
        );
        assert_eq!(resp, expected_response);

        // 2. Set zero balance

        let resp = update_lock(
            deps_binding.as_mut(),
            &env,
            &info,
            &user_addr,
            Uint64::from(env.block.time.seconds().to_owned())
        ).unwrap();

        assert_eq!(
            BALANCES.load(deps_binding.as_mut().storage, &user_addr).unwrap(),
            Uint128::zero()
        ); //(1000 * 0)/126144000 = 0

        let expected_response: Response<Empty> = Response::new().add_attributes(
            vec![attr("action", "burn"), attr("from", user_addr.to_string()), attr("amount", "2")]
        );
        assert_eq!(resp, expected_response);
    }

    #[test]
    fn test_set_balance_burn() {
        let mut deps_binding = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps_binding.as_mut(), env.to_owned(), info.to_owned());

        TOKEN_STATE.update(
            deps_binding.as_mut().storage,
            |mut state| -> StdResult<_> {
                state.reward_per_token = Uint128::from(10u128);
                state.total_supply = Uint128::from(1000u128);
                Ok(state)
            }
        ).unwrap();

        let user_addr = Addr::unchecked("user");
        let mut user_state = UserState::default();
        user_state.reward_snapshot = Uint128::from(10u16);
        user_state.balance = Uint128::from(100u16);
        USER_STATE.save(deps_binding.as_mut().storage, &user_addr, &user_state).unwrap();

        let mut cw20_info = info.clone();
        cw20_info.sender = env.clone().contract.address;

        execute_mint(
            deps_binding.as_mut(),
            env.to_owned(),
            cw20_info,
            "user".to_string(),
            Uint128::from(100u16)
        ).unwrap();

        assert_eq!(
            BALANCES.load(deps_binding.as_ref().storage, &user_addr).unwrap(),
            Uint128::from(100u16)
        );

        let resp = set_balance(
            deps_binding.as_mut(),
            &env,
            &info,
            &user_addr,
            Uint128::from(30 as u16)
        ).unwrap();

        let expected_response: Response<Empty> = Response::new().add_attributes(
            vec![attr("action", "burn"), attr("from", user_addr.to_string()), attr("amount", "70")]
        );
        assert_eq!(resp, expected_response);

        // Ensure that cw20 state and our states are synced
        let token_state = TOKEN_STATE.load(deps_binding.as_ref().storage).unwrap();
        let user_state = USER_STATE.load(deps_binding.as_ref().storage, &user_addr).unwrap();

        let cw20_state = TOKEN_INFO.load(deps_binding.as_ref().storage).unwrap();
        let cw20_balance = BALANCES.load(deps_binding.as_ref().storage, &user_addr).unwrap();

        assert_eq!(token_state.total_supply, cw20_state.total_supply);
        assert_eq!(user_state.balance, cw20_balance);
    }

    #[test]
    fn test_set_balance_mint() {
        let mut deps_binding = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps_binding.as_mut(), env.to_owned(), info.to_owned());

        let deps = deps_binding.as_mut();

        TOKEN_STATE.update(
            deps.storage,
            |mut state| -> StdResult<_> {
                state.reward_per_token = Uint128::from(10u128);
                state.total_supply = Uint128::from(1000u128);
                Ok(state)
            }
        ).unwrap();

        let user_addr = Addr::unchecked("user");
        let mut user_state = UserState::default();
        user_state.reward_snapshot = Uint128::from(10u16);
        USER_STATE.save(deps.storage, &user_addr, &user_state).unwrap();

        let resp = set_balance(
            deps_binding.as_mut(),
            &env,
            &info,
            &user_addr,
            Uint128::from(100 as u16)
        ).unwrap();

        let expected_response: Response<Empty> = Response::new().add_attributes(
            vec![attr("action", "mint"), attr("to", user_addr.to_string()), attr("amount", "100")]
        );

        assert_eq!(resp, expected_response);

        // Ensure that cw20 state and our states are synced
        let token_state = TOKEN_STATE.load(deps_binding.as_ref().storage).unwrap();
        let user_state = USER_STATE.load(deps_binding.as_ref().storage, &user_addr).unwrap();

        let cw20_state = TOKEN_INFO.load(deps_binding.as_ref().storage).unwrap();
        let cw20_balance = BALANCES.load(deps_binding.as_ref().storage, &user_addr).unwrap();

        assert_eq!(token_state.total_supply, cw20_state.total_supply);
        assert_eq!(user_state.balance, cw20_balance);
    }

    #[test]
    fn test_set_balance_errors() {
        let mut deps_binding = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps_binding.as_mut(), env.to_owned(), info.to_owned());

        TOKEN_STATE.update(
            deps_binding.as_mut().storage,
            |mut state| -> StdResult<_> {
                state.reward_per_token = Uint128::from(10u128);
                state.total_supply = Uint128::from(1000u128);
                Ok(state)
            }
        ).unwrap();

        let user_addr = Addr::unchecked("user");

        // Claim first error
        let mut user_state = UserState::default();
        user_state.reward_snapshot = Uint128::from(5 as u16);
        USER_STATE.save(deps_binding.as_mut().storage, &user_addr, &user_state).unwrap();

        let error = set_balance(
            deps_binding.as_mut(),
            &env,
            &info,
            &user_addr,
            Uint128::from(100 as u16)
        ).unwrap_err();
        assert_eq!(error, ContractError::ClaimFirst {});
    }

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
        let env = mock_env();
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
