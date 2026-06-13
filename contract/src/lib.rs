use near_sdk::{
    near, env, NearToken, Promise, PromiseOrValue, Gas,
    json_types::U128, AccountId, store::LookupMap, store::Vector,
    ext_contract,
};
use serde::{Deserialize, Serialize};
use borsh::{BorshSerialize, BorshDeserialize};

pub type TokenId = String;

pub const LOCK_10D: u64 = 864_000;
pub const LOCK_20D: u64 = 1_728_000;
pub const LOCK_30D: u64 = 2_592_000;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Stake {
    pub owner_id: AccountId,
    pub token_id: TokenId,
    pub nft_contract: AccountId,
    pub staked_at: u64,
    pub lock_duration: u64,
    pub unlocked_at: u64,
    pub last_epoch: u64,
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
pub struct Metadata {
    pub owner_id: AccountId,
    pub total_staked: u64,
    pub reward_pool: U128,
    pub epoch_duration: u64,
}

#[near(contract_state)]
pub struct Contract {
    owner_id: AccountId,
    stakes: LookupMap<TokenId, Stake>,
    user_stakes: LookupMap<AccountId, Vector<TokenId>>,
    all_stakes: Vector<TokenId>,
    total_staked: u64,
    reward_pool: u128,
    epoch_duration: u64,
    last_update: u64,
    pending: LookupMap<AccountId, u128>,
}

#[near]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            stakes: LookupMap::new(b"s"),
            user_stakes: LookupMap::new(b"u"),
            all_stakes: Vector::new(b"a"),
            total_staked: 0,
            reward_pool: 0,
            epoch_duration: 86_400,
            last_update: env::block_timestamp() / 86_400_000_000_000,
            pending: LookupMap::new(b"p"),
        }
    }

    fn only_owner(&self) {
        assert_eq!(env::predecessor_account_id(), self.owner_id, "owner only");
    }

    fn cur_epoch(&self) -> u64 {
        (env::block_timestamp() / 1_000_000_000) / self.epoch_duration
    }

    fn is_locked(&self, s: &Stake) -> bool {
        let now = env::block_timestamp() / 1_000_000_000;
        s.unlocked_at > now
    }

    #[payable]
    pub fn deposit_rewards(&mut self) {
        self.only_owner();
        self.reward_pool += env::attached_deposit().as_yoctonear();
    }

    pub fn withdraw_rewards(&mut self, amount: U128) {
        self.only_owner();
        let a: u128 = amount.into();
        assert!(a <= self.reward_pool, "not enough");
        self.reward_pool -= a;
        Promise::new(self.owner_id.clone()).transfer(NearToken::from_yoctonear(a));
    }

    pub fn set_epoch_duration(&mut self, secs: u64) {
        self.only_owner();
        assert!(secs >= 3600, "min 1h");
        self.epoch_duration = secs;
    }

    pub fn stake(&mut self, nft_contract_id: AccountId, token_id: TokenId, lock_duration_sec: u64) -> Promise {
        let owner = env::predecessor_account_id();
        let msg = format!("{}:{}", lock_duration_sec, owner);
        nft_contract::ext(nft_contract_id)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(Gas::from_tgas(10))
            .nft_transfer_call(
                env::current_account_id(),
                token_id,
                msg,
                None::<String>,
            )
    }

    #[private]
    pub fn nft_on_transfer(
        &mut self,
        _sender_id: AccountId,
        _prev: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let parts: Vec<&str> = msg.split(':').collect();
        if parts.len() != 2 { return PromiseOrValue::Value(false); }
        let dur: u64 = match parts[0].parse() {
            Ok(v) if v == LOCK_10D || v == LOCK_20D || v == LOCK_30D => v,
            _ => return PromiseOrValue::Value(false),
        };
        let owner: AccountId = match parts[1].parse() {
            Ok(v) => v,
            _ => return PromiseOrValue::Value(false),
        };
        let nft = env::predecessor_account_id();
        let now = env::block_timestamp() / 1_000_000_000;

        let stake = Stake {
            owner_id: owner.clone(),
            token_id: token_id.clone(),
            nft_contract: nft,
            staked_at: now,
            lock_duration: dur,
            unlocked_at: now + dur,
            last_epoch: 0,
        };

        if self.stakes.insert(token_id.clone(), stake).is_some() {
            return PromiseOrValue::Value(false);
        }

        let prefix = format!("us{}", &owner);
        let items: Vec<TokenId> = self.user_stakes.get(&owner)
            .map(|v| v.iter().map(|t| t.clone()).collect())
            .unwrap_or_default();
        let mut list = Vector::new(prefix.as_bytes());
        for t in items { list.push(t); }
        list.push(token_id.clone());
        self.user_stakes.insert(owner.clone(), list);
        self.all_stakes.push(token_id.clone());
        self.total_staked += 1;

        PromiseOrValue::Value(true)
    }

    pub fn unstake(&mut self, token_id: TokenId) -> Promise {
        let owner = env::predecessor_account_id();

        let existing = self.stakes.get(&token_id)
            .unwrap_or_else(|| env::panic_str("not found"));
        assert_eq!(existing.owner_id, owner, "not yours");
        let nft_contract = existing.nft_contract.clone();
        let token = existing.token_id.clone();

        self.calc_rewards();
        self.stakes.remove(&token_id);

        let prefix = format!("us{}", &owner);
        let items: Vec<TokenId> = self.user_stakes.get(&owner)
            .map(|v| v.iter().map(|t| t.clone()).collect())
            .unwrap_or_default();
        let mut nv = Vector::new(prefix.as_bytes());
        for t in items {
            if t != token_id { nv.push(t); }
        }
        self.user_stakes.insert(owner.clone(), nv);

        let all: Vec<TokenId> = self.all_stakes.iter().map(|t| t.clone()).collect();
        let mut nv2 = Vector::new(b"a");
        for t in all {
            if t != token_id { nv2.push(t); }
        }
        self.all_stakes = nv2;
        self.total_staked -= 1;

        nft_contract::ext(nft_contract)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(Gas::from_tgas(10))
            .nft_transfer(owner, token, None::<String>)
    }

    pub fn claim(&mut self) -> Promise {
        let owner = env::predecessor_account_id();
        self.calc_rewards();
        let amt = self.pending.get(&owner).copied().unwrap_or(0u128);
        assert!(amt > 0, "no rewards");
        self.pending.insert(owner.clone(), 0u128);
        self.reward_pool -= amt;
        Promise::new(owner).transfer(NearToken::from_yoctonear(amt))
    }

    fn calc_rewards(&mut self) {
        let cur = self.cur_epoch();
        if cur <= self.last_update { return; }
        if self.total_staked == 0 || self.reward_pool == 0 {
            self.last_update = cur;
            return;
        }
        let pool = self.reward_pool;
        let ts = self.total_staked as u128;
        let mut pending_add: Vec<(AccountId, u128)> = Vec::new();

        for tid in self.all_stakes.iter() {
            if let Some(stake) = self.stakes.get(tid) {
                if !self.is_locked(&stake) { continue; }
                let count = self.user_stakes.get(&stake.owner_id)
                    .map(|v| v.len() as u128).unwrap_or(0);
                if count == 0 { continue; }
                let share = pool * count / ts;
                if share > 0 {
                    let prev = self.pending.get(&stake.owner_id).copied().unwrap_or(0u128);
                    pending_add.push((stake.owner_id.clone(), prev + share));
                }
            }
        }
        for (id, amt) in pending_add { self.pending.insert(id, amt); }
        self.last_update = cur;
    }

    pub fn get_user_stakes(&self, account_id: AccountId) -> Vec<StakeView> {
        let mut result = Vec::new();
        if let Some(list) = self.user_stakes.get(&account_id) {
            for tid in list.iter() {
                if let Some(s) = self.stakes.get(tid) {
                    result.push(StakeView {
                        owner_id: s.owner_id.clone(),
                        token_id: s.token_id.clone(),
                        nft_contract_id: s.nft_contract.clone(),
                        staked_at: s.staked_at,
                        lock_duration: s.lock_duration,
                        unlocked_at: s.unlocked_at,
                        active: self.is_locked(&s),
                    });
                }
            }
        }
        result
    }

    pub fn get_user_rewards(&self, account_id: AccountId) -> U128 {
        U128::from(self.pending.get(&account_id).copied().unwrap_or(0u128))
    }

    pub fn get_metadata(&self) -> Metadata {
        Metadata {
            owner_id: self.owner_id.clone(),
            total_staked: self.total_staked,
            reward_pool: U128::from(self.reward_pool),
            epoch_duration: self.epoch_duration,
        }
    }

    pub fn get_stake(&self, token_id: TokenId) -> Option<StakeView> {
        self.stakes.get(&token_id).map(|s| StakeView {
            owner_id: s.owner_id.clone(),
            token_id: s.token_id.clone(),
            nft_contract_id: s.nft_contract.clone(),
            staked_at: s.staked_at,
            lock_duration: s.lock_duration,
            unlocked_at: s.unlocked_at,
            active: self.is_locked(&s),
        })
    }

    pub fn get_total_staked(&self) -> u64 { self.total_staked }
    pub fn get_reward_pool(&self) -> U128 { U128::from(self.reward_pool) }
}

#[ext_contract(nft_contract)]
pub trait NftContract {
    fn nft_transfer(receiver_id: AccountId, token_id: String, memo: Option<String>);
    fn nft_transfer_call(receiver_id: AccountId, token_id: String, msg: String, memo: Option<String>) -> Promise;
}