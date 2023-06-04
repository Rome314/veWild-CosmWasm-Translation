use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ Addr, Deps, Uint128, Uint64, Response, Storage };
use cw_storage_plus::{ Item, Map };

use crate::consts::TOKEN_DECIMALS;
use crate::cw20_client::CW20Client;
use crate::error::ContractError;
use crate::{ cw20_client, events::* };

// Does this need to be in CosmWasm?
// uint8  public constant decimals = 18;

pub const TOKEN_STATE: Item<TokenState> = Item::new("token_state");
pub const USER_STATE: Map<&Addr, UserState> = Map::new("user_state");

#[cw_serde]
pub struct TokenState {
    pub total_supply: Uint128,
    pub total_locked: Uint128,
    pub distribution_period: Uint64,

    // utility values
    pub locked_token: Addr, // address of the token contract
    pub last_accrue_block: Uint64,
    pub last_income_block: Uint64,
    pub reward_per_token: Uint128,
    pub reward_rate_stored: Uint128, //TODO: Don't return in Query
}

pub struct UpdateRewardRateInput {
    pub add_amount: Uint128,
    pub new_distribution_period: Uint64,
    pub current_block: Uint64,
}

impl TokenState {
    pub fn default() -> Self {
        Self {
            total_supply: Uint128::zero(),
            total_locked: Uint128::zero(),
            distribution_period: Uint64::zero(),
            locked_token: Addr::unchecked(""),
            last_accrue_block: Uint64::zero(),
            last_income_block: Uint64::zero(),
            reward_per_token: Uint128::zero(),
            reward_rate_stored: Uint128::zero(),
        }
    }

    pub fn locked_token_client<'a>(&self, deps: &'a Deps<'a>) -> CW20Client<'a> {
        cw20_client::CW20Client::new(deps, self.locked_token.clone())
    }

    pub fn set_distribution_period(
        &mut self,
        storage: &mut dyn Storage,
        current_block: Uint64,
        new_distribution_period: Uint64
    ) -> Result<Response, ContractError> {
        if new_distribution_period.is_zero() {
            return Result::Err(ContractError::ZeroDistributionPeriod {});
        }

        self.accrue(storage, current_block)?;

        self.update_reward_rate(storage, UpdateRewardRateInput {
            add_amount: Uint128::zero(),
            new_distribution_period: new_distribution_period,
            current_block: current_block,
        })?;

        let event = ContractEvent::NewDistributionPeriod { value: new_distribution_period };
        let resp = Response::new().add_attributes(event.to_attributes());

        Ok(resp)
    }

    pub fn accrue(
        &mut self,
        storage: &mut dyn Storage,
        current_block: Uint64
    ) -> Result<(), ContractError> {
        self.reward_per_token += self.pending_reward_per_token(current_block);
        self.last_accrue_block = current_block;

        TOKEN_STATE.save(storage, self)?;
        Ok(())
    }

    pub fn pending_reward_per_token(&self, current_block: Uint64) -> Uint128 {
        if self.total_supply.is_zero() {
            return Uint128::zero();
        }

        let blocks_elapsed = Uint128::from(current_block - self.last_accrue_block);
        return (blocks_elapsed * self.reward_rate(current_block)) / self.total_supply;
    }

    pub fn update_reward_rate(
        &mut self,
        storage: &mut dyn Storage,
        input: UpdateRewardRateInput
    ) -> Result<Uint128, ContractError> {
        /*
        Avoid inflation of blocksElapsed inside of _pendingRewardPerToken()
        Ensures _pendingRewardPerToken() is 0 and all rewards are accounted for
        */
        if !input.current_block.eq(&self.last_accrue_block) {
            return Result::Err(ContractError::AccrueFirst {});
        }
        let blocks_elapsed: Uint64 = self.distribution_period.min(
            input.current_block - self.last_income_block
        );

        let unvested_income =
            self.reward_rate_stored * Uint128::from(self.distribution_period - blocks_elapsed);

        self.reward_rate_stored =
            (unvested_income + input.add_amount) / Uint128::from(input.new_distribution_period);
        self.distribution_period = input.new_distribution_period;
        self.last_income_block = input.current_block;

        TOKEN_STATE.save(storage, &self)?;
        Ok(unvested_income)
    }
    // Reward only while distribution period since last income
    pub fn reward_rate(&self, current_block: Uint64) -> Uint128 {
        let blocks_elapsed: Uint64 = current_block - self.last_income_block;
        let resp = if blocks_elapsed < self.distribution_period {
            self.reward_rate_stored
        } else {
            Uint128::zero()
        };
        return resp;
    }
}

#[cw_serde]
pub struct UserState {
    pub balance: Uint128, // veBalance
    pub locked_balance: Uint128, // locked
    pub locked_until: Uint64,
    pub reward_snapshot: Uint128,
    pub withdraw_at: Uint64,
}

impl UserState {
    pub fn pending_reward(
        &self,
        reward_per_token: Uint128,
        pending_reward_per_token: Uint128
    ) -> Uint128 {
        let pending_reward_per_token = reward_per_token + pending_reward_per_token;
        let reward_per_token_delta = pending_reward_per_token - self.reward_snapshot;

        return (reward_per_token_delta * self.balance) / Uint128::from(TOKEN_DECIMALS);
    }
}

#[cfg(test)]
mod state_tests {
    use cosmwasm_std::testing::{ mock_dependencies };

    use super::{ * };

