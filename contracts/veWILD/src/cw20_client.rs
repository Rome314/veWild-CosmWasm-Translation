use cosmwasm_std::{ to_binary, Addr, Deps, StdResult, Uint128, WasmMsg };
use cw20::{ BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg };

pub struct CW20Client<'a> {
    deps: Deps<'a>,
    contract_addr: Addr,
}

impl<'a> CW20Client<'a> {
    pub fn new(deps: &Deps, contract_addr: Addr) -> Self {
        Self {
            deps,
            contract_addr,
        }
    }

    // Query balance
    pub fn balance(&self, address: Addr) -> StdResult<Uint128> {
        let balance_query_msg = Cw20QueryMsg::Balance {
            address: address.into(),
        };

        let balance: BalanceResponse = self.deps.querier.query_wasm_smart(
            &self.contract_addr,
            &balance_query_msg
        )?;

        Ok(balance.balance)
    }

    // Transfer tokens
    pub fn transfer(&self, recipient: Addr, amount: Uint128) -> StdResult<WasmMsg> {
        let transfer_execute_msg = Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount,
        };

        self.get_message(transfer_execute_msg)
    }

    // Transfer tokens from a given source to a recipient
    pub fn transfer_from(
        &self,
        owner: Addr,
        recipient: Addr,
        amount: Uint128
    ) -> StdResult<WasmMsg> {
        let transfer_from_execute_msg = Cw20ExecuteMsg::TransferFrom {
            owner: owner.into(),
            recipient: recipient.into(),
            amount,
        };

        self.get_message(transfer_from_execute_msg)
    }

    fn get_message(&self, msg: Cw20ExecuteMsg) -> StdResult<WasmMsg> {
        let execute_msg = WasmMsg::Execute {
            contract_addr: self.contract_addr.clone().into(),
            msg: to_binary(&msg)?,
            funds: vec![],
        };

        Ok(execute_msg)
    }
}
