use crate::error::ContractError;
use crate::msg::*;
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

    use cosmwasm_std::{to_binary, Addr, BankMsg, CosmosMsg, Event, Uint128, Uint64, WasmMsg};
    use cw20::BalanceResponse;

    use super::*;

    pub fn check_reserves(deps: Deps,env:Env)->Result<_,ContractError>{
        
        let balance_of_msg = cw20::Cw20QueryMsg::Balance { address: env.contract.address.into_string() }; 
    
        let token_state = TOKEN_STATE.load(deps.storage)?;
        let balance_response: cw20::BalanceResponse = deps.querier.query_wasm_smart(
            &token_state.locked_token, 
            &to_binary(&balance_of_msg)?
        )?;
        
        let reserve_balance = balance_response.balance;

        let current_block = Uint64::from(env.block.height);
        let blocks_elapsed = token_state.distribution_period.min(current_block-token_state.last_income_block);

        let unvested_income = token_state.reward_per_token * (token_state.distribution_period-blocks_elapsed);
        
        if reserve_balance < token_state.total_locked +unvested_income{
            return Err(ContractError::InsufficientReserves{});
        }
        Ok(())

    }

    pub fn claim(deps: DepsMut, env: Env, msg: MessageInfo) -> Result<Response, ContractError> {
        accrue(deps, env);

        let token_state = TOKEN_STATE.load(&deps.storage)?;
        let user_state = USER_STATE.load(&deps.storage, &msg.sender)?;

        let current_block = Uint64::from(env.block.height);
        let pending_reward = user_state.pending_reward(
            token_state.reward_per_token,
            token_state.pending_reward_per_token(current_block),
        );

        let mut messages: Vec<CosmosMsg>;
        if !pending_reward.is_zero() {
            let token_transfer_msg = ExecuteMsg::Transfer {
                recipient: msg.sender.clone().into_string(),
                pending_reward,
            };
            messages.append(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: token_state.locked_token.clone().into_string(),
                msg: to_binary(&token_transfer_msg)?,
                funds: vec![],
            }))
        }

        user_state.reward_snapshot = token_state.reward_per_token;
        let mut events = updateLock(deps, env, msg.sender, user_state.locked_until)?;
        events.add(
            Event::new("claim")
                .add_attribute("ve_balances", &user_state.balance.to_string())
                .add_attribute("claim_amount", &pending_reward.to_string()),
        );

        Ok(Response::new().add_messages(messages).add_events(events))
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
    match msg {
        // LockMsg { amount,new_locked_until } => exec::execute_lock(deps, _env, info, LockMsg { amount: (), new_locked_until: () }{amount,newlocked_until}),
        // Claim {} => exec::execute_claim(deps, _env, info),
        // Transfer { recipient, amount } => exec::execute_transfer(deps, _env, info, recipient, amount),
        // SetDistributionPeriod { blocks } => exec::execute_set_distribution_period(deps, _env, info, blocks),
        // UpdateRewardRate { add_amount, new_distribution_period } => exec::execute_update_reward_rate(deps, _env, info, add_amount, new_distribution_period),
    }
}

mod exec {
    use std::mem::align_of;
    use schemars::_serde_json::de;
    use utils::{claim}
    use cosmwasm_std::{coins, BankMsg, Event, CosmosMsg, to_binary, WasmMsg, Uint128};
    use super::{*, utils::UpdateRewardRateInput};



