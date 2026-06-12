use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap},
    json_types::U128,
    near, store::Vector,
    AccountId, Balance, Gas, Promise, PromiseOrValue,
};
use serde::{Deserialize, Serialize};

pub type TokenId = String;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Stake {
    pub token_id: TokenId,
    pub nft_contract_id: AccountId,
    pub owner_id: AccountId,
    pub staked_at: u64,
    pub lock_duration: u64,
    pub unlocked_at: u64,
    pub last_claim_epoch: u64,
    pub active: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeView {
    pub owner_id: AccountId,
    pub token_id: TokenId,
    pub nft_contract_id: AccountId,
    pub staked_at: u64,
    pub lock_duration: u64,
    pub unlocked_at: u64,
    pub active: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractMetadata {
    pub owner_id: AccountId,
    pub reward_token: AccountId,
    pub total_staked: u64,
    pub reward_pool: U128,
    pub epoch_duration: u64,
}

#[near(contract_state, serializers = [borsh])]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakingContract {
    pub owner_id: AccountId,
    pub reward_token: AccountId,
    pub stakes: UnorderedMap<TokenId, Stake>,
    pub user_stakes: LookupMap<AccountId, Vector<TokenId>>,
    pub total_staked: u64,
    pub reward_pool: Balance,
    pub epoch_duration: u64,
    pub last_epoch_update: u64,
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
            epoch_duration: 86400,
            last_epoch_update: near_sdk::env::block_timestamp() / 1_000_000_000,
            epoch_rewards: LookupMap::new(b"r"),
        }
    }

    // --- Admin ---
    #[payable]
    pub fn deposit_rewards(&mut self) {
        self.assert_owner();
        let deposit = near_sdk::env::attached_deposit();
        assert!(deposit > 0, "Must attach NEAR");
        self.reward_pool += deposit;
    }

    pub fn unstake(&mut self, token_id: TokenId) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        let stake = self.stakes.get(&token_id).unwrap_or_else(|| near_sdk::env::panic_str("Not found"));
        assert_eq!(stake.owner_id, owner_id, "Not yours");

        self.process_rewards();
        self.stakes.remove(&token_id);

        if let Some(mut list) = self.user_stakes.get(&owner_id) {
            let new_ids: Vec<TokenId> = list.iter().filter(|t| t != &token_id).collect();
            let mut updated = Vector::new(format!("u:{}", owner_id).as_bytes());
            for tid in new_ids { updated.push(&tid); }
            self.user_stakes.insert(&owner_id, &updated);
        }
        self.total_staked -= 1;

        Self::ext(stake.nft_contract_id.clone())
            .with_attached_deposit(1)
            .with_static_gas(Gas(10_000_000_000_000))
            .nft_transfer(owner_id.clone(), token_id, None::<String>)
    }

    pub fn claim(&mut self) -> Promise {
        let owner_id = near_sdk::env::predecessor_account_id();
        self.process_rewards();
        let amount = self.epoch_rewards.get(&owner_id).unwrap_or(0);
        assert!(amount > 0, "No rewards");
        self.epoch_rewards.insert(&owner_id, &0);
        self.reward_pool -= amount;
        near_sdk::Promise::new(owner_id).transfer(amount)
    }

    // --- NFT callback ---
    #[private]
    pub fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let parts: Vec<&str> = msg.split(':').collect();
        if parts.len() < 2 { return PromiseOrValue::Value(false); }
        let lock_duration: u64 = match parts[0].parse() { Ok(v) => v, _ => return PromiseOrValue::Value(false) };
        let owner_id: AccountId = match parts[1].parse() { Ok(v) => v, _ => return PromiseOrValue::Value(false) };
        if lock_duration != 864000 && lock_duration != 1728000 && lock_duration != 2592000 {
            return PromiseOrValue::Value(false);
        }
        let nft = near_sdk::env::predecessor_account_id();
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let stake = Stake {
            owner_id: owner_id.clone(),
            token_id: token_id.clone(),
            nft_contract_id: nft,
            staked_at: now,
            lock_duration,
            unlocked_at: now + lock_duration,
            last_claim_epoch: self.to_epoch(now),
            active: true,
        };
        if self.stakes.insert(&token_id, &stake).is_some() {
            return PromiseOrValue::Value(false);
        }
        let mut list = self.user_stakes.get(&owner_id).unwrap_or_else(|| {
            Vector::new(format!("u:{}", owner_id).as_bytes())
        });
        list.push(&token_id);
        self.user_stakes.insert(&owner_id, &list);
        self.total_staked += 1;
        PromiseOrValue::Value(true)
    }

    // --- Rewards ---
    fn process_rewards(&mut self) {
        let now = near_sdk::env::block_timestamp() / 1_000_000_000;
        let cur = self.to_epoch(now);
        if cur <= self.last_epoch_update { return; }
        if self.total_staked == 0 || self.reward_pool == 0 {
            self.last_epoch_update = cur;
            return;
        }
        for stake in self.stakes.values() {
            if !stake.active { continue; }
            let end = self.to_epoch(stake.unlocked_at);
            let claim_until = end.min(cur);
            if claim_until <= stake.last_claim_epoch { continue; }
            let user_count = self.user_stakes.get(&stake.owner_id).map(|v| v.len()).unwrap_or(0) as u128;
            if user_count == 0 { continue; }
            let reward = self.reward_pool * user_count / (self.total_staked as u128);
            if reward > 0 {
                let prev = self.epoch_rewards.get(&stake.owner_id).unwrap_or(0);
                self.epoch_rewards.insert(&stake.owner_id, &(prev + reward));
            }
        }
        self.last_epoch_update = cur;
    }

    // --- Views ---
    pub fn get_user_stakes(&self, account_id: AccountId) -> Vec<StakeView> {
        self.user_stakes.get(&account_id).map(|list| {
            list.iter().filter_map(|tid| {
                self.stakes.get(&tid).map(|s| StakeView {
                    owner_id: s.owner_id,
                    token_id: s.token_id,
                    nft_contract_id: s.nft_contract_id,
                    staked_at: s.staked_at,
                    lock_duration: s.lock_duration,
                    unlocked_at: s.unlocked_at,
                    active: s.active,
                })
            }).collect()
        }).unwrap_or_default()
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
            epoch_duration: self.epoch_duration,
        }
    }

    fn to_epoch(&self, ts: u64) -> u64 { ts / self.epoch_duration }
    fn assert_owner(&self) {
        assert_eq!(near_sdk::env::predecessor_account_id(), self.owner_id, "Only owner");
    }
}

#[near(protocol = "ext")]
pub mod nft_contract {
    use near_sdk::PromiseOrValue;
    pub fn nft_transfer(receiver_id: AccountId, token_id: String, memo: Option<String>) -> Promise;
    pub fn nft_transfer_call(receiver_id: AccountId, token_id: String, msg: String, memo: Option<String>) -> PromiseOrValue<bool>;
}