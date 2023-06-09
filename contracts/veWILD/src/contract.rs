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
    _info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, ContractError> {
    // nonpayable(&info)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // store token info using cw20-base format
    let data = TokenInfo {
        name: "veWILD".to_string(),
        symbol: "veWILD".to_string(),
        decimals: TOKEN_DECIMALS as u8,
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

    let current_block = Uint64::from(env.block.height.clone());
    let response = token_state.set_distribution_period(
        deps.storage,
        current_block,
        msg.distribution_period
    )?;

    token_state.locked_token = msg.locked_token;
    token_state.last_accrue_block = current_block;

    TOKEN_STATE.save(deps.storage, &token_state)?;

    //TODO: set/manage owner (?)
    //TODO: emit ownership transfer event (?)

    Ok(response)
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
    use cosmwasm_std::StdError;
    use utils::*;

    // TODO: nonReentrant(?)
    pub fn execute_lock(
        mut deps: DepsMut,
        env: Env,
        info: MessageInfo,
        amount: Uint128,
        new_locked_until: Uint64
    ) -> Result<Response, ContractError> {
        let current_ts = Uint64::from(env.block.time.seconds());

        let lock_seconds: Uint64 = new_locked_until.checked_sub(current_ts).unwrap_or(Uint64::zero());

        if lock_seconds < Uint64::from(MIN_LOCK_PERIOD) {
            return Result::Err(ContractError::LockPeriodTooShort {});
        }
        if lock_seconds > Uint64::from(MAX_LOCK_PERIOD) {
            return Result::Err(ContractError::LockPeriodTooLong {});
        }

        let mut user_state = USER_STATE.load(deps.storage, &info.sender).unwrap_or_else(|_err| {
            let state = UserState::default();
            USER_STATE.save(deps.storage, &info.sender, &state).unwrap();
            state
        });
        if new_locked_until < user_state.locked_until {
            return Result::Err(ContractError::CannotReduceLockedTime {});
        }

        // TODO:implement
        /*         if is_contract(&info.sender) {
                   return Result::Err(ContractError::CannotLockContract {})
               }
        */

        let mut response = Response::new();

        let claim_response = utils::claim(deps.branch(), &env, &info)?;
        let cosmos_messages: Vec<CosmosMsg> = claim_response.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();
        response = response
            .add_messages(cosmos_messages)
            .add_events(claim_response.events)
            .add_attributes(claim_response.attributes);

        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let mut messages: Vec<CosmosMsg> = vec![];
        if !amount.is_zero() {
            // TODO: Do I need to handle it on Reply? 
            
            user_state.locked_balance += amount;
            token_state.total_locked += amount;

            // TODO: check returns
            let msg = token_state
                .locked_token_client(&deps.as_ref())
                .transfer_from(info.sender.to_owned(), env.contract.address.to_owned(), amount)?;
            messages.push(CosmosMsg::Wasm(msg));
        }
        // TODO: check for submessage
        response = response.add_messages(messages);

        USER_STATE.save(deps.storage, &info.sender, &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let update_lock_response = utils::update_lock(
            deps.branch(),
            &env,
            &info,
            &info.sender,
            new_locked_until
        )?;
        let cosmos_messages: Vec<CosmosMsg> = update_lock_response.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();
        response = response
            .add_messages(cosmos_messages)
            .add_events(update_lock_response.events)
            .add_attributes(update_lock_response.attributes);

        utils::check_reserves(deps.as_ref(), &env)?;

        let ve_balance = query_balance(deps.as_ref(), info.sender.to_owned().into_string())?;

        let event = ContractEvent::Lock {
            account: info.sender.to_string(),
            locked_until: new_locked_until,
            locked_balance: user_state.locked_balance,
            ve_balance: ve_balance.balance,
        };

        response = response.add_event(event.to_cosmos_event());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_request_withdraw(
        mut deps: DepsMut,
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

        let mut response = utils::claim(deps.branch(), &env, &info)?;

        user_state.withdraw_at = current_time + Uint64::from(WITHDRAW_DELAY);
        USER_STATE.save(deps.storage, &info.sender.clone(), &user_state)?;

        let event = ContractEvent::WithdrawRequest {
            account: info.sender.to_string(),
            amount: withdraw_amount,
            withdraw_at: user_state.withdraw_at,
        };

        response = response.add_event(event.to_cosmos_event());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_withdraw(
        mut deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let withdraw_at = user_state.withdraw_at;
        let current_time = Uint64::from(env.block.time.seconds());

        if current_time < withdraw_at || withdraw_at.is_zero() {
            return Result::Err(ContractError::WithdrawDelayNotOver {});
        }

        utils::claim(deps.branch(), &env, &info)?;

        let withdraw_amount = user_state.locked_balance;
        user_state.withdraw_at = Uint64::zero();

        let mut token_state: TokenState = TOKEN_STATE.load(deps.storage)?;
        token_state.total_locked -= withdraw_amount;
        user_state.locked_balance = Uint128::zero();

        USER_STATE.save(deps.storage, &info.sender, &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let mut response = Response::new();

        let set_balance_resp = utils::set_balance(
            deps.branch(),
            &env,
            &info,
            &info.sender.to_owned(),
            Uint128::zero()
        )?;
        let cosmos_messages: Vec<CosmosMsg> = set_balance_resp.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();
        response = response.add_messages(cosmos_messages).add_events(set_balance_resp.events);

        let cosmos_messages = token_state
            .locked_token_client(&deps.as_ref())
            .transfer(info.sender.to_owned(), withdraw_amount)?;
        response = response.add_message(cosmos_messages);

        utils::check_reserves(deps.as_ref(), &env)?;

        let event = ContractEvent::Withdraw {
            amount: withdraw_amount,
            account: info.sender.to_string(),
        };

        response = response.add_event(event.to_cosmos_event());

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_claim(
        mut deps: DepsMut,
        env: Env,
        info: MessageInfo
    ) -> Result<Response, ContractError> {
        let mut response = Response::new();

        let claim_resp = utils::claim(deps.branch(), &env, &info)?;
        let cosmos_messages: Vec<CosmosMsg> = claim_resp.messages
            .iter()
            .map(|msg| msg.msg.clone())
            .collect();

        response = response.add_messages(cosmos_messages).add_events(claim_resp.events);
        utils::check_reserves(deps.as_ref(), &env)?;

        Ok(response)
    }

    // TODO: nonReentrant(?)
    pub fn execute_add_income(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        add_amount: Uint128
    ) -> Result<Response, ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.accrue(deps.storage, current_block)?;

        let transfer_message = token_state
            .locked_token_client(&deps.as_ref())
            .transfer_from(info.sender, env.contract.address.clone(), add_amount)?;
        let response = Response::new().add_message(transfer_message);

        let unvested_income = token_state.update_reward_rate(deps.storage, UpdateRewardRateInput {
            add_amount: add_amount,
            new_distribution_period: token_state.distribution_period,
            current_block,
        })?;
        utils::check_reserves(deps.as_ref(), &env)?;

        TOKEN_STATE.save(deps.storage, &token_state)?;

        let event = ContractEvent::NewIncome {
            add_amount: add_amount,
            remaining_amount: unvested_income,
            reward_rate: token_state.reward_rate_stored,
        };
        let response = response.add_event(event.to_cosmos_event());

        Ok(response)
    }

    pub fn execute_set_distribution_period(
        deps: DepsMut,
        env: Env,
        _info: MessageInfo,
        new_distribution_period: Uint64
    ) -> Result<Response, ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);
        return token_state.set_distribution_period(
            deps.storage,
            current_block,
            new_distribution_period
        );
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    use QueryMsg::*;

    match msg {
    }
}

//  Internal functions
pub(crate) mod utils {
    use cosmwasm_std::WasmMsg;

    use super::*;

    pub fn check_reserves(deps: Deps, env: &Env) -> Result<(), ContractError> {
        let token_state = TOKEN_STATE.load(deps.storage)?;

        let reserve_balance = token_state
            .locked_token_client(&deps)
            .balance(env.contract.address.clone())?;

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

    pub fn claim(
        mut deps: DepsMut,
        env: &Env,
        info: &MessageInfo
    ) -> Result<Response, ContractError> {
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        token_state.accrue(deps.storage, current_block)?;

        let mut user_state = USER_STATE.load(deps.storage, &info.sender)?;

        let pending_reward = user_state.pending_reward(token_state.reward_per_token.clone());

        let mut messages: Vec<WasmMsg> = vec![];
        if !pending_reward.is_zero() {
            let msg = token_state
                .locked_token_client(&deps.as_ref())
                .transfer(info.sender.to_owned(), pending_reward)?;
            messages.push(msg.into());
        }

        user_state.reward_snapshot = token_state.reward_per_token;

        let user_address = info.sender.clone().to_string();

        USER_STATE.save(deps.storage, &info.sender.to_owned(), &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let response: Response = update_lock(
            deps.branch(),
            env,
            info,
            &info.sender,
            user_state.locked_until
        )?;
        let user_balance = query_balance(deps.as_ref(), user_address.clone())?.balance;

        let event = ContractEvent::Claim {
            account: user_address,
            claim_amount: pending_reward,
            ve_balance: user_balance,
        };
        let response = response.add_event(event.to_cosmos_event()).add_messages(messages);

        Ok(response)
    }

    pub fn update_lock(
        deps: DepsMut,
        env: &Env,
        info: &MessageInfo,
        account: &Addr,
        new_locked_until: Uint64
    ) -> Result<Response, ContractError> {
        let current_ts = Uint64::from(env.block.time.seconds());

        let lock_seconds = if new_locked_until > current_ts {
            Uint128::from(new_locked_until - current_ts)
        } else {
            Uint128::zero()
        };

        let mut user_state = USER_STATE.load(deps.storage, &account).unwrap_or_default();

        let new_balance =
            (user_state.locked_balance * lock_seconds) / Uint128::from(MAX_LOCK_PERIOD);

        user_state.locked_until = new_locked_until;

        USER_STATE.save(deps.storage, &account, &user_state)?;

        return set_balance(deps, &env, &info, &account, new_balance);
    }

    pub fn set_balance(
        mut deps: DepsMut,
        env: &Env,
        info: &MessageInfo,
        account: &Addr,
        amount: Uint128
    ) -> Result<Response, ContractError> {
        let mut user_state: UserState = USER_STATE.load(deps.storage, account).unwrap_or_default();
        let token_state = TOKEN_STATE.load(deps.storage)?;

        if !user_state.reward_snapshot.eq(&token_state.reward_per_token) {
            return Result::Err(ContractError::ClaimFirst {});
        }

        // TODO: check if this is correct way for internal transactions
        let mut cw_info = info.clone();
        let mut cw20_result: Result<Response, cw20_base::ContractError> = Ok(Response::default());
        if amount > user_state.balance {
            cw_info.sender = env.contract.address.clone();
            cw20_result = execute_mint(
                deps.branch(),
                env.to_owned(),
                cw_info, //contract info, because user can't mint by itself
                account.to_string(),
                amount - user_state.balance
            );
        } else if amount < user_state.balance {
            cw_info.sender = account.clone();
            // TODO: ensure that amount is burnt from user
            cw20_result = execute_burn(
                deps.branch(),
                env.to_owned(),
                cw_info, // original info, because we need burn from that user
                user_state.balance - amount
            );
        }

        let total_supply = query_token_info(deps.as_ref()).unwrap().total_supply;
        TOKEN_STATE.update(
            deps.storage,
            |mut state| -> Result<_, ContractError> {
                state.total_supply = total_supply;
                Ok(state)
            }
        )?;

        user_state.balance = amount;
        USER_STATE.save(deps.storage, account, &user_state)?;

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
