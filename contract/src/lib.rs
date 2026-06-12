use near_sdk::{
    near, store::{LookupMap, Vector}, env, log, NearToken, Promise, PromiseOrValue,
    json_types::U128, AccountId, Gas,
};
use serde::{Deserialize, Serialize};

pub type TokenId = String;

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
    pub total_staked: u64,
    pub reward_pool: U128,
    pub epoch_duration: u64,
}

#[near(contract_state)]
pub struct StakingContract {
    owner_id: AccountId,
    stakes: LookupMap<TokenId, TokenStake>,
    user_stakes: LookupMap<AccountId, Vector<TokenId>>,
    total_staked: u64,
    reward_pool: NearToken,
    epoch_duration: u64,
    last_update: u64,
    pending: LookupMap<AccountId, NearToken>,
}

struct TokenStake {
    owner_id: AccountId,
    token_id: TokenId,
    nft_contract: AccountId,
    staked_at: u64,
    lock_duration: u64,
    unlocked_at: u64,
    last_epoch: u64,
}

#[near]
impl StakingContract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        let now = env::block_timestamp() / 1_000_000_000;
        Self {
            owner_id,
            stakes: LookupMap::new(b"s"),
            user_stakes: LookupMap::new(b"u"),
            total_staked: 0,
            reward_pool: NearToken::from_yoctonear(0),
            epoch_duration: 86400,
            last_update: now,
            pending: LookupMap::new(b"p"),
        }
    }

    #[payable]
    pub fn deposit_rewards(&mut self) {
        self.assert_owner();
        self.reward_pool = self.reward_pool
            .checked_add(env::attached_deposit())
            .expect("Overflow");
    }

    pub fn unstake(&mut self, token_id: TokenId) -> Promise {
        let owner = env::predecessor_account_id();
        let stake = self.stakes.get(&token_id)
            .unwrap_or_else(|| env::panic_str("Stake not found"));
        assert_eq!(stake.owner_id, owner, "Not yours");
        self.calc();
        self.stakes.remove(&token_id);
        if let Some(mut list) = self.user_stakes.get(&owner) {
            let ids: Vec<TokenId> = list.iter().filter(|t| t != &&token_id).cloned().collect();
            let mut nv = Vector::new(format!("u{owner}").as_bytes());
            for id in ids { nv.push(&id); }
            self.user_stakes.insert(owner.clone(), nv);
        }
        self.total_staked -= 1;
        nft_ext::ext(stake.nft_contract)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(Gas::from_tgas(10))
            .nft_transfer(owner, token_id, None)
    }

    pub fn claim(&mut self) -> Promise {
        let owner = env::predecessor_account_id();
        self.calc();
        let amount = self.pending.get(&owner).cloned()
            .unwrap_or(NearToken::from_yoctonear(0));
        assert!(amount.as_yoctonear() > 0, "No rewards");
        self.pending.insert(owner.clone(), NearToken::from_yoctonear(0));
        self.reward_pool = self.reward_pool.checked_sub(amount).expect("Underflow");
        Promise::new(owner).transfer(amount)
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
        if parts.len() < 2 { return PromiseOrValue::Value(false); }
        let dur: u64 = match parts[0].parse() { Ok(v) => v, _ => return PromiseOrValue::Value(false) };
        let owner: AccountId = match parts[1].parse() { Ok(v) => v, _ => return PromiseOrValue::Value(false) };
        if dur != 864000 && dur != 1728000 && dur != 2592000 {
            return PromiseOrValue::Value(false);
        }
        let nft = env::predecessor_account_id();
        let now = env::block_timestamp() / 1_000_000_000;
        let stake = TokenStake {
            owner_id: owner.clone(),
            token_id: token_id.clone(),
            nft_contract: nft,
            staked_at: now,
            lock_duration: dur,
            unlocked_at: now + dur,
            last_epoch: now / self.epoch_duration,
        };
        if self.stakes.insert(token_id.clone(), stake).is_some() {
            return PromiseOrValue::Value(false);
        }
        let mut list = self.user_stakes.get(&owner).unwrap_or_else(||
            Vector::new(format!("u{owner}").as_bytes())
        );
        list.push(&token_id);
        self.user_stakes.insert(owner, list);
        self.total_staked += 1;
        PromiseOrValue::Value(true)
    }

    fn calc(&mut self) {
        let now = env::block_timestamp() / 1_000_000_000;
        let cur = now / self.epoch_duration;
        if cur <= self.last_update || self.total_staked == 0 { return; }
        let pool = self.reward_pool.as_yoctonear();
        if pool == 0 { self.last_update = cur; return; }
        for entry in self.stakes.iter() {
            let (tid, stake) = entry;
            if !stake.active() { continue; }
            let end = stake.unlocked_at / self.epoch_duration;
            let until = end.min(cur);
            if until <= stake.last_epoch { continue; }
            let owner_stakes = self.user_stakes.get(&stake.owner_id)
                .map(|v| v.len() as u128).unwrap_or(0);
            if owner_stakes == 0 { continue; }
            let share = pool * owner_stakes / self.total_staked as u128;
            if share > 0 {
                let prev = self.pending.get(&stake.owner_id).cloned()
                    .unwrap_or(NearToken::from_yoctonear(0));
                self.pending.insert(stake.owner_id.clone(),
                    prev.checked_add(NearToken::from_yoctonear(share)).unwrap());
            }
        }
        self.last_update = cur;
    }

    pub fn get_user_stakes(&self, account_id: AccountId) -> Vec<StakeView> {
        self.user_stakes.get(&account_id).map(|list| {
            list.iter().filter_map(|tid| {
                self.stakes.get(&tid).map(|s| StakeView {
                    owner_id: s.owner_id,
                    token_id: s.token_id,
                    nft_contract_id: s.nft_contract,
                    staked_at: s.staked_at,
                    lock_duration: s.lock_duration,
                    unlocked_at: s.unlocked_at,
                    active: s.active(),
                })
            }).collect()
        }).unwrap_or_default()
    }

    pub fn get_user_rewards(&self, account_id: AccountId) -> U128 {
        let v = self.pending.get(&account_id).cloned()
            .unwrap_or(NearToken::from_yoctonear(0));
        U128::from(v.as_yoctonear())
    }

    pub fn get_metadata(&self) -> ContractMetadata {
        ContractMetadata {
            owner_id: self.owner_id.clone(),
            total_staked: self.total_staked,
            reward_pool: U128::from(self.reward_pool.as_yoctonear()),
            epoch_duration: self.epoch_duration,
        }
    }

    fn assert_owner(&self) {
        assert_eq!(env::predecessor_account_id(), self.owner_id, "Only owner");
    }
}

// --- NFT interface ---
#[near(contract_state)]
pub struct TokenStake {
    owner_id: AccountId,
    token_id: TokenId,
    nft_contract: AccountId,
    staked_at: u64,
    lock_duration: u64,
    unlocked_at: u64,
    last_epoch: u64,
}

impl TokenStake {
    fn active(&self) -> bool {
        let now = env::block_timestamp() / 1_000_000_000;
        self.unlocked_at > now || self.staked_at + self.lock_duration > now
    }
}

#[near]
mod nft_ext {
    use near_sdk::{Promise, AccountId, PromiseOrValue};

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