    #[test]
    fn test_accrue(){
        let mut state = TokenState::default();
        state.last_accrue_block = Uint64::from(100u64);
        state.total_supply = Uint128::from(100u128);

        let current_block = Uint64::from(101u64);

        // no blocks since last accrue
        assert_eq!(Uint128::zero(), state.pending_reward_per_token(current_block));

        // not zero response
        let reward_rate = Uint128::from(100u128);
        let distribution_period = Uint64::from(10u64);

        state.reward_rate_stored = reward_rate.clone();
        state.distribution_period = distribution_period.clone();

        let expected = (reward_rate * distribution_period) / Uint128::from(100u128);
        assert_eq!(expected, state.pending_reward_per_token(current_block));
        todo!()
    }

    #[test]
    fn test_pending_reward_per_token() {
        let mut state = TokenState::default();
        state.last_income_block = Uint64::from(100u64);
        state.distribution_period = Uint64::from(10u64);

        let current_block = Uint64::from(101u64);

        // zero supply
        assert_eq!(Uint128::zero(), state.pending_reward_per_token(current_block));

        // no blocks since last accrue
        let total_supply = Uint128::from(100u128);
        let last_accrue_block = Uint64::from(100u64);

        state.last_accrue_block = last_accrue_block.clone();
        state.total_supply = total_supply.clone();

        assert_eq!(Uint128::zero(), state.pending_reward_per_token(last_accrue_block.clone()));

        // not zero response
        let reward_rate = Uint128::from(100u128);
        state.reward_rate_stored = reward_rate.clone();

        assert_eq!(Uint128::from(5u128), state.pending_reward_per_token(Uint64::from(105u64))); // (5 blocks elapsed * 100)/100
    }

    #[test]
    fn test_reward_rate() {
        let mut state = TokenState::default();

        let reward_rate_stored = Uint128::from(10u128);
        let distribution_period = Uint64::from(10u64);

        state.reward_rate_stored = reward_rate_stored.clone();
        state.distribution_period = distribution_period.clone();
        state.last_income_block = Uint64::from(100u64);

        // Distribution period is over
        assert_eq!(Uint128::zero(), state.reward_rate(Uint64::from(110u64)));

        // Distribution period is not over
        assert_eq!(reward_rate_stored, state.reward_rate(Uint64::from(101u64)));
    }

    #[test]
    fn test_update_reward_rate() {
        let mut binding = mock_dependencies();
        let deps = binding.as_mut();

        fn default_state() -> TokenState {
            TokenState {
                total_supply: Uint128::from(1000u128),
                total_locked: Uint128::from(1000u128),
                distribution_period: Uint64::from(10u64),
                locked_token: Addr::unchecked(""),
                last_accrue_block: Uint64::from(100u64),
                last_income_block: Uint64::from(100u64),
                reward_per_token: Uint128::from(1u128),
                reward_rate_stored: Uint128::from(1u128),
            }
        }

        let mut state = default_state();
        TOKEN_STATE.save(deps.storage, &state).unwrap();

        let input = UpdateRewardRateInput {
            add_amount: Uint128::from(100u128),
            new_distribution_period: Uint64::from(100u64),
            current_block: Uint64::from(90u64),
        };

        // 1. Test when accrue was not called
        let err = state.update_reward_rate(deps.storage, input).unwrap_err();
        assert_eq!(err, ContractError::AccrueFirst {}); // throw error
        assert_eq!(state, TOKEN_STATE.load(deps.storage).unwrap()); // state is unchanged

        // 2. Test when no left unvested income (blocks elapsed => distribution period)
        let new_distribution_period = Uint64::from(100u64);
        let current_block = Uint64::from(state.last_income_block + state.distribution_period);
        let input = UpdateRewardRateInput {
            add_amount: Uint128::from(100u128),
            new_distribution_period: new_distribution_period.clone(),
            current_block: current_block.clone(),
        };
        // accrue is happened
        state.last_accrue_block = input.current_block;

        let unvested_income = state.update_reward_rate(deps.storage, input).unwrap();

        let mut expected_state = state.clone();
        expected_state.distribution_period = new_distribution_period.clone();
        expected_state.last_income_block = current_block;
        expected_state.reward_rate_stored = Uint128::from(1u128); // (0 + 10)/10

        assert_eq!(expected_state, state);
        assert_eq!(expected_state, TOKEN_STATE.load(deps.storage).unwrap());
        assert_eq!(unvested_income, Uint128::zero());

        // 3. Test when there is left unvested  (blocks elapsed < distribution period)
        let mut state = default_state();

        let current_block = Uint64::from(101u64); // for maximal unvested income for current distribution period
        state.last_accrue_block = current_block;

        let new_distribution_period = Uint64::from(5u64);
        let input = UpdateRewardRateInput {
            add_amount: Uint128::from(100u128),
            new_distribution_period: new_distribution_period.clone(),
            current_block: current_block.clone(),
        };
        // accrue is happened
        state.last_accrue_block = input.current_block;

        let unvested_income = state.update_reward_rate(deps.storage, input).unwrap();

        let mut expected_state = state.clone();
        expected_state.distribution_period = new_distribution_period;
        expected_state.last_income_block = current_block;
        expected_state.reward_rate_stored = Uint128::from(21u128); // (9 + 100)/5

        assert_eq!(expected_state, state);
        assert_eq!(expected_state, TOKEN_STATE.load(deps.storage).unwrap());
        assert_eq!(unvested_income, Uint128::from(9u128));
    }
}
