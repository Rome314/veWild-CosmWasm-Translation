use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::*;
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}

//  Internal functions
// TODO: Check is there better way to do this
mod utils {
    use cosmwasm_std::{StdError, Uint64};

    use super::*;

    pub fn updateRewardRate(deps: DepsMut, env: Env) -> Result<_, ContractError> {
        let mut state = TOKEN_STATE.load(deps.storage)?;
        /*
        Avoid inflation of blocksElapsed inside of _pendingRewardPerToken()
        Ensures _pendingRewardPerToken() is 0 and all rewards are accounted for
        */
        if !env.block.height.eq(&state.last_accrue_block) {
            return Result::Err(ContractError::AccrueFirst {});
        }
        let blocks_elapsed: Uint64 = Uint64::min(
            &state.distribution_period,
            env.block.height - state.last_income_block,
        );

        TOKEN_STATE.save(deps.storage, &state)?;
        Ok(())
    }

    // TODO: check return values
    pub fn accrue(deps: DepsMut, env: Env) -> Result<_, ContractError> {
        let mut state = TOKEN_STATE.load(deps.storage)?;
        state.reward_per_token += state.pending_reward_per_token(env.block.height);
        state.last_accrue_block = env.block.height;
        TOKEN_STATE.save(deps.storage, &state)?;
        Ok(())
    }
}

mod query {
    use crate::msg::*;

    use super::*;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    use ExecuteMsg::*;
    match msg {}
}

mod exec {
    use cosmwasm_std::{coins, BankMsg, Event};

    use super::*;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    use QueryMsg::*;

    match msg {}
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{coins, Addr};
    use cw_multi_test::{App, ContractWrapper, Executor};

    use super::*;
}
