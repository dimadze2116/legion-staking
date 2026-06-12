use crate::contract::*;
use near_sdk::{near, Balance};

#[near]
impl StakingContract {
    /// Process rewards for a user: calculate pending epoch rewards and add to balance
    pub fn process_rewards(&mut self, owner_id: &near_sdk::AccountId) -> Balance {
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let current_epoch = self.to_epoch(now);

        // Get user stakes
        let user_stakes = match self.user_stakes.get(owner_id) {
            Some(v) => v,
            None => return 0,
        };

        let mut total_reward: Balance = 0;
        let mut min_unclaimed_epoch = current_epoch;

        for token_id in user_stakes.iter() {
            let stake = match self.stakes.get(&token_id) {
                Some(s) => s,
                None => continue,
            };

            if !stake.active {
                // Stake is completed and already processed
                continue;
            }

            let stake_end_epoch = self.to_epoch(stake.unlocked_at);
            let last_claim = stake.last_claim_epoch;
            let claim_until = std::cmp::min(stake_end_epoch, current_epoch);

            if claim_until <= last_claim {
                continue;
            }

            // Calculate epochs since last claim (capped at lock end)
            let epochs_to_claim = claim_until - last_claim;
            if epochs_to_claim == 0 {
                continue;
            }

            // Calculate user weight for this stake
            let user_count = user_stakes.len();
            let duration_mult = self.get_duration_multiplier(stake.lock_duration);
            let tier_mult = self.get_tier_bonus(user_count);
            let weight_base = user_count as u128;
            let weight = weight_base
                * duration_mult as u128
                * tier_mult as u128
                / 10_000 // adjust for bps
                / 100;  // tier is also bps

            // Total weight of all stakers
            let total_weight = self.calculate_total_weight();
            if total_weight == 0 {
                continue;
            }

            // Reward per epoch per unit of weight
            let reward_per_epoch_per_weight = if total_weight > 0 {
                let pool = self.reward_pool;
                // Distribute equally: pool divided by epochs remaining (rough)
                // Simplified: per epoch = pool / (total_weight * epochs_remaining)
                pool / total_weight.max(1)
            } else {
                0
            };

            let reward_share = weight * reward_per_epoch_per_weight;
            total_reward += reward_share;

            // Update stake's last claim epoch
            let mut updated_stake = stake.clone();
            updated_stake.last_claim_epoch = claim_until;
            updated_stake.active = claim_until < stake_end_epoch;
            self.stakes.insert(&token_id, &updated_stake);
        }

        if total_reward > 0 {
            let current = self.epoch_rewards.get(owner_id).unwrap_or(0);
            self.epoch_rewards.insert(owner_id, &(current + total_reward));
        }

        self.last_epoch_update = current_epoch;
        total_reward
    }

    /// Calculate the weight of a single user
    pub fn get_user_weight(&self, owner_id: &near_sdk::AccountId) -> u128 {
        let user_stakes = match self.user_stakes.get(owner_id) {
            Some(s) => s,
            None => return 0,
        };

        let count = user_stakes.len();
        if count == 0 {
            return 0;
        }

        let mut weighted_count = 0u128;
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let current_epoch = self.to_epoch(now);

        for token_id in user_stakes.iter() {
            let stake = match self.stakes.get(&token_id) {
                Some(s) => s,
                None => continue,
            };

            let duration_mult = self.get_duration_multiplier(stake.lock_duration) as u128;
            let tier_mult = self.get_tier_bonus(count) as u128;

            // Weight = duration_mult (bps/10000) * tier_mult (bps/10000)
            // Normalized: weight_factor = duration_mult * tier_mult / 100000000
            let weight_factor = duration_mult * tier_mult / 100_000_000;
            weighted_count += weight_factor.max(1); // min 1x
        }

        weighted_count
    }

    /// Calculate total weight across all stakers
    pub fn calculate_total_weight(&self) -> u128 {
        let mut total = 0u128;

        // Iterate all stakes (expensive in large sets, but accurate)
        // For production: maintain a running total
        for stake in self.stakes.values() {
            if !stake.active { continue; }

            let user_list = self.user_stakes.get(&stake.owner_id);
            let count = user_list.map(|v| v.len()).unwrap_or(0);

            let duration_mult = self.get_duration_multiplier(stake.lock_duration) as u128;
            let tier_mult = self.get_tier_bonus(count) as u128;
            let weight_factor = duration_mult * tier_mult / 100_000_000;

            total += weight_factor.max(1);
        }

        total
    }

    fn get_duration_multiplier(&self, duration: u64) -> u32 {
        for d in &self.duration_multipliers {
            if d.duration_sec == duration {
                return d.multiplier_bps;
            }
        }
        10_000 // default 1x
    }

    fn get_tier_bonus(&self, nft_count: u32) -> u32 {
        let mut bonus = 10_000; // default 1x (10000 bps)
        for tier in &self.tier_bonuses {
            if nft_count >= tier.min_nfts {
                bonus = 10_000 + tier.bonus_bps;
            }
        }
        bonus
    }
}