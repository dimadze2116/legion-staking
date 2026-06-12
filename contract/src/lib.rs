use near_sdk::{
    near, collections::{LookupMap, UnorderedMap, Vector}, env, NearToken, Promise, PromiseOrValue,
    json_types::U128, AccountId, Gas,
};
use serde::{Deserialize, Serialize};

type TokenId = String;

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
pub struct Metadata {
    pub owner_id: AccountId,
    pub total_staked: u64,
    pub reward_pool: U128,
    pub epoch_duration: u64,
}

#[near(contract_state)]
#[derive(Default)]
pub struct Contract {
    owner_id: AccountId,
    stakes: UnorderedMap<TokenId, StakeData>,
    user_stakes: LookupMap<AccountId, Vector<TokenId>>,
    total_staked: u64,
    reward_pool: u128,
    epoch_duration: u64,
    last_update: u64,
    pending: LookupMap<AccountId, u128>,
}

#[derive(Default)]
#[near(serializers=[borsh])]
pub struct StakeData {
    pub owner_id: AccountId,
    pub token_id: TokenId,
    pub nft_contract: AccountId,
    pub staked_at: u64,
    pub lock_duration: u64,
    pub unlocked_at: u64,
    pub last_epoch: u64,
}

#[near]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            epoch_duration: 86400,
            ..Default::default()
        }
    }

    fn only_owner(&self) {
        assert_eq!(env::predecessor_account_id(), self.owner_id, "only owner");
    }
    fn now_epoch(&self) -> u64 {
        (env::block_timestamp() / 1_000_000_000) / self.epoch_duration
    }

    #[payable]
    pub fn deposit_rewards(&mut self) {
        self.only_owner();
        self.reward_pool += env::attached_deposit().as_yoctonear();
    }

    pub fn unstake(&mut self, token_id: TokenId) -> Promise {
        let owner = env::predecessor_account_id();
        let s = self.stakes.get(&token_id).expect("not found");
        assert_eq!(s.owner_id, owner, "not yours");
        self.calc();
        self.stakes.remove(&token_id);
        if let Some(mut list) = self.user_stakes.get(&owner) {
            let ids: Vec<TokenId> = list.to_vec().into_iter().filter(|t| t != &token_id).collect();
            let mut nv = Vector::new(format!("uv{}", owner).as_bytes());
            for id in ids { nv.push(&id); }
            self.user_stakes.insert(&owner, &nv);
        }
        self.total_staked -= 1;
        Self::ext(s.nft_contract)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(Gas::from_tgas(10))
            .nft_transfer(owner, token_id, None)
    }

    pub fn claim(&mut self) -> Promise {
        let owner = env::predecessor_account_id();
        self.calc();
        let amt = self.pending.get(&owner).unwrap_or(0);
        assert!(amt > 0, "no rewards");
        self.pending.insert(&owner, &0);
        self.reward_pool -= amt;
        Promise::new(owner).transfer(NearToken::from_yoctonear(amt))
    }

    #[private]
    pub fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        prev: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let parts: Vec<&str> = msg.split(':').collect();
        if parts.len() < 2 { return PromiseOrValue::Value(false); }
        let dur: u64 = match parts[0].parse() { Ok(v) => v, _ => return PromiseOrValue::Value(false) };
        let own: AccountId = match parts[1].parse() { Ok(v) => v, _ => return PromiseOrValue::Value(false) };
        if dur != 864000 && dur != 1728000 && dur != 2592000 { return PromiseOrValue::Value(false); }
        let nft = env::predecessor_account_id();
        let now = env::block_timestamp() / 1_000_000_000;
        let stake = StakeData {
            owner_id: own.clone(),
            token_id: token_id.clone(),
            nft_contract: nft,
            staked_at: now,
            lock_duration: dur,
            unlocked_at: now + dur,
            last_epoch: now / self.epoch_duration,
        };
        if self.stakes.insert(&token_id, &stake).is_some() {
            return PromiseOrValue::Value(false);
        }
        let mut list = self.user_stakes.get(&own).unwrap_or_else(|| {
            Vector::new(format!("uv{}", own).as_bytes())
        });
        list.push(&token_id);
        self.user_stakes.insert(&own, &list);
        self.total_staked += 1;
        PromiseOrValue::Value(true)
    }

    fn calc(&mut self) {
        let cur = self.now_epoch();
        if cur <= self.last_update || self.total_staked == 0 { return; }
        if self.reward_pool == 0 { self.last_update = cur; return; }
        for s in self.stakes.values() {
            let now = env::block_timestamp() / 1_000_000_000;
            if s.unlocked_at <= now && s.staked_at + s.lock_duration <= now { continue; }
            let end = s.unlocked_at / self.epoch_duration;
            let until = end.min(cur);
            if until <= s.last_epoch { continue; }
            let count = self.user_stakes.get(&s.owner_id).map(|v| v.len() as u128).unwrap_or(0);
            if count == 0 { continue; }
            let share = self.reward_pool * count / self.total_staked as u128;
            if share > 0 {
                let prev = self.pending.get(&s.owner_id).unwrap_or(0);
                self.pending.insert(&s.owner_id, &(prev + share));
            }
        }
        self.last_update = cur;
    }

    pub fn get_user_stakes(&self, account_id: AccountId) -> Vec<StakeView> {
        self.user_stakes.get(&account_id).map(|list| {
            list.to_vec().into_iter().filter_map(|tid| {
                self.stakes.get(&tid).map(|s| {
                    let now = env::block_timestamp() / 1_000_000_000;
                    StakeView {
                        owner_id: s.owner_id,
                        token_id: s.token_id,
                        nft_contract_id: s.nft_contract,
                        staked_at: s.staked_at,
                        lock_duration: s.lock_duration,
                        unlocked_at: s.unlocked_at,
                        active: s.unlocked_at > now || s.staked_at + s.lock_duration > now,
                    }
                })
            }).collect()
        }).unwrap_or_default()
    }

    pub fn get_user_rewards(&self, account_id: AccountId) -> U128 {
        U128::from(self.pending.get(&account_id).unwrap_or(0))
    }

    pub fn get_metadata(&self) -> Metadata {
        Metadata {
            owner_id: self.owner_id.clone(),
            total_staked: self.total_staked,
            reward_pool: U128::from(self.reward_pool),
            epoch_duration: self.epoch_duration,
        }
    }
}

#[ext_contract(nft_ext)]
pub trait NftContract {
    fn nft_transfer(&mut self, receiver_id: AccountId, token_id: String, memo: Option<String>);
    fn nft_transfer_call(&mut self, receiver_id: AccountId, token_id: String, msg: String, memo: Option<String>) -> PromiseOrValue<bool>;
}