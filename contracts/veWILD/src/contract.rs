use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::*;
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint64,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    use utils::setDistributionPeriod;
    // TODO: check for double initialization (?)
    let mut token_state: TokenState = TokenState::default();
    let user_state: UserState = UserState::default();

    token_state.locked_token = msg.locked_token;
    token_state.last_accrue_block = Uint64::from(env.block.height);

    TOKEN_STATE.save(deps.storage, &token_state)?;
    USER_STATE.save(deps.storage, &user_state)?;

    let events = setDistributionPeriod(deps, env, msg.distribution_period)?;

    //TODO: set/manage owner (?)
    //TODO: emit ownership transfer event (?)

    let resp = Response::new().add_events(events);

    Ok(resp)
}

//  Internal functions
// TODO: Check is there better way to do this
mod utils {
    use std::ops::Mul;

    use cosmwasm_std::{Addr, Event, Uint128, Uint64};

    use super::*;

    pub fn claim(deps: DepsMut, env: Env, msg: MessageInfo) -> Result<[Event], ContractError> {
        accrue(deps, env);

        let token_state = TOKEN_STATE.load(&deps.storage)?;
        let user_state = USER_STATE.load(&deps.storage, &msg.sender)?;

        let current_block = Uint64::from(env.block.height);
        let pending_reward = user_state.pending_reward(
            token_state.reward_per_token,
            token_state.pending_reward_per_token(current_block),
        );

        // TODO:implement
        /*         if !pending_reward.is_zero(){
                   IERC20(lockedToken).transfer(msg.sender, pendingReward);
               }
        */
        user_state.reward_snapshot = token_state.reward_per_token;

        let events = [Event::new("claim")];

        Ok(events)
    }

    pub fn updateLock(
        deps: DepsMut,
        env: Env,
        account: Addr,
        new_locked_until: Uint128,
    ) -> Result<[Event], ContractError> {
        let current_block = Uint64::from(env.block.height);

        let lock_seconds = if new_locked_until > current_block {
            new_locked_until - current_block
        } else {
            0
        };

        let user_state = USER_STATE.load(&deps.storage, &account)?;

        let new_balance = (user_state.locaked_balance * lock_seconds) / MAX_LOCK_PERIOD;
        user_state.locked_until = new_locked_until;
        USER_STATE.save(deps.storage, &account, &user_state)?;
        setBalance(deps, &account, new_balance);

        Ok(());
    }

    pub fn setBalance(
        deps: DepsMut,
        account: &Addr,
        amount: Uint128,
    ) -> Result<[Event], ContractError> {
        let mut user_state: UserState = USER_STATE.key(account)?;
        let token_state = TOKEN_STATE.load(&deps.storage)?;
        if !user_state.reward_snapshot.eq(&token_state.reward_per_token) {
            Result::Err(ContractError::ClaimFirst {})
        }

        let user_banalance = user_state.balance;
        let events: [Event];
        if amount > user_banalance {
            events = mint(deps, account, amount - user_banalance);
        } else if amount < user_banalance {
            events = burn(deps, account, user_banalance - amount);
        }

        Ok(events)
    }

    pub fn mint(deps: DepsMut, account: &Addr, amount: Uint128) -> Event {
        let mut token_state: TokenState = TOKEN_STATE.load(&deps.storage)?;
        let mut user_state: UserState = USER_STATE.key(account)?;

        user_state.balance += amount;
        token_state.total_supply += amount;

        TOKEN_STATE.save(deps.storage, &token_state);
        USER_STATE.save(deps.storage, account, &user_state);

        // TODO: Check for cw20 events
        Event::new("transfer").add_attributes(vec![
            ("from", "0"),
            ("to", &account.to_string()),
            ("amount", &amount.to_string()),
        ])
    }

    pub fn burn(deps: DepsMut, account: &Addr, amount: Uint128) -> Event {
        let mut token_state: TokenState = TOKEN_STATE.load(&deps.storage)?;
        let mut user_state: UserState = USER_STATE.key(account)?;

        user_state.balance -= amount;
        token_state.total_supply += amount;

        TOKEN_STATE.save(deps.storage, &token_state);
        USER_STATE.save(deps.storage, account, &user_state);

        // TODO: Check for cw20 events
        Event::new("transfer").add_attributes(vec![
            ("from", &account.to_string()),
            ("to", "0"),
            ("amount", &amount.to_string()),
        ])
    }

    pub fn setDistributionPeriod(
        deps: DepsMut,
        env: Env,
        blocks: Uint64,
    ) -> Result<[Event], ContractError> {
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
        let events =
            [Event::new("new_distribution_period").add_attribute("value", blocks.to_string())];

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
