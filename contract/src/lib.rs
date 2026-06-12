use near_sdk::near;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::AccountId;

#[near(contract_state)]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Contract {
    pub owner_id: AccountId,
}

impl Default for Contract {
    fn default() -> Self {
        Self { owner_id: AccountId::new_unchecked("test.near".to_string()) }
    }
}

#[near]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self { owner_id }
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }
}