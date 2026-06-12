use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap},
    json_types::U128,
    near, store::Vector,
    AccountId, Balance, Gas, Promise, PromiseOrValue, Timestamp,
};
use serde::{Deserialize, Serialize};

pub const STORAGE_COST: Balance = 1_000_000_000_000_000_000_000;
pub const GAS_FOR_NFT_TRANSFER: Gas = Gas(10_000_000_000_000);
pub const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas(10_000_000_000_000);

pub type TokenId = String;
pub type EpochTimestamp = u64;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeView {
    pub owner_id: AccountId,
    pub token_id: TokenId,
    pub nft_contract_id: AccountId,
    pub staked_at: EpochTimestamp,
    pub lock_duration: u64,
    pub unlocked_at: EpochTimestamp,
    pub last_claim_epoch: EpochTimestamp,
    pub active: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractMetadata {
    pub owner_id: AccountId,
    pub reward_token: AccountId,
    pub total_staked: u64,
    pub reward_pool: U128,
    pub total_weight: U128,
    pub epoch_duration: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Stake {
    pub token_id: TokenId,
    pub nft_contract_id: AccountId,
    pub owner_id: AccountId,
    pub staked_at: EpochTimestamp,
    pub lock_duration: u64,
    pub unlocked_at: EpochTimestamp,
    pub last_claim_epoch: EpochTimestamp,
    pub active: bool,
}

#[near(contract_state)]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakingContract {
    pub owner_id: AccountId,
    pub reward_token: AccountId,
    pub stakes: UnorderedMap<TokenId, Stake>,
    pub user_stakes: LookupMap<AccountId, Vector<TokenId>>,
    pub total_staked: u64,
    pub reward_pool: Balance,
    pub total_weight: u128,
    pub epoch_duration: u64,
    pub last_epoch_update: EpochTimestamp,
    pub epoch_rewards: LookupMap<AccountId, Balance>,
}

#[near]
impl StakingContract {
    #[init]
    pub fn new(owner_id: AccountId, reward_token: AccountId) -> Self {
        Self {
            owner_id,
            reward_token,
            stakes: UnorderedMap::new(b"s"),
            user_stakes: LookupMap::new(b"u"),
            total_staked: 0,
            reward_pool: 0,
            total_weight: 0,
            epoch_duration: 86_400,
            last_epoch_update: near_sdk::env::block_timestamp() / 1_000_000_000,
            epoch_rewards: LookupMap::new(b"r"),
        }
    }

    #[payable]
    pub fn deposit_rewards(&mut self) {
        self.assert_owner();
        let deposit = near_sdk::env::attached_deposit();
        near_sdk::assert!(deposit > 0, "Must attach NEAR");
        self.reward_pool += deposit;
        near_sdk::env::log_str(
            &format!("EVENT_JSON:{{\"event\":\"reward_deposit\",\"data\":{{\"amount\":\"{}\"}}}}", deposit)
        );
    }

    pub fn stake(&mut self, nft_contract_id: AccountId, token_id: TokenId, lock_duration_sec: u64) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        let msg = format!("{}:{}", lock_duration_sec, owner_id);
        Self::ext(nft_contract_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_NFT_TRANSFER)
            .nft_transfer_call(
                near_sdk::env::current_account_id(),
                token_id,
                msg,
                None::<String>,
            )
    }

    #[private]
    pub fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let parts: Vec<&str> = msg.split(':').collect();
        if parts.len() < 2 {
            return PromiseOrValue::Value(false);
        }
        let lock_duration: u64 = parts[0].parse().unwrap_or(0);
        let owner_id: AccountId = parts[1].parse().unwrap_or_else(|_| { return PromiseOrValue::Value(false); });
        let nft_contract_id = near_sdk::env::predecessor_account_id();
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;

        if lock_duration != 864000 && lock_duration != 1728000 && lock_duration != 2592000 {
            return PromiseOrValue::Value(false);
        }

        let stake = Stake {
            owner_id: owner_id.clone(),
            token_id: token_id.clone(),
            nft_contract_id,
            staked_at: now,
            lock_duration,
            unlocked_at: now + lock_duration,
            last_claim_epoch: self.to_epoch(now),
            active: true,
        };

        if self.stakes.insert(&token_id, &stake).is_some() {
            return PromiseOrValue::Value(false);
        }

        let mut user_list = self.user_stakes.get(&owner_id).unwrap_or_else(|| {
            Vector::new(format!("uv:{}", owner_id).as_bytes())
        });
        user_list.push(&token_id);
        self.user_stakes.insert(&owner_id, &user_list);
        self.total_staked += 1;
        self.total_weight += 1;

        near_sdk::env::log_str(
            &format!("EVENT_JSON:{{\"event\":\"stake\",\"data\":{{\"owner\":\"{}\",\"token_id\":\"{}\",\"duration\":{}}}}}", owner_id, token_id, lock_duration)
        );
        PromiseOrValue::Value(true)
    }

    pub fn unstake(&mut self, token_id: TokenId) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        let stake = self.stakes.get(&token_id)
            .unwrap_or_else(|| near_sdk::env::panic_str("Stake not found"));

        near_sdk::assert_eq!(stake.owner_id, owner_id, "Not your stake");
        self.process_rewards();

        self.stakes.remove(&token_id);

        let mut user_list = self.user_stakes.get(&owner_id).unwrap();
        let new_list: Vec<TokenId> = user_list.iter()
            .filter(|tid| tid != &token_id)
            .collect();
        let mut updated = Vector::new(format!("uv:{}", owner_id).as_bytes());
        for tid in new_list { updated.push(&tid); }
        self.user_stakes.insert(&owner_id, &updated);
        self.total_staked -= 1;
        if self.total_weight > 0 { self.total_weight -= 1; }

        near_sdk::ext::nft_contract::ext(stake.nft_contract_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
            .nft_transfer(owner_id.clone(), token_id.clone(), None::<String>)
    }

    pub fn claim(&mut self) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        self.process_rewards();
        let amount = self.epoch_rewards.get(&owner_id).unwrap_or(0);
        if amount == 0 {
            near_sdk::env::panic_str("No rewards");
        }
        self.epoch_rewards.insert(&owner_id, &0);
        self.reward_pool -= amount;
        near_sdk::env::log_str(
            &format!("EVENT_JSON:{{\"event\":\"claim\",\"data\":{{\"owner\":\"{}\",\"amount\":\"{}\"}}}}", owner_id, amount)
        );
        near_sdk::Promise::new(owner_id).transfer(amount)
    }

    fn process_rewards(&mut self) {
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let current_epoch = self.to_epoch(now);
        let last_epoch = self.last_epoch_update;
        if current_epoch <= last_epoch { return; }
        let epochs_passed = current_epoch - last_epoch;
        let tw = self.total_weight;
        if tw == 0 || self.reward_pool == 0 { 
            self.last_epoch_update = current_epoch;
            return; 
        }

        // Record what each user gets
        let reward_per_epoch_per_weight = self.reward_pool / tw;
        let total_epoch_reward = reward_per_epoch_per_weight * tw;

        // Distribute proportionally to active stakers
        for stake in self.stakes.values() {
            if !stake.active { continue; }
            let user_epochs = std::cmp::min(
                std::cmp::min(stake.unlocked_at / self.epoch_duration, current_epoch) - stake.last_claim_epoch,
                epochs_passed
            );
            if user_epochs == 0 { continue; }
            let reward = reward_per_epoch_per_weight * user_epochs as u128;
            if reward > 0 {
                let existing = self.epoch_rewards.get(&stake.owner_id).unwrap_or(0);
                self.epoch_rewards.insert(&stake.owner_id, &(existing + reward));
            }
        }

        self.last_epoch_update = current_epoch;
    }

    pub fn get_user_stakes(&self, account_id: AccountId) -> Vec<StakeView> {
        let user_list = match self.user_stakes.get(&account_id) {
            Some(v) => v,
            None => return vec![],
        };
        user_list.iter()
            .filter_map(|tid| self.stakes.get(&tid))
            .map(|s| StakeView {
                owner_id: s.owner_id,
                token_id: s.token_id,
                nft_contract_id: s.nft_contract_id,
                staked_at: s.staked_at,
                lock_duration: s.lock_duration,
                unlocked_at: s.unlocked_at,
                last_claim_epoch: s.last_claim_epoch,
                active: s.active,
            })
            .collect()
    }

    pub fn get_user_rewards(&self, account_id: AccountId) -> U128 {
        U128::from(self.epoch_rewards.get(&account_id).unwrap_or(0))
    }

    pub fn get_contract_metadata(&self) -> ContractMetadata {
        ContractMetadata {
            owner_id: self.owner_id.clone(),
            reward_token: self.reward_token.clone(),
            total_staked: self.total_staked,
            reward_pool: U128::from(self.reward_pool),
            total_weight: U128::from(self.total_weight),
            epoch_duration: self.epoch_duration,
        }
    }

    pub fn get_stake(&self, token_id: TokenId) -> Option<StakeView> {
        self.stakes.get(&token_id).map(|s| StakeView {
            owner_id: s.owner_id,
            token_id: s.token_id,
            nft_contract_id: s.nft_contract_id,
            staked_at: s.staked_at,
            lock_duration: s.lock_duration,
            unlocked_at: s.unlocked_at,
            last_claim_epoch: s.last_claim_epoch,
            active: s.active,
        })
    }

    fn to_epoch(&self, timestamp: EpochTimestamp) -> u64 {
        timestamp / self.epoch_duration
    }

    fn assert_owner(&self) {
        near_sdk::assert_eq!(
            near_sdk::env::predecessor_account_id(),
            self.owner_id,
            "Only owner"
        );
    }
}

#[near(protocol = "ext")]
pub mod nft_contract {
    use near_sdk::PromiseOrValue;

    pub fn nft_transfer(
        receiver_id: AccountId,
        token_id: String,
        memo: Option<String>,
    ) -> Promise;

    pub fn nft_transfer_call(
        receiver_id: AccountId,
        token_id: String,
        msg: String,
        memo: Option<String>,
    ) -> PromiseOrValue<bool>;
}