use near_sdk::{near, borsh::{self, BorshDeserialize, BorshSerialize}, AccountId};

#[near(contract_state)]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Contract {
    pub owner_id: AccountId,
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