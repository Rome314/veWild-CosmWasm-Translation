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
    use std::ops::Mul;

    use cosmwasm_std::{Uint128, Uint64, Event};

    use super::*;

    pub fn setDistributionPeriod(
        deps: DepsMut,
        env: Env,
        blocks: Uint64,
    ) -> Result<Event[], ContractError> {
        if blocks.is_zero() {
            return Result::Err(ContractError::ZeroDistributionPeriod {});
        }
        accrue(deps, env);
        updateRewardRate(
            deps,
            env,
            UpdateRewardRateInput {
                add_amount: Uint128::zero(),
                new_distribution_period: blocks,
            },
        )?;

        // TODO: check for better way to emit events
        let events = [
            Event::new("new_distribution_period").add_attribute("value", blocks.to_string()),
        ];

        Ok(events)
    }

    pub struct UpdateRewardRateInput {
        add_amount: Uint128,
        new_distribution_period: Uint64,
    }

    pub fn updateRewardRate(
        deps: DepsMut,
        env: Env,
        input: UpdateRewardRateInput,
    ) -> Result<_, ContractError> {
        let mut state = TOKEN_STATE.load(deps.storage)?;
        let current_block: Uint64 = Uint64::from(env.block.height);
        /*
        Avoid inflation of blocksElapsed inside of _pendingRewardPerToken()
        Ensures _pendingRewardPerToken() is 0 and all rewards are accounted for
        */
        if !current_block.eq(&state.last_accrue_block) {
            return Result::Err(ContractError::AccrueFirst {});
        }
        let blocks_elapsed: Uint64 = state
            .distribution_period
            .min(current_block - state.last_income_block);

        let unvested_income = state
            .reward_rate_stored
            .mul(Uint128::from(state.distribution_period - blocks_elapsed));

        state.reward_rate_stored =
            (unvested_income + input.add_amount) / Uint128::from(input.new_distribution_period);
        state.distribution_period = input.new_distribution_period;
        state.last_income_block = current_block;

        TOKEN_STATE.save(deps.storage, &state)?;
        Ok(())
    }

    // TODO: check return values
    pub fn accrue(deps: DepsMut, env: Env) -> Result<_, ContractError> {
        let mut state = TOKEN_STATE.load(deps.storage)?;

        let current_block = Uint64::from(env.block.height);

        state.reward_per_token += state.pending_reward_per_token(current_block);
        state.last_accrue_block = current_block;
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
