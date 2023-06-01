use cosmwasm_std::{ StdError };
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")] Std(#[from] StdError),
    #[error("VeToken: accrue first")] AccrueFirst {},
    #[error("VeToken: claim first")] ClaimFirst {},
    #[error("VeToken: distribution period must be >= 100 blocks")] ZeroDistributionPeriod {},
    #[error("VeToken: reserve balance too low")] InsufficientReserves {},
    #[error("Unauthorized")] Unauthorized {},

    #[error("VeToken: lock time too long")] LockPeriodTooLong {},
    #[error("VeToken: cannot reduce locked time")] CannotReduceLockedTime {},
    #[error("VeToken: lock time too short")] LockPeriodTooShort {},

    #[error("VeToken: nothing to withdraw")] NothingToWithdraw {},
    #[error("VeToken: cannot withdraw before unlock")] WithdrawBeforeUnlock {},
    #[error("VeToken: withdraw delay not over")] WithdrawDelayNotOver {},

    #[error("Unimplemented")] Unimplemented {},

    #[error("{0}")] CW20BaseError(String),
}
