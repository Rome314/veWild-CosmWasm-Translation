#[cfg(test)]
mod tests {
    use crate::contract::*;
    use crate::msg::*;
    use cosmwasm_std::{ Addr, Uint64 };
    use cw_multi_test::{ App, ContractWrapper, Executor };
    use cosmwasm_std::testing::{ mock_dependencies, mock_env, mock_info };

    #[test]
    fn proper_instantiation() {
        let mut app = App::default();

        let code = ContractWrapper::new(execute, instantiate, query);
        let _code_id = app.store_code(Box::new(code));

        let resp = instantiate(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("creator", &[]),
            InstantiateMsg {
                locked_token: Addr::unchecked("cw20"),
                distribution_period: Uint64::from(1000 as u16),
            }
        ).unwrap();

        println!("{:?}", resp.messages);
        println!("{:?}", resp.events);
        println!("{:?}", resp.attributes);
        assert_eq!(0, resp.messages.len());

        //TODO: check events
        //TODO: check state
    }
}
