use crate::contract::*;
use near_sdk::near;

#[near]
impl StakingContract {
    #[payable]
    pub fn deposit_rewards(&mut self) {
        self.only_owner();
        let deposit = near_sdk::env::attached_deposit();
        near_sdk::assert!(deposit > 0, "Must attach NEAR");
        self.reward_pool += deposit;
        emit_reward_deposit(&near_sdk::env::predecessor_account_id(), deposit);
    }

    pub fn set_duration_multipliers(&mut self, multipliers: Vec<DurationMultiplier>) {
        self.only_owner();
        near_sdk::assert!(!multipliers.is_empty(), "At least one duration required");
        self.duration_multipliers = multipliers;
    }

    pub fn set_tier_bonuses(&mut self, bonuses: Vec<TierBonus>) {
        self.only_owner();
        near_sdk::assert!(!bonuses.is_empty(), "At least one tier required");
        self.tier_bonuses = bonuses;
    }

    pub fn set_epoch_duration(&mut self, epoch_duration: u64) {
        self.only_owner();
        near_sdk::assert!(epoch_duration >= 3600, "Epoch min 1 hour");
        self.epoch_duration = epoch_duration;
    }

    #[payable]
    pub fn withdraw_rewards(&mut self, amount: U128) {
        self.only_owner();
        let a: Balance = amount.into();
        near_sdk::assert!(a <= self.reward_pool, "Not enough in pool");
        self.reward_pool -= a;
        near_sdk::Promise::new(self.owner_id.clone()).transfer(a);
    }

    pub fn force_update_stake_owner(&mut self, token_id: TokenId, new_owner: AccountId) {
        self.only_owner();
        let mut stake = self.stakes.get(&token_id)
            .unwrap_or_else(|| near_sdk::env::panic_str("Stake not found"));

        // Remove from old owner
        let old_list = self.user_stakes.get(&stake.owner_id).unwrap();
        let updated: Vec<TokenId> = old_list.iter()
            .filter(|tid| tid != &token_id)
            .collect();
        let mut new_old = Vector::new(format!("uv:{}", stake.owner_id).as_bytes());
        for tid in updated { new_old.push(&tid); }
        self.user_stakes.insert(&stake.owner_id, &new_old);

        // Add to new owner
        stake.owner_id = new_owner.clone();
        self.stakes.insert(&token_id, &stake);

        let mut new_list = self.user_stakes.get(&new_owner).unwrap_or_else(|| {
            Vector::new(format!("uv:{}", new_owner).as_bytes())
        });
        new_list.push(&token_id);
        self.user_stakes.insert(&new_owner, &new_list);
    }

    fn only_owner(&self) {
        near_sdk::assert_eq!(
            near_sdk::env::predecessor_account_id(),
            self.owner_id,
            "Only owner"
        );
    }
}