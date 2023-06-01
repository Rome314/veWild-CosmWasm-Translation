use crate::consts::TOKEN_DECIMALS;
use crate::consts::TOKEN_NAME;
use crate::consts::TOKEN_SYMBOL;
use crate::error::ContractError;
use crate::events::ContractEvent;
use crate::msg::*;
use crate::state::*;
use cosmwasm_std::Uint128;
use cosmwasm_std::{
    entry_point,
    Binary,
    Deps,
    DepsMut,
    Env,
    MessageInfo,
    Response,
    StdResult,
    Uint64,
};
use cw2::set_contract_version;
use cw20_base::state::MinterData;
use cw20_base::state::TOKEN_INFO;
use cw20_base::state::TokenInfo;
use cw_utils::{ must_pay, nonpayable };

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-bonding";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, ContractError> {
    // nonpayable(&info)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // store token info using cw20-base format
    let data = TokenInfo {
        name: TOKEN_NAME,
        symbol: TOKEN_SYMBOL,
        decimals: TOKEN_DECIMALS,
        total_supply: Uint128::zero(),
        // set self as minter, so we can properly execute mint and burn
        mint: Some(MinterData {
            minter: env.contract.address,
            cap: None,
        }),
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    let mut token_state: TokenState = TokenState::default();
    let current_block = Uint64::from(env.block.height);

    let response = token_state.set_distribution_period(current_block, msg.distribution_period)?;

    token_state.locked_token = msg.locked_token;
    token_state.last_accrue_block = Uint64::from(env.block.height);

    TOKEN_STATE.save(deps.storage, &token_state)?;

    //TODO: set/manage owner (?)
    //TODO: emit ownership transfer event (?)

    Ok(response)
}

//  Internal functions
// TODO: Check is there better way to do this
mod utils {
    use std::env;

    use cosmwasm_std::{ Addr, Uint128, Uint64 };
    use cw20_base::contract::{ execute_mint, execute_burn, query_balance, query_token_info };

    use crate::consts::MAX_LOCK_PERIOD;

    use super::*;

    pub fn pending_account_reward(deps: DepsMut, env: Env, info: MessageInfo) -> Uint128 {
        let token_state = TOKEN_STATE.load(deps.storage).unwrap();
        let user_state = USER_STATE.load(deps.storage, &info.sender).unwrap();
        let pending_reward_per_token =
            token_state.reward_per_token + pending_reward_per_token(deps, env);
        let reward_per_token_delta = pending_reward_per_token - user_state.reward_snapshot;

        let balance = query_balance(deps.as_ref(), info.sender.into_string()).unwrap();

        return (reward_per_token_delta * balance.balance) / Uint128::from(TOKEN_DECIMALS); //Decimals?
    }

    pub fn pending_reward_per_token(deps: DepsMut, env: Env) -> Uint128 {
        let token_state = TOKEN_STATE.load(deps.storage).unwrap();
        let current_block = Uint64::from(env.block.height);
        let total_supply = query_token_info(deps.as_ref()).unwrap().total_supply;

        if total_supply.is_zero() {
            return Uint128::zero();
        }

        let blocks_elapsed: Uint64 = current_block - token_state.last_accrue_block;
        return (
            (Uint128::from(blocks_elapsed) * token_state.reward_rate(current_block)) / total_supply
        );
    }

    pub fn check_reserves(deps: DepsMut, env: Env) -> Result<(), ContractError> {
        let token_state = TOKEN_STATE.load(deps.storage)?;

        let reserve_balance = token_state.locked_token_client(deps).balance(env.contract.address)?;

        let current_block = Uint64::from(env.block.height);
        let blocks_elapsed = token_state.distribution_period.min(
            current_block - token_state.last_income_block
        );

        let unvested_income =
            token_state.reward_per_token *
            Uint128::from(token_state.distribution_period - blocks_elapsed);

        if reserve_balance < token_state.total_locked + unvested_income {
            return Err(ContractError::InsufficientReserves {});
        }
        Ok(())
    }

