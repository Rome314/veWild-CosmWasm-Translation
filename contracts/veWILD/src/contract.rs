use crate::consts::*;
use crate::error::*;
use crate::events::*;
use crate::msg::*;
use crate::state::*;
use cosmwasm_std::{
    Addr,
    CosmosMsg,
    entry_point,
    Binary,
    Deps,
    DepsMut,
    Env,
    MessageInfo,
    Response,
    StdResult,
    Uint64,
    Uint128,
};
use cw2::set_contract_version;
use cw20_base::state::{ MinterData, TOKEN_INFO, TokenInfo };
use cw20_base::contract::{ query_balance, query_token_info, execute_mint, execute_burn };

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
    TOKEN_STATE.save(deps.storage, &token_state)?;

    let response = utils::set_distribution_period(&deps, &env., msg.distribution_period)?;

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
    use super::*;

    pub fn set_distribution_period(
        deps: &DepsMut,
        env: &Env,
        blocks: Uint64
    ) -> Result<Response, ContractError> {
        if blocks.is_zero() {
            return Result::Err(ContractError::ZeroDistributionPeriod {});
        }

        accrue(deps, env)?;

        let token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.update_reward_rate(UpdateRewardRateInput {
            add_amount: Uint128::zero(),
            new_distribution_period: blocks,
            current_block: current_block,
        })?;

        TOKEN_STATE.save(deps.storage, &token_state)?;

        let event = ContractEvent::NewDistributionPeriod { value: blocks };
        let resp = Response::new().add_attributes(event.to_attributes());

        Ok(resp)
    }

    pub fn accrue(deps: &DepsMut, env: &Env) -> Result<(), ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.reward_per_token += pending_reward_per_token(deps.as_ref(), *env);
        token_state.last_accrue_block = current_block;

        TOKEN_STATE.save(deps.storage, &token_state);
        Ok(())
    }

    pub fn pending_account_reward(deps: Deps, env: Env, info: MessageInfo) -> Uint128 {
        let token_state = TOKEN_STATE.load(deps.storage).unwrap();
        let user_state = USER_STATE.load(deps.storage, &info.sender).unwrap();

        let pending_reward_per_token =
            token_state.reward_per_token + pending_reward_per_token(deps, env);
        let reward_per_token_delta = pending_reward_per_token - user_state.reward_snapshot;

        let balance = query_balance(deps, info.sender.into_string()).unwrap();

        return (reward_per_token_delta * balance.balance) / Uint128::from(TOKEN_DECIMALS); //Decimals?
    }

    pub fn pending_reward_per_token(deps: Deps, env: Env) -> Uint128 {
        let token_state = TOKEN_STATE.load(deps.storage).unwrap();
        let current_block = Uint64::from(env.block.height);
        let total_supply = query_token_info(deps).unwrap().total_supply;

        if total_supply.is_zero() {
            return Uint128::zero();
        }

        let blocks_elapsed = Uint128::from(current_block - token_state.last_accrue_block);
        return (blocks_elapsed * token_state.reward_rate(current_block)) / total_supply;
    }

    pub fn check_reserves(deps: Deps, env: Env) -> Result<(), ContractError> {
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
        accrue(&deps, &env)?;

        let token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let pending_reward = utils::pending_account_reward(deps.as_ref(), env, info);

        let locked_token_client = token_state.locked_token_client(deps.as_ref());
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
        // TODO: Do I need refs here?
        account: Addr,
        new_locked_until: Uint64
    ) -> Result<Response, ContractError> {
        let current_block = Uint64::from(env.block.height);

        let lock_seconds = if new_locked_until > current_block {
            new_locked_until - current_block
        } else {
            Uint64::zero()
        };

        let mut user_state = USER_STATE.load(deps.storage, &account)?;

        let new_balance =
            (user_state.locked_balance * Uint128::from(lock_seconds)) /
            Uint128::from(MAX_LOCK_PERIOD);
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
            return Result::Err(ContractError::ClaimFirst {});
        }

        let mut cw20_result: Result<Response, cw20_base::ContractError>;
        let user_balance = token_state
            .locked_token_client(deps.as_ref())
            .balance(account.to_owned())
            .unwrap();
        if amount > user_balance {
            cw20_result = execute_mint(
                deps,
                env,
                info,
                account.into_string(),
                amount - user_balance
            );
        } else if amount < user_balance {
            // TODO: ensure that amount is burnt from user
            cw20_result = execute_burn(deps, env, info, user_balance - amount);
        }

        match cw20_result {
            Ok(resp) => {
                return Ok(resp);
            }
            Err(err) => {
                return Err(ContractError::CW20BaseError(err.to_string()));
            }
        }
    }
}

