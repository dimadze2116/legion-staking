use near_sdk::near;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::AccountId;

#[near(contract_state, serializers = [borsh])]
#[derive(BorshSerialize, BorshDeserialize)]
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
}