    pub fn claim(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
        let token_state = TOKEN_STATE.load(deps.storage)?;

        let current_block = Uint64::from(env.block.height);
        token_state.accrue(current_block);

        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let pending_reward = utils::pending_account_reward(deps, env, info);

        let locked_token_client = token_state.locked_token_client(deps);
        if !pending_reward.is_zero() {
            locked_token_client.transfer(info.sender, pending_reward);
        }

        user_state.reward_snapshot = token_state.reward_per_token;

        USER_STATE.save(deps.storage, &info.sender, &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let mut response: Response = updateLock(
            deps,
            env,
            info,
            info.sender,
            user_state.locked_until
        )?;
        let user_balance = query_balance(deps.as_ref(), info.sender.into_string())?.balance;

        let event = ContractEvent::Claim {
            account: info.sender,
            claim_amount: pending_reward,
            ve_balance: user_balance,
        };
        response.add_attributes(event.to_attributes());

        Ok(response)
    }

    pub fn updateLock(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        account: Addr,
        new_locked_until: Uint64
    ) -> Result<Response, ContractError> {
        let current_block = Uint64::from(env.block.height);

        let lock_seconds = if new_locked_until > current_block {
            new_locked_until - current_block
        } else {
            Uint64::zero()
        };

        let user_state = USER_STATE.load(deps.storage, &account)?;

        let new_balance = (user_state.locked_balance * lock_seconds) / MAX_LOCK_PERIOD;
        user_state.locked_until = new_locked_until;

        USER_STATE.save(deps.storage, &account, &user_state)?;

        return setBalance(deps, env, info, &account, new_balance);
    }

    pub fn setBalance(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        account: &Addr,
        amount: Uint128
    ) -> Result<Response, ContractError> {
        let mut user_state: UserState = USER_STATE.load(deps.storage, account)?;
        let token_state = TOKEN_STATE.load(deps.storage)?;

        if !user_state.reward_snapshot.eq(&token_state.reward_per_token) {
            Result::Err(ContractError::ClaimFirst {});
        }

        let user_balance = token_state
            .locked_token_client(deps)
            .balance(account.to_owned())
            .unwrap();
        if amount > user_balance {
            return execute_mint(deps, env, info, account.into_string(), amount - user_balance);
        } else if amount < user_balance {
            // TODO: ensure that amount is burnt from user
            return execute_burn(deps, env, info, user_balance - amount);
        }

        Ok(Response::default())
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
    msg: ExecuteMsg
) -> Result<Response, ContractError> {
    use ExecuteMsg::*;
    match msg {
        LockMsg { amount, new_locked_until } => exec::execute_lock(deps, _env, info, msg),
        ClaimMsg {} => exec::exec_claim(deps, _env, info),
        SetDistributionPeriodMsg { blocks } =>
            exec::execute_set_distribution_period(deps, _env, info, blocks),
    }
}

mod exec {
    use cw20_base::contract::query_balance;
    use utils::{ claim };
    use cosmwasm_std::{ Event, CosmosMsg, to_binary, WasmMsg, Uint128 };
    use crate::consts::{ MIN_LOCK_PERIOD, MAX_LOCK_PERIOD, WITHDRAW_DELAY };

    use super::{ * };

    // TODO: nonReentrant(?)
    pub fn execute_lock(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: LockMsg
    ) -> Result<Response, ContractError> {
        let current_block = Uint64::from(env.block.height);
        let current_block_ts = Uint64::from(env.block.time);

        let lock_seconds: Uint64 = msg.new_locked_until - current_block;

        if lock_seconds < MIN_LOCK_PERIOD {
            return Result::Err(ContractError::LockPeriodTooShort {});
        }
        if lock_seconds > MAX_LOCK_PERIOD {
            return Result::Err(ContractError::LockPeriodTooLong {});
        }

        let mut user_state = USER_STATE.load(&info.sender)?;
        if msg.new_locked_until < user_state.locked_until {
            return Result::Err(ContractError::CannotReduceLockedTime {});
        }

        // TODO:implement
        /*         if is_contract(&info.sender) {
                   return Result::Err(ContractError::CannotLockContract {})
               }
        */

        let response = Response::new();

        let claim_response = utils::claim(deps, env, msg);
        response = response.add_messages(claim_response.messages).add_events(claim_response.events);

        let token_state = TOKEN_STATE.load(deps.storage)?;

        let messages: Vec<CosmosMsg> = vec![];
        if !msg.amount.is_zero() {
            user_state.locked_balance += msg.amount;
            token_state.total_locked += msg.amount;

            // TODO: check returns
            token_state
                .locked_token_client(deps)?
                .transfer_from(info.sender, env.contract.address, msg.amount)?;
        }

        USER_STATE.save(deps.storage, &info.sender, user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let update_lock_response = utils::updateLock(
            deps,
            env,
            info,
            info.sender,
            msg.new_locked_until
        )?;
        response = response
            .add_messages(update_lock_response.messages)
            .add_events(update_lock_response.events);

        let check_reserves_response = utils::check_reserves(deps, env)?;
        response = response
            .add_messages(check_reserves_response.messages)
            .add_events(check_reserves_response.events);

        let ve_balance = query_balance(deps, info.sender);

        let event = ContractEvent::Lock {
            account: info.sender,
            locked_until: msg.new_locked_until,
            locked_balance: user_state.locked_balance,
            ve_balance: ve_balance,
        };

        response = response.add_attributes(event.to_attributes());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn exec_request_withdraw(
        deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let user_state: UserState = USER_STATE.key(&info.sender)?;

        let withdraw_amount = user_state.locked_balance;
        if withdraw_amount.is_zero() {
            return Result::Err(ContractError::NothinToWithdraw {});
        }

        let current_time = Uint64::from(env.block.time);
        if current_time < user_state.locked_until {
            return Result::Err(ContractError::WithdrawBeforeUnlock {});
        }
        let response = Response::new();

        let claim_response = utils::claim(deps, env, info)?;
        response = response.add_messages(claim_response.messages).add_events(claim_response.events);

        user_state.withdraw_at = current_time + WITHDRAW_DELAY;
        USER_STATE.save(deps.storage, &info.sender, &user_state)?;

        let event = ContractEvent::WithdrawRequest {
            account: info.sender,
            amount: withdraw_amount,
            withdraw_at: user_state.withdraw_at,
        };

        response = response.add_attributes(event.to_attributes());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn exec_withdraw(
        deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let mut user_state = USER_STATE.load(deps, &info.sender)?;

        let withdraw_at = user_state.withdraw_at;
        let current_time = Uint64::from(env.block.time);

        if current_time < withdraw_at || withdraw_at.is_zero() {
            return Result::Err(ContractError::WithdrawDelayNotOver {});
        }

        utils::claim(deps, env, info)?;

        let withdraw_amount = user_state.locked_balance;
        user_state.withdraw_at = 0;

        let mut token_state: TokenState = TOKEN_STATE.load(deps.storage)?;
        token_state.total_locked -= withdraw_amount;
        user_state.locked_balance = 0;

        USER_STATE.save(deps.storage, &info.sender, &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let mut response = Response::new();

        let set_balance_resp = utils::setBalance(deps, env, info, &info.sender, 0)?;
        response = response
            .add_messages(set_balance_resp.messages)
            .add_events(set_balance_resp.events);

        token_state.locked_token_client(deps)?.transfer(info.sender, withdraw_amount)?;

        let check_reserves_resp = utils::check_reserves(deps, env)?;
        response = response
            .add_messages(check_reserves_resp.messages)
            .add_events(check_reserves_resp.events);

        let event = ContractEvent::Withdraw {
            amount: withdraw_amount,
            account: info.sender,
        };

        response = response.add_attributes(event.to_attributes());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn exec_claim(
        deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let response = Response::new();

        let claim_resp = utils::claim(deps, env, info)?;
        response = response.add_messages(claim_resp.messages).add_events(claim_resp.events);

        let check_reserves_resp = utils::check_reserves(deps, env)?;
        response = response
            .add_messages(check_reserves_resp.messages)
            .add_events(check_reserves_resp.events);

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn exec_add_income(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: AddIncomeMsg
    ) -> Result<Response, ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.accrue(current_block)?;

        token_state
            .locked_token_client(deps)?
            .transfer_from(info.sender, env.contract.address, msg.add_amount)?;

        let unvested_income = token_state.update_reward_rate(UpdateRewardRateInput {
            add_amount: msg.add_amount,
            new_distribution_period: token_state.distribution_period,
            current_block,
        })?;
        let resp = utils::check_reserves(deps, env)?;

        TOKEN_STATE.save(deps.storage, &token_state)?;

        let event = ContractEvent::NewIncome {
            add_amount: msg.add_amount,
            remaining_amount: unvested_income,
            reward_rate: token_state.reward_rate_stored,
        };
        resp.add_attributes(event.to_attributes());

        Ok(resp)
    }

    pub fn exec_set_distribution_period(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: SetDistributionPeriodMsg
    ) -> Result<Response, ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.set_distribution_period(current_block, SetDistributionPeriodMsg.blocks);

        token_state.accrue(current_block)?;

        let unvested_income = token_state.update_reward_rate(UpdateRewardRateInput {
            add_amount: Uint128::zero(),
            new_distribution_period: msg.new_distribution_period,
            current_block,
        })?;
        let resp = utils::check_reserves(deps, env)?;

        TOKEN_STATE.save(deps.storage, &token_state)?;

        let event = ContractEvent::NewIncome {
            add_amount: Uint128::zero(),
            remaining_amount: unvested_income,
            reward_rate: token_state.reward_rate_stored,
        };
        resp.add_attributes(event.to_attributes());

        Ok(resp)
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    use QueryMsg::*;

    match msg {
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{ coins, Addr };
    use cw_multi_test::{ App, ContractWrapper, Executor };

    use super::*;
}
