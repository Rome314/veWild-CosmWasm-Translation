use cosmwasm_std::{Addr, StdError};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("VeToken: accrue first")]
    AccrueFirst {},
    #[error("VeToken: distribution period must be >= 100 blocks")]
    ZeroDistributionPeriod {},
}
