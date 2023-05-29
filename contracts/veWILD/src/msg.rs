use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    pub locked_token: Addr,
    pub distribution_period: u64,
}

#[cw_serde]
pub enum ExecuteMsg {}

#[cw_serde]
pub struct GreetResp {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