    // TODO: nonReentrant(?)
    pub fn exec_add_income(deps: DepsMut,env: Env,info:MessageInfo,msg:AddIncomeMsg)->Result<Response,ContractError>{
        let mut token_state = TOKEN_STATE.load(deps.storage)?;
        let current_block = Uint64::from(env.block.height);

        accrue(deps, env)?;

        
        // TODO: wrap somewhere
        let token_transfer_msg = cw20::Cw20ExecuteMsg::TransferFrom {
            recipient: env.contract.address.clone().into_string(),
            owner: msg.sender.clone().into_string(),
            amount: msg.amount,
        };
        messages.append(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_state.locked_token.clone().into_string(),
            msg: to_binary(&token_transfer_msg)?,
            funds: vec![],
        }));

        let unvested_income = utils::updateRewardRate(deps, env, UpdateRewardRateInput{
            add_amount: msg.amount,
            new_distribution_period: token_state.distribution_period,
        })?;

        utils::check_reserves(deps, env)?;


        let resp = Response::new()
            .add_messages(messages)
            .add_attributes(vec![
                ("action", "new_income"),
                ("reward_rate", token_state.reward_rate_stored.to_string().as_str()),
                ("add_amount", msg.amount.to_string().as_str()),
                ("remaining_amount", unvested_income.to_string().as_str()),
            ]);       
        Ok(resp)
    }

    // TODO: nonReentrant(?)
    pub fn exec_claim(deps:DepsMut ,env: Env,info: MessageInfo)->Result<Response,ContractError>{
        utils::claim(deps, env, info)?;
        utils::check_reserves(deps, env)?;
        Ok(Response::new())
    }

    // TODO: nonReentrant(?)
    pub fn exec_withdraw(deps: DepsMut,env: Env,info: MessageInfo)->Result<Response,ContractError>{
        let mut user_state = USER_STATE.key(&info.sender)?;

        let withdraw_at = user_state.withdraw_at;
        let current_time = Uint64::from(env.block.time);
        

        if(current_time < withdraw_at || withdraw_at.is_zero()){
            return Result::Err(ContractError::WithdrawDelayNotOver{});
        }

        utils::claim(deps, env, info)?;

        let withdraw_amount = user_state.locked_balance;
        user_state.withdraw_at = 0;

        let mut token_state:TokenState = TOKEN_STATE.load(deps.storage)?;
        token_state.total_locked -= withdraw_amount;
        user_state.locked_balance = 0;

        USER_STATE.save(deps.storage, &user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        let events = utils::setBalance(deps, &info.sender, Uint128::zero())?;

        let messages:Vec<CosmosMsg>;

        let token_transfer_msg = ExecuteMsg::Transfer {
            recipient: msg.sender.clone().into_string(),
            pending_reward,
        };
        messages.append(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_state.locked_token.clone().into_string(),
            msg: to_binary(&token_transfer_msg)?,
            funds: vec![],
        }));

        utils::check_reserves(deps, env)?;

        let resp = Response::new()
            .add_messages(messages)
            .add_events(events).add_event(
                Event::new("withdraw")
                .add_attribute("amount", withdraw_amount.to_string())
                .add_attribute("account", info.sender));

        Ok(resp)

    }
        // TODO: nonReentrant(?)
    pub fn exec_request_withdraw(deps:DepsMut,env:Env,info:MessageInfo)->Result<Response,ContractError>{
        let user_state:UserState = USER_STATE.key(&info.sender)?;

        let withdraw_amount = user_state.locked_balance;
        if(withdraw_amount.is_zero()){
            return Result::Err(ContractError::NothinToWithdraw{});
        }

        let current_time = Uint64::from(env.block.time);
        if(current_time < user_state.locked_until){
            return Result::Err(ContractError::WithdrawBeforeUnlock{});
        }

        utils::claim(deps, env, info)?;
        user_state.withdraw_at = current_time + WITHDRAW_DELAY;

        USER_STATE.save(deps.storage, &user_state)?;

        Ok(Response::new()
        .add_event(
            Event::new("withdraw_request")
            .add_attribute("account",info.sender.to_string() )
            .add_attribute("amount", withdraw_amount.to_string()))
            .add_attribute("withdraw_at", user_state.withdraw_at.to_string()))

    }

    }

    // TODO: nonReentrant(?)
    pub fn execute_lock(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: LockMsg,
    ) -> Result<Response, ContractError> {
        let current_block = Uint64::from(env.block.height);
        let current_block_ts = Uint64::from(env.block.time);

        let lock_seconds = msg.new_locked_until - current_block;

        if lock_seconds < MIN_LOCK_PERIOD {
            return Result::Err(ContractError::LockPeriodTooShort {});
        }
        if lock_seconds > MAX_LOCK_PERIOD {
            return Result::Err(ContractError::LockPeriodTooLong {});
        }

        let mut user_state = USER_STATE.key(&info.sender)?;
        if msg.new_locked_until < user_state.locked_until {
            return Result::Err(ContractError::CannotReduceLockedTime {});
        }

        // TODO:implement
        /*         if is_contract(&info.sender) {
                   return Result::Err(ContractError::CannotLockContract {})
               }
        */

        utils::claim(deps, env, msg);

        let token_state = TOKEN_STATE.load(deps.storage)?;

        let messages: Vec<CosmosMsg> = vec![];
        if !msg.amount.is_zero(){
            user_state.locked_balance += msg.amount;
            token_state.total_locked += msg.amount;

            let token_transfer_msg = cw20::Cw20ExecuteMsg::TransferFrom {
                recipient: env.contract.address.clone().into_string(),
                owner: msg.sender.clone().into_string(),
                amount: msg.amount,
            };
            messages.append(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: token_state.locked_token.clone().into_string(),
                msg: to_binary(&token_transfer_msg)?,
                funds: vec![],
            }))
        }

        let events = utils::updateLock(deps, env, info.sender, msg.new_locked_until)?.unwrap();
        utils::check_reserves(deps, env)?;

        USER_STATE.save(deps.storage, &info.sender, user_state)?;
        TOKEN_STATE.save(deps.storage, &token_state)?;

        Ok(Response::new()
            .add_messages(messages)
            .add_events(events)
            .add_event(Event::new("lock").add_attributes(vec![
                ("account", info.sender.as_str()),
                ("locked_balance",user_state.lcoked_balance.to_string()),
                ("ve_balance", user_state.balance.to_string()),
                ("locked_until", &msg.new_locked_until.to_string()),
            ]))
        )
    }
    }

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
