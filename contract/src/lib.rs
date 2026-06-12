use near_sdk::{near, env, NearToken, Promise, json_types::U128, AccountId};
use near_sdk::collections::UnorderedMap;
use serde::{Serialize, Deserialize};

#[near(contract_state)]
#[derive(Default)]
pub struct Contract {
    pub owner_id: AccountId,
    pub stakes: UnorderedMap<String, String>,
}

#[near]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self { owner_id, stakes: UnorderedMap::new(b"s") }
    }

    pub fn stake_len(&self) -> u64 { self.stakes.len() }
    pub fn get_owner(&self) -> AccountId { self.owner_id.clone() }

    #[payable]
    pub fn deposit(&mut self) {
        assert_eq!(env::predecessor_account_id(), self.owner_id, "owner only");
        self.stakes.insert(&"dep".to_string(), &env::attached_deposit().as_yoctonear().to_string());
    }
}