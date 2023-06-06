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

use crate::test_helpers::{ * };

#[cfg(test)]
mod utils_tests {
    use cosmwasm_std::{ StdResult, Decimal, StdError };
    use cw20_base::{ state::{ BALANCES, TOKEN_INFO }, contract::execute_mint };
    use crate::{
        contract::utils::{ * },
        error::ContractError,
        state::{ USER_STATE, UserState, TOKEN_STATE },
    };

    use super::*;

    #[test]
    fn test_claim() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        mock_instantiate(deps.as_mut(), env.to_owned(), info.to_owned());

        let user_addr = Addr::unchecked("user");

        set_balance(
            deps.as_mut(),
            &env,
            &info,
            &user_addr,
            apply_decimals(Uint128::from(1000u16))
        ).unwrap();

        USER_STATE.update(
            deps.as_mut().storage,
            &user_addr,
            |mut state| -> StdResult<_> {
                let mut user = state.unwrap_or_default();
                user.locked_balance = apply_decimals(Uint128::from(1000u16));
                user.locked_until = Uint64::from(env.block.time.plus_seconds(1000).seconds());
                Ok(user)
            }
        ).unwrap();

        let user_state = USER_STATE.load(deps.as_mut().storage, &user_addr).unwrap();

        let current_block = Uint64::from(env.block.height.clone());

        TOKEN_STATE.update(
            deps.as_mut().storage,
            |mut state| -> StdResult<_> {
                state.reward_per_token = Uint128::from(300000u64);
                Ok(state)
            }
        ).unwrap();

        let mut tmp_deps = mock_dependencies();
        let mut expected_token_state = TOKEN_STATE.load(deps.as_mut().storage).unwrap();
        TOKEN_STATE.save(tmp_deps.as_mut().storage, &expected_token_state).unwrap();
        expected_token_state.accrue(tmp_deps.as_mut().storage, current_block).unwrap();

        let pending_reward = user_state.pending_reward(
            expected_token_state.reward_per_token,
            expected_token_state.pending_reward_per_token(current_block)
        );

        assert_ne!(pending_reward, Uint128::zero());

        let mut user_info = info.clone();
        user_info.sender = user_addr.clone();
        let resp = claim(deps.as_mut(), &env, &user_info).unwrap();
        
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

        assert_has_events(
            &resp,
            vec![ContractEvent::Mint { amount: Uint128::from(2u8), to: user_addr.to_string() }]
        );

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

        assert_has_events(
            &resp,
            vec![ContractEvent::Burn { amount: Uint128::from(2u8), from: user_addr.to_string() }]
        );
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

        assert_has_events(
            &resp,
            vec![ContractEvent::Burn { amount: Uint128::from(70u16), from: user_addr.to_string() }]
        );

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

        assert_has_events(
            &resp,
            vec![ContractEvent::Mint { amount: Uint128::from(100u16), to: user_addr.to_string() }]
        );

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
        let mut env = mock_env();
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

        // User not exist error
        // REPLACED WITH DEFAULT  TODO: Check is it required
        // let err = set_balance(
        //     deps_binding.as_mut(),
        //     &env,
        //     &info,
        //     &user_addr,
        //     Uint128::from(100 as u16)
        // ).unwrap_err();
        // assert_eq!(
        //     err,
        //     ContractError::Std(StdError::NotFound { kind: "veWILD::state::UserState".to_string() })
        // );

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
    use cw20_base::state::{ TokenInfo, TOKEN_INFO, MinterData };

    use crate::state::{ TOKEN_STATE, TokenState };

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
            decimals: TOKEN_DECIMALS as u8,
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
