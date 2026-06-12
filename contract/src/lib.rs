use near_sdk::{near, env, NearToken, Promise, PromiseOrValue, json_types::U128, AccountId, Gas};
use near_sdk::collections::{LookupMap, UnorderedMap, Vector};
use serde::{Deserialize, Serialize};

type TokenId = String;

#[near(contract_state)]
#[derive(Default)]
pub struct Contract {
    pub owner_id: AccountId,
    pub total_staked: u64,
    pub reward_pool: u128,
    pub stakes: UnorderedMap<Vec<u8>, Vec<u8>>,
}

#[near]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            stakes: UnorderedMap::new(b"s"),
            ..Default::default()
        }
    }

    pub fn get_stake_count(&self) -> u64 {
        self.stakes.len()
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }
}