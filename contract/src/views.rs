use crate::contract::*;
use near_sdk::json_types::U128;
use near_sdk::{near, AccountId, Balance};

#[near]
impl StakingContract {
    pub fn get_user_stakes(&self, account_id: AccountId) -> Vec<Stake> {
        let user_list = match self.user_stakes.get(&account_id) {
            Some(v) => v,
            None => return vec![],
        };

        user_list.iter()
            .filter_map(|tid| self.stakes.get(&tid))
            .collect()
    }

    pub fn get_user_stake_count(&self, account_id: AccountId) -> u32 {
        self.user_stakes.get(&account_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    pub fn get_stake(&self, token_id: TokenId) -> Option<Stake> {
        self.stakes.get(&token_id)
    }

    pub fn get_user_weight(&self, account_id: AccountId) -> U128 {
        U128::from(self.get_user_weight_internal(&account_id))
    }

    pub fn get_total_weight(&self) -> U128 {
        U128::from(self.calculate_total_weight())
    }

    pub fn get_user_rewards(&self, account_id: AccountId) -> U128 {
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let current_epoch = self.to_epoch(now);

        let user_list = match self.user_stakes.get(&account_id) {
            Some(v) => v,
            None => return U128::from(0),
        };

        let mut simulated_reward: Balance = 0;
        let total_weight = self.calculate_total_weight();
        if total_weight == 0 {
            return U128::from(0);
        }

        for token_id in user_list.iter() {
            let stake = match self.stakes.get(&token_id) {
                Some(s) => s,
                None => continue,
            };

            if !stake.active {
                continue;
            }

            let stake_end_epoch = self.to_epoch(stake.unlocked_at);
            let claim_until = std::cmp::min(stake_end_epoch, current_epoch);

            if claim_until <= stake.last_claim_epoch {
                continue;
            }

            let epochs_to_claim = claim_until - stake.last_claim_epoch;
            if epochs_to_claim == 0 {
                continue;
            }

            let count = user_list.len();
            let duration_mult = self.get_duration_multiplier(stake.lock_duration) as u128;
            let tier_mult = self.get_tier_bonus(count) as u128;
            let weight_factor = duration_mult * tier_mult / 100_000_000;
            let weight = (weight_factor.max(1)) as u128;

            let reward_per_epoch = if total_weight > 0 {
                self.reward_pool / total_weight.max(1)
            } else {
                0
            };

            simulated_reward += weight * reward_per_epoch * epochs_to_claim as u128;
        }

        let existing = self.epoch_rewards.get(&account_id).unwrap_or(0);
        U128::from(existing + simulated_reward)
    }

    pub fn get_contract_metadata(&self) -> ContractMetadata {
        ContractMetadata {
            owner_id: self.owner_id.clone(),
            reward_token: self.reward_token.clone(),
            total_staked: self.total_staked,
            reward_pool: U128::from(self.reward_pool),
            epoch_duration: self.epoch_duration,
            duration_multipliers: self.duration_multipliers.clone(),
            tier_bonuses: self.tier_bonuses.clone(),
        }
    }

    pub fn get_all_stakes(&self, from_index: u64, limit: u64) -> Vec<Stake> {
        let keys = self.stakes.keys();
        let l = std::cmp::min(limit, keys.len() as u64);
        let start = std::cmp::min(from_index, keys.len() as u64 - 1);

        let mut result = vec![];
        for i in start..(start + l) {
            if let Some(key) = keys.get(i as u64) {
                if let Some(stake) = self.stakes.get(&key) {
                    result.push(stake);
                }
            }
        }
        result
    }

    pub fn get_total_staked(&self) -> u64 {
        self.total_staked
    }

    pub fn get_reward_pool(&self) -> U128 {
        U128::from(self.reward_pool)
    }
}