mod query {
    use crate::msg::*;
    use super::*;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg
) -> Result<Response, ContractError> {
    use crate::msg::ExecuteMsg::*;
    match msg {
        LockMsg { amount, new_locked_until } =>
            exec::execute_lock(deps, env, info, amount, new_locked_until),
        ClaimMsg {} => exec::execute_claim(deps, env, info),
        SetDistributionPeriodMsg { blocks } =>
            exec::execute_set_distribution_period(deps, env, info, blocks),
        RequestWithdrawMsg {} => exec::execute_request_withdraw(deps, env, info),
        WithdrawMsg {} => exec::execute_withdraw(deps, env, info),
        AddIncomeMsg { add_amount } => exec::execute_add_income(deps, env, info, add_amount),
        _ => Result::Err(ContractError::Unimplemented {}),
    }
}

mod exec {
    use super::*;
    use utils::*;

    // TODO: nonReentrant(?)
    pub fn execute_lock(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        amount: Uint128,
        new_locked_until: Uint64
    ) -> Result<Response, ContractError> {
        let current_block = Uint64::from(env.block.height);
        let current_block_ts = Uint64::from(env.block.time.seconds());

        let lock_seconds: Uint64 = new_locked_until - current_block;

        if lock_seconds < Uint64::from(MIN_LOCK_PERIOD) {
            return Result::Err(ContractError::LockPeriodTooShort {});
        }
        if lock_seconds > Uint64::from(MAX_LOCK_PERIOD) {
            return Result::Err(ContractError::LockPeriodTooLong {});
        }

        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;
        if new_locked_until < user_state.locked_until {
            return Result::Err(ContractError::CannotReduceLockedTime {});
        }

        // TODO:implement
        /*         if is_contract(&info.sender) {
                   return Result::Err(ContractError::CannotLockContract {})
               }
        */

        let mut response = Response::new();

        let claim_response = utils::claim(deps, env, info)?;
        let cosmos_messages: Vec<CosmosMsg> = claim_response.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();
        response = response.add_messages(cosmos_messages).add_events(claim_response.events);

        let mut token_state = TOKEN_STATE.load(deps.storage)?;

        let messages: Vec<CosmosMsg> = vec![];
        if !amount.is_zero() {
            user_state.locked_balance += amount;
            token_state.total_locked += amount;

            // TODO: check returns
            token_state
                .locked_token_client(deps.as_ref())
                .transfer_from(info.sender, env.contract.address, amount)?;
        }

        USER_STATE.save(deps.storage, &info.sender, &user_state);
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let update_lock_response = utils::updateLock(
            deps,
            env,
            info,
            info.sender,
            new_locked_until
        )?;
        let cosmos_messages: Vec<CosmosMsg> = update_lock_response.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();
        response = response.add_messages(cosmos_messages).add_events(update_lock_response.events);

        utils::check_reserves(deps.as_ref(), env)?;

        let ve_balance = query_balance(deps.as_ref(), info.sender.into_string())?;

        let event = ContractEvent::Lock {
            account: info.sender,
            locked_until: new_locked_until,
            locked_balance: user_state.locked_balance,
            ve_balance: ve_balance.balance,
        };

        response = response.add_attributes(event.to_attributes());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_request_withdraw(
        deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let mut user_state: UserState = USER_STATE.load(deps.storage, &info.sender)?;

        let withdraw_amount = user_state.locked_balance;
        if withdraw_amount.is_zero() {
            return Result::Err(ContractError::NothingToWithdraw {});
        }

        let current_time = Uint64::from(env.block.time.seconds());
        if current_time < user_state.locked_until {
            return Result::Err(ContractError::WithdrawBeforeUnlock {});
        }

        let mut response = utils::claim(deps, env, info)?;

        user_state.withdraw_at = current_time + Uint64::from(WITHDRAW_DELAY);
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
    pub fn execute_withdraw(
        deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let withdraw_at = user_state.withdraw_at;
        let current_time = Uint64::from(env.block.time.seconds());

        if current_time < withdraw_at || withdraw_at.is_zero() {
            return Result::Err(ContractError::WithdrawDelayNotOver {});
        }

        utils::claim(deps, env, info)?;

        let withdraw_amount = user_state.locked_balance;
        user_state.withdraw_at = Uint64::zero();

        let mut token_state: TokenState = TOKEN_STATE.load(deps.storage)?;
        token_state.total_locked -= withdraw_amount;
        user_state.locked_balance = Uint128::zero();

        USER_STATE.save(deps.storage, &info.sender, &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let mut response = Response::new();

        let set_balance_resp = utils::setBalance(deps, env, info, &info.sender, Uint128::zero())?;
        let cosmos_messages: Vec<CosmosMsg> = set_balance_resp.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();
        response = response.add_messages(cosmos_messages).add_events(set_balance_resp.events);

        let cosmos_messages = token_state
            .locked_token_client(deps.as_ref())
            .transfer(info.sender, withdraw_amount)?;
        response = response.add_message(cosmos_messages);

        utils::check_reserves(deps.as_ref(), env)?;

        let event = ContractEvent::Withdraw {
            amount: withdraw_amount,
            account: info.sender,
        };

        response = response.add_attributes(event.to_attributes());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_claim(
        deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let mut response = Response::new();

        let claim_resp = utils::claim(deps, env, info)?;
        let cosmos_messages: Vec<CosmosMsg> = claim_resp.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();

        response = response.add_messages(cosmos_messages).add_events(claim_resp.events);
        utils::check_reserves(deps.as_ref(), env)?;

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_add_income(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        add_amount: Uint128
    ) -> Result<Response, ContractError> {
        accrue(&deps, &env)?;

        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        let transfer_message = token_state
            .locked_token_client(deps.as_ref())
            .transfer_from(info.sender, env.contract.address, add_amount)?;

        let response = Response::new().add_message(transfer_message);

        let unvested_income = token_state.update_reward_rate(UpdateRewardRateInput {
            add_amount: add_amount,
            new_distribution_period: token_state.distribution_period,
            current_block,
        })?;
        utils::check_reserves(deps.as_ref(), env)?;

        TOKEN_STATE.save(deps.storage, &token_state)?;

        let event = ContractEvent::NewIncome {
            add_amount: add_amount,
            remaining_amount: unvested_income,
            reward_rate: token_state.reward_rate_stored,
        };
        response.add_attributes(event.to_attributes());

        Ok(response)
    }

    pub fn execute_set_distribution_period(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        blocks: Uint64
    ) -> Result<Response, ContractError> {
        let mut response = set_distribution_period(&deps, &env, blocks)?;
        accrue(&deps, &env)?;

        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        let unvested_income = token_state.update_reward_rate(UpdateRewardRateInput {
            add_amount: Uint128::zero(),
            new_distribution_period: blocks,
            current_block,
        })?;
        utils::check_reserves(deps.as_ref(), env)?;

        TOKEN_STATE.save(deps.storage, &token_state)?;

        let event = ContractEvent::NewIncome {
            add_amount: Uint128::zero(),
            remaining_amount: unvested_income,
            reward_rate: token_state.reward_rate_stored,
        };
        response = response.add_attributes(event.to_attributes());

        Ok(response)
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
