#[cfg(test)]
mod contract_tests {
    use crate::consts::TOKEN_DECIMALS;
    use crate::contract::*;
    use crate::msg::*;
    use crate::state::TOKEN_STATE;
    use crate::state::TokenState;
    use cosmwasm_std::Uint128;
    use cosmwasm_std::{ Addr, Uint64 };
    use cw20_base::state::MinterData;
    use cw20_base::state::TOKEN_INFO;
    use cw20_base::state::TokenInfo;
    use cw_multi_test::{ App, ContractWrapper, Executor };
    use cosmwasm_std::testing::{ mock_dependencies, mock_env, mock_info };

    #[test]
    fn proper_instantiation() {
        let mut app = App::default();

        let code = ContractWrapper::new(execute, instantiate, query);
        let _code_id = app.store_code(Box::new(code));

        let mut deps_binding = mock_dependencies();
        let env = mock_env();

        let resp = instantiate(
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
}
