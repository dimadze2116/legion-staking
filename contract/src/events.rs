use near_sdk::{near, AccountId, Balance};
use serde::{Deserialize, Serialize};

use crate::contract::TokenId;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum StakeEvent {
    Stake(StakeData),
    Unstake(UnstakeData),
    Claim(ClaimData),
    RewardDeposit(RewardDepositData),
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeData {
    pub owner_id: AccountId,
    pub token_id: TokenId,
    pub lock_duration: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct UnstakeData {
    pub owner_id: AccountId,
    pub token_id: TokenId,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ClaimData {
    pub owner_id: AccountId,
    pub amount: Balance,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RewardDepositData {
    pub depositor_id: AccountId,
    pub amount: Balance,
}

pub fn emit_stake(owner_id: &AccountId, token_id: &TokenId, lock_duration: u64) {
    let event = StakeEvent::Stake(StakeData {
        owner_id: owner_id.clone(),
        token_id: token_id.clone(),
        lock_duration,
    });
    near_sdk::env::log_str(
        &format!("EVENT_JSON:{}", serde_json::to_string(&event).unwrap())
    );
}

pub fn emit_unstake(owner_id: &AccountId, token_id: &TokenId) {
    let event = StakeEvent::Unstake(UnstakeData {
        owner_id: owner_id.clone(),
        token_id: token_id.clone(),
    });
    near_sdk::env::log_str(
        &format!("EVENT_JSON:{}", serde_json::to_string(&event).unwrap())
    );
}

pub fn emit_claim(owner_id: &AccountId, amount: Balance) {
    let event = StakeEvent::Claim(ClaimData {
        owner_id: owner_id.clone(),
        amount,
    });
    near_sdk::env::log_str(
        &format!("EVENT_JSON:{}", serde_json::to_string(&event).unwrap())
    );
}

pub fn emit_reward_deposit(depositor_id: &AccountId, amount: Balance) {
    let event = StakeEvent::RewardDeposit(RewardDepositData {
        depositor_id: depositor_id.clone(),
        amount,
    });
    near_sdk::env::log_str(
        &format!("EVENT_JSON:{}", serde_json::to_string(&event).unwrap())
    